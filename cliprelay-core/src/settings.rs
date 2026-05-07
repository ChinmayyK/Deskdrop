//! ClipRelay settings — persistent user configuration.
//!
//! Settings are stored as JSON at:
//!   Linux/macOS: $XDG_CONFIG_HOME/cliprelay/settings.json
//!   Windows:     %APPDATA%\cliprelay\settings.json
//!
//! The file is written atomically (tmp → rename) on every change.
//! Platform layers can watch the file for changes to hot-reload.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SyncMode {
    Auto,
    Manual,
}

impl Default for SyncMode {
    fn default() -> Self {
        Self::Auto
    }
}

// ── Settings struct ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    // ── Network ──────────────────────────────────────────────────────────────
    /// TCP port for the ClipRelay service.
    pub port: u16,

    /// Override the device name shown to peers. Empty = use hostname.
    pub device_name: String,

    // ── Sync behaviour ───────────────────────────────────────────────────────
    /// Enable clipboard syncing (master switch).
    pub sync_enabled: bool,

    /// Sync plain text content.
    pub sync_text: bool,

    /// Sync image content.
    pub sync_images: bool,

    /// Sync file content.
    pub sync_files: bool,

    /// Auto sync on local copy, or wait for explicit user send.
    pub sync_mode: SyncMode,

    /// Maximum payload size in bytes that will be synced (0 = unlimited).
    pub max_payload_bytes: u64,

    /// Number of clipboard history entries retained locally.
    pub history_limit: usize,

    /// Maximum text bytes stored per history entry for re-paste.
    pub max_history_text_bytes: usize,

    // ── Privacy ──────────────────────────────────────────────────────────────
    /// If true, show a native notification on every clipboard receive.
    pub show_receive_notification: bool,

    /// If true, require explicit TOFU confirmation for every new device.
    /// If false, auto-trust devices on the same LAN (lower security).
    pub require_tofu_confirmation: bool,

    /// List of device UUIDs that are explicitly blocked (revoked).
    pub blocked_device_ids: Vec<String>,

    /// If true, heuristic filtering blocks likely passwords and secrets.
    pub block_sensitive_text: bool,

    /// Case-insensitive substrings that should suppress syncing.
    pub ignore_patterns: Vec<String>,

    // ── Performance ──────────────────────────────────────────────────────────
    /// How often to poll the local clipboard for changes (milliseconds).
    pub clipboard_poll_ms: u64,

    /// Maximum clipboard pushes per second per peer (rate limiter).
    pub max_pushes_per_sec: f64,

    /// Burst allowance for the rate limiter.
    pub rate_limit_burst: f64,

    /// Suppress identical clipboard pushes repeated inside this window.
    pub smart_sync_duplicate_window_ms: u64,

    /// Coalesce rapid repeat copy events before syncing.
    pub smart_sync_debounce_ms: u64,

    // ── UI ───────────────────────────────────────────────────────────────────
    /// Start ClipRelay automatically on login.
    pub start_on_login: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            port: crate::protocol::DEFAULT_PORT,
            device_name: String::new(),
            sync_enabled: true,
            sync_text: true,
            sync_images: true,
            sync_files: true,
            sync_mode: SyncMode::Auto,
            max_payload_bytes: 64 * 1024 * 1024, // 64 MB
            history_limit: 50,
            max_history_text_bytes: 64 * 1024,
            show_receive_notification: true,
            require_tofu_confirmation: true,
            blocked_device_ids: Vec::new(),
            block_sensitive_text: true,
            ignore_patterns: Vec::new(),
            clipboard_poll_ms: 100,
            max_pushes_per_sec: 10.0,
            rate_limit_burst: 3.0,
            smart_sync_duplicate_window_ms: 1_500,
            smart_sync_debounce_ms: 150,
            start_on_login: false,
        }
    }
}

impl Settings {
    /// Resolved device name: custom name if set, else hostname.
    pub fn resolved_device_name(&self) -> String {
        if self.device_name.is_empty() {
            whoami::devicename()
        } else {
            self.device_name.clone()
        }
    }

    /// Is the given device UUID in the block list?
    pub fn is_blocked(&self, device_id: &str) -> bool {
        self.blocked_device_ids.iter().any(|id| id == device_id)
    }

