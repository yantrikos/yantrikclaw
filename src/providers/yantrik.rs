//! Yantrik Companion provider — bridges ZeroClaw to the Yantrik cognitive brain.
//!
//! The companion binary handles LLM inference, cognitive memory (YantrikDB),
//! tool execution (50+ tools), bond tracking, personality evolution, and
//! proactive cognition (urge pipeline). This provider forwards chat requests
//! to the companion's HTTP API and maps responses back to ZeroClaw's types.

use crate::providers::traits::{
    ChatMessage, Provider, ProviderCapabilities, StreamChunk, StreamOptions, StreamResult,
};
use async_trait::async_trait;
use futures_util::{stream, StreamExt};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

const DEFAULT_COMPANION_URL: &str = "http://127.0.0.1:8080";

pub struct YantrikProvider {
    base_url: String,
    client: Client,
}

// ── Companion HTTP API types ─────────────────────────────────────────────────

#[derive(Debug, Serialize)]
struct CompanionChatRequest {
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    context: Option<CompanionChatContext>,
}

#[derive(Debug, Serialize)]
struct CompanionChatContext {
    #[serde(skip_serializing_if = "Option::is_none")]
    session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    channel: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    user_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CompanionChatResponse {
    response: String,
    #[serde(default)]
    #[allow(dead_code)]
    proactive_messages: Vec<serde_json::Value>,
    #[serde(default)]
    metadata: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct CompanionHealthResponse {
    ok: bool,
}

// ── OpenAI-compatible streaming types (companion /v1/chat/completions) ───────

#[derive(Debug, Serialize)]
struct OaiStreamRequest {
    model: String,
    messages: Vec<OaiMessage>,
    stream: bool,
}

#[derive(Debug, Serialize)]
struct OaiMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct OaiStreamChunk {
    choices: Vec<OaiStreamChoice>,
}

#[derive(Debug, Deserialize)]
struct OaiStreamChoice {
    delta: OaiDelta,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OaiDelta {
    content: Option<String>,
}

// ── Implementation ───────────────────────────────────────────────────────────

impl YantrikProvider {
    pub fn new(api_url: Option<&str>, _api_key: Option<&str>) -> Self {
        let base_url = api_url
            .map(|u| u.trim().trim_end_matches('/'))
            .filter(|u| !u.is_empty())
            .unwrap_or(DEFAULT_COMPANION_URL)
            .to_string();

        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .unwrap_or_else(|_| Client::new());

        Self { base_url, client }
    }

    /// Extract the last user message from a chat history.
    fn extract_user_message(messages: &[ChatMessage]) -> String {
        messages
            .iter()
            .rfind(|m| m.role == "user")
            .map(|m| m.content.clone())
            .unwrap_or_default()
    }

    /// POST /chat — non-streaming, companion handles tools/memory internally.
    async fn chat_complete(&self, user_text: &str) -> anyhow::Result<CompanionChatResponse> {
        let url = format!("{}/chat", self.base_url);
        debug!("yantrik /chat request: {} chars", user_text.len());

        // Retry on 503 (companion busy with think cycle) up to 3 times.
        let mut last_err = None;
        for attempt in 0..3 {
            let res = self
                .client
                .post(&url)
                .json(&CompanionChatRequest {
                    message: user_text.to_string(),
                    context: None,
                })
                .send()
                .await;

            match res {
                Ok(r) if r.status() == reqwest::StatusCode::SERVICE_UNAVAILABLE && attempt < 2 => {
                    warn!(
                        "yantrik companion busy (503), retrying in 10s (attempt {})",
                        attempt + 1
                    );
                    tokio::time::sleep(std::time::Duration::from_secs(10)).await;
                    continue;
                }
                Ok(r) if !r.status().is_success() => {
                    let status = r.status();
                    let body = r.text().await.unwrap_or_default();
                    anyhow::bail!("yantrik companion error {status}: {body}");
                }
                Ok(r) => {
                    let companion_res: CompanionChatResponse = r
                        .json()
                        .await
                        .map_err(|e| anyhow::anyhow!("failed to parse companion response: {e}"))?;

                    let memories = companion_res
                        .metadata
                        .get("memories_recalled")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0);
                    let tools = companion_res
                        .metadata
                        .get("tool_calls_made")
                        .and_then(|v| v.as_array())
                        .map(|a| a.len())
                        .unwrap_or(0);

                    debug!(
                        "yantrik response: {} chars, {} memories, {} tools",
                        companion_res.response.len(),
                        memories,
                        tools,
                    );

                    return Ok(companion_res);
                }
                Err(e) => {
                    last_err = Some(e);
                    if attempt < 2 {
                        warn!(
                            "yantrik companion request failed, retrying in 5s (attempt {})",
                            attempt + 1
                        );
                        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    }
                }
            }
        }

        Err(anyhow::anyhow!(
            "yantrik companion request failed after 3 attempts: {}",
            last_err.map(|e| e.to_string()).unwrap_or_default()
        ))
    }
}

#[async_trait]
impl Provider for YantrikProvider {
    fn capabilities(&self) -> ProviderCapabilities {
        // Companion handles tools internally — from ZeroClaw's perspective,
        // it's a text-in/text-out provider. No native tool calling needed
        // because tools are executed inside the companion binary.
        ProviderCapabilities {
            native_tool_calling: false,
            vision: false,
            prompt_caching: false,
        }
    }

    async fn chat_with_system(
        &self,
        _system_prompt: Option<&str>,
        message: &str,
        _model: &str,
        _temperature: f64,
    ) -> anyhow::Result<String> {
        // The companion manages its own system prompt, personality, and
        // temperature. We just forward the user message.
        let res = self.chat_complete(message).await?;
        Ok(res.response.replace("__REPLACE__", ""))
    }

    async fn chat_with_history(
        &self,
        messages: &[ChatMessage],
        _model: &str,
        _temperature: f64,
    ) -> anyhow::Result<String> {
        let user_text = Self::extract_user_message(messages);
        if user_text.is_empty() {
            return Ok(String::new());
        }
        let res = self.chat_complete(&user_text).await?;
        Ok(res.response.replace("__REPLACE__", ""))
    }

    fn supports_streaming(&self) -> bool {
        true
    }

    fn stream_chat_with_system(
        &self,
        _system_prompt: Option<&str>,
        message: &str,
        model: &str,
        _temperature: f64,
        _options: StreamOptions,
    ) -> stream::BoxStream<'static, StreamResult<StreamChunk>> {
        let url = format!("{}/v1/chat/completions", self.base_url);
        let client = self.client.clone();
        let model_name = if model.is_empty() {
            "yantrik".to_string()
        } else {
            model.to_string()
        };
        let message = message.to_string();

        let (tx, rx) = tokio::sync::mpsc::channel::<StreamResult<StreamChunk>>(100);

        tokio::spawn(async move {
            let req_body = OaiStreamRequest {
                model: model_name,
                messages: vec![OaiMessage {
                    role: "user".to_string(),
                    content: message,
                }],
                stream: true,
            };

            let res = match client
                .post(&url)
                .header("Accept", "text/event-stream")
                .json(&req_body)
                .send()
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    let _ = tx
                        .send(Ok(StreamChunk::error(format!("yantrik stream error: {e}"))))
                        .await;
                    return;
                }
            };

            if !res.status().is_success() {
                let status = res.status();
                let body = res.text().await.unwrap_or_default();
                let _ = tx
                    .send(Ok(StreamChunk::error(format!(
                        "yantrik stream {status}: {body}"
                    ))))
                    .await;
                return;
            }

            let mut bytes_stream = res.bytes_stream();
            let mut buffer = String::new();

            while let Some(chunk_result) = bytes_stream.next().await {
                let chunk_bytes = match chunk_result {
                    Ok(b) => b,
                    Err(e) => {
                        let _ = tx
                            .send(Ok(StreamChunk::error(format!("stream read error: {e}"))))
                            .await;
                        return;
                    }
                };

                buffer.push_str(&String::from_utf8_lossy(&chunk_bytes));

                // Parse SSE lines from buffer.
                while let Some(line_end) = buffer.find('\n') {
                    let line = buffer[..line_end].trim().to_string();
                    buffer = buffer[line_end + 1..].to_string();

                    if line.is_empty() || line.starts_with(':') {
                        continue;
                    }

                    if let Some(data) = line.strip_prefix("data: ") {
                        let data = data.trim();
                        if data == "[DONE]" {
                            let _ = tx.send(Ok(StreamChunk::final_chunk())).await;
                            return;
                        }

                        if let Ok(chunk) = serde_json::from_str::<OaiStreamChunk>(data) {
                            for choice in &chunk.choices {
                                if let Some(ref content) = choice.delta.content {
                                    if !content.is_empty() && content != "__REPLACE__" {
                                        if tx
                                            .send(Ok(
                                                StreamChunk::delta(content).with_token_estimate()
                                            ))
                                            .await
                                            .is_err()
                                        {
                                            return; // Receiver dropped.
                                        }
                                    }
                                }
                                if choice.finish_reason.as_deref() == Some("stop") {
                                    let _ = tx.send(Ok(StreamChunk::final_chunk())).await;
                                    return;
                                }
                            }
                        }
                    }
                }
            }

            let _ = tx.send(Ok(StreamChunk::final_chunk())).await;
        });

