# Changelog

All notable changes to Deskdrop are documented here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Versioning follows [Semantic Versioning](https://semver.org/).

---

## [Unreleased]

### Added (v4)
- **Android:** Added "Finish Onboarding" re-entry flow to empty dashboard.
- **Android:** Added OEM-specific battery restriction diagnostics (Xiaomi, Samsung) to help prevent background service termination.
- **macOS:** Implemented `ProcessInfo.beginActivity` to prevent App Nap from throttling the background daemon.
- **Windows:** Implemented `SetThreadExecutionState` to prevent Modern Standby sleep during active file transfers.
- **Core:** Received files are now saved directly to the root `Downloads` directory across all platforms.

### Added (v3)
- `ClipboardContent::is_empty()` — single guard method, eliminating per-call-site duplication (#16)
- `CompressionStats: Display` — consistent log format, usable in IPC responses (#12)
- `QualityProbe::degraded_from()` — detect link quality regressions between probe cycles with `quality_severity` ordering (#13)
- `PeerManager::sync_eligible_count()` — O(1) count of connected+trusted+sync-enabled peers (#14)
- TCP connect timeout (5 s) via `tokio::time::timeout` on all outbound connects (#1)
- TCP keepalive (`SO_KEEPALIVE`, idle 30 s / interval 5 s / 3 probes) on both accept and connect paths (#9)
- Unit tests: `network.rs` (framing, nonce XOR, nonce-echo verification, handshake integration), `discovery.rs` (address preference, version validation), `peer_manager.rs` (`connected_count`, `sync_eligible_count`), `probe.rs` (`degraded_from`, quality ordering), `compress.rs` (Display), `chunked.rs` (size cap rejection), `identity.rs` (fingerprint format), `protocol.rs` (is_empty)

### Fixed (v3)
- **Critical** — Handshake nonce verification was a stub; replay protection now enforced on the initiator side (#2)
- **Critical** — `set_nodelay(true)` was absent on the outbound (initiator) TCP path, adding ~40 ms Nagle delay (#3)
- **Critical** — `pairing.rs` `expire_stale()` silently dropped oneshot channels; now sends explicit `false` so waiters receive `Ok(false)` instead of a channel error (#5)
- **Critical** — `fuzz_sanity_test.rs` used `ExtensionFilter` before it existed; `bincode` added to `[dev-dependencies]` so integration tests compile without a separate crate override (#4)
- **Security** — mDNS version TXT record `v` was advertised but never validated on the browsing side; incompatible peers are now skipped at mDNS time (#6)
- **Security** — mDNS address selection used `HashSet::iter().next()` (arbitrary order); now prefers IPv4 → IPv6 global → IPv6 link-local to avoid silent connect failures from fe80:: addresses without a scope_id (#7)
- **Security** — `Reassembler::feed(Start)` had no total-payload size cap; malicious peers could announce `u64::MAX` bytes and tie up in-flight state indefinitely; capped at 512 MB / 8 192 chunks (#8)
- **Correctness** — `fingerprint_display()` doc comment showed wrong format (16 pairs of 2 chars) vs actual output (8 groups of 4 chars); corrected and test added (#10)
- **Correctness** — `crypto_vectors_test.rs` session key vector was a non-asserting stub; real HKDF-SHA256 vector computed and asserted (#11)
- **Correctness** — CLI unknown command printed bare `"Unknown command: foo"` with no quoting and wrong help hint; now prints `"Unknown command: 'foo'\n\nRun \`deskdrop-cli help\` to see all available commands."` (#15)

- Rust core engine (`deskdrop-core`) with:
  - X25519 ephemeral ECDH key exchange
  - HKDF-SHA256 session key derivation
  - ChaCha20-Poly1305 AEAD encryption with monotonic nonce counter
  - Replay attack protection
  - mDNS-SD service discovery (`_deskdrop._tcp.local.`)
  - Framed TCP transport with sub-500 ms LAN propagation
  - TOFU (Trust On First Use) device trust model
  - PIN-based pairing as alternative to TOFU
  - Chunked transfer for payloads > 128 KB (streaming, pipeline-friendly)
  - Echo suppression (deduplication) with per-peer rate limiting
  - Clipboard history ring buffer (100 entries, persisted as NDJSON)
  - Per-peer metrics: latency p50/p95, throughput, session duration
  - Bandwidth throttle (token bucket, default 4 MB/s for large payloads)
  - Content filter chain: size limits, type allow-list, extension block-list,
    optional sensitive-text heuristics
  - Connection retry with exponential back-off and ±25 % jitter
  - Settings system with atomic JSON persistence and hot-patch API
  - Unix domain socket IPC server (CLI ↔ daemon)
  - C FFI exports for macOS / Windows platform wrappers
  - JNI bridge for Android

- **macOS** platform (`platforms/macos/`):
  - Menu-bar–only app (LSUIElement)
  - NSPasteboard watcher at 100 ms poll
  - TOFU sheet with fingerprint display
  - SwiftUI clipboard history popover (last 100 entries, searchable)
  - SwiftUI preferences panel with live settings editing
  - Hardened runtime + App Sandbox entitlements
  - Universal dylib (arm64 + x86_64) build script
  - Code-signing and optional DMG packaging

- **Windows** platform (`platforms/windows/`):
  - System-tray WinForms application
  - Win32 clipboard sequence-number watcher
  - P/Invoke bridge to Rust DLL
  - TOFU MessageBox with fingerprint
  - Clipboard history floating panel (searchable, re-push)
  - Preferences dialog with Registry persistence
  - Auto-start via `HKCU\...\Run`
  - Named-pipe IPC client (`DaemonClient`)
  - Daemon status poller (`DaemonPoller`)
  - WiX v4 MSI installer with firewall exception

- **Android** platform (`platforms/android/`):
  - Foreground service with `foregroundServiceType=dataSync`
  - JNI calls into libdeskdrop_core.so
  - Clipboard monitoring via `ClipboardManager`
  - PIN-based pairing full-screen activity
  - Settings activity
  - Auto-start `BootReceiver`
  - Notification channel (low-importance, persistent)

- **Linux** platform (`platforms/linux/`):
  - Headless daemon mode (arboard for X11 + Wayland)
  - `notify-send` desktop notifications
  - `.desktop` file for XDG autostart
  - systemd user service unit with security hardening

- **CLI** (`deskdrop-cli`):
  - `status`, `ping`, `push`, `peers`
  - `devices list`, `devices revoke`
  - `history`, `history --last N`, `history --search`, `history clear`
  - `settings get/set/reset`
  - `sync on/off`, `stop`
  - Live IPC when daemon running; offline file-read fallback

- **CI / CD** (`.github/workflows/`):
  - `ci.yml`: Rust fmt/clippy/test on Linux, macOS, Windows;
    Android cross-compile for arm64/armv7/x86_64;
    macOS Swift type-check; Windows dotnet build;
    `cargo audit` security scan; SBOM generation
  - `release.yml`: Produces universal macOS dylib, Windows zip + MSI,
    Android APK, Linux tarball; creates GitHub Release with all artifacts

- **Tests**:
  - Unit tests in every module (crypto, trust, dedup, chunked, settings,
    history, metrics, filter, retry, throttle, pairing, sim)
  - In-process `SimNetwork` harness for deterministic two-node tests
  - Criterion benchmarks: X25519 handshake, ChaCha20 encrypt/decrypt
    (1 KB – 4 MB), chunk reassembly, content hashing, dedup
  - Integration test: two real engine instances exchanging clipboard text

---

## Links

- [Security Policy](SECURITY.md)
- [Contributing](CONTRIBUTING.md)
- [GitHub](https://github.com/deskdrop/deskdrop)
