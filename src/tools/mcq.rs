//! MCQ batched tool selection for Tiny/Small models.
//!
//! When a model is too small for native function calling, present candidate
//! tools as a multiple-choice prompt (A/B/C/D/E) and parse the single-letter
//! response. This avoids requiring the model to produce structured JSON tool
//! calls, which sub-4B models struggle with.
//!
//! Flow:
//! 1. **Auto-select**: if the top-ranked tool score is high enough (>0.85)
//!    with sufficient margin over #2 (>0.10), skip the LLM entirely.
//! 2. **MCQ batches**: present 5 tools at a time as A/B/C/D/E choices.
//!    Parse the model's response for a single letter.
//! 3. **NO_TOOL**: if the model responds NO_TOOL, advance to the next batch.
//! 4. **Max rounds**: stop after `max_rounds` batches.
//!
//! Ported from yantrik-companion/src/companion.rs `mcq_batch_select()`.

use crate::providers::Provider;
use tracing::{debug, info, warn};

/// MCQ labels for batch tool selection.
const MCQ_LABELS: &[char] = &['A', 'B', 'C', 'D', 'E'];

/// A candidate tool for MCQ selection, pre-ranked by the selector.
#[derive(Debug, Clone)]
pub struct McqCandidate {
    /// Tool name (matches `Tool::name()`).
    pub name: String,
    /// One-line description shown to the model.
    pub description: String,
    /// Relevance score from the selector (higher = better).
    pub score: f64,
}

/// Check if scores are confident enough to auto-select without asking the LLM.
///
/// Returns the tool name if top-1 score is high and margin over top-2 is large.
pub fn embedding_auto_select(ranked: &[McqCandidate]) -> Option<&str> {
    if ranked.len() < 2 {
        return None;
    }
    let score1 = ranked[0].score;
    let score2 = ranked[1].score;
    let margin = score1 - score2;

    if score1 > 0.85 && margin > 0.10 {
        info!(
            tool = ranked[0].name.as_str(),
            score = score1,
            margin,
            "MCQ auto-select — skipping LLM"
        );
        Some(&ranked[0].name)
    } else {
        None
    }
}

/// Build an MCQ prompt for a batch of tools.
///
/// Returns a compact prompt (~200-400 tokens) asking the model to pick A-E or NO_TOOL.
fn build_mcq_prompt(query: &str, batch: &[McqCandidate]) -> String {
    let mut prompt = String::with_capacity(512);
    prompt.push_str(
        "Select the best tool for the user's request.\n\
         Output exactly one of: A, B, C, D, E, NO_TOOL\n\n",
    );

    for (i, candidate) in batch.iter().enumerate() {
        if i >= MCQ_LABELS.len() {
            break;
        }
        prompt.push(MCQ_LABELS[i]);
        prompt.push_str(". ");
        prompt.push_str(&candidate.name);
        prompt.push_str(" — ");
        // Truncate description to keep prompt short
        let desc = if candidate.description.len() > 120 {
            format!("{}…", &candidate.description[..117])
        } else {
            candidate.description.clone()
        };
        prompt.push_str(&desc);
        prompt.push('\n');
    }

    prompt.push_str("\nUser request: \"");
    // Truncate query to avoid blowing up context for tiny models
    let q = if query.len() > 200 {
        format!("{}…", &query[..197])
    } else {
        query.to_string()
    };
    prompt.push_str(&q);
    prompt.push_str("\"\n\nAnswer:");
    prompt
}

