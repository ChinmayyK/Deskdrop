use crate::crypto::fingerprint_of;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum TrustState {
    #[default]
    Untrusted,
    Trusted,
    Rejected,
    Revoked,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TrustRecord {
    pub device_id: Uuid,
    pub device_name: String,
    #[serde(default)]
    pub display_name: Option<String>,
    pub public_key: [u8; 32],
    pub key_fingerprint: [u8; 32],
    pub state: TrustState,
    pub first_seen: u64,
    pub trusted_since: Option<u64>,
    pub last_seen: u64,
}

impl Default for TrustRecord {
    fn default() -> Self {
        Self {
            device_id: Uuid::nil(),
            device_name: String::new(),
            display_name: None,
            public_key: [0; 32],
            key_fingerprint: [0; 32],
            state: TrustState::Untrusted,
            first_seen: 0,
            trusted_since: None,
            last_seen: 0,
        }
    }
}

impl TrustRecord {
    pub fn effective_name(&self) -> &str {
        self.display_name
            .as_deref()
            .filter(|name| !name.is_empty())
            .unwrap_or(&self.device_name)
    }
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct StoreData {
    devices: HashMap<Uuid, TrustRecord>,
}

pub struct TrustStore {
    data: StoreData,
    path: PathBuf,
}

impl TrustStore {
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let mut data = if path.exists() {
            let bytes = std::fs::read(&path).context("reading trust store")?;
            if bytes.is_empty() {
                StoreData::default()
            } else {
                serde_json::from_slice(&bytes).context("parsing trust store")?
            }
        } else {
            StoreData::default()
        };
        for record in data.devices.values_mut() {
            if record.state == TrustState::Untrusted
                && record.trusted_since.is_some()
                && record.key_fingerprint != [0; 32]
            {
                record.state = TrustState::Trusted;
            }
        }
        Ok(Self { data, path })
    }

    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).context("creating trust dir")?;
        }
        let tmp = self.path.with_extension("tmp");
        let bytes = serde_json::to_vec_pretty(&self.data)?;
        std::fs::write(&tmp, &bytes).context("writing trust store")?;
        std::fs::rename(&tmp, &self.path).context("renaming trust store")?;
        Ok(())
    }

    pub fn observe_peer(
        &mut self,
        device_id: Uuid,
        device_name: String,
        public_key: &[u8; 32],
    ) -> Result<TrustRecord> {
        let fingerprint = fingerprint_of(public_key);
        let now = now_secs();
        self.migrate_matching_identity(device_id, device_name.clone(), public_key, fingerprint, now)?;
        let record = self
            .data
            .devices
            .entry(device_id)
            .or_insert_with(|| TrustRecord {
                device_id,
                device_name: device_name.clone(),
                display_name: None,
                public_key: *public_key,
                key_fingerprint: fingerprint,
                state: TrustState::Untrusted,
                first_seen: now,
                trusted_since: None,
                last_seen: now,
            });

        if record.key_fingerprint != [0; 32] && record.key_fingerprint != fingerprint {
            anyhow::bail!(
                "fingerprint mismatch for device {}: expected {}, got {}",
                device_id,
                format_fingerprint(&record.key_fingerprint),
                format_fingerprint(&fingerprint)
            );
        }

        record.device_name = device_name;
        record.public_key = *public_key;
        record.key_fingerprint = fingerprint;
        record.last_seen = now;
        let snapshot = record.clone();
        self.save()?;
        Ok(snapshot)
    }

    fn migrate_matching_identity(
        &mut self,
        device_id: Uuid,
        device_name: String,
        public_key: &[u8; 32],
        fingerprint: [u8; 32],
        now: u64,
    ) -> Result<()> {
        if self.data.devices.contains_key(&device_id) {
            return Ok(());
        }

        let previous_id = self
            .data
            .devices
            .iter()
            .find_map(|(existing_id, record)| {
                if record.public_key == *public_key || record.key_fingerprint == fingerprint {
                    Some(*existing_id)
                } else {
                    None
                }
            });

        let Some(previous_id) = previous_id else {
            return Ok(());
        };

        if let Some(mut record) = self.data.devices.remove(&previous_id) {
            record.device_id = device_id;
            record.device_name = device_name;
            record.public_key = *public_key;
            record.key_fingerprint = fingerprint;
            record.last_seen = now;
            self.data.devices.insert(device_id, record);
        }

        Ok(())
    }

    pub fn is_trusted(&self, device_id: Uuid) -> bool {
        self.data
            .devices
            .get(&device_id)
            .map(|record| record.state == TrustState::Trusted)
            .unwrap_or(false)
    }

    pub fn trust_peer(&mut self, device_id: Uuid) -> Result<Option<TrustRecord>> {
        let now = now_secs();
        let snapshot = self.data.devices.get_mut(&device_id).map(|record| {
            record.state = TrustState::Trusted;
            if record.trusted_since.is_none() {
                record.trusted_since = Some(now);
            }
            record.last_seen = now;
            record.clone()
        });
        if snapshot.is_some() {
            self.save()?;
        }
        Ok(snapshot)
    }

    pub fn reject_peer(&mut self, device_id: Uuid) -> Result<Option<TrustRecord>> {
        let snapshot = self.data.devices.get_mut(&device_id).map(|record| {
            record.state = TrustState::Rejected;
            record.clone()
        });
        if snapshot.is_some() {
            self.save()?;
        }
        Ok(snapshot)
    }

    pub fn revoke_peer(&mut self, device_id: Uuid) -> Result<bool> {
        let changed = if let Some(record) = self.data.devices.get_mut(&device_id) {
            record.state = TrustState::Revoked;
            true
        } else {
            false
        };
        if changed {
            self.save()?;
        }
        Ok(changed)
    }

    pub fn get(&self, device_id: Uuid) -> Option<&TrustRecord> {
        self.data.devices.get(&device_id)
    }

    pub fn rename_peer(
        &mut self,
        device_id: Uuid,
        display_name: String,
    ) -> Result<Option<TrustRecord>> {
        let snapshot = self.data.devices.get_mut(&device_id).map(|record| {
            record.display_name = Some(display_name);
            record.clone()
        });
        if snapshot.is_some() {
            self.save()?;
        }
        Ok(snapshot)
    }

    pub fn all_devices(&self) -> impl Iterator<Item = &TrustRecord> {
        self.data.devices.values()
    }

    pub fn device_count(&self) -> usize {
        self.data.devices.len()
    }

    pub fn check(
        &mut self,
        device_id: Uuid,
        public_key: &[u8; 32],
        device_name: String,
    ) -> Result<Option<TrustRecord>> {
        let record = self.observe_peer(device_id, device_name, public_key)?;
        if record.state == TrustState::Trusted {
            Ok(Some(record))
        } else {
            Ok(None)
        }
    }

    pub fn trust(
        &mut self,
        device_id: Uuid,
        device_name: String,
        pubkey_bytes: &[u8],
    ) -> Result<TrustRecord> {
        let public_key: [u8; 32] = pubkey_bytes
            .try_into()
            .context("trust requires a 32-byte public key")?;
        self.observe_peer(device_id, device_name, &public_key)?;
        self.trust_peer(device_id)?
            .context("peer disappeared while trusting")
    }

    pub fn touch(&mut self, device_id: Uuid) -> Result<()> {
        if let Some(record) = self.data.devices.get_mut(&device_id) {
            record.last_seen = now_secs();
            self.save()?;
        }
        Ok(())
    }

    pub fn revoke(&mut self, device_id: Uuid) -> Result<bool> {
        self.revoke_peer(device_id)
    }
}

pub fn format_fingerprint(fp: &[u8; 32]) -> String {
    fp[..16]
        .iter()
        .map(|b| format!("{:02X}", b))
        .collect::<Vec<_>>()
        .chunks(2)
        .map(|c| c.join(""))
        .collect::<Vec<_>>()
        .join(":")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn observe_then_trust_roundtrip() {
        let file = NamedTempFile::new().unwrap();
        let mut store = TrustStore::load(file.path()).unwrap();
        let id = Uuid::new_v4();
        let pubkey = [42u8; 32];

        let observed = store.observe_peer(id, "Desk".into(), &pubkey).unwrap();
        assert_eq!(observed.state, TrustState::Untrusted);
        assert!(!store.is_trusted(id));

        store.trust_peer(id).unwrap();
        assert!(store.is_trusted(id));
    }

    #[test]
    fn fingerprint_mismatch_is_err() {
        let file = NamedTempFile::new().unwrap();
        let mut store = TrustStore::load(file.path()).unwrap();
        let id = Uuid::new_v4();
        store.observe_peer(id, "Device".into(), &[1u8; 32]).unwrap();

        assert!(store.observe_peer(id, "Device".into(), &[2u8; 32]).is_err());
    }
}
