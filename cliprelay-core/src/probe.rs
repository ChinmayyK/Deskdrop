//! Network quality probe.
//!
//! Before sending a large payload ClipRelay probes the link to the target
//! peer and selects the optimal chunk size. A slow / lossy link gets smaller
//! chunks (less re-send on loss); a fast link gets larger chunks (fewer
//! round-trips).
//!
//! The probe also feeds latency samples into `metrics::LatencyTracker` so
//! the UI can show live RTT stats.
//!
//! # Probe protocol
//! 1. Sender issues `AppMessage::Ping { timestamp_ms }`.
//! 2. Receiver echoes `AppMessage::Pong { timestamp_ms }`.
//! 3. RTT = now - send_time.  (One-way latency ≈ RTT / 2.)
//! 4. Sender repeats 5 times (PROBE_COUNT), records samples.
//! 5. From p50 RTT derive a `LinkQuality` rating → chunk size table.

use crate::protocol::AppMessage;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

// ── Constants ─────────────────────────────────────────────────────────────────

/// Number of pings per probe cycle.
pub const PROBE_COUNT: usize = 5;

/// Minimum chunk size (high-latency / poor links).
pub const CHUNK_SIZE_MIN: usize = 16 * 1024; // 16 KB
/// Default chunk size (normal LAN).
pub const CHUNK_SIZE_DEFAULT: usize = 64 * 1024; // 64 KB
/// Maximum chunk size (low-latency, high-bandwidth links).
pub const CHUNK_SIZE_MAX: usize = 256 * 1024; // 256 KB

// ── Link quality ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkQuality {
    /// p50 RTT > 100 ms — likely cross-subnet or congested.
    Poor,
    /// p50 RTT 20–100 ms — normal Wi-Fi.
    Fair,
    /// p50 RTT 5–20 ms — good Wi-Fi or wired.
    Good,
    /// p50 RTT < 5 ms — same-switch or loopback.
    Excellent,
}

impl LinkQuality {
    pub fn from_rtt_us(p50_rtt_us: u64) -> Self {
        match p50_rtt_us {
            0..=4_999 => LinkQuality::Excellent,
            5_000..=19_999 => LinkQuality::Good,
            20_000..=99_999 => LinkQuality::Fair,
            _ => LinkQuality::Poor,
        }
    }

    /// Recommended chunk size for this link quality.
    pub fn chunk_size(self) -> usize {
        match self {
            LinkQuality::Poor => CHUNK_SIZE_MIN,
            LinkQuality::Fair => CHUNK_SIZE_DEFAULT / 2,
            LinkQuality::Good => CHUNK_SIZE_DEFAULT,
            LinkQuality::Excellent => CHUNK_SIZE_MAX,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            LinkQuality::Poor => "poor",
            LinkQuality::Fair => "fair",
            LinkQuality::Good => "good",
            LinkQuality::Excellent => "excellent",
        }
    }
}

// ── ProbeResult ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ProbeResult {
    /// Raw RTT samples (microseconds), in order.
    pub samples_us: Vec<u64>,
    pub min_us: u64,
    pub max_us: u64,
    pub p50_us: u64,
    pub p95_us: u64,
    pub jitter_us: u64, // mean absolute deviation
    pub quality: LinkQuality,
    pub recommended_chunk_size: usize,
}

impl ProbeResult {
    pub fn from_samples(mut samples: Vec<u64>) -> Self {
        assert!(!samples.is_empty());
        samples.sort_unstable();

        let min_us = samples[0];
        let max_us = *samples.last().unwrap();
        let p50_us = samples[samples.len() / 2];
        let p95_us = samples[((samples.len() as f64 * 0.95) as usize).min(samples.len() - 1)];

        let mean = samples.iter().sum::<u64>() / samples.len() as u64;
        let jitter_us = samples
            .iter()
            .map(|&s| (s as i64 - mean as i64).unsigned_abs())
            .sum::<u64>()
            / samples.len() as u64;

        let quality = LinkQuality::from_rtt_us(p50_us);
        let recommended_chunk_size = quality.chunk_size();

        ProbeResult {
            samples_us: samples,
            min_us,
            max_us,
            p50_us,
            p95_us,
            jitter_us,
            quality,
            recommended_chunk_size,
        }
    }

    pub fn p50_ms(&self) -> f64 {
        self.p50_us as f64 / 1000.0
    }
    pub fn p95_ms(&self) -> f64 {
        self.p95_us as f64 / 1000.0
    }

