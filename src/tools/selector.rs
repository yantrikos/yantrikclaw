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
/// Returns the names of selected tools, combining:
/// 1. **Always-on tools** for this tier (from `ModelCapabilityProfile::always_on_tools`)
/// 2. **Query-relevant tools** discovered via ToolFamily keyword routing
/// 3. **Session-activated tools** (previously discovered via `discover_tools`)
///
/// Total is capped at `max_tools_per_prompt`.
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

    let always_on = profile.always_on_tools();
    let mut selected: Vec<String> = Vec::new();

    // Step 2: Always-on tools first (if they exist in the registry)
    for &name in always_on {
        if permitted.iter().any(|t| t.name() == name) && !selected.contains(&name.to_string()) {
            selected.push(name.to_string());
        }
    }

    // Step 3: Query-relevant tools via ToolFamily routing (fill remaining budget)
    if profile.use_family_routing {
        let family_scores = ToolFamily::route_query(user_input);
        let top_families: Vec<&ToolFamily> = family_scores.iter().map(|(f, _)| f).collect();

        let mut scored: Vec<(&dyn Tool, f64)> = permitted
            .iter()
            .filter(|t| !selected.contains(&t.name().to_string()))
            .map(|&tool| {
                let cat = tool.category();
                let mut score = 0.0;

                for (i, family) in top_families.iter().enumerate() {
                    if family.matches_category(cat) {
                        let family_score = family_scores[i].1;
                        score += family_score * (1.0 / (i as f64 + 1.0));
                    }
                }

                (tool, score)
            })
            .filter(|(_, score)| *score > 0.0) // Only add if actually relevant
            .collect();

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let remaining_budget = profile.max_tools_per_prompt.saturating_sub(selected.len());
        for (tool, _) in scored.iter().take(remaining_budget) {
            selected.push(tool.name().to_string());
        }
    }

    debug!(
        tier = %profile.tier,
        always_on = always_on.len(),
        total_selected = selected.len(),
        budget = profile.max_tools_per_prompt,
        tools = ?selected,
        "tool-selector: always-on + family routing"
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

/// Merge session-activated tool names (from discover_tools) into the selected set.
/// Returns the union, capped at the budget.
pub fn merge_activated_tools(
    mut selected: Vec<String>,
    activated: &std::collections::HashSet<String>,
    budget: usize,
) -> Vec<String> {
    for name in activated {
        if !selected.contains(name) && selected.len() < budget {
            selected.push(name.clone());
        }
    }
    selected
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
    fn family_routing_adds_query_relevant_tools() {
        let t_file = make_tool("file_read", "files", PermissionLevel::Safe);
        let t_git = make_tool("git_operations", "git", PermissionLevel::Standard);
        let t_mem = make_tool("memory_recall", "memory", PermissionLevel::Safe);
        let t_browse = make_tool("web_search", "browse", PermissionLevel::Safe);
        let tools: Vec<&dyn Tool> = vec![&t_file, &t_git, &t_mem, &t_browse];

        // Query about files — Files family tools should appear in the selected set
        let profile = ModelCapabilityProfile::detect("qwen3.5:9b");
        let selected = select_tools_for_tier(
            "read the config file and check git status",
            &profile,
            &tools,
            PermissionLevel::Admin,
        );

        // file_read should be selected (always-on for medium) and git_operations
        // should be added via family routing
        assert!(selected.contains(&"file_read".to_string()));
        assert!(selected.contains(&"git_operations".to_string()));
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

        // Tiny model: max 5 tools
        let profile = ModelCapabilityProfile::detect("qwen3.5:0.6b");
        let selected = select_tools_for_tier(
            "do something",
            &profile,
            &tools,
            PermissionLevel::Admin,
        );
        assert!(selected.len() <= profile.max_tools_per_prompt);
    }

    #[test]
    fn large_model_uses_always_on_plus_routing() {
        let t1 = make_tool("file_read", "files", PermissionLevel::Safe);
        let t2 = make_tool("shell", "system", PermissionLevel::Dangerous);
        let t3 = make_tool("memory_recall", "memory", PermissionLevel::Safe);
        let tools: Vec<&dyn Tool> = vec![&t1, &t2, &t3];

        let profile = ModelCapabilityProfile::detect("gpt-4o");

        let selected = select_tools_for_tier(
            "read a file",
            &profile,
            &tools,
            PermissionLevel::Admin,
        );
        // file_read and memory_recall are always-on for large; shell only if query-relevant
        assert!(selected.contains(&"file_read".to_string()));
        assert!(selected.contains(&"memory_recall".to_string()));
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
