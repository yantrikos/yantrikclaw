//! Model tier detection — adaptive tool strategy based on model size.
//!
//! Auto-detects model parameters from the model name and creates a capability
//! profile that the runtime uses to adjust tool exposure, selection mode, and
//! agent loop depth. Ported from yantrik-ml/src/capability.rs.

use serde::{Deserialize, Serialize};

/// Broad capability tier derived from model parameter count.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ModelTier {
    /// 0.5–1.5B params. Very constrained — MCQ routing, 10 tools max.
    Tiny,
    /// 1.5–4B params. Limited — structured JSON, 20 tools, basic multi-step.
    Small,
    /// 4–14B params. Capable — structured JSON, 25 tools, family routing.
    Medium,
    /// 14B+ params. Strong — native function calls, 30 tools, full agent loop.
    Large,
}

impl ModelTier {
    /// Classify a model into a tier based on its name/identifier.
    ///
    /// Parses parameter count from common naming conventions:
    /// - `qwen3.5:0.6b`, `qwen3.5:27b-nothink`, `llama3.2:3b` (Ollama)
    /// - `Qwen3.5-9B`, `Llama-3.2-1B` (HuggingFace)
    /// - Cloud models (claude, gpt-, gemini) → Large
    pub fn from_model_name(model: &str) -> Self {
        if let Some(params_b) = Self::extract_param_count(model) {
            match params_b {
                x if x < 1.5 => ModelTier::Tiny,
                x if x < 4.0 => ModelTier::Small,
                x if x < 14.0 => ModelTier::Medium,
                _ => ModelTier::Large,
            }
        } else {
            let lower = model.to_lowercase();
            if lower.contains("claude")
                || lower.contains("gpt-")
                || lower.contains("gemini")
                || lower.contains("minimax")
            {
                ModelTier::Large
            } else {
                ModelTier::Medium // safe default for unknown local models
            }
        }
    }

    /// Extract parameter count in billions from model name.
    fn extract_param_count(model: &str) -> Option<f64> {
        let lower = model.to_lowercase();

        // Pattern 1: `:Xb` (Ollama tag format)
        if let Some(colon_idx) = lower.rfind(':') {
            let after_colon = &lower[colon_idx + 1..];
            if let Some(b_idx) = after_colon.find('b') {
                if let Ok(val) = after_colon[..b_idx].parse::<f64>() {
                    if val > 0.0 && val < 1000.0 {
                        return Some(val);
                    }
                }
            }
        }

        // Pattern 2: `-XB` or `_XB` (HuggingFace format)
        for sep in ['-', '_'] {
            for part in lower.split(sep) {
                if part.ends_with('b') && part.len() > 1 {
                    let num_part = &part[..part.len() - 1];
                    if let Ok(val) = num_part.parse::<f64>() {
                        if val > 0.0 && val < 1000.0 {
                            return Some(val);
                        }
                    }
                }
            }
        }

        None
    }
}

impl std::fmt::Display for ModelTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModelTier::Tiny => write!(f, "tiny"),
            ModelTier::Small => write!(f, "small"),
            ModelTier::Medium => write!(f, "medium"),
            ModelTier::Large => write!(f, "large"),
        }
    }
}

/// How the model should express tool calls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolCallMode {
    /// Multiple-choice: "Which tool? A) recall B) web_search" → model outputs "A"
    MCQ,
    /// Model outputs structured JSON: `{"tool": "recall", "args": {"query": "..."}}`
    StructuredJSON,
    /// Standard OpenAI/Anthropic function-calling format via API tools parameter.
    NativeFunctionCall,
}

/// Complete capability profile for adapting tool exposure to model size.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelCapabilityProfile {
    pub tier: ModelTier,
    pub max_tools_per_prompt: usize,
    pub tool_call_mode: ToolCallMode,
    pub use_family_routing: bool,
    pub max_agent_steps: usize,
    pub supports_repair_loop: bool,
    pub max_repair_attempts: usize,
    pub confidence_threshold: f64,
}

