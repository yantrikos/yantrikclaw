//! YantrikDB memory backend — cognitive memory with relevance scoring,
//! consolidation, entity graphs, and cross-session continuity.
//!
//! Connects to a running Yantrik Companion server which has YantrikDB built in.
//! The companion exposes memory operations via HTTP endpoints that this backend
//! delegates to.

use crate::memory::traits::{Memory, MemoryCategory, MemoryEntry};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

const DEFAULT_COMPANION_URL: &str = "http://127.0.0.1:8080";

pub struct YantrikDbMemory {
    base_url: String,
    client: Client,
}

// ── HTTP request/response types ──────────────────────────────────────────────

#[derive(Debug, Serialize)]
struct StoreRequest {
    key: String,
    content: String,
    category: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    session_id: Option<String>,
}

#[derive(Debug, Serialize)]
struct RecallRequest {
    query: String,
    limit: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    session_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RecallResponse {
    memories: Vec<RecallEntry>,
}

#[derive(Debug, Deserialize)]
struct RecallEntry {
    #[serde(default)]
    id: String,
    #[serde(default)]
    key: String,
    content: String,
    #[serde(default)]
    category: String,
    #[serde(default)]
    timestamp: String,
    #[serde(default)]
    session_id: Option<String>,
    #[serde(default)]
    score: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct CountResponse {
    count: usize,
}

#[derive(Debug, Deserialize)]
struct ForgetResponse {
    deleted: bool,
}

#[derive(Debug, Deserialize)]
struct HealthResponse {
    ok: bool,
}

#[derive(Debug, Deserialize)]
struct StatusResponse {
    #[allow(dead_code)]
    status: String,
    memory_count: i64,
}

// ── Implementation ───────────────────────────────────────────────────────────

impl YantrikDbMemory {
    pub fn new(api_url: Option<&str>) -> Self {
        let base_url = api_url
            .map(|u| u.trim().trim_end_matches('/'))
            .filter(|u| !u.is_empty())
            .unwrap_or(DEFAULT_COMPANION_URL)
            .to_string();

        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap_or_else(|_| Client::new());

        Self { base_url, client }
    }

    fn parse_category(s: &str) -> MemoryCategory {
        match s {
            "core" => MemoryCategory::Core,
            "daily" => MemoryCategory::Daily,
            "conversation" => MemoryCategory::Conversation,
            other => MemoryCategory::Custom(other.to_string()),
        }
    }

    fn recall_entry_to_memory_entry(entry: RecallEntry) -> MemoryEntry {
        MemoryEntry {
            id: if entry.id.is_empty() {
                entry.key.clone()
            } else {
                entry.id
            },
            key: entry.key,
            content: entry.content,
            category: Self::parse_category(&entry.category),
            timestamp: entry.timestamp,
            session_id: entry.session_id,
            score: entry.score,
        }
    }
}

#[async_trait]
impl Memory for YantrikDbMemory {
    fn name(&self) -> &str {
        "yantrikdb"
    }

    async fn store(
        &self,
        key: &str,
        content: &str,
        category: MemoryCategory,
        session_id: Option<&str>,
    ) -> anyhow::Result<()> {
        let url = format!("{}/memory/store", self.base_url);
        debug!("yantrikdb store: key={}, {} chars", key, content.len());

        let res = self
            .client
            .post(&url)
            .json(&StoreRequest {
                key: key.to_string(),
                content: content.to_string(),
                category: category.to_string(),
                session_id: session_id.map(|s| s.to_string()),
            })
            .send()
            .await;

        match res {
            Ok(r) if r.status().is_success() => Ok(()),
            Ok(r) => {
                let status = r.status();
                let body = r.text().await.unwrap_or_default();
                anyhow::bail!("yantrikdb store error {status}: {body}");
            }
            Err(e) => {
                warn!("yantrikdb store failed: {e}");
                anyhow::bail!("yantrikdb store failed: {e}");
            }
        }
    }

    async fn recall(
        &self,
        query: &str,
        limit: usize,
        session_id: Option<&str>,
    ) -> anyhow::Result<Vec<MemoryEntry>> {
        let url = format!("{}/memory/recall", self.base_url);
        debug!("yantrikdb recall: query={}, limit={}", query, limit);

        let res = self
            .client
            .post(&url)
            .json(&RecallRequest {
                query: query.to_string(),
                limit,
                session_id: session_id.map(|s| s.to_string()),
            })
            .send()
            .await;

        match res {
            Ok(r) if r.status().is_success() => {
                let recall_res: RecallResponse = r.json().await?;
                Ok(recall_res
                    .memories
                    .into_iter()
                    .map(Self::recall_entry_to_memory_entry)
                    .collect())
            }
            Ok(r) => {
                let status = r.status();
                let body = r.text().await.unwrap_or_default();
                warn!("yantrikdb recall error {status}: {body}");
                Ok(Vec::new())
            }
            Err(e) => {
                warn!("yantrikdb recall failed: {e}");
                Ok(Vec::new())
            }
        }
    }

    async fn get(&self, key: &str) -> anyhow::Result<Option<MemoryEntry>> {
        let url = format!("{}/memory/{}", self.base_url, urlencoding::encode(key));

        match self.client.get(&url).send().await {
            Ok(r) if r.status().is_success() => {
                let entry: RecallEntry = r.json().await?;
                Ok(Some(Self::recall_entry_to_memory_entry(entry)))
            }
            Ok(r) if r.status() == reqwest::StatusCode::NOT_FOUND => Ok(None),
            Ok(r) => {
                let status = r.status();
                warn!("yantrikdb get error: {status}");
                Ok(None)
            }
            Err(e) => {
                warn!("yantrikdb get failed: {e}");
                Ok(None)
            }
        }
    }

