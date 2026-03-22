use super::traits::{PermissionLevel, Tool, ToolResult};
use crate::security::CredentialVault;
use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;

/// Delete a credential from the encrypted vault.
pub struct VaultDeleteTool {
    vault: Arc<CredentialVault>,
}

impl VaultDeleteTool {
    pub fn new(vault: Arc<CredentialVault>) -> Self {
        Self { vault }
    }
}

#[async_trait]
impl Tool for VaultDeleteTool {
    fn name(&self) -> &str {
        "vault_delete"
    }

    fn description(&self) -> &str {
        "Delete a credential from the encrypted vault."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "key": {
                    "type": "string",
                    "description": "The credential key to delete"
                }
            },
            "required": ["key"]
        })
    }

    fn category(&self) -> &str {
        "security"
    }

    fn permission(&self) -> PermissionLevel {
        PermissionLevel::Standard
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let key = args
            .get("key")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'key' parameter"))?
            .trim()
            .to_lowercase();

        if key.is_empty() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("'key' is required.".into()),
            });
        }

        match self.vault.remove(&key) {
            Ok(true) => Ok(ToolResult {
                success: true,
                output: format!("Credential '{key}' deleted from the vault."),
                error: None,
            }),
            Ok(false) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Credential '{key}' not found in the vault.")),
            }),
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Failed to delete credential: {e}")),
            }),
        }
    }
}
