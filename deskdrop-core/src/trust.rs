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
        self.migrate_matching_identity(
            device_id,
            device_name.clone(),
            public_key,
            fingerprint,
            now,
        )?;
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

        let previous_id = self.data.devices.iter().find_map(|(existing_id, record)| {
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
        if let Some(record) = self.data.devices.get(&device_id) {
            if record.public_key == [0; 32] || record.key_fingerprint == [0; 32] {
                anyhow::bail!("cannot trust peer without valid public key");
            }
        }
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

    /// Count of devices in `Trusted` state.
    pub fn trusted_count(&self) -> usize {
        self.data
            .devices
            .values()
            .filter(|r| r.state == TrustState::Trusted)
            .count()
    }

    /// Remove stale `Untrusted` and `Rejected` records not seen in `max_age_secs`.
    /// Trusted and Revoked records are always kept.
    /// Returns the number of records pruned.
    pub fn prune_stale(&mut self, max_age_secs: u64) -> Result<usize> {
        let now = now_secs();
        let before = self.data.devices.len();
        self.data.devices.retain(|_, record| match record.state {
            TrustState::Trusted | TrustState::Revoked => true,
            TrustState::Untrusted | TrustState::Rejected => {
                now.saturating_sub(record.last_seen) <= max_age_secs
            }
        });
        let pruned = before - self.data.devices.len();
        if pruned > 0 {
            self.save()?;
        }
        Ok(pruned)
    }

    /// Iterator over all `Trusted` records.
    pub fn all_trusted(&self) -> impl Iterator<Item = &TrustRecord> {
        self.data
            .devices
            .values()
            .filter(|r| r.state == TrustState::Trusted)
    }

    /// True if the device has been explicitly rejected.
    pub fn is_rejected(&self, device_id: Uuid) -> bool {
        self.data
            .devices
            .get(&device_id)
            .map(|r| r.state == TrustState::Rejected)
            .unwrap_or(false)
    }

    /// True if the device had trust but it was revoked.
    pub fn is_revoked(&self, device_id: Uuid) -> bool {
        self.data
            .devices
            .get(&device_id)
            .map(|r| r.state == TrustState::Revoked)
            .unwrap_or(false)
    }

    /// Check whether a device should be permitted to sync (trusted and not revoked).
    pub fn is_sync_allowed(&self, device_id: Uuid) -> bool {
        self.data
            .devices
            .get(&device_id)
            .map(|r| r.state == TrustState::Trusted)
            .unwrap_or(false)
    }

    /// Export a human-readable summary of all known devices for diagnostics.
    pub fn export_summary(&self) -> String {
        let mut lines = vec![format!(
            "{} device(s) known ({} trusted):",
            self.device_count(),
            self.trusted_count()
        )];
        let mut records: Vec<_> = self.data.devices.values().collect();
        records.sort_by_key(|r| r.last_seen);
        for r in records.iter().rev() {
            lines.push(format!(
                "  [{:?}] {}  fp={}  last_seen={}",
                r.state,
                r.effective_name(),
                format_fingerprint(&r.key_fingerprint),
                r.last_seen,
            ));
        }
        lines.join("\n")
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

/// Format a 32-byte fingerprint as 8 groups of 4 hex chars, colon-separated.
///
/// Example: `"A1B2:C3D4:E5F6:0708:1920:3040:5060:7080"`
///
/// Consistent with `IdentityKey::fingerprint_display()` in `crypto.rs`.
pub fn format_fingerprint(fp: &[u8; 32]) -> String {
    let hex: String = fp[..16].iter().map(|b| format!("{:02X}", b)).collect();
    hex.chars()
        .collect::<Vec<_>>()
        .chunks(4)
        .map(|chunk| chunk.iter().collect::<String>())
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
        assert_eq!(store.trusted_count(), 1);
    }

    #[test]
    fn fingerprint_mismatch_is_err() {
        let file = NamedTempFile::new().unwrap();
        let mut store = TrustStore::load(file.path()).unwrap();
        let id = Uuid::new_v4();
        store.observe_peer(id, "Device".into(), &[1u8; 32]).unwrap();

        assert!(store.observe_peer(id, "Device".into(), &[2u8; 32]).is_err());
    }

    #[test]
    fn reject_and_revoke_states() {
        let file = NamedTempFile::new().unwrap();
        let mut store = TrustStore::load(file.path()).unwrap();
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();

        store.observe_peer(a, "A".into(), &[1u8; 32]).unwrap();
        store.observe_peer(b, "B".into(), &[2u8; 32]).unwrap();

        store.reject_peer(a).unwrap();
        assert!(store.is_rejected(a));
        assert!(!store.is_sync_allowed(a));

        store.trust_peer(b).unwrap();
        store.revoke_peer(b).unwrap();
        assert!(store.is_revoked(b));
        assert!(!store.is_trusted(b));
        assert!(!store.is_sync_allowed(b));
    }

    #[test]
    fn all_trusted_filters_correctly() {
        let file = NamedTempFile::new().unwrap();
        let mut store = TrustStore::load(file.path()).unwrap();

        for (i, key) in [[1u8; 32], [2u8; 32], [3u8; 32]].iter().enumerate() {
            let id = Uuid::new_v4();
            store.observe_peer(id, format!("Dev{}", i), key).unwrap();
            if i < 2 {
                store.trust_peer(id).unwrap();
            }
        }
        let trusted: Vec<_> = store.all_trusted().collect();
        assert_eq!(trusted.len(), 2);
    }

    #[test]
    fn format_fingerprint_produces_8_groups() {
        let fp = [
            0xA1, 0xB2, 0xC3, 0xD4, 0xE5, 0xF6, 0x07, 0x08, 0x19, 0x20, 0x30, 0x40, 0x50, 0x60,
            0x70, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00u8,
        ];
        let s = format_fingerprint(&fp);
        let parts: Vec<_> = s.split(':').collect();
        assert_eq!(parts.len(), 8, "fingerprint: {}", s);
        for p in &parts {
            assert_eq!(p.len(), 4, "part '{}' in {}", p, s);
        }
    }

    #[test]
    fn export_summary_contains_device_names() {
        let file = NamedTempFile::new().unwrap();
        let mut store = TrustStore::load(file.path()).unwrap();
        let id = Uuid::new_v4();
        store
            .observe_peer(id, "MyPhone".into(), &[7u8; 32])
            .unwrap();
        store.trust_peer(id).unwrap();

        let summary = store.export_summary();
        assert!(summary.contains("MyPhone"), "summary: {}", summary);
        assert!(summary.contains("Trusted"), "summary: {}", summary);
    }
}
