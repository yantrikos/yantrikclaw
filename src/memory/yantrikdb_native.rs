//! YantrikDB Native memory backend — in-process cognitive memory engine.
//!
//! Uses the `yantrikdb` crate directly (no HTTP, no companion server).
//! Opens a local SQLite database and provides semantic recall, relevance
//! scoring, consolidation, and entity graphs — all in-process.

use crate::memory::traits::{Memory, MemoryCategory, MemoryEntry};
use async_trait::async_trait;
use parking_lot::Mutex;
use std::path::PathBuf;
use tracing::{debug, warn};
use yantrikdb::YantrikDB;

const DEFAULT_EMBEDDING_DIM: usize = 384;
const DEFAULT_DB_FILENAME: &str = "yantrikdb.sqlite";

pub struct YantrikDbNativeMemory {
    db: Mutex<YantrikDB>,
    #[allow(dead_code)]
    db_path: PathBuf,
}

impl YantrikDbNativeMemory {
    pub fn new(workspace_dir: &std::path::Path) -> anyhow::Result<Self> {
        let db_path = workspace_dir.join(DEFAULT_DB_FILENAME);
        let db_path_str = db_path
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("invalid UTF-8 in workspace path"))?;

        let db = YantrikDB::new(db_path_str, DEFAULT_EMBEDDING_DIM)
            .map_err(|e| anyhow::anyhow!("failed to open YantrikDB at {db_path_str}: {e}"))?;

        debug!("yantrikdb-native opened at {db_path_str}");

        Ok(Self {
            db: Mutex::new(db),
            db_path,
        })
    }

    fn category_to_memory_type(category: &MemoryCategory) -> &str {
        match category {
            MemoryCategory::Core => "fact",
            MemoryCategory::Daily => "episode",
            MemoryCategory::Conversation => "episode",
            MemoryCategory::Custom(name) => name.as_str(),
        }
    }

    fn memory_type_to_category(memory_type: &str) -> MemoryCategory {
        match memory_type {
            "fact" => MemoryCategory::Core,
            "episode" => MemoryCategory::Daily,
            other => MemoryCategory::Custom(other.to_string()),
        }
    }
}

#[async_trait]
impl Memory for YantrikDbNativeMemory {
    fn name(&self) -> &str {
        "yantrikdb-native"
    }

    async fn store(
        &self,
        key: &str,
        content: &str,
        category: MemoryCategory,
        session_id: Option<&str>,
    ) -> anyhow::Result<()> {
        let memory_type = Self::category_to_memory_type(&category);
        let importance = match category {
            MemoryCategory::Core => 0.8,
            MemoryCategory::Daily => 0.5,
            MemoryCategory::Conversation => 0.4,
            MemoryCategory::Custom(_) => 0.6,
        };

        let metadata = serde_json::json!({
            "key": key,
            "session_id": session_id,
        });

        let db = self.db.lock();
        match db.record_text(
            content,
            memory_type,
            importance,
            0.0,   // valence (neutral)
            168.0, // half_life (1 week in hours)
            &metadata,
            "default",
            0.9,   // certainty
            "general",
            "yantrikclaw",
            None, // emotional_state
        ) {
            Ok(rid) => {
                debug!("yantrikdb-native stored: key={key}, rid={rid}");
                Ok(())
            }
            Err(e) => {
                warn!("yantrikdb-native store failed: {e}");
                anyhow::bail!("yantrikdb-native store failed: {e}")
            }
        }
    }

