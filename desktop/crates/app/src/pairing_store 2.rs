//! Persistent per-UDID `pairingKey` storage (ADR-011 §3.4).
//! File layout: TOML at `<config>/clearcam/pairings.toml`:
//!
//! ```toml
//! [pairings."UDID-1"]
//! pairing_key_b64 = "..."
//! paired_at = "2026-05-28T20:00:00Z"
//! ```

use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};

use base64::{engine::general_purpose::STANDARD_NO_PAD, Engine as _};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::RwLock;

#[derive(Debug, Error)]
pub enum PairingStoreError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("toml decode: {0}")]
    TomlDe(#[from] toml::de::Error),
    #[error("toml encode: {0}")]
    TomlSer(#[from] toml::ser::Error),
    #[error("base64 decode: {0}")]
    B64(#[from] base64::DecodeError),
    #[error("no config dir")]
    NoConfigDir,
}

#[derive(Clone, PartialEq, Eq)]
pub struct PairingMaterial(Vec<u8>);

impl fmt::Debug for PairingMaterial {
    /// Redacts the secret bytes so accidental `tracing::debug!(?material)` or
    /// `dbg!(...)` calls cannot leak the pairing key to logs or stderr.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("PairingMaterial")
            .field(&format_args!("<redacted {} bytes>", self.0.len()))
            .finish()
    }
}

impl PairingMaterial {
    pub fn new_random() -> Self {
        let mut buf = vec![0u8; 32];
        rand::thread_rng().fill_bytes(&mut buf);
        Self(buf)
    }

    pub fn bytes(&self) -> &[u8] {
        &self.0
    }

    pub fn as_b64(&self) -> String {
        STANDARD_NO_PAD.encode(&self.0)
    }

    pub fn from_b64(s: &str) -> Result<Self, PairingStoreError> {
        Ok(Self(STANDARD_NO_PAD.decode(s)?))
    }
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct OnDisk {
    pairings: HashMap<String, OnDiskEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
struct OnDiskEntry {
    pairing_key_b64: String,
    paired_at: String,
}

pub struct PairingStore {
    path: PathBuf,
    data: RwLock<OnDisk>,
}

impl PairingStore {
    pub async fn open_default() -> Result<Self, PairingStoreError> {
        let dirs = directories::ProjectDirs::from("dev", "ClearCam", "ClearCam")
            .ok_or(PairingStoreError::NoConfigDir)?;
        let path = dirs.config_dir().join("pairings.toml");
        Self::open_at(path).await
    }

    pub async fn open_at(path: impl AsRef<Path>) -> Result<Self, PairingStoreError> {
        let path = path.as_ref().to_path_buf();
        if let Some(dir) = path.parent() {
            tokio::fs::create_dir_all(dir).await?;
        }
        let data: OnDisk = if path.exists() {
            let text = tokio::fs::read_to_string(&path).await?;
            toml::from_str(&text)?
        } else {
            OnDisk::default()
        };
        Ok(Self {
            path,
            data: RwLock::new(data),
        })
    }

    pub async fn get(&self, udid: &str) -> Result<Option<PairingMaterial>, PairingStoreError> {
        let g = self.data.read().await;
        match g.pairings.get(udid) {
            Some(entry) => Ok(Some(PairingMaterial::from_b64(&entry.pairing_key_b64)?)),
            None => Ok(None),
        }
    }

    pub async fn put(&self, udid: &str, mat: PairingMaterial) -> Result<(), PairingStoreError> {
        let mut g = self.data.write().await;
        g.pairings.insert(
            udid.to_owned(),
            OnDiskEntry {
                pairing_key_b64: mat.as_b64(),
                paired_at: chrono::Utc::now().to_rfc3339(),
            },
        );
        let text = toml::to_string_pretty(&*g)?;
        write_atomically(&self.path, text).await
    }

    pub async fn forget(&self, udid: &str) -> Result<(), PairingStoreError> {
        let mut g = self.data.write().await;
        g.pairings.remove(udid);
        let text = toml::to_string_pretty(&*g)?;
        write_atomically(&self.path, text).await
    }

    pub async fn list(&self) -> Vec<String> {
        self.data.read().await.pairings.keys().cloned().collect()
    }
}

/// Crash-safe replacement: write to a sibling `.tmp` file, then rename onto
/// the destination. POSIX `rename` is atomic so a crash mid-write leaves the
/// previous good file in place rather than a truncated empty TOML.
async fn write_atomically(path: &Path, contents: String) -> Result<(), PairingStoreError> {
    let tmp = path.with_extension("toml.tmp");
    tokio::fs::write(&tmp, contents).await?;
    tokio::fs::rename(&tmp, path).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn round_trip_save_and_load_pairing_key() {
        let tmp = tempdir().unwrap();
        let store = PairingStore::open_at(tmp.path().join("pairings.toml"))
            .await
            .unwrap();
        store
            .put("UDID-A", PairingMaterial::new_random())
            .await
            .unwrap();
        let key = store.get("UDID-A").await.unwrap();
        assert!(key.is_some());

        let reopened = PairingStore::open_at(tmp.path().join("pairings.toml"))
            .await
            .unwrap();
        let key2 = reopened.get("UDID-A").await.unwrap();
        assert_eq!(key.unwrap().bytes(), key2.unwrap().bytes());
    }

    #[tokio::test]
    async fn forget_removes_pairing() {
        let tmp = tempdir().unwrap();
        let store = PairingStore::open_at(tmp.path().join("pairings.toml"))
            .await
            .unwrap();
        store
            .put("UDID-A", PairingMaterial::new_random())
            .await
            .unwrap();
        store.forget("UDID-A").await.unwrap();
        assert!(store.get("UDID-A").await.unwrap().is_none());
    }
}
