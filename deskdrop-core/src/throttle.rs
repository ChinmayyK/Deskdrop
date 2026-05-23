//! Bandwidth throttling for clipboard payload delivery.
//!
//! On heavily shared Wi-Fi a 32 MB screenshot push without throttling can
//! cause visible lag for other traffic. The throttle applies only to *large*
//! payloads (above `THROTTLE_THRESHOLD`); small clipboard text flies through
//! unimpeded.
//!
//! Implementation: async token-bucket. A background task refills tokens at
//! `rate_bps / 8` bytes per second. `acquire(n_bytes)` sleeps until enough
//! tokens are available, then atomically deducts them.
//!
//! Default limit: 4 MB/s (≈ 32 Mbit/s) — well below typical Wi-Fi capacity
//! yet fast enough to deliver a 32 MB image in ~8 s.

use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tracing::debug;

// ── Config ────────────────────────────────────────────────────────────────────

/// Payloads smaller than this are never throttled.
pub const THROTTLE_THRESHOLD: usize = 256 * 1024; // 256 KB
/// Default sustained rate: 4 MB/s.
pub const DEFAULT_RATE_BPS: u64 = 4 * 1024 * 1024;
/// Maximum burst (bucket capacity): 2× the rate.
pub const DEFAULT_BURST_BYTES: u64 = DEFAULT_RATE_BPS * 2;

// ── Token bucket ──────────────────────────────────────────────────────────────

struct Bucket {
    tokens: f64,   // available bytes
    capacity: f64, // max bucket size (burst)
    rate: f64,     // bytes per second
    last_refill: Instant,
    enabled: bool,
}

impl Bucket {
    fn new(rate_bps: u64, burst_bytes: u64) -> Self {
        // Clamp burst_bytes to at least 1 to avoid a 0-capacity bucket which
        // produces NaN/Inf token arithmetic and makes every acquire block
        // indefinitely (LOW-02).
        let burst = burst_bytes.max(1);
        Self {
            tokens: burst as f64,
            capacity: burst as f64,
            rate: rate_bps as f64,
            last_refill: Instant::now(),
            enabled: true,
        }
    }

    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        self.tokens = (self.tokens + elapsed * self.rate).min(self.capacity);
        self.last_refill = now;
    }

    /// Try to consume `bytes`. Returns delay needed before retry if insufficient.
    fn try_consume(&mut self, bytes: usize) -> Result<(), Duration> {
        self.refill();
        let need = bytes as f64;
        if self.tokens >= need {
            self.tokens -= need;
            Ok(())
        } else {
            let deficit = need - self.tokens;
            let wait_secs = deficit / self.rate;
            Err(Duration::from_secs_f64(wait_secs))
        }
    }
}

// ── Throttle ──────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct Throttle {
    bucket: Arc<Mutex<Bucket>>,
}

impl Throttle {
    /// Create a throttle with the given sustained rate and burst size.
    pub fn new(rate_bps: u64, burst_bytes: u64) -> Self {
        Self {
            bucket: Arc::new(Mutex::new(Bucket::new(rate_bps, burst_bytes))),
        }
    }

    /// Create a throttle with default settings.
    pub fn default_rate() -> Self {
        Self::new(DEFAULT_RATE_BPS, DEFAULT_BURST_BYTES)
    }

    /// Disabled throttle — `acquire` always returns immediately.
    pub fn unlimited() -> Self {
        let mut b = Bucket::new(u64::MAX / 2, u64::MAX / 2);
        b.enabled = false;
        Self {
            bucket: Arc::new(Mutex::new(b)),
        }
    }

    /// Wait until `bytes` can be sent within the rate limit.
    /// Small payloads (< THROTTLE_THRESHOLD) skip the throttle entirely.
    pub async fn acquire(&self, bytes: usize) {
        if bytes < THROTTLE_THRESHOLD {
            return; // never throttle small payloads
        }

        loop {
            let wait = {
                let mut b = self.bucket.lock().await;
                if !b.enabled {
                    return;
                }
                b.try_consume(bytes)
            };

            match wait {
                Ok(()) => {
                    debug!("[throttle] acquired {} bytes", bytes);
                    return;
                }
                Err(delay) => {
                    debug!("[throttle] waiting {:?} for {} bytes", delay, bytes);
                    tokio::time::sleep(delay).await;
                }
            }
        }
    }