    async fn recall(
        &self,
        query: &str,
        limit: usize,
        _session_id: Option<&str>,
    ) -> anyhow::Result<Vec<MemoryEntry>> {
        let db = self.db.lock();
        match db.recall_text(query, limit) {
            Ok(results) => {
                debug!(
                    "yantrikdb-native recall: query={}, {} results",
                    query,
                    results.len()
                );
                Ok(results
                    .into_iter()
                    .map(|r| {
                        let key = r
                            .metadata
                            .get("key")
                            .and_then(|v| v.as_str())
                            .unwrap_or(&r.rid)
                            .to_string();
                        let session_id = r
                            .metadata
                            .get("session_id")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());

                        MemoryEntry {
                            id: r.rid,
                            key,
                            content: r.text,
                            category: Self::memory_type_to_category(&r.memory_type),
                            timestamp: format_epoch(r.created_at),
                            session_id,
                            score: Some(r.score),
                        }
                    })
                    .collect())
            }
            Err(e) => {
                warn!("yantrikdb-native recall failed: {e}");
                Ok(Vec::new())
            }
        }
    }

    async fn get(&self, key: &str) -> anyhow::Result<Option<MemoryEntry>> {
        // YantrikDB doesn't have a direct get-by-key — do a targeted recall.
        let db = self.db.lock();
        match db.recall_text(key, 5) {
            Ok(results) => {
                let entry = results.into_iter().find(|r| {
                    r.metadata
                        .get("key")
                        .and_then(|v| v.as_str())
                        .map_or(false, |k| k == key)
                });
                Ok(entry.map(|r| {
                    let session_id = r
                        .metadata
                        .get("session_id")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    MemoryEntry {
                        id: r.rid,
                        key: key.to_string(),
                        content: r.text,
                        category: Self::memory_type_to_category(&r.memory_type),
                        timestamp: format_epoch(r.created_at),
                        session_id,
                        score: Some(r.score),
                    }
                }))
            }
            Err(e) => {
                warn!("yantrikdb-native get failed: {e}");
                Ok(None)
            }
        }
    }

    async fn list(
        &self,
        _category: Option<&MemoryCategory>,
        _session_id: Option<&str>,
    ) -> anyhow::Result<Vec<MemoryEntry>> {
        // YantrikDB doesn't have a list-all API — return a broad recall.
        let db = self.db.lock();
        match db.recall_text("*", 100) {
            Ok(results) => Ok(results
                .into_iter()
                .map(|r| {
                    let key = r
                        .metadata
                        .get("key")
                        .and_then(|v| v.as_str())
                        .unwrap_or(&r.rid)
                        .to_string();
                    let session_id = r
                        .metadata
                        .get("session_id")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    MemoryEntry {
                        id: r.rid,
                        key,
                        content: r.text,
                        category: Self::memory_type_to_category(&r.memory_type),
                        timestamp: format_epoch(r.created_at),
                        session_id,
                        score: Some(r.score),
                    }
                })
                .collect()),
            Err(e) => {
                warn!("yantrikdb-native list failed: {e}");
                Ok(Vec::new())
            }
        }
    }

    async fn forget(&self, key: &str) -> anyhow::Result<bool> {
        // First find the RID by key, then tombstone it.
        let db = self.db.lock();
        match db.recall_text(key, 5) {
            Ok(results) => {
                let entry = results.into_iter().find(|r| {
                    r.metadata
                        .get("key")
                        .and_then(|v| v.as_str())
                        .map_or(false, |k| k == key)
                });
                if let Some(found) = entry {
                    match db.forget(&found.rid) {
                        Ok(deleted) => {
                            debug!("yantrikdb-native forget: key={key}, rid={}, deleted={deleted}", found.rid);
                            Ok(deleted)
                        }
                        Err(e) => {
                            warn!("yantrikdb-native forget failed: {e}");
                            Ok(false)
                        }
                    }
                } else {
                    Ok(false)
                }
            }
            Err(e) => {
                warn!("yantrikdb-native forget lookup failed: {e}");
                Ok(false)
            }
        }
    }

    async fn count(&self) -> anyhow::Result<usize> {
        let db = self.db.lock();
        match db.stats(None) {
            Ok(stats) => Ok(stats.active_memories as usize),
            Err(e) => {
                warn!("yantrikdb-native count failed: {e}");
                Ok(0)
            }
        }
    }

    async fn health_check(&self) -> bool {
        let db = self.db.lock();
        db.stats(None).is_ok()
    }
}

fn format_epoch(epoch_secs: f64) -> String {
    chrono::DateTime::from_timestamp(epoch_secs as i64, 0)
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn new_creates_db_in_workspace() {
        let tmp = TempDir::new().unwrap();
        let mem = YantrikDbNativeMemory::new(tmp.path()).unwrap();
        assert_eq!(mem.name(), "yantrikdb-native");
        assert!(tmp.path().join(DEFAULT_DB_FILENAME).exists());
    }

    #[test]
    fn category_to_memory_type_mappings() {
        assert_eq!(
            YantrikDbNativeMemory::category_to_memory_type(&MemoryCategory::Core),
            "fact"
        );
        assert_eq!(
            YantrikDbNativeMemory::category_to_memory_type(&MemoryCategory::Daily),
            "episode"
        );
        assert_eq!(
            YantrikDbNativeMemory::category_to_memory_type(&MemoryCategory::Conversation),
            "episode"
        );
        assert_eq!(
            YantrikDbNativeMemory::category_to_memory_type(&MemoryCategory::Custom(
                "notes".into()
            )),
            "notes"
        );
    }

    #[test]
    fn memory_type_to_category_mappings() {
        assert_eq!(
            YantrikDbNativeMemory::memory_type_to_category("fact"),
            MemoryCategory::Core
        );
        assert_eq!(
            YantrikDbNativeMemory::memory_type_to_category("episode"),
            MemoryCategory::Daily
        );
        assert_eq!(
            YantrikDbNativeMemory::memory_type_to_category("custom_type"),
            MemoryCategory::Custom("custom_type".into())
        );
    }

    #[tokio::test]
    async fn store_and_recall_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let mem = YantrikDbNativeMemory::new(tmp.path()).unwrap();

        mem.store("fav_lang", "Rust is my favorite language", MemoryCategory::Core, None)
            .await
            .unwrap();

        let results = mem.recall("favorite language", 5, None).await.unwrap();
        assert!(!results.is_empty());
        assert!(results[0].content.contains("Rust"));
    }

    #[tokio::test]
    async fn store_and_count() {
        let tmp = TempDir::new().unwrap();
        let mem = YantrikDbNativeMemory::new(tmp.path()).unwrap();

        assert_eq!(mem.count().await.unwrap(), 0);

        mem.store("k1", "first memory", MemoryCategory::Core, None)
            .await
            .unwrap();
        mem.store("k2", "second memory", MemoryCategory::Daily, None)
            .await
            .unwrap();

        assert_eq!(mem.count().await.unwrap(), 2);
    }

    #[tokio::test]
    async fn health_check_returns_true() {
        let tmp = TempDir::new().unwrap();
        let mem = YantrikDbNativeMemory::new(tmp.path()).unwrap();
        assert!(mem.health_check().await);
    }

    #[tokio::test]
    async fn store_and_forget() {
        let tmp = TempDir::new().unwrap();
        let mem = YantrikDbNativeMemory::new(tmp.path()).unwrap();

        mem.store("to_forget", "temporary fact", MemoryCategory::Core, None)
            .await
            .unwrap();

        assert_eq!(mem.count().await.unwrap(), 1);

        let deleted = mem.forget("to_forget").await.unwrap();
        assert!(deleted);
    }
}