    async fn list(
        &self,
        category: Option<&MemoryCategory>,
        session_id: Option<&str>,
    ) -> anyhow::Result<Vec<MemoryEntry>> {
        let mut url = format!("{}/memory/list", self.base_url);
        let mut params = Vec::new();
        if let Some(cat) = category {
            params.push(format!("category={}", cat));
        }
        if let Some(sid) = session_id {
            params.push(format!("session_id={}", urlencoding::encode(sid)));
        }
        if !params.is_empty() {
            url.push('?');
            url.push_str(&params.join("&"));
        }

        match self.client.get(&url).send().await {
            Ok(r) if r.status().is_success() => {
                let entries: Vec<RecallEntry> = r.json().await?;
                Ok(entries
                    .into_iter()
                    .map(Self::recall_entry_to_memory_entry)
                    .collect())
            }
            Ok(r) => {
                let status = r.status();
                warn!("yantrikdb list error: {status}");
                Ok(Vec::new())
            }
            Err(e) => {
                warn!("yantrikdb list failed: {e}");
                Ok(Vec::new())
            }
        }
    }

    async fn forget(&self, key: &str) -> anyhow::Result<bool> {
        let url = format!("{}/memory/{}", self.base_url, urlencoding::encode(key));
        debug!("yantrikdb forget: key={}", key);

        match self.client.delete(&url).send().await {
            Ok(r) if r.status().is_success() => {
                let res: ForgetResponse =
                    r.json().await.unwrap_or(ForgetResponse { deleted: true });
                Ok(res.deleted)
            }
            Ok(r) => {
                let status = r.status();
                warn!("yantrikdb forget error: {status}");
                Ok(false)
            }
            Err(e) => {
                warn!("yantrikdb forget failed: {e}");
                Ok(false)
            }
        }
    }

    async fn count(&self) -> anyhow::Result<usize> {
        // Use companion /status endpoint which includes memory_count.
        let url = format!("{}/status", self.base_url);

        match self.client.get(&url).send().await {
            Ok(r) if r.status().is_success() => {
                let status: StatusResponse = r.json().await?;
                Ok(status.memory_count as usize)
            }
            Ok(_) | Err(_) => {
                // Fallback: try dedicated count endpoint.
                let url = format!("{}/memory/count", self.base_url);
                match self.client.get(&url).send().await {
                    Ok(r) if r.status().is_success() => {
                        let res: CountResponse = r.json().await?;
                        Ok(res.count)
                    }
                    _ => Ok(0),
                }
            }
        }
    }

    async fn health_check(&self) -> bool {
        let url = format!("{}/health", self.base_url);
        match self.client.get(&url).send().await {
            Ok(r) if r.status().is_success() => r
                .json::<HealthResponse>()
                .await
                .map(|h| h.ok)
                .unwrap_or(false),
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_uses_default_url() {
        let mem = YantrikDbMemory::new(None);
        assert_eq!(mem.base_url, DEFAULT_COMPANION_URL);
    }

    #[test]
    fn new_uses_custom_url() {
        let mem = YantrikDbMemory::new(Some("http://myhost:9000/"));
        assert_eq!(mem.base_url, "http://myhost:9000");
    }

    #[test]
    fn parse_category_known() {
        assert_eq!(
            YantrikDbMemory::parse_category("core"),
            MemoryCategory::Core
        );
        assert_eq!(
            YantrikDbMemory::parse_category("daily"),
            MemoryCategory::Daily
        );
        assert_eq!(
            YantrikDbMemory::parse_category("conversation"),
            MemoryCategory::Conversation
        );
    }

    #[test]
    fn parse_category_custom() {
        assert_eq!(
            YantrikDbMemory::parse_category("project_notes"),
            MemoryCategory::Custom("project_notes".to_string())
        );
    }

    #[test]
    fn name_returns_yantrikdb() {
        let mem = YantrikDbMemory::new(None);
        assert_eq!(mem.name(), "yantrikdb");
    }

    #[test]
    fn recall_entry_to_memory_entry_maps_fields() {
        let entry = RecallEntry {
            id: "mem-123".to_string(),
            key: "favorite_food".to_string(),
            content: "pizza".to_string(),
            category: "core".to_string(),
            timestamp: "2026-03-20T00:00:00Z".to_string(),
            session_id: Some("sess-1".to_string()),
            score: Some(0.95),
        };

        let result = YantrikDbMemory::recall_entry_to_memory_entry(entry);
        assert_eq!(result.id, "mem-123");
        assert_eq!(result.key, "favorite_food");
        assert_eq!(result.content, "pizza");
        assert_eq!(result.category, MemoryCategory::Core);
        assert_eq!(result.score, Some(0.95));
    }

    #[test]
    fn recall_entry_falls_back_to_key_for_empty_id() {
        let entry = RecallEntry {
            id: String::new(),
            key: "mykey".to_string(),
            content: "value".to_string(),
            category: "daily".to_string(),
            timestamp: String::new(),
            session_id: None,
            score: None,
        };

        let result = YantrikDbMemory::recall_entry_to_memory_entry(entry);
        assert_eq!(result.id, "mykey");
    }
}
