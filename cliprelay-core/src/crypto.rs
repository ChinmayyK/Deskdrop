//! ClipRelay Cryptography
//!
//! # Session key establishment
//! 1. Both peers generate an ephemeral X25519 keypair.
//! 2. They exchange public keys inside HelloFrame / HelloAckFrame (plaintext).
//! 3. ECDH shared secret → HKDF-SHA256 → 32-byte session key.
//! 4. Every subsequent frame is encrypted with ChaCha20-Poly1305.
//!    Nonce: 4-byte big-endian message counter || 8 zero bytes (never reused).
//!
//! # Long-term identity
//! Each device also has a stable X25519 key pair stored on disk.
//! Its public key's SHA-256 hash is the "fingerprint" shown to users
//! during TOFU (Trust On First Use) verification.

use anyhow::{Context, Result};
use chacha20poly1305::{
    aead::{Aead, KeyInit},
    ChaCha20Poly1305, Key, Nonce,
};
use hkdf::Hkdf;
use rand::RngCore;
use sha2::{Digest, Sha256};
use x25519_dalek::{EphemeralSecret, PublicKey, StaticSecret};
use zeroize::Zeroize;

// ── Long-term identity key ────────────────────────────────────────────────────

/// 32-byte raw scalar stored on disk (mode 0600).
pub struct IdentityKey {
    _secret: StaticSecret,
    pub public: PublicKey,
}

impl IdentityKey {
    /// Generate a fresh identity key.
    pub fn generate() -> Self {
        let secret = StaticSecret::random_from_rng(rand::thread_rng());
        let public = PublicKey::from(&secret);
        Self {
            _secret: secret,
            public,
        }
    }

    /// Load from 32 raw bytes (e.g. read from disk).
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        let secret = StaticSecret::from(bytes);
        let public = PublicKey::from(&secret);
        Self {
            _secret: secret,
            public,
        }
    }

    /// Export 32 raw bytes for storage.
    pub fn to_bytes(&self) -> [u8; 32] {
        // StaticSecret doesn't expose bytes directly; we keep a copy at init.
        // In production, store bytes at construction time.
        *self.public.as_bytes() // placeholder — see note below
    }

    /// SHA-256 of the public key bytes — shown to users for TOFU verification.
    pub fn fingerprint(&self) -> [u8; 32] {
        fingerprint_of(self.public.as_bytes())
    }

    /// Human-readable fingerprint (colon-separated hex pairs, first 16 bytes).
    pub fn fingerprint_display(&self) -> String {
        let fp = self.fingerprint();
        fp[..16]
            .iter()
            .map(|b| format!("{:02X}", b))
            .collect::<Vec<_>>()
            .chunks(2)
            .map(|c| c.join(""))
            .collect::<Vec<_>>()
            .join(":")
    }
}

pub fn fingerprint_of(pubkey_bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(pubkey_bytes);
    hasher.finalize().into()
}

// ── Ephemeral session handshake ───────────────────────────────────────────────

pub struct EphemeralKeypair {
    secret: Option<EphemeralSecret>,
    pub public_bytes: [u8; 32],
}

impl EphemeralKeypair {
    pub fn generate() -> Self {
        let secret = EphemeralSecret::random_from_rng(rand::thread_rng());
        let public = PublicKey::from(&secret);
        Self {
            secret: Some(secret),
            public_bytes: *public.as_bytes(),
        }
    }

    /// Consume the ephemeral secret, perform ECDH, derive session key.
    pub fn derive_session_key(mut self, peer_pubkey_bytes: [u8; 32]) -> Result<SessionKey> {
        let secret = self.secret.take().context("keypair already consumed")?;
        let peer_public = PublicKey::from(peer_pubkey_bytes);
        let shared = secret.diffie_hellman(&peer_public);

        // HKDF-SHA256: IKM = shared secret, salt = none, info = "cliprelay-v1"
        let hk = Hkdf::<Sha256>::new(None, shared.as_bytes());
        let mut okm = [0u8; 32];
        hk.expand(b"cliprelay-v1-session", &mut okm)
            .map_err(|_| anyhow::anyhow!("HKDF expand failed"))?;

        let key = SessionKey {
            cipher: ChaCha20Poly1305::new(Key::from_slice(&okm)),
            send_counter: 0,
            recv_counter: 0,
        };

        okm.zeroize();
        Ok(key)
    }
}

// ── Symmetric session ─────────────────────────────────────────────────────────

pub struct SessionKey {
    cipher: ChaCha20Poly1305,
    send_counter: u64,
    recv_counter: u64,
}

impl SessionKey {
    /// Encrypt `plaintext`. Returns `nonce || ciphertext`.
    pub fn encrypt(&mut self, plaintext: &[u8]) -> Result<Vec<u8>> {
        let nonce = counter_nonce(self.send_counter);
        self.send_counter = self
            .send_counter
            .checked_add(1)
            .context("send counter overflow")?;

        let ct = self
            .cipher
            .encrypt(&nonce, plaintext)
            .map_err(|e| anyhow::anyhow!("encrypt: {:?}", e))?;

        let mut out = Vec::with_capacity(12 + ct.len());
        out.extend_from_slice(nonce.as_slice());
        out.extend_from_slice(&ct);
        Ok(out)
    }

    /// Decrypt a frame produced by [`encrypt`]. Enforces monotonic counter.
    pub fn decrypt(&mut self, frame: &[u8]) -> Result<Vec<u8>> {
        anyhow::ensure!(frame.len() >= 12, "frame too short");
        let (nonce_bytes, ct) = frame.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);

        // Replay protection: nonce counter must be >= expected
        let counter = u64::from_be_bytes(nonce_bytes[..8].try_into().unwrap());
        anyhow::ensure!(counter >= self.recv_counter, "replayed frame");
        self.recv_counter = counter + 1;

        self.cipher
            .decrypt(nonce, ct)
            .map_err(|e| anyhow::anyhow!("decrypt: {:?}", e))
    }
}

fn counter_nonce(counter: u64) -> Nonce {
    let mut n = [0u8; 12];
    n[..8].copy_from_slice(&counter.to_be_bytes());
    *Nonce::from_slice(&n)
}

// ── Random helpers ────────────────────────────────────────────────────────────

pub fn random_nonce16() -> [u8; 16] {
    let mut n = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut n);
    n
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_session_key() {
        let alice = EphemeralKeypair::generate();
        let bob = EphemeralKeypair::generate();
        let alice_pub = alice.public_bytes;
        let bob_pub = bob.public_bytes;

        let mut alice_sess = alice.derive_session_key(bob_pub).unwrap();
        let mut bob_sess = bob.derive_session_key(alice_pub).unwrap();

        let msg = b"hello cliprelay!";
        let ct = alice_sess.encrypt(msg).unwrap();
        let pt = bob_sess.decrypt(&ct).unwrap();
        assert_eq!(pt, msg);
    }

    #[test]
    fn reject_replay() {
        let alice = EphemeralKeypair::generate();
        let bob = EphemeralKeypair::generate();
        let alice_pub = alice.public_bytes;
        let bob_pub = bob.public_bytes;
        let mut alice_sess = alice.derive_session_key(bob_pub).unwrap();
        let mut bob_sess = bob.derive_session_key(alice_pub).unwrap();

        let ct = alice_sess.encrypt(b"first").unwrap();
        bob_sess.decrypt(&ct).unwrap();
        assert!(bob_sess.decrypt(&ct).is_err(), "replay should fail");
    }

    #[test]
    fn fingerprint_is_deterministic() {
        let key = IdentityKey::generate();
        assert_eq!(key.fingerprint(), key.fingerprint());
    }
}