/// Parse an MCQ response to extract the selected label (A-E) or NO_TOOL.
///
/// Returns the index (0-4) of the selected tool, or None for NO_TOOL / invalid.
fn parse_mcq_response(response: &str) -> Option<usize> {
    let trimmed = response.trim();

    // Check for NO_TOOL first
    if trimmed.contains("NO_TOOL")
        || trimmed.contains("no_tool")
        || trimmed.contains("NONE")
        || trimmed.contains("none")
    {
        return None;
    }

    // Strategy 1: If the response is just 1-2 chars, match directly
    if trimmed.len() <= 2 {
        return match trimmed.chars().next()? {
            'A' | 'a' => Some(0),
            'B' | 'b' => Some(1),
            'C' | 'c' => Some(2),
            'D' | 'd' => Some(3),
            'E' | 'e' => Some(4),
            _ => None,
        };
    }

    // Strategy 2: Look for uppercase A-E (standalone or followed by . / ) / space)
    let bytes = trimmed.as_bytes();
    for (i, &b) in bytes.iter().enumerate() {
        if matches!(b, b'A' | b'B' | b'C' | b'D' | b'E') {
            // Check it's not inside a word (preceded by a letter)
            let preceded_by_letter =
                i > 0 && bytes[i - 1].is_ascii_alphabetic();
            if !preceded_by_letter {
                return match b {
                    b'A' => Some(0),
                    b'B' => Some(1),
                    b'C' => Some(2),
                    b'D' => Some(3),
                    b'E' => Some(4),
                    _ => None,
                };
            }
        }
    }

    // Strategy 3: Fallback — look for standalone lowercase a-e at start of response
    match trimmed.chars().next()? {
        'a' => Some(0),
        'b' => Some(1),
        'c' => Some(2),
        'd' => Some(3),
        'e' => Some(4),
        _ => None,
    }
}

/// Run batched MCQ tool selection for small models.
///
/// Sends batches of 5 tools to the LLM as an MCQ prompt. If the model picks
/// a tool, returns its name. If NO_TOOL, advances to the next batch.
///
/// # Arguments
/// - `provider`: LLM provider for the MCQ call
/// - `model`: model name to use for MCQ (same as the main model)
/// - `query`: the user's original query
/// - `ranked_tools`: candidates sorted by relevance (best first)
/// - `batch_size`: tools per MCQ round (default: 5)
/// - `max_rounds`: maximum MCQ rounds before giving up (default: 3)
pub async fn mcq_batch_select(
    provider: &dyn Provider,
    model: &str,
    query: &str,
    ranked_tools: &[McqCandidate],
    batch_size: usize,
    max_rounds: usize,
) -> Option<String> {
    // Try auto-select first — if embedding scores are decisive, skip the LLM
    if let Some(name) = embedding_auto_select(ranked_tools) {
        return Some(name.to_string());
    }

    for round in 0..max_rounds {
        let start = round * batch_size;
        if start >= ranked_tools.len() {
            break;
        }
        let end = (start + batch_size).min(ranked_tools.len());
        let batch = &ranked_tools[start..end];

        if batch.is_empty() {
            break;
        }

        let prompt = build_mcq_prompt(query, batch);

        // Use low temperature for deterministic selection
        match provider.simple_chat(&prompt, model, 0.1).await {
            Ok(response) => {
                let text = response.trim().to_string();
                info!(
                    round = round + 1,
                    batch_start = start,
                    batch_end = end,
                    response = text.as_str(),
                    "MCQ batch selection round"
                );

                if let Some(idx) = parse_mcq_response(&text) {
                    if idx < batch.len() {
                        let tool_name = batch[idx].name.clone();
                        info!(
                            tool = tool_name.as_str(),
                            round = round + 1,
                            label = %MCQ_LABELS[idx],
                            "MCQ selected tool"
                        );
                        return Some(tool_name);
                    }
                }
                // NO_TOOL or invalid — continue to next batch
                debug!(round = round + 1, "MCQ round returned NO_TOOL, trying next batch");
            }
            Err(e) => {
                warn!(round = round + 1, error = %e, "MCQ batch LLM call failed");
                break;
            }
        }
    }

    info!(
        rounds = max_rounds,
        "MCQ batch selection exhausted — no tool selected"
    );
    None
}

