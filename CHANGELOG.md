# Changelog

All notable changes to ClipRelay are documented here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Versioning follows [Semantic Versioning](https://semver.org/).

---

## [Unreleased]

### Added
- Initial public release of ClipRelay
- Rust core engine (`cliprelay-core`) with:
  - X25519 ephemeral ECDH key exchange
  - HKDF-SHA256 session key derivation
  - ChaCha20-Poly1305 AEAD encryption with monotonic nonce counter
  - Replay attack protection
  - mDNS-SD service discovery (`_cliprelay._tcp.local.`)
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
  - JNI calls into libcliprelay_core.so
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

- **CLI** (`cliprelay-cli`):
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
- [GitHub](https://github.com/cliprelay/cliprelay)