    /// Change the rate limit at runtime (e.g. from settings update).
    pub async fn set_rate(&self, rate_bps: u64, burst_bytes: u64) {
        let mut b = self.bucket.lock().await;
        b.rate = rate_bps as f64;
        // Clamp burst to at least 1 — see Bucket::new (LOW-02).
        b.capacity = burst_bytes.max(1) as f64;
        b.tokens = b.tokens.min(b.capacity);
    }

    /// Enable or disable throttling.
    pub async fn set_enabled(&self, enabled: bool) {
        self.bucket.lock().await.enabled = enabled;
    }

    /// Current token count (for diagnostics / metrics).
    pub async fn available_bytes(&self) -> u64 {
        let mut b = self.bucket.lock().await;
        b.refill();
        b.tokens as u64
    }
}

// ── Throughput estimator ──────────────────────────────────────────────────────

/// Measures actual bytes-per-second over a sliding 5-second window.
pub struct ThroughputEstimator {
    samples: std::collections::VecDeque<(Instant, u64)>, // (timestamp, bytes)
    window: Duration,
}

impl ThroughputEstimator {
    pub fn new() -> Self {
        Self {
            samples: std::collections::VecDeque::new(),
            window: Duration::from_secs(5),
        }
    }

    /// Record `bytes` transferred right now.
    pub fn record(&mut self, bytes: u64) {
        let now = Instant::now();
        self.samples.push_back((now, bytes));
        // Evict samples older than the window.
        while self
            .samples
            .front()
            .map(|(t, _)| now - *t > self.window)
            .unwrap_or(false)
        {
            self.samples.pop_front();
        }
    }

    /// Estimated bytes/second over the last window.
    pub fn bps(&self) -> f64 {
        if self.samples.len() < 2 {
            return 0.0;
        }
        let total: u64 = self.samples.iter().map(|(_, b)| b).sum();
        let span = self
            .samples
            .back()
            .unwrap()
            .0
            .duration_since(self.samples.front().unwrap().0)
            .as_secs_f64();
        if span < 0.001 {
            return 0.0;
        }
        total as f64 / span
    }

    /// Human-readable throughput string.
    pub fn display(&self) -> String {
        let bps = self.bps();
        if bps < 1_024.0 {
            format!("{:.0} B/s", bps)
        } else if bps < 1_048_576.0 {
            format!("{:.1} KB/s", bps / 1_024.0)
        } else {
            format!("{:.2} MB/s", bps / 1_048_576.0)
        }
    }
}

impl Default for ThroughputEstimator {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[tokio::test]
    async fn small_payload_no_delay() {
        let t = Throttle::new(1, 1); // absurdly slow rate
        let start = Instant::now();
        t.acquire(THROTTLE_THRESHOLD - 1).await; // below threshold
        assert!(start.elapsed() < Duration::from_millis(10));
    }

    #[tokio::test]
    async fn large_payload_is_delayed() {
        // Rate = 1 MB/s, burst = 512 KB
        let t = Throttle::new(1024 * 1024, 512 * 1024);
        // First 512 KB should pass immediately (burst).
        t.acquire(512 * 1024).await;
        // Second 512 KB needs to wait ~0.5 s.
        let start = Instant::now();
        t.acquire(512 * 1024).await;
        assert!(start.elapsed() >= Duration::from_millis(400));
    }

    #[tokio::test]
    async fn unlimited_never_blocks() {
        let t = Throttle::unlimited();
        let start = Instant::now();
        t.acquire(1024 * 1024 * 1024).await; // 1 GB — would take forever otherwise
        assert!(start.elapsed() < Duration::from_millis(10));
    }

    #[test]
    fn throughput_estimator_basic() {
        let mut est = ThroughputEstimator::new();
        est.record(1_000_000);
        std::thread::sleep(Duration::from_millis(100));
        est.record(1_000_000);
        let bps = est.bps();
        assert!(bps > 0.0, "bps = {}", bps);
    }
}
