import sys

new_crypto = """//! Deskdrop Cryptography
//!
//! # Session key establishment
//! We use the Noise Protocol Framework (Noise_XX_25519_ChaChaPoly_SHA256).
//! This provides mutual authentication and forward secrecy.
//!
//! # Long-term identity
//! Each device has a stable X25519 key pair stored on disk.
//! Its public key's SHA-256 hash is the "fingerprint" shown to users.

use anyhow::{Context, Result};
use rand::RngCore;
use sha2::{Digest, Sha256};
use x25519_dalek::{PublicKey, StaticSecret};
use zeroize::Zeroize;

pub const NOISE_PARAMS: &str = "Noise_XX_25519_ChaChaPoly_SHA256";

// ── Long-term identity key ────────────────────────────────────────────────────

/// 32-byte raw scalar stored on disk (mode 0600).
pub struct IdentityKey {
    secret_bytes: [u8; 32],
    pub public: PublicKey,
}

impl IdentityKey {
    pub fn generate() -> Self {
        let mut secret_bytes = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut secret_bytes);
        let secret = StaticSecret::from(secret_bytes);
        let public = PublicKey::from(&secret);
        Self {
            secret_bytes,
            public,
        }
    }

    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        let secret = StaticSecret::from(bytes);
        let public = PublicKey::from(&secret);
        Self {
            secret_bytes: bytes,
            public,
        }
    }

    pub fn to_bytes(&self) -> [u8; 32] {
        self.secret_bytes
    }

    pub fn fingerprint(&self) -> [u8; 32] {
        fingerprint_of(self.public.as_bytes())
    }

    pub fn fingerprint_display(&self) -> String {
        let fp = self.fingerprint();
        let hex: String = fp[..16].iter().map(|b| format!("{:02X}", b)).collect();
        hex.chars()
            .collect::<Vec<_>>()
            .chunks(4)
            .map(|chunk| chunk.iter().collect::<String>())
            .collect::<Vec<_>>()
            .join(":")
    }

    pub fn secret_bytes(&self) -> [u8; 32] {
        self.secret_bytes
    }
}

impl Drop for IdentityKey {
    fn drop(&mut self) {
        self.secret_bytes.zeroize();
    }
}

pub fn fingerprint_of(pubkey_bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(pubkey_bytes);
    hasher.finalize().into()
}

// ── IdentityStore ─────────────────────────────────────────────────────────────

use std::path::{Path, PathBuf};

pub struct IdentityStore {
    path: PathBuf,
}

impl IdentityStore {
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
        }
    }

    pub fn default_path() -> PathBuf {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("deskdrop")
            .join("identity.key")
    }

    pub fn load_or_create(&self) -> Result<IdentityKey> {
        if self.path.exists() {
            self.load()
        } else {
            let key = IdentityKey::generate();
            self.save(&key)?;
            Ok(key)
        }
    }

    pub fn load(&self) -> Result<IdentityKey> {
        let bytes = std::fs::read(&self.path)
            .with_context(|| format!("reading identity key from {:?}", self.path))?;

        anyhow::ensure!(
            bytes.len() == 32,
            "identity key file corrupt: expected 32 bytes, got {}",
            bytes.len()
        );

        let mut raw = [0u8; 32];
        raw.copy_from_slice(&bytes);
        Ok(IdentityKey::from_bytes(raw))
    }

    pub fn save(&self, key: &IdentityKey) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).context("creating key directory")?;
        }

        let tmp = self.path.with_extension("tmp");

        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            let mut options = std::fs::OpenOptions::new();
            options.write(true).create(true).truncate(true).mode(0o600);
            let mut file = options
                .open(&tmp)
                .context("creating temporary identity key file")?;
            std::io::Write::write_all(&mut file, &key.secret_bytes())
                .context("writing identity key")?;
        }

        #[cfg(not(unix))]
        {
            std::fs::write(&tmp, key.secret_bytes()).context("writing identity key")?;
        }

        std::fs::rename(&tmp, &self.path).context("renaming identity key")?;
        Ok(())
    }

    pub fn rotate(&self) -> Result<IdentityKey> {
        let new_key = IdentityKey::generate();
        self.save(&new_key)?;
        tracing::info!(
            "Identity key rotated. New fingerprint: {}",
            new_key.fingerprint_display()
        );
        Ok(new_key)
    }

    pub fn delete(&self) -> Result<()> {
        if self.path.exists() {
            std::fs::remove_file(&self.path).context("deleting identity key")?;
        }
        Ok(())
    }

    pub fn exists(&self) -> bool {
        self.path.exists()
    }
}

// ── Noise Transport ───────────────────────────────────────────────────────────

pub struct NoiseTransport {
    pub transport: snow::TransportState,
}

impl NoiseTransport {
    /// Encrypts `plaintext` into a newly allocated buffer, returning the ciphertext.
    /// The length of the ciphertext will be `plaintext.len() + 16` bytes.
    pub fn encrypt(&mut self, plaintext: &[u8]) -> Result<Vec<u8>> {
        let mut ct = vec![0u8; plaintext.len() + 16];
        let len = self.transport.write_message(plaintext, &mut ct)
            .map_err(|e| anyhow::anyhow!("Noise encrypt error: {:?}", e))?;
        ct.truncate(len);
        Ok(ct)
    }

    /// Decrypts `frame` and returns the newly allocated plaintext.
    pub fn decrypt(&mut self, frame: &[u8]) -> Result<Vec<u8>> {
        let mut pt = vec![0u8; frame.len()];
        let len = self.transport.read_message(frame, &mut pt)
            .map_err(|e| anyhow::anyhow!("Noise decrypt error: {:?}", e))?;
        pt.truncate(len);
        Ok(pt)
    }
}

// ── Random helpers ────────────────────────────────────────────────────────────

pub fn generate_pairing_pin() -> String {
    let mut bytes = [0u8; 4];
    rand::thread_rng().fill_bytes(&mut bytes);
    let n = u32::from_le_bytes(bytes) % 1_000_000;
    format!("{:06}", n)
}

pub fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter()
        .zip(b.iter())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}
"""

with open("deskdrop-core/src/crypto.rs", "w") as f:
    f.write(new_crypto)
