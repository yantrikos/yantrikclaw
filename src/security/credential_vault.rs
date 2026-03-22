//! Encrypted key-value credential vault built on top of [`SecretStore`].
//!
//! Stores credentials as a JSON file where each value is encrypted using
//! ChaCha20-Poly1305 via the existing `SecretStore`. The file lives at
//! `~/.yantrikclaw/credentials.json`.

use super::secrets::SecretStore;
use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

const CREDENTIALS_FILE: &str = "credentials.json";

/// An encrypted key-value credential store backed by a JSON file.
pub struct CredentialVault {
    path: PathBuf,
    store: SecretStore,
}

impl CredentialVault {
    /// Create a vault rooted at the given YantrikClaw config directory.
    pub fn new(yantrikclaw_dir: &Path, store: SecretStore) -> Self {
        Self {
            path: yantrikclaw_dir.join(CREDENTIALS_FILE),
            store,
        }
    }

    /// Store a credential. Encrypts the value and persists to disk.
    pub fn set(&self, key: &str, value: &str) -> Result<()> {
        let mut map = self.load_map()?;
        let encrypted = self
            .store
            .encrypt(value)
            .with_context(|| format!("failed to encrypt credential '{key}'"))?;
        map.insert(key.to_lowercase(), encrypted);
        self.save_map(&map)
    }

    /// Retrieve and decrypt a credential by key.
    pub fn get(&self, key: &str) -> Result<Option<String>> {
        let map = self.load_map()?;
        match map.get(&key.to_lowercase()) {
            Some(encrypted) => {
                let plaintext = self
                    .store
                    .decrypt(encrypted)
                    .with_context(|| format!("failed to decrypt credential '{key}'"))?;
                Ok(Some(plaintext))
            }
            None => Ok(None),
        }
    }

    /// Check if a credential key exists.
    pub fn exists(&self, key: &str) -> bool {
        self.load_map()
            .map(|m| m.contains_key(&key.to_lowercase()))
            .unwrap_or(false)
    }

    /// List all credential keys (not values).
    pub fn list_keys(&self) -> Result<Vec<String>> {
        let map = self.load_map()?;
        Ok(map.keys().cloned().collect())
    }

    /// Delete a credential by key. Returns true if it existed.
    pub fn remove(&self, key: &str) -> Result<bool> {
        let mut map = self.load_map()?;
        let existed = map.remove(&key.to_lowercase()).is_some();
        if existed {
            self.save_map(&map)?;
        }
        Ok(existed)
    }

    fn load_map(&self) -> Result<BTreeMap<String, String>> {
        if !self.path.exists() {
            return Ok(BTreeMap::new());
        }
        let data = fs::read_to_string(&self.path)
            .with_context(|| format!("failed to read {}", self.path.display()))?;
        let map: BTreeMap<String, String> =
            serde_json::from_str(&data).with_context(|| "invalid credentials.json format")?;
        Ok(map)
    }

    fn save_map(&self, map: &BTreeMap<String, String>) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(map)?;
        fs::write(&self.path, json)
            .with_context(|| format!("failed to write {}", self.path.display()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn test_vault(dir: &Path) -> CredentialVault {
        let store = SecretStore::new(dir, true);
        CredentialVault::new(dir, store)
    }

    #[test]
    fn store_and_retrieve() {
        let dir = tempdir().unwrap();
        let vault = test_vault(dir.path());
        vault.set("netflix", "hunter2").unwrap();
        assert_eq!(vault.get("netflix").unwrap(), Some("hunter2".to_string()));
    }

    #[test]
    fn case_insensitive_keys() {
        let dir = tempdir().unwrap();
        let vault = test_vault(dir.path());
        vault.set("Netflix", "pass123").unwrap();
        assert_eq!(vault.get("NETFLIX").unwrap(), Some("pass123".to_string()));
    }

    #[test]
    fn list_keys() {
        let dir = tempdir().unwrap();
        let vault = test_vault(dir.path());
        vault.set("aws", "key1").unwrap();
        vault.set("gmail", "key2").unwrap();
        let keys = vault.list_keys().unwrap();
        assert_eq!(keys, vec!["aws", "gmail"]);
    }

    #[test]
    fn exists_and_remove() {
        let dir = tempdir().unwrap();
        let vault = test_vault(dir.path());
        vault.set("temp", "val").unwrap();
        assert!(vault.exists("temp"));
        assert!(vault.remove("temp").unwrap());
        assert!(!vault.exists("temp"));
        assert!(!vault.remove("temp").unwrap());
    }

    #[test]
    fn get_nonexistent() {
        let dir = tempdir().unwrap();
        let vault = test_vault(dir.path());
        assert_eq!(vault.get("nope").unwrap(), None);
    }
}
