//! PIN-based pairing — a more explicit trust ceremony than TOFU.
//!
//! # Flow
//! 1. Both devices are on the same LAN and discover each other via mDNS.
//! 2. Device A displays: "Connecting to 'Bob's PC' — PIN: **482 917**"
//! 3. Device B simultaneously displays: "Pairing request from 'Alice's Mac' — PIN: **482 917**"
//! 4. The user visually confirms both PINs match, then taps "Trust".
//! 5. A hash of the ECDH shared secret is used to derive the PIN, so:
//!    - An active MITM attacker would produce a different PIN.
//!    - A passive eavesdropper learns nothing useful.
//!
//! # PIN derivation
//! PIN = HKDF-SHA256(shared_secret, info="cliprelay-pin") → u64 mod 10^6
//! Displayed as zero-padded 6 digits split into two groups: "482 917".
//!
//! # Security properties
//! - PIN changes with every new session (ephemeral ECDH).
//! - A brute-force attacker needs 1,000,000 guesses per connection attempt.
//! - The pairing window is short-lived (30 seconds before timeout).
//! - After pairing, the device's long-term fingerprint is stored in the
//!   trust store exactly as in TOFU mode.

use anyhow::{Context, Result};
use hkdf::Hkdf;
use sha2::Sha256;
use std::time::{Duration, Instant};

// ── PIN derivation ────────────────────────────────────────────────────────────

/// Derive a 6-digit PIN from the raw X25519 shared secret bytes.
///
/// Both devices compute the same PIN because ECDH is commutative:
/// alice.dh(bob_pub) == bob.dh(alice_pub).
pub fn derive_pin(shared_secret_bytes: &[u8]) -> PairingPin {
    let hk = Hkdf::<Sha256>::new(None, shared_secret_bytes);
    let mut okm = [0u8; 8];
    hk.expand(b"cliprelay-pin", &mut okm)
        .expect("HKDF expand never fails for 8 bytes");

    let n = u64::from_be_bytes(okm);
    let pin = (n % 1_000_000) as u32;
    PairingPin(pin)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PairingPin(u32);

impl PairingPin {
    /// Zero-padded 6-digit string split into two groups: "048 291".
    pub fn display(&self) -> String {
        let s = format!("{:06}", self.0);
        format!("{} {}", &s[..3], &s[3..])
    }

    /// Raw 6-digit number for comparison.
    pub fn value(&self) -> u32 {
        self.0
    }
}

impl std::fmt::Display for PairingPin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display())
    }
}

// ── Pairing session ───────────────────────────────────────────────────────────

/// How long a pairing request is valid before it auto-expires.
pub const PAIRING_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug)]
pub enum PairingDecision {
    Approved,
    Denied,
    TimedOut,
}

/// A pending pairing request from a new device.
#[derive(Debug)]
pub struct PendingPairing {
    pub device_id: uuid::Uuid,
    pub device_name: String,
    pub pin: PairingPin,
    pub pubkey_bytes: [u8; 32],
    created_at: Instant,
}

impl PendingPairing {
    pub fn new(
        device_id: uuid::Uuid,
        device_name: String,
        shared_secret: &[u8],
        pubkey_bytes: [u8; 32],
    ) -> Self {
        Self {
            device_id,
            device_name,
            pin: derive_pin(shared_secret),
            pubkey_bytes,
            created_at: Instant::now(),
        }
    }

    pub fn is_expired(&self) -> bool {
        self.created_at.elapsed() > PAIRING_TIMEOUT
    }

    pub fn time_remaining(&self) -> Duration {
        PAIRING_TIMEOUT.saturating_sub(self.created_at.elapsed())
    }
}

// ── Pairing manager ───────────────────────────────────────────────────────────

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, Mutex};
use uuid::Uuid;

pub struct PairingManager {
    /// Pending pairings awaiting user decision. device_id → (pairing, decision_tx)
    pending: Arc<Mutex<HashMap<Uuid, (PendingPairing, oneshot::Sender<bool>)>>>,
    /// Channel to notify UI of new pairing requests.
    ui_tx: mpsc::Sender<PairingRequest>,
}

/// Sent to the platform UI layer when user action is needed.
#[derive(Debug)]
pub struct PairingRequest {
    pub device_id: Uuid,
    pub device_name: String,
    pub pin: PairingPin,
    pub time_remaining_secs: u64,
}

impl PairingManager {
    pub fn new(ui_tx: mpsc::Sender<PairingRequest>) -> Self {
        Self {
            pending: Arc::new(Mutex::new(HashMap::new())),
            ui_tx,
        }
    }

    /// Register a new pairing request. Returns a future that resolves to the user's decision.
    pub async fn request_pairing(&self, pairing: PendingPairing) -> Result<bool> {
        let (decision_tx, decision_rx) = oneshot::channel();

        let pr = PairingRequest {
            device_id: pairing.device_id,
            device_name: pairing.device_name.clone(),
            pin: pairing.pin,
            time_remaining_secs: pairing.time_remaining().as_secs(),
        };

        self.pending
            .lock()
            .await
            .insert(pairing.device_id, (pairing, decision_tx));

        self.ui_tx.send(pr).await.ok();

        // Wait for user decision with timeout.
        match tokio::time::timeout(PAIRING_TIMEOUT, decision_rx).await {
            Ok(Ok(approved)) => Ok(approved),
            Ok(Err(_)) => Ok(false), // sender dropped
            Err(_) => {
                // Timed out — clean up and deny.
                self.pending.lock().await.remove(&Uuid::nil()); // placeholder
                Ok(false)
            }
        }
    }

    /// Called by the platform UI when the user approves or denies.
    pub async fn resolve(&self, device_id: Uuid, approved: bool) {
        if let Some((_, tx)) = self.pending.lock().await.remove(&device_id) {
            tx.send(approved).ok();
        }
    }

    /// Get all pending pairing requests (for UI display).
    pub async fn pending_requests(&self) -> Vec<PairingRequest> {
        self.pending
            .lock()
            .await
            .values()
            .filter(|(p, _)| !p.is_expired())
            .map(|(p, _)| PairingRequest {
                device_id: p.device_id,
                device_name: p.device_name.clone(),
                pin: p.pin,
                time_remaining_secs: p.time_remaining().as_secs(),
            })
            .collect()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pin_derivation_deterministic() {
        let secret = [0xAB; 32];
        let p1 = derive_pin(&secret);
        let p2 = derive_pin(&secret);
        assert_eq!(p1, p2);
    }

    #[test]
    fn pin_display_format() {
        let p = PairingPin(48291);
        assert_eq!(p.display(), "048 291");
    }

    #[test]
    fn pin_display_max() {
        let p = PairingPin(999999);
        assert_eq!(p.display(), "999 999");
    }

    #[test]
    fn different_secrets_different_pins() {
        let p1 = derive_pin(&[0xAA; 32]);
        let p2 = derive_pin(&[0xBB; 32]);
        // Extremely unlikely to collide.
        assert_ne!(p1.value(), p2.value());
    }

    #[test]
    fn pin_in_valid_range() {
        for seed in 0u8..=255 {
            let p = derive_pin(&[seed; 32]);
            assert!(p.value() < 1_000_000);
        }
    }
}
