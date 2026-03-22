use super::traits::{PermissionLevel, Tool, ToolResult};
use crate::security::CredentialVault;
use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;

/// List all credential keys in the encrypted vault (values are not shown).
pub struct VaultListTool {
    vault: Arc<CredentialVault>,
}

impl VaultListTool {
    pub fn new(vault: Arc<CredentialVault>) -> Self {
        Self { vault }
    }
}

#[async_trait]
impl Tool for VaultListTool {
    fn name(&self) -> &str {
        "vault_list"
    }

    fn description(&self) -> &str {
        "List all credential keys stored in the encrypted vault. \
         Only shows key names, not the secret values."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {}
        })
    }

    fn category(&self) -> &str {
        "security"
    }

    fn permission(&self) -> PermissionLevel {
        PermissionLevel::Safe
    }

    async fn execute(&self, _args: serde_json::Value) -> anyhow::Result<ToolResult> {
        match self.vault.list_keys() {
            Ok(keys) => {
                if keys.is_empty() {
                    Ok(ToolResult {
                        success: true,
                        output: "The vault is empty. No credentials stored.".into(),
                        error: None,
                    })
                } else {
                    let list = keys.join(", ");
                    Ok(ToolResult {
                        success: true,
                        output: format!("Stored credentials ({} total): {list}", keys.len()),
                        error: None,
                    })
                }
            }
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Failed to list vault keys: {e}")),
            }),
        }
    }
}
