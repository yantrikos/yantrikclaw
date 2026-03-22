use super::traits::{PermissionLevel, Tool, ToolResult};
use crate::security::{CredentialVault, OtpValidator};
use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;

/// Retrieve a credential from the encrypted vault, gated by TOTP authenticator code.
pub struct VaultGetTool {
    vault: Arc<CredentialVault>,
    otp: Option<Arc<OtpValidator>>,
}

impl VaultGetTool {
    pub fn new(vault: Arc<CredentialVault>, otp: Option<Arc<OtpValidator>>) -> Self {
        Self { vault, otp }
    }
}

#[async_trait]
impl Tool for VaultGetTool {
    fn name(&self) -> &str {
        "vault_get"
    }

    fn description(&self) -> &str {
        "Retrieve a credential from the encrypted vault. Requires a 6-digit \
         authenticator code (from Google Authenticator, Authy, etc.) for verification. \
         Ask the user for their current authenticator code before calling this tool."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "key": {
                    "type": "string",
                    "description": "The credential key to retrieve (e.g. 'netflix', 'gmail')"
                },
                "otp_code": {
                    "type": "string",
                    "description": "The 6-digit code from the user's authenticator app"
                }
            },
            "required": ["key", "otp_code"]
        })
    }

    fn category(&self) -> &str {
        "security"
    }

    fn permission(&self) -> PermissionLevel {
        PermissionLevel::Sensitive
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let key = args
            .get("key")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'key' parameter"))?
            .trim()
            .to_lowercase();
        let otp_code = args
            .get("otp_code")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'otp_code' parameter"))?
            .trim();

        if key.is_empty() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("'key' is required.".into()),
            });
        }

        if otp_code.is_empty() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("'otp_code' is required. Ask the user for their authenticator code.".into()),
            });
        }

        // Verify OTP if enabled
        if let Some(ref otp) = self.otp {
            match otp.validate(otp_code) {
                Ok(true) => {} // valid
                Ok(false) => {
                    return Ok(ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some(
                            "Invalid authenticator code. Ask the user to check their app and try again."
                                .into(),
                        ),
                    });
                }
                Err(e) => {
                    return Ok(ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some(format!("OTP validation error: {e}")),
                    });
                }
            }
        }

        // Check key exists
        if !self.vault.exists(&key) {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("No credential named '{key}' found in the vault.")),
            });
        }

        // Retrieve credential
        match self.vault.get(&key) {
            Ok(Some(value)) => Ok(ToolResult {
                success: true,
                output: value,
                error: None,
            }),
            Ok(None) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Credential '{key}' not found.")),
            }),
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Failed to retrieve credential: {e}")),
            }),
        }
    }
}