/// Build McqCandidates from the selector's output + tool registry.
///
/// Takes the selected tool names (from `select_tools_for_tier`) and the
/// registry, returns scored candidates suitable for MCQ selection.
pub fn build_candidates(
    selected_names: &[String],
    tools: &[Box<dyn super::Tool>],
) -> Vec<McqCandidate> {
    selected_names
        .iter()
        .enumerate()
        .filter_map(|(rank, name)| {
            tools
                .iter()
                .find(|t| t.name() == name)
                .map(|t| McqCandidate {
                    name: name.clone(),
                    description: t.description().to_string(),
                    // Score decays with rank (first = 1.0, second = 0.95, etc.)
                    score: 1.0 - (rank as f64 * 0.05),
                })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidates(names: &[(&str, f64)]) -> Vec<McqCandidate> {
        names
            .iter()
            .map(|(name, score)| McqCandidate {
                name: name.to_string(),
                description: format!("Tool for {name}"),
                score: *score,
            })
            .collect()
    }

    #[test]
    fn auto_select_high_confidence() {
        let c = candidates(&[("recall", 0.92), ("web_search", 0.70), ("file_read", 0.65)]);
        assert_eq!(embedding_auto_select(&c), Some("recall"));
    }

    #[test]
    fn auto_select_low_margin_returns_none() {
        let c = candidates(&[("recall", 0.90), ("web_search", 0.85)]);
        // margin is only 0.05 < 0.10 threshold
        assert_eq!(embedding_auto_select(&c), None);
    }

    #[test]
    fn auto_select_low_score_returns_none() {
        let c = candidates(&[("recall", 0.70), ("web_search", 0.40)]);
        // score < 0.85 threshold
        assert_eq!(embedding_auto_select(&c), None);
    }

    #[test]
    fn auto_select_single_candidate_returns_none() {
        let c = candidates(&[("recall", 0.99)]);
        assert_eq!(embedding_auto_select(&c), None);
    }

    #[test]
    fn parse_mcq_single_letter() {
        assert_eq!(parse_mcq_response("A"), Some(0));
        assert_eq!(parse_mcq_response("B"), Some(1));
        assert_eq!(parse_mcq_response("C"), Some(2));
        assert_eq!(parse_mcq_response("D"), Some(3));
        assert_eq!(parse_mcq_response("E"), Some(4));
    }

    #[test]
    fn parse_mcq_lowercase() {
        assert_eq!(parse_mcq_response("b"), Some(1));
        assert_eq!(parse_mcq_response("d"), Some(3));
    }

    #[test]
    fn parse_mcq_with_noise() {
        assert_eq!(parse_mcq_response("The answer is B."), Some(1));
        assert_eq!(parse_mcq_response("  C) web_search"), Some(2));
    }

    #[test]
    fn parse_mcq_no_tool() {
        assert_eq!(parse_mcq_response("NO_TOOL"), None);
        assert_eq!(parse_mcq_response("no_tool"), None);
        assert_eq!(parse_mcq_response("NONE"), None);
    }

    #[test]
    fn parse_mcq_empty_returns_none() {
        assert_eq!(parse_mcq_response(""), None);
        assert_eq!(parse_mcq_response("   "), None);
    }

    #[test]
    fn parse_mcq_no_tool_before_letter() {
        // "NO_TOOL" contains letters A-E but should return None
        assert_eq!(parse_mcq_response("NO_TOOL is my answer"), None);
    }

    #[test]
    fn build_mcq_prompt_format() {
        let c = candidates(&[("recall", 0.9), ("web_search", 0.8), ("file_read", 0.7)]);
        let prompt = build_mcq_prompt("what did I say yesterday", &c);

        assert!(prompt.contains("A. recall"));
        assert!(prompt.contains("B. web_search"));
        assert!(prompt.contains("C. file_read"));
        assert!(prompt.contains("what did I say yesterday"));
        assert!(prompt.contains("NO_TOOL"));
    }

    #[test]
    fn build_mcq_prompt_truncates_long_query() {
        let long_query = "x".repeat(300);
        let c = candidates(&[("recall", 0.9)]);
        let prompt = build_mcq_prompt(&long_query, &c);

        // Should be truncated to ~200 chars
        assert!(prompt.len() < 500);
    }
}
