//! Tier-aware tool selection — adaptive tool filtering based on model capability.
//!
//! Injects between the tool registry and the LLM prompt to ensure smaller models
//! see fewer, better-matched tools. Uses ToolFamily keyword routing to boost
//! relevant tools, then caps at the model's tool budget.
//!
//! Ported from yantrik-companion/src/companion.rs `select_tools_adaptive()`.

use super::family::ToolFamily;
use super::tier::ModelCapabilityProfile;
use super::traits::{PermissionLevel, Tool};
use tracing::debug;

/// Select which tools to expose to the LLM for a given user query.
///
/// Returns the names of selected tools, ordered by relevance score (best first).
///
/// Selection logic:
/// 1. Filter out tools above the allowed permission level.
/// 2. If `use_family_routing`: score each tool by ToolFamily keyword match to
///    the user query, boosting tools in matching families.
/// 3. Always-include tools (category "general") get a baseline score.
/// 4. Cap at `max_tools_per_prompt` from the capability profile.
pub fn select_tools_for_tier(
    user_input: &str,
    profile: &ModelCapabilityProfile,
    tools: &[&dyn Tool],
    max_permission: PermissionLevel,
) -> Vec<String> {
    // Step 1: Permission filter
    let permitted: Vec<&dyn Tool> = tools
        .iter()
        .filter(|t| t.permission() <= max_permission)
        .copied()
        .collect();

    // Large models with family routing disabled: return all permitted tools (up to budget)
    if !profile.use_family_routing {
        let selected: Vec<String> = permitted
            .iter()
            .take(profile.max_tools_per_prompt)
            .map(|t| t.name().to_string())
            .collect();
        debug!(
            tier = %profile.tier,
            total = permitted.len(),
            selected = selected.len(),
            "tool-selector: no family routing, returning all permitted (capped)"
        );
        return selected;
    }

    // Step 2: Score tools using ToolFamily routing
    let family_scores = ToolFamily::route_query(user_input);
    let top_families: Vec<&ToolFamily> = family_scores.iter().map(|(f, _)| f).collect();

    let mut scored: Vec<(&dyn Tool, f64)> = permitted
        .iter()
        .map(|&tool| {
            let cat = tool.category();
            let mut score = 0.0;

            // Boost tools whose category matches a top-scoring family
            for (i, family) in top_families.iter().enumerate() {
                if family.matches_category(cat) {
                    // Higher boost for better-matching families, decay by rank
                    let family_score = family_scores[i].1;
                    score += family_score * (1.0 / (i as f64 + 1.0));
                }
            }

            // Baseline score for "general" category tools (always somewhat relevant)
            if cat == "general" {
                score += 0.1;
            }

            // Small baseline so tools with no family match still have a chance
            // if we haven't filled the budget
            score += 0.01;

            (tool, score)
        })
        .collect();

    // Sort by score descending
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    // Step 3: Cap at budget
    let selected: Vec<String> = scored
        .iter()
        .take(profile.max_tools_per_prompt)
        .map(|(t, _)| t.name().to_string())
        .collect();

    debug!(
        tier = %profile.tier,
        families = ?top_families.iter().map(|f| f.to_string()).collect::<Vec<_>>(),
        total = permitted.len(),
        selected = selected.len(),
        budget = profile.max_tools_per_prompt,
        "tool-selector: family routing applied"
    );

    selected
}