impl ModelCapabilityProfile {
    /// Create a capability profile from a model name string.
    pub fn detect(model: &str) -> Self {
        let tier = ModelTier::from_model_name(model);
        match tier {
            ModelTier::Tiny => Self {
                tier,
                max_tools_per_prompt: 10,
                tool_call_mode: ToolCallMode::MCQ,
                use_family_routing: false,
                max_agent_steps: 3,
                supports_repair_loop: false,
                max_repair_attempts: 0,
                confidence_threshold: 0.9,
            },
            ModelTier::Small => Self {
                tier,
                max_tools_per_prompt: 20,
                tool_call_mode: ToolCallMode::StructuredJSON,
                use_family_routing: true,
                max_agent_steps: 5,
                supports_repair_loop: true,
                max_repair_attempts: 1,
                confidence_threshold: 0.85,
            },
            ModelTier::Medium => Self {
                tier,
                max_tools_per_prompt: 25,
                tool_call_mode: ToolCallMode::StructuredJSON,
                use_family_routing: true,
                max_agent_steps: 10,
                supports_repair_loop: true,
                max_repair_attempts: 2,
                confidence_threshold: 0.75,
            },
            ModelTier::Large => Self {
                tier,
                max_tools_per_prompt: 30,
                tool_call_mode: ToolCallMode::NativeFunctionCall,
                use_family_routing: false,
                max_agent_steps: 15,
                supports_repair_loop: true,
                max_repair_attempts: 3,
                confidence_threshold: 0.6,
            },
        }
    }

    /// Whether this tier uses MCQ batched selection instead of native function calling.
    pub fn uses_mcq_selection(&self) -> bool {
        self.tool_call_mode == ToolCallMode::MCQ
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_ollama_tiny() {
        assert_eq!(ModelTier::from_model_name("qwen3.5:0.6b"), ModelTier::Tiny);
        assert_eq!(
            ModelTier::from_model_name("qwen2.5:1b-instruct"),
            ModelTier::Tiny
        );
    }

    #[test]
    fn detect_ollama_small() {
        assert_eq!(ModelTier::from_model_name("qwen2.5:3b"), ModelTier::Small);
        assert_eq!(
            ModelTier::from_model_name("llama3.2:3b-instruct"),
            ModelTier::Small
        );
    }

    #[test]
    fn detect_ollama_medium() {
        assert_eq!(
            ModelTier::from_model_name("qwen3.5:9b-nothink"),
            ModelTier::Medium
        );
        assert_eq!(ModelTier::from_model_name("llama3.1:8b"), ModelTier::Medium);
    }

    #[test]
    fn detect_ollama_large() {
        assert_eq!(
            ModelTier::from_model_name("qwen3.5:27b-nothink"),
            ModelTier::Large
        );
        assert_eq!(
            ModelTier::from_model_name("llama3.3:70b"),
            ModelTier::Large
        );
    }

    #[test]
    fn detect_huggingface_format() {
        assert_eq!(ModelTier::from_model_name("Qwen3.5-9B"), ModelTier::Medium);
        assert_eq!(
            ModelTier::from_model_name("Llama-3.2-1B"),
            ModelTier::Tiny
        );
    }

    #[test]
    fn detect_cloud_models_as_large() {
        assert_eq!(
            ModelTier::from_model_name("claude-sonnet-4-20250514"),
            ModelTier::Large
        );
        assert_eq!(ModelTier::from_model_name("gpt-4o"), ModelTier::Large);
        assert_eq!(
            ModelTier::from_model_name("gemini-1.5-pro"),
            ModelTier::Large
        );
    }

    #[test]
    fn detect_unknown_defaults_to_medium() {
        assert_eq!(
            ModelTier::from_model_name("some-custom-model"),
            ModelTier::Medium
        );
    }

    #[test]
    fn profile_tiny_uses_mcq() {
        let profile = ModelCapabilityProfile::detect("qwen3.5:0.6b");
        assert_eq!(profile.tier, ModelTier::Tiny);
        assert!(profile.uses_mcq_selection());
        assert_eq!(profile.max_tools_per_prompt, 10);
    }

    #[test]
    fn profile_large_uses_native() {
        let profile = ModelCapabilityProfile::detect("gpt-4o");
        assert_eq!(profile.tier, ModelTier::Large);
        assert_eq!(profile.tool_call_mode, ToolCallMode::NativeFunctionCall);
        assert_eq!(profile.max_tools_per_prompt, 30);
    }

    #[test]
    fn profile_medium_uses_family_routing() {
        let profile = ModelCapabilityProfile::detect("qwen3.5:9b");
        assert!(profile.use_family_routing);
    }
}
