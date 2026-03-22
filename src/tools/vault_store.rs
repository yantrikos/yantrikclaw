use super::traits::{PermissionLevel, Tool, ToolResult};
use crate::security::CredentialVault;
use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;

/// Store a credential in the encrypted vault.
pub struct VaultStoreTool {
    vault: Arc<CredentialVault>,
}

impl VaultStoreTool {
    pub fn new(vault: Arc<CredentialVault>) -> Self {
        Self { vault }
    }
}

#[async_trait]
impl Tool for VaultStoreTool {
    fn name(&self) -> &str {
        "vault_store"
    }

    fn description(&self) -> &str {
        "Store a credential (password, API key, secret) in the encrypted vault. \
         The value is encrypted with ChaCha20-Poly1305 and persisted to disk."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "key": {
                    "type": "string",
                    "description": "Name/label for the credential (e.g. 'netflix', 'gmail', 'aws')"
                },
                "value": {
                    "type": "string",
                    "description": "The secret value to store"
                }
            },
            "required": ["key", "value"]
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
        let value = args
            .get("value")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'value' parameter"))?;

        if key.is_empty() || value.is_empty() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("Both 'key' and 'value' are required.".into()),
            });
        }

        match self.vault.set(&key, value) {
            Ok(()) => Ok(ToolResult {
                success: true,
                output: format!("Credential '{key}' stored in the encrypted vault."),
                error: None,
            }),
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Failed to store credential: {e}")),
            }),
        }
    }
}