        // Convert channel receiver to stream.
        stream::unfold(rx, |mut rx| async move {
            rx.recv().await.map(|chunk| (chunk, rx))
        })
        .boxed()
    }

    fn stream_chat_with_history(
        &self,
        messages: &[ChatMessage],
        model: &str,
        temperature: f64,
        options: StreamOptions,
    ) -> stream::BoxStream<'static, StreamResult<StreamChunk>> {
        let user_text = Self::extract_user_message(messages);
        self.stream_chat_with_system(None, &user_text, model, temperature, options)
    }

    async fn warmup(&self) -> anyhow::Result<()> {
        let url = format!("{}/health", self.base_url);
        match self.client.get(&url).send().await {
            Ok(res) if res.status().is_success() => {
                let health: CompanionHealthResponse = res
                    .json()
                    .await
                    .unwrap_or(CompanionHealthResponse { ok: false });
                if health.ok {
                    debug!("yantrik companion health check OK");
                } else {
                    warn!("yantrik companion health check returned ok=false");
                }
            }
            Ok(res) => {
                warn!("yantrik companion health check returned {}", res.status());
            }
            Err(e) => {
                warn!("yantrik companion health check failed: {e}");
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_user_message_gets_last_user() {
        let messages = vec![
            ChatMessage::user("hello"),
            ChatMessage::assistant("hi"),
            ChatMessage::user("how are you?"),
        ];
        assert_eq!(
            YantrikProvider::extract_user_message(&messages),
            "how are you?"
        );
    }

    #[test]
    fn extract_user_message_empty_when_no_user() {
        let messages = vec![ChatMessage::assistant("hi")];
        assert!(YantrikProvider::extract_user_message(&messages).is_empty());
    }

    #[test]
    fn new_uses_default_url_when_none() {
        let provider = YantrikProvider::new(None, None);
        assert_eq!(provider.base_url, DEFAULT_COMPANION_URL);
    }

    #[test]
    fn new_uses_custom_url() {
        let provider = YantrikProvider::new(Some("http://myhost:9000/"), None);
        assert_eq!(provider.base_url, "http://myhost:9000");
    }

    #[test]
    fn capabilities_no_native_tools() {
        let provider = YantrikProvider::new(None, None);
        let caps = provider.capabilities();
        assert!(!caps.native_tool_calling);
        assert!(!caps.vision);
    }
}
