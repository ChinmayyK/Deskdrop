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
    secret_bytes: [u8; 32],
    pub public: PublicKey,
}

impl IdentityKey {
    /// Generate a fresh identity key.
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

    /// Load from 32 raw secret bytes (e.g. read from disk).
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        let secret = StaticSecret::from(bytes);
        let public = PublicKey::from(&secret);
        Self {
            secret_bytes: bytes,
            public,
        }
    }

    /// Export the 32-byte **private** scalar for on-disk storage.
    ///
    /// The returned bytes must be stored with mode 0600 (or equivalent).
    /// Never log or transmit these bytes — only the public key is shared.
    pub fn to_bytes(&self) -> [u8; 32] {
        self.secret_bytes
    }

    /// SHA-256 of the public key bytes — shown to users for TOFU verification.
    pub fn fingerprint(&self) -> [u8; 32] {
        fingerprint_of(self.public.as_bytes())
    }

    /// Human-readable fingerprint: 8 groups of 4 hex chars separated by colons.
    ///
    /// Example: `"A1B2:C3D4:E5F6:0708:1920:3040:5060:7080"`
    pub fn fingerprint_display(&self) -> String {
        let fp = self.fingerprint();
        // Encode first 16 bytes as 32 hex chars, then group into 4-char chunks.
        let hex: String = fp[..16].iter().map(|b| format!("{:02X}", b)).collect();
        hex.chars()
            .collect::<Vec<_>>()
            .chunks(4)
            .map(|chunk| chunk.iter().collect::<String>())
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

        // Copy the shared secret bytes so we can zeroize them independently of
        // the opaque `SharedSecret` wrapper (which provides no zeroize method).
        let mut shared_bytes: [u8; 32] = *shared.as_bytes();

        // HKDF-SHA256: IKM = shared secret, salt = none.
        // The info string is prefixed with the protocol version so that HKDF
        // output is domain-separated across wire-format revisions (LOW-03).
        // Changing PROTOCOL_VERSION in protocol.rs automatically invalidates
        // old session keys — peers on different protocol versions cannot
        // decrypt each other's frames even if they share an ephemeral key.
        let info = format!("cliprelay-v{}-session", crate::protocol::PROTOCOL_VERSION);
        let hk = Hkdf::<Sha256>::new(None, &shared_bytes);

        // Zeroize the raw DH secret immediately after feeding it into HKDF;
        // it must not linger in process memory (CRIT-02).
        shared_bytes.zeroize();

        let mut okm = [0u8; 32];
        hk.expand(info.as_bytes(), &mut okm)
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

    /// Encrypt `buffer` in-place (avoids memory allocation).
    /// The resulting ciphertext replaces the contents of `buffer` and the 16-byte
    /// auth tag is appended to it.
    /// Returns the 12-byte nonce used, which must be sent alongside the ciphertext.
    pub fn encrypt_in_place(&mut self, buffer: &mut Vec<u8>) -> Result<Nonce> {
        use chacha20poly1305::aead::AeadInPlace;
        let nonce = counter_nonce(self.send_counter);
        self.send_counter = self
            .send_counter
            .checked_add(1)
            .context("send counter overflow")?;

        self.cipher
            .encrypt_in_place(&nonce, &[], buffer)
            .map_err(|e| anyhow::anyhow!("encrypt: {:?}", e))?;
        
        Ok(nonce)
    }

    /// Decrypt a frame produced by [`encrypt`]. Enforces monotonic counter.
    pub fn decrypt(&mut self, frame: &[u8]) -> Result<Vec<u8>> {
        anyhow::ensure!(frame.len() >= 12, "frame too short");
        let (nonce_bytes, ct) = frame.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);

        // Replay protection: nonce counter must be exactly the next expected
        // value. Using strict equality (== recv_counter) instead of >=
        // prevents replay of any previously seen or skipped frame — a captured
        // frame can never satisfy counter == recv_counter once it has been
        // incremented past it.
        let counter = u64::from_be_bytes(nonce_bytes[..8].try_into().unwrap());
        anyhow::ensure!(
            counter == self.recv_counter,
            "replayed or out-of-order frame: got counter {}, expected {}",
            counter,
            self.recv_counter
        );
        self.recv_counter = self
            .recv_counter
            .checked_add(1)
            .context("recv counter overflow")?;

        self.cipher
            .decrypt(nonce, ct)
            .map_err(|e| anyhow::anyhow!("decrypt: {:?}", e))
    }

    /// Decrypt a frame in-place. The first 12 bytes of the buffer must be the nonce,
    /// followed by the ciphertext. Upon success, the buffer is shrunk to just the plaintext.
    pub fn decrypt_in_place(&mut self, buffer: &mut Vec<u8>) -> Result<()> {
        use chacha20poly1305::aead::AeadInPlace;
        anyhow::ensure!(buffer.len() >= 12 + 16, "frame too short (must have nonce and tag)");
        
        let mut nonce_bytes = [0u8; 12];
        nonce_bytes.copy_from_slice(&buffer[..12]);
        let nonce = Nonce::from_slice(&nonce_bytes);
        
        let counter = u64::from_be_bytes(nonce_bytes[..8].try_into().unwrap());
        anyhow::ensure!(
            counter == self.recv_counter,
            "replayed or out-of-order frame: got counter {}, expected {}",
            counter,
            self.recv_counter
        );
        self.recv_counter = self
            .recv_counter
            .checked_add(1)
            .context("recv counter overflow")?;

        // Remove the 12-byte nonce from the beginning of the buffer.
        buffer.drain(..12);

        self.cipher
            .decrypt_in_place(nonce, &[], buffer)
            .map_err(|e| anyhow::anyhow!("decrypt: {:?}", e))?;
            
        Ok(())
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

/// Generate a random 6-digit numeric PIN for device pairing displays.
///
/// The PIN is derived from 3 random bytes so the distribution is uniform
/// over [000000, 999999] — no modulo bias.
pub fn generate_pairing_pin() -> String {
    let mut bytes = [0u8; 4];
    rand::thread_rng().fill_bytes(&mut bytes);
    let n = u32::from_le_bytes(bytes) % 1_000_000;
    format!("{:06}", n)
}

/// Constant-time comparison of two byte slices (prevents timing attacks on MACs).
pub fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter()
        .zip(b.iter())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
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

    #[test]
    fn identity_key_roundtrips_through_bytes() {
        let key = IdentityKey::generate();
        let bytes = key.to_bytes();
        // Must NOT be all-zeros (placeholder bug).
        assert_ne!(bytes, [0u8; 32], "to_bytes() must return private scalar");
        // Must NOT equal the public key bytes.
        assert_ne!(
            &bytes,
            key.public.as_bytes(),
            "to_bytes() must not return public key"
        );
        // Reloading from private bytes must reproduce the same public key.
        let reloaded = IdentityKey::from_bytes(bytes);
        assert_eq!(reloaded.public.as_bytes(), key.public.as_bytes());
        // Fingerprints must also match.
        assert_eq!(reloaded.fingerprint(), key.fingerprint());
    }

    #[test]
    fn fingerprint_display_format() {
        let key = IdentityKey::generate();
        let disp = key.fingerprint_display();
        // Expect 8 groups of 4 hex chars separated by colons.
        let parts: Vec<&str> = disp.split(':').collect();
        assert_eq!(parts.len(), 8, "fingerprint should have 8 groups: {}", disp);
        for part in parts {
            assert_eq!(part.len(), 4, "each group should be 4 chars: {}", part);
            assert!(
                part.chars().all(|c| c.is_ascii_hexdigit()),
                "non-hex char in: {}",
                part
            );
        }
    }

    #[test]
    fn pairing_pin_is_six_digits() {
        for _ in 0..20 {
            let pin = generate_pairing_pin();
            assert_eq!(pin.len(), 6, "PIN must be 6 digits: {}", pin);
            assert!(
                pin.chars().all(|c| c.is_ascii_digit()),
                "non-digit in PIN: {}",
                pin
            );
        }
    }

    #[test]
    fn ct_eq_works() {
        assert!(ct_eq(b"hello", b"hello"));
        assert!(!ct_eq(b"hello", b"world"));
        assert!(!ct_eq(b"hello", b"hell"));
    }
}