/// Filter a `Vec<ToolSpec>` to only include specs whose names appear in the
/// selected set. Preserves ordering from the original specs list.
pub fn filter_specs_by_selection(
    specs: Vec<super::ToolSpec>,
    selected_names: &[String],
) -> Vec<super::ToolSpec> {
    specs
        .into_iter()
        .filter(|spec| selected_names.iter().any(|name| name == &spec.name))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::tier::ModelCapabilityProfile;
    use crate::tools::traits::{PermissionLevel, ToolResult};
    use async_trait::async_trait;

    // Test tool with configurable category and permission
    struct TestTool {
        tool_name: &'static str,
        cat: &'static str,
        perm: PermissionLevel,
    }

    #[async_trait]
    impl Tool for TestTool {
        fn name(&self) -> &str {
            self.tool_name
        }
        fn description(&self) -> &str {
            "test tool"
        }
        fn parameters_schema(&self) -> serde_json::Value {
            serde_json::json!({"type": "object", "properties": {}})
        }
        async fn execute(&self, _args: serde_json::Value) -> anyhow::Result<ToolResult> {
            Ok(ToolResult {
                success: true,
                output: String::new(),
                error: None,
            })
        }
        fn category(&self) -> &str {
            self.cat
        }
        fn permission(&self) -> PermissionLevel {
            self.perm
        }
    }

    fn make_tool(name: &'static str, cat: &'static str, perm: PermissionLevel) -> TestTool {
        TestTool {
            tool_name: name,
            cat,
            perm,
        }
    }

    #[test]
    fn permission_filter_removes_dangerous_tools() {
        let t1 = make_tool("file_read", "files", PermissionLevel::Safe);
        let t2 = make_tool("shell", "system", PermissionLevel::Dangerous);
        let tools: Vec<&dyn Tool> = vec![&t1, &t2];

        let profile = ModelCapabilityProfile::detect("qwen3.5:9b");
        let selected =
            select_tools_for_tier("read a file", &profile, &tools, PermissionLevel::Standard);

        assert!(selected.contains(&"file_read".to_string()));
        assert!(!selected.contains(&"shell".to_string()));
    }

    #[test]
    fn family_routing_boosts_matching_tools() {
        let t_file = make_tool("file_read", "files", PermissionLevel::Safe);
        let t_git = make_tool("git_operations", "git", PermissionLevel::Standard);
        let t_mem = make_tool("memory_recall", "memory", PermissionLevel::Safe);
        let t_browse = make_tool("web_search", "browse", PermissionLevel::Safe);
        let tools: Vec<&dyn Tool> = vec![&t_file, &t_git, &t_mem, &t_browse];

        // Query about files — Files family should rank higher
        let profile = ModelCapabilityProfile::detect("qwen3.5:9b");
        let selected = select_tools_for_tier(
            "read the config file and check git status",
            &profile,
            &tools,
            PermissionLevel::Admin,
        );

        // file_read and git_operations should be ranked first (Files family matches "file" and "git")
        assert_eq!(selected[0], "file_read");
        assert_eq!(selected[1], "git_operations");
    }

    #[test]
    fn budget_caps_tool_count() {
        let tools_data: Vec<TestTool> = (0..30)
            .map(|i| {
                let name = Box::leak(format!("tool_{i}").into_boxed_str());
                make_tool(name, "general", PermissionLevel::Safe)
            })
            .collect();
        let tools: Vec<&dyn Tool> = tools_data.iter().map(|t| t as &dyn Tool).collect();

        // Tiny model: max 10 tools
        let profile = ModelCapabilityProfile::detect("qwen3.5:0.6b");
        let selected = select_tools_for_tier(
            "do something",
            &profile,
            &tools,
            PermissionLevel::Admin,
        );
        assert_eq!(selected.len(), 10);
    }

    #[test]
    fn large_model_no_family_routing_returns_all() {
        let t1 = make_tool("file_read", "files", PermissionLevel::Safe);
        let t2 = make_tool("shell", "system", PermissionLevel::Dangerous);
        let t3 = make_tool("memory_recall", "memory", PermissionLevel::Safe);
        let tools: Vec<&dyn Tool> = vec![&t1, &t2, &t3];

        let profile = ModelCapabilityProfile::detect("gpt-4o");
        assert!(!profile.use_family_routing);

        let selected = select_tools_for_tier(
            "read a file",
            &profile,
            &tools,
            PermissionLevel::Admin,
        );
        // All 3 tools returned (no family filtering for large models)
        assert_eq!(selected.len(), 3);
    }

    #[test]
    fn memory_query_boosts_memory_tools() {
        let t_mem = make_tool("memory_recall", "memory", PermissionLevel::Safe);
        let t_file = make_tool("file_read", "files", PermissionLevel::Safe);
        let t_browse = make_tool("web_search", "browse", PermissionLevel::Safe);
        let tools: Vec<&dyn Tool> = vec![&t_file, &t_browse, &t_mem];

        let profile = ModelCapabilityProfile::detect("llama3.2:3b");
        let selected = select_tools_for_tier(
            "remember what I said about my preference",
            &profile,
            &tools,
            PermissionLevel::Admin,
        );

        assert_eq!(selected[0], "memory_recall");
    }

    #[test]
    fn filter_specs_by_selection_preserves_order() {
        let specs = vec![
            super::super::ToolSpec {
                name: "a".into(),
                description: "tool a".into(),
                parameters: serde_json::json!({}),
            },
            super::super::ToolSpec {
                name: "b".into(),
                description: "tool b".into(),
                parameters: serde_json::json!({}),
            },
            super::super::ToolSpec {
                name: "c".into(),
                description: "tool c".into(),
                parameters: serde_json::json!({}),
            },
        ];

        let selected = vec!["c".to_string(), "a".to_string()];
        let filtered = filter_specs_by_selection(specs, &selected);

        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].name, "a"); // preserves original spec order
        assert_eq!(filtered[1].name, "c");
    }
}