    pub fn effective_history_limit(&self) -> usize {
        self.history_limit.clamp(20, 100)
    }
}

// ── SettingsStore ─────────────────────────────────────────────────────────────

pub struct SettingsStore {
    settings: Settings,
    path: PathBuf,
}

impl SettingsStore {
    /// Load settings from disk, or return defaults if the file doesn't exist.
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let settings = if path.exists() {
            let bytes = std::fs::read(&path).context("reading settings")?;
            if bytes.is_empty() {
                Settings::default()
            } else {
                // Use serde default for any missing fields so old configs work.
                serde_json::from_slice(&bytes).unwrap_or_else(|e| {
                    tracing::warn!("Settings parse error ({}), using defaults", e);
                    Settings::default()
                })
            }
        } else {
            Settings::default()
        };
        Ok(Self { settings, path })
    }

    pub fn get(&self) -> &Settings {
        &self.settings
    }

    pub fn get_mut(&mut self) -> &mut Settings {
        &mut self.settings
    }

    /// Persist to disk atomically.
    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).context("creating config dir")?;
        }
        let tmp = self.path.with_extension("tmp");
        let bytes = serde_json::to_vec_pretty(&self.settings)?;
        std::fs::write(&tmp, bytes).context("writing settings tmp")?;
        std::fs::rename(&tmp, &self.path).context("renaming settings")?;
        Ok(())
    }

    /// Apply a partial JSON patch to settings, then save.
    pub fn patch(&mut self, patch: &str) -> Result<()> {
        let mut value = serde_json::to_value(&self.settings)?;
        let patch_value: serde_json::Value = serde_json::from_str(patch)?;
        json_merge(&mut value, &patch_value);
        self.settings = serde_json::from_value(value)?;
        self.save()
    }

    /// Reset to defaults and save.
    pub fn reset(&mut self) -> Result<()> {
        self.settings = Settings::default();
        self.save()
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// Recursive JSON merge (patch overwrites matching keys).
fn json_merge(target: &mut serde_json::Value, patch: &serde_json::Value) {
    if let (Some(t), Some(p)) = (target.as_object_mut(), patch.as_object()) {
        for (k, v) in p {
            json_merge(t.entry(k).or_insert(serde_json::Value::Null), v);
        }
    } else {
        *target = patch.clone();
    }
}

// ── Platform config paths ─────────────────────────────────────────────────────

pub fn default_settings_path() -> PathBuf {
    #[cfg(target_os = "windows")]
    let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    #[cfg(not(target_os = "windows"))]
    let base = dirs::config_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")));

    base.join("cliprelay").join("settings.json")
}

pub fn default_trust_store_path() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("cliprelay")
        .join("trust.json")
}

pub fn default_peer_store_path() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("cliprelay")
        .join("peers.json")
}

pub fn default_history_path() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("cliprelay")
        .join("history.json")
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn settings_roundtrip() {
        let tmp = NamedTempFile::new().unwrap();
        let mut store = SettingsStore::load(tmp.path()).unwrap();
        store.get_mut().device_name = "Test Device".into();
        store.get_mut().port = 9999;
        store.save().unwrap();

        let store2 = SettingsStore::load(tmp.path()).unwrap();
        assert_eq!(store2.get().device_name, "Test Device");
        assert_eq!(store2.get().port, 9999);
    }

    #[test]
    fn patch_applies_partial_update() {
        let tmp = NamedTempFile::new().unwrap();
        let mut store = SettingsStore::load(tmp.path()).unwrap();
        store
            .patch(r#"{"sync_images": false, "port": 12345}"#)
            .unwrap();
        assert!(!store.get().sync_images);
        assert_eq!(store.get().port, 12345);
        assert!(store.get().sync_text); // untouched
    }

    #[test]
    fn reset_returns_defaults() {
        let tmp = NamedTempFile::new().unwrap();
        let mut store = SettingsStore::load(tmp.path()).unwrap();
        store.get_mut().port = 9999;
        store.reset().unwrap();
        assert_eq!(store.get().port, crate::protocol::DEFAULT_PORT);
    }
}
