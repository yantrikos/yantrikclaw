//! `discover_tools` meta-tool — lets the model find and activate additional tools.
//!
//! Always available at every model tier. The model describes what it needs in
//! natural language; this tool returns matching tools grouped by family, without
//! exposing their full schemas until they're activated.

use super::family::ToolFamily;
use super::traits::{PermissionLevel, Tool, ToolResult};
use async_trait::async_trait;
use serde_json::json;

/// Registry of all tools available for discovery.
/// Stores (name, description, category, permission) tuples.
#[derive(Debug, Clone)]
pub struct DiscoverableToolInfo {
    pub name: String,
    pub description: String,
    pub category: String,
    pub permission: PermissionLevel,
}

pub struct DiscoverToolsTool {
    /// All tools in the system (for discovery lookup).
    registry: Vec<DiscoverableToolInfo>,
}

impl DiscoverToolsTool {
    pub fn new(registry: Vec<DiscoverableToolInfo>) -> Self {
        Self { registry }
    }
}

#[async_trait]
impl Tool for DiscoverToolsTool {
    fn name(&self) -> &str {
        "discover_tools"
    }

    fn description(&self) -> &str {
        "Find and activate additional tools for your current task. Describe what you need \
         in natural language (e.g. 'I need to edit a file' or 'browse a website'). \
         Returns matching tools you can then use. Call this whenever you need a capability \
         that isn't available in your current tool set."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Describe what capability you need. Examples: \
                        'edit a file', 'run a shell command', 'browse a URL', \
                        'schedule a task', 'send a notification', 'check git status'"
                },
                "activate": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Optional: tool names to activate immediately (from a previous discover_tools result)"
                }
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");

        // Handle explicit activation
        if let Some(activate) = args.get("activate").and_then(|v| v.as_array()) {
            let mut global = super::DISCOVER_TOOLS_ACTIVATED.lock();
            let mut activated_names = Vec::new();
            for name_val in activate {
                if let Some(name) = name_val.as_str() {
                    if self.registry.iter().any(|t| t.name == name) {
                        global.insert(name.to_string());
                        activated_names.push(name.to_string());
                    }
                }
            }
            if !activated_names.is_empty() {
                return Ok(ToolResult {
                    success: true,
                    output: format!(
                        "Activated {} tool(s): {}. These tools are now available for use.",
                        activated_names.len(),
                        activated_names.join(", ")
                    ),
                    error: None,
                });
            }
        }

        // Route query to tool families
        let family_scores = ToolFamily::route_query(query);
        let already_activated = super::DISCOVER_TOOLS_ACTIVATED.lock().clone();

        let mut results: Vec<serde_json::Value> = Vec::new();

        if family_scores.is_empty() {
            // No family match — show all available tools grouped by family
            for &family in ToolFamily::ALL {
                let tools_in_family: Vec<&DiscoverableToolInfo> = self
                    .registry
                    .iter()
                    .filter(|t| family.matches_category(&t.category))
                    .collect();

                if !tools_in_family.is_empty() {
                    let tool_list: Vec<serde_json::Value> = tools_in_family
                        .iter()
                        .map(|t| {
                            json!({
                                "name": t.name,
                                "description": t.description,
                                "active": already_activated.contains(&t.name),
                            })
                        })
                        .collect();

                    results.push(json!({
                        "family": family.to_string(),
                        "tools": tool_list,
                    }));
                }
            }
        } else {
            // Show tools from matching families, best match first
            for (family, score) in &family_scores {
                let tools_in_family: Vec<&DiscoverableToolInfo> = self
                    .registry
                    .iter()
                    .filter(|t| family.matches_category(&t.category))
                    .collect();

                if !tools_in_family.is_empty() {
                    let tool_list: Vec<serde_json::Value> = tools_in_family
                        .iter()
                        .map(|t| {
                            json!({
                                "name": t.name,
                                "description": t.description,
                                "active": already_activated.contains(&t.name),
                            })
                        })
                        .collect();

                    results.push(json!({
                        "family": family.to_string(),
                        "relevance": format!("{:.0}%", score * 100.0),
                        "tools": tool_list,
                    }));
                }
            }
        }

        // Auto-activate the top-scoring tools from matching families
        let mut auto_activated = Vec::new();
        if !family_scores.is_empty() {
            let mut global = super::DISCOVER_TOOLS_ACTIVATED.lock();
            for (family, _) in &family_scores {
                for tool in &self.registry {
                    if family.matches_category(&tool.category) && !global.contains(&tool.name) {
                        global.insert(tool.name.clone());
                        auto_activated.push(tool.name.clone());
                    }
                }
            }
        }

        let mut output = serde_json::to_string_pretty(&results).unwrap_or_default();
        if !auto_activated.is_empty() {
            output.push_str(&format!(
                "\n\nAuto-activated: {}. These tools are now available for use in this session.",
                auto_activated.join(", ")
            ));
        } else {
            output.push_str(
                "\n\nTo activate tools, call discover_tools again with the 'activate' parameter \
                 containing the tool names you want to use.",
            );
        }

        Ok(ToolResult {
            success: true,
            output,
            error: None,
        })
    }

    fn category(&self) -> &str {
        "utility"
    }

    fn permission(&self) -> PermissionLevel {
        PermissionLevel::Safe
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_registry() -> Vec<DiscoverableToolInfo> {
        vec![
            DiscoverableToolInfo {
                name: "shell".into(),
                description: "Execute shell commands".into(),
                category: "system".into(),
                permission: PermissionLevel::Dangerous,
            },
            DiscoverableToolInfo {
                name: "file_write".into(),
                description: "Write to a file".into(),
                category: "files".into(),
                permission: PermissionLevel::Standard,
            },
            DiscoverableToolInfo {
                name: "file_edit".into(),
                description: "Edit a file".into(),
                category: "files".into(),
                permission: PermissionLevel::Standard,
            },
            DiscoverableToolInfo {
                name: "web_fetch".into(),
                description: "Fetch a URL".into(),
                category: "browse".into(),
                permission: PermissionLevel::Safe,
            },
            DiscoverableToolInfo {
                name: "git_operations".into(),
                description: "Git operations".into(),
                category: "git".into(),
                permission: PermissionLevel::Standard,
            },
        ]
    }

    fn clear_global() {
        super::super::DISCOVER_TOOLS_ACTIVATED.lock().clear();
    }

    #[tokio::test]
    async fn discover_file_tools() {
        clear_global();
        let tool = DiscoverToolsTool::new(test_registry());
        let result = tool
            .execute(json!({"query": "I need to edit a file"}))
            .await
            .unwrap();
        assert!(result.success);
        assert!(result.output.contains("file_write"));
        assert!(result.output.contains("file_edit"));
    }

    #[tokio::test]
    async fn discover_web_tools() {
        clear_global();
        let tool = DiscoverToolsTool::new(test_registry());
        let result = tool
            .execute(json!({"query": "fetch a website URL"}))
            .await
            .unwrap();
        assert!(result.success);
        assert!(result.output.contains("web_fetch"));
    }

    #[tokio::test]
    async fn explicit_activate() {
        clear_global();
        let tool = DiscoverToolsTool::new(test_registry());
        let result = tool
            .execute(json!({"query": "activate", "activate": ["shell", "file_write"]}))
            .await
            .unwrap();
        assert!(result.success);
        assert!(result.output.contains("Activated 2 tool(s)"));

        let activated = super::super::DISCOVER_TOOLS_ACTIVATED.lock();
        assert!(activated.contains("shell"));
        assert!(activated.contains("file_write"));
    }

    #[tokio::test]
    async fn auto_activates_matching_tools() {
        clear_global();
        let tool = DiscoverToolsTool::new(test_registry());
        let result = tool
            .execute(json!({"query": "I need to edit a file and check git status"}))
            .await
            .unwrap();
        assert!(result.success);
        assert!(result.output.contains("Auto-activated"));

        let activated = super::super::DISCOVER_TOOLS_ACTIVATED.lock();
        assert!(activated.contains("file_write") || activated.contains("file_edit"));
        assert!(activated.contains("git_operations"));
    }
}