    pub fn summary(&self) -> String {
        format!(
            "RTT p50={:.1}ms p95={:.1}ms jitter={:.1}ms quality={} chunk={}KB",
            self.p50_ms(),
            self.p95_ms(),
            self.jitter_us as f64 / 1000.0,
            self.quality.label(),
            self.recommended_chunk_size / 1024,
        )
    }
}

// ── Adaptive probe controller ─────────────────────────────────────────────────

/// Tracks link quality over time and re-probes when stale.
pub struct QualityProbe {
    last_result: Option<ProbeResult>,
    last_probe_at: Option<Instant>,
    /// How long before a probe result is considered stale.
    max_age: Duration,
    /// Peer device name (for log context).
    peer_name: String,
}

impl QualityProbe {
    pub fn new(peer_name: impl Into<String>) -> Self {
        Self {
            last_result: None,
            last_probe_at: None,
            max_age: Duration::from_secs(30),
            peer_name: peer_name.into(),
        }
    }

    pub fn is_stale(&self) -> bool {
        match self.last_probe_at {
            None => true,
            Some(t) => t.elapsed() > self.max_age,
        }
    }

    /// Record a completed probe result.
    pub fn record(&mut self, result: ProbeResult) {
        tracing::debug!("[probe:{}] {}", self.peer_name, result.summary());
        self.last_probe_at = Some(Instant::now());
        self.last_result = Some(result);
    }

    /// Recommended chunk size — uses last probe, falls back to default.
    pub fn chunk_size(&self) -> usize {
        self.last_result
            .as_ref()
            .map(|r| r.recommended_chunk_size)
            .unwrap_or(CHUNK_SIZE_DEFAULT)
    }

    pub fn quality(&self) -> Option<LinkQuality> {
        self.last_result.as_ref().map(|r| r.quality)
    }
}

// ── Probe message helpers ─────────────────────────────────────────────────────

/// Generate a Ping message with the current timestamp.
pub fn make_ping() -> AppMessage {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    AppMessage::Ping { timestamp_ms: ts }
}

/// Measure RTT from a Ping send to a Pong receipt.
/// `sent_at` = Instant::now() captured just before `make_ping()` was sent.
pub fn measure_rtt_us(sent_at: Instant) -> u64 {
    sent_at.elapsed().as_micros() as u64
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_result_from_samples() {
        let samples = vec![1_000u64, 2_000, 3_000, 4_000, 5_000];
        let result = ProbeResult::from_samples(samples);
        assert_eq!(result.min_us, 1_000);
        assert_eq!(result.max_us, 5_000);
        assert_eq!(result.p50_us, 3_000);
    }

    #[test]
    fn link_quality_classification() {
        assert_eq!(LinkQuality::from_rtt_us(1_000), LinkQuality::Excellent);
        assert_eq!(LinkQuality::from_rtt_us(10_000), LinkQuality::Good);
        assert_eq!(LinkQuality::from_rtt_us(50_000), LinkQuality::Fair);
        assert_eq!(LinkQuality::from_rtt_us(150_000), LinkQuality::Poor);
    }

    #[test]
    fn chunk_size_increases_with_quality() {
        assert!(LinkQuality::Poor.chunk_size() < LinkQuality::Fair.chunk_size());
        assert!(LinkQuality::Fair.chunk_size() < LinkQuality::Good.chunk_size());
        assert!(LinkQuality::Good.chunk_size() < LinkQuality::Excellent.chunk_size());
    }

    #[test]
    fn probe_controller_stale_initially() {
        let probe = QualityProbe::new("TestPeer");
        assert!(probe.is_stale());
        assert_eq!(probe.chunk_size(), CHUNK_SIZE_DEFAULT);
    }

    #[test]
    fn probe_controller_not_stale_after_record() {
        let mut probe = QualityProbe::new("TestPeer");
        let result = ProbeResult::from_samples(vec![2_000u64; PROBE_COUNT]);
        probe.record(result);
        assert!(!probe.is_stale());
        assert_eq!(probe.quality(), Some(LinkQuality::Excellent));
    }

    #[test]
    fn jitter_calculation() {
        // Samples with high variance → high jitter.
        let high = ProbeResult::from_samples(vec![1_000u64, 100_000, 1_000, 100_000, 50_000]);
        // Samples with low variance → low jitter.
        let low = ProbeResult::from_samples(vec![5_000u64, 5_100, 4_900, 5_050, 4_950]);
        assert!(high.jitter_us > low.jitter_us);
    }
}
