# Deskdrop

<div align="center">

  <p><strong>Proximity-based, local-first shared clipboard and secure file relay</strong></p>

  <div>
    <img src="https://img.shields.io/badge/Platform-macOS-lightgrey?style=for-the-badge&logo=apple" alt="macOS" />
    <img src="https://img.shields.io/badge/Platform-Android-brightgreen?style=for-the-badge&logo=android" alt="Android" />
    <img src="https://img.shields.io/badge/Platform-Linux-orange?style=for-the-badge&logo=linux" alt="Linux" />
    <img src="https://img.shields.io/badge/Platform-Windows-blue?style=for-the-badge&logo=windows" alt="Windows" />
  </div>

  <br />
</div>

Deskdrop is a **production-grade, zero-server shared clipboard and peer-to-peer file transfer engine**. Designed to replace bloated cloud-dependent clipboard syncing tools, it keeps your data entirely within your local area network (LAN) or private VPN. Powered by a shared **Rust core**, Deskdrop is blazing fast, light on system resources, and offers premium native integrations for macOS, Android, Linux, and Windows.

---

## 📑 Table of Contents

- [⚡ Key Value Propositions](#-key-value-propositions)
- [🏗️ System Architecture](#️-system-architecture)
- [🌟 Technical Feature Highlights](#-technical-feature-highlights)
- [💻 Command-Line Interface (CLI) Usage](#-command-line-interface-cli-usage)
- [📂 Configuration & Data Layout](#-configuration--data-layout)
- [🛠️ Detailed Building & Installation](#️-detailed-building--installation)
- [🧪 Testing & CI/CD](#-testing--cicd)
- [📚 Deep Architecture Documentation](#-deep-architecture-documentation)
- [🤝 Contributing & License](#-contributing--license)

---

## ⚡ Key Value Propositions

Unlike generic clipboard managers or complex sync utilities, Deskdrop focuses on three core pillars:

1. **Absolute Local-First Privacy (Zero Cloud)**: Your data never contacts an external server. Discovery, handshake, and payload transfers are completely peer-to-peer over the local network using mDNS and secure sockets.
2. **Timeline-First Clipboard UX**: Say goodbye to remote copies silently hijacking your local clipboard. Deskdrop lands incoming text, images, and files in a rich **Activity Feed** first. You preview, tag, pin, and explicitly apply items when you are ready.
3. **Premium Hardware Continuity**: Experience high-end ecosystem features (like Apple's Universal Clipboard and Handoff) on *all* your devices—including call continuity controls, background sleep prevention (App Nap / Modern Standby), and resumable file sharing.

---

## 🏗️ System Architecture

Deskdrop employs a modular, decoupled architecture centered around a high-performance **Rust Engine Library** (`deskdrop-core`). Native platforms interface with the core via direct C FFI, JNI (Android), or Named Pipe IPC (CLI & Daemons).

```text
 ┌───────────────────────────────────────────────────────────────────────────────────────────┐
 │                                       PLATFORM LAYER                                      │
 │   macOS (Swift UI)       Android (Kotlin/JNI)       Windows (C#/WinUI)       Linux (GTK)  │
 └───────────┬───────────────────────┬─────────────────────────┬──────────────────────┬──────┘
             │                       │                         │                      │
             │ Native C FFI          │ JNI Direct              │ Named Pipe IPC       │ Unix IPC
             ▼                       ▼                         ▼                      ▼
 ┌───────────────────────────────────────────────────────────────────────────────────────────┐
 │                                  DESKDROP-CORE (RUST)                                     │
 │                                                                                           │
 │   ┌───────────────────────┐   ┌───────────────────────┐   ┌───────────────────────────┐   │
 │   │       COORDINATOR     │   │      NETWORKING       │   │       SECURITY SUITE      │   │
 │   │   • engine.rs         │   │   • network.rs        │   │   • crypto.rs             │   │
 │   │   • engine_support.rs │   │   • network_manager.rs│   │     - X25519 Ephemeral    │   │
 │   │   • peer_manager.rs   │   │   • discovery.rs      │   │     - HKDF-SHA256         │   │
 │   └───────────┬───────────┘   │     - mDNS Private    │   │     - ChaCha20-Poly1305   │   │
 │               │               └───────────┬───────────┘   │   • pairing.rs            │   │
 │               ▼                           ▼               │     - PINmod10^6          │   │
 │   ┌───────────────────────┐   ┌───────────────────────┐   │   • trust.rs (TOFU)       │   │
 │   │      DATA STORES      │   │     SYNC CONTEXT      │   └───────────────────────────┘   │
 │   │   • history.rs        │   │   • sync_controller.rs│   ┌───────────────────────────┐   │
 │   │   • activity.rs       │   │   • dedup.rs          │   │      FILE TRANSFER        │   │
 │   │   • settings.rs       │   │   • filter.rs         │   │   • chunked.rs (256KB)    │   │
 │   └───────────────────────┘   │   • throttle.rs       │   │   • probe.rs (Adaptive)   │   │
 │                               └───────────────────────┘   └───────────────────────────┘   │
 └───────────────────────────────────────────────────────────────────────────────────────────┘
```

### Protocol & Mesh Lifecycle
1. **Discovery**: Nodes advertise on `_deskdrop._tcp.local.` using **mDNS**. Incompatible versions are pruned early, and IPv4/IPv6 connections are prioritized dynamically.
2. **Handshake (`HelloFrame` / `HelloAckFrame`)**: Peers establish identity and platform metadata. Ephemeral **X25519 ECDH** keys are exchanged in plaintext.
3. **Session Secret**: Diffie-Hellman secret is combined via **HKDF-SHA256** to derive a symmetric session key.
4. **Encrypted Tunnel**: Subsequent frames (`ClipboardPush`, `FileChunk`, `CallStateUpdate`, `BatteryStatus`) are encrypted with **ChaCha20-Poly1305** using strictly monotonic counter nonces.
5. **Peer State & Mesh Dedup**: Device lifecycle passes through five states (Cryptographic Approval, Persistent Pairing, Runtime Connection, Sync Enabled, Auto-connect). Mesh-aware deduplication safely suppresses echoes from all connected peers simultaneously.

---

## 🌟 Technical Feature Highlights

### 🔒 Cryptographic Security Model
* **End-to-End Encryption**: Every session is locked down with standard Curve25519 ECDH key exchange and ChaCha20-Poly1305 AEAD. 
* **Zeroization (`CRIT-02`)**: Memory containing Diffie-Hellman shared secret bytes is explicitly zeroized in RAM immediately after HKDF expansion to eliminate cold-boot vulnerabilities.
* **Strict Replay Protection**: Each frame relies on a strictly monotonic, 64-bit big-endian message counter, enforcing strict chronological order.
* **PIN-Based Secure Pairing**: Combats active Man-in-the-Middle (MITM) attacks by displaying a commutative numeric PIN (`PairingPin`).
* **mDNS Privacy Enforcer (`TRU-06`)**: Friendly device names are never broadcast in plain-text mDNS TXT records. Only opaque UUIDs are published. Canonical device names and OS telemetry are exchanged only *after* a successful encrypted handshake.

### 📦 Resumable Chunked File Transfers
* **High-Speed Chunking**: Files are broken down into 256 KB chunks (`FILE_CHUNK_SIZE`), capped strictly at 512 MB per payload to prevent resource exhaustion attacks.
* **Resumable Pipelines**: If a transfer drops mid-stream, the receiver stores a progressive chunk acknowledgment state. On reconnect, the transfer resumes starting at index `last_confirmed_chunk + 1`.
* **Adaptive Chunk Sizing & Throttling**: Dynamic latency probes monitor link quality degradation to optimize buffer scale and timeout backoffs. The system incorporates an intelligent Token Bucket bandwidth throttle (defaulting to 4 MB/s limits on heavy payloads) to prevent saturating the user's local network. TCP connect timeouts and keepalives guarantee connections drop gracefully under network partitions.
* **Native OS File Handling**: Files are robustly routed directly to the OS-native root `Downloads` directory across all platforms, shielding against path-traversal attacks (`../`).
* **Integrity Guarantee**: Each file includes a pre-transfer SHA-256 validation checksum ensuring payload integrity before final reconstruction.

### 📞 Ecosystem Continuity
* **OS-Level Sleep Immunity**: Deskdrop implements native API calls on each system to maintain seamless background connections.
  * **macOS:** Employs `ProcessInfo.beginActivity` to prevent Apple's App Nap from throttling background daemon syncs.
  * **Windows:** Utilizes `SetThreadExecutionState` to halt Modern Standby sleep transitions during active file transfers.
  * **Android:** Features OEM-specific battery restriction diagnostics (Xiaomi, Samsung) to actively prevent background service termination.
* **Cross-Platform Call Continuity**: Android devices track call states (`ringing`, `offhook`, `idle`) and relay them to macOS peers, bringing premium glassmorphic notification controls to remote screens.
* **Telemetry & Battery Sync**: Passive battery monitoring synchronizes device charge levels (0-100%) and charging states across peers.

### 🛡️ Resilience & Content Filtering
* **Connection Retry Strategy**: Employs an exponential back-off connection retry algorithm featuring ±25% randomized jitter to prevent thundering herd problems during network partitions.
* **Content Filter Chain**: Implements highly configurable payload filtering, including size limits, file extension block-lists (e.g., `.exe`, `.sh`), and sensitive-text heuristics to aggressively block OTPs, API keys, or passwords from bleeding out to other devices.

---

## 💻 Command-Line Interface (CLI) Usage

The `deskdrop-cli` binary communicates with the background Rust daemon over local IPC channels (Unix Domain Sockets / Windows Named Pipes). It offers a powerful admin utility for power users and automation scripts.

### 1. Daemon Management
```bash
# Start the core background daemon natively
cargo run -p deskdrop-core --bin deskdrop-daemon

# Check the health of the daemon
cargo run -p deskdrop-cli -- status
# Expected Output: Daemon is running. Active Connections: 3, Uptime: 4h 23m

# Safely shut down the background daemon
cargo run -p deskdrop-cli -- stop
```

### 2. Network & Telemetry
```bash
# Ping the daemon to test IPC latency
cargo run -p deskdrop-cli -- ping
# Expected Output: PONG (1.2ms)

# View real-time metrics (latency p50/p95, bandwidth throughput)
cargo run -p deskdrop-cli -- metrics
```

### 3. History & Activity Feed
```bash
# Query the most recent 20 items in the clipboard history
cargo run -p deskdrop-cli -- history --last 20

# Pin an item in the history so it isn't evicted
cargo run -p deskdrop-cli -- history pin <id>

# Add a custom tag to an item for searching later
cargo run -p deskdrop-cli -- history tag <id> "work"

# Export the entire history feed to a JSON file for backup
cargo run -p deskdrop-cli -- history export json > backup.json

# Clear all historical payloads from the ring buffer
cargo run -p deskdrop-cli -- history clear
```

### 4. Device Lifecycle & Trust Management
```bash
# List all discovered and paired devices
cargo run -p deskdrop-cli -- devices list

# Manually trust a device via its UUID (TOFU bypass)
cargo run -p deskdrop-cli -- devices trust <device-id>

# Temporarily pause or resume synchronization with a specific peer
cargo run -p deskdrop-cli -- devices peer-settings <device-id> pause
cargo run -p deskdrop-cli -- devices peer-settings <device-id> resume

# Enable auto-connection on daemon startup for a device
cargo run -p deskdrop-cli -- devices auto-connect <device-id> on

# Completely forget a device (removes it from trust registry)
cargo run -p deskdrop-cli -- devices forget <device-id>

# Permanently revoke cryptographic trust for a device
cargo run -p deskdrop-cli -- devices revoke <device-id>
```

---

## 📂 Configuration & Data Layout

Deskdrop organizes its persistent stores according to platform-native user directory standards to avoid polluting your home directory:

| Platform | Base Config & Data Directory |
| :--- | :--- |
| **macOS** | `~/Library/Application Support/deskdrop/` |
| **Linux** | `~/.config/deskdrop/` |
| **Windows** | `%APPDATA%\deskdrop\` |

### Primary Files & Registries
* `settings.json`: Global configuration, regular expression filters, payload thresholds, and general clipboard behavior.
* `trust.json`: Device trust registries, pairing dates, and long-term cryptographical identity public keys.
* `peers.json`: Discovery tables, connection details, and friendly name mappings (e.g. `My MacBook Pro`).
* `history.json`: Bounded activity logs (defaults to a 500-entry ring-buffer history stored as highly resilient NDJSON format).
* `identity.json`: Stores the stable 32-byte X25519 identity key pair. 

> [!CAUTION]
> The private identity scalar in `identity.json` is your absolute identity on the mesh network. It must be kept strictly secure (mode `0600` on Unix-like environments). **Never share or commit these key files.**

---

## 🛠️ Detailed Building & Installation

To compile Deskdrop from source, follow the instructions for your specific platform below. 

### Global Prerequisites
* **Rust Toolchain**: Minimum Supported Rust Version (MSRV) `1.75+`. Install via [rustup](https://rustup.rs/).
* **CMake**: Required for building the underlying cryptographic and hashing C-libraries (`ring`, etc).

### Core Compilation
Before building any UI wrapper, you must ensure the core workspace compiles successfully:
```bash
# Clone the repository
git clone https://github.com/deskdrop/deskdrop.git
cd deskdrop

# Build the Rust daemon workspace
cargo build --release --workspace
```

### Native Client Platforms

> [!NOTE]
> The shared Rust core must be compiled into target libraries (static/dynamic `.a`, `.dylib`, `.so`, or `.dll`) before compiling the platform wrappers. Scripts to automate this are located in `scripts/`.

#### macOS (`platforms/macos`)
Written in native Swift, featuring a status bar manager, SwiftUI history popovers, glassmorphic dashboards, and real-time continuity overlays.
1. Build the universal `libdeskdrop_core.dylib` binary via the provided script `scripts/build-macos.sh`.
2. Open `platforms/macos/Deskdrop.xcodeproj` in Xcode 15+.
3. Select your signing identity and build the project. The project leverages hardened runtimes and App Sandbox entitlements.
4. Note: The application uses `LSUIElement` to run strictly in the menu bar without a dock icon.

#### Android (`platforms/android`)
Leverages JNI to bridge `jni_android.rs` to Kotlin background services. Features a low-importance persistent notification, a rich in-app activity feed to eliminate clipboard noise, and a dedicated full-screen pairing activity for PIN verification.
1. Ensure the Android NDK is installed.
2. Run `scripts/build-android.sh` to generate `libdeskdrop_core.so` for `arm64`, `armv7`, and `x86_64`.
3. Open `platforms/android` in Android Studio.
4. Build the APK. The application utilizes a `BootReceiver` for seamless auto-start on device reboot.

#### Linux (`platforms/linux`)
Built on top of the GTK graphical framework.
1. Ensure development headers for GTK3 and D-Bus are installed.
2. Run the Linux wrapper directly via Cargo:
```bash
cargo run -p deskdrop-linux
```
3. A `.desktop` file is provided for XDG autostart, alongside a `systemd` user service unit featuring extensive security bounding.

#### Windows (`platforms/windows`)
Leverages native WinUI/WPF integration, running quietly in the System Tray with a searchable clipboard history floating panel.
1. Ensure the Windows SDK and MSVC build tools are installed.
2. Build the Rust `deskdrop_core.dll`.
3. Open the C# solution in Visual Studio 2022.
4. Build and run the project. Deskdrop utilizes a Win32 clipboard sequence-number watcher and automatically registers for startup via `HKCU\...\Run`.
5. For deployment, a WiX v4 MSI installer is available that automatically registers the required Firewall exceptions.

---

## 🧪 Testing & CI/CD

Deskdrop relies on a comprehensive automated testing pipeline to enforce correctness across platforms:
* **SimNetwork Harness**: Employs an in-process mock networking harness for deterministic, multi-node mesh integration tests without utilizing raw sockets.
* **Criterion Benchmarking**: Real-time performance tracking for X25519 handshakes, ChaCha20 bulk encryption, chunk reassembly, and hash-based deduplication algorithms.
* **CI/CD (`ci.yml` & `release.yml`)**: Automated pipelines perform Rust formatting, `clippy` checks, and run integration tests on Linux, macOS, and Windows. Release actions automatically generate Software Bill of Materials (SBOM), run `cargo audit` security scans, and produce signed macOS `.dylib` and Windows `.msi` artifacts.

---

## 📚 Deep Architecture Documentation

For contributors and security researchers, we maintain in-depth documentation covering the internal mechanics of Deskdrop. Please review these documents located in the `docs/architecture/` folder:

* [CORE_ENGINE.md](docs/architecture/CORE_ENGINE.md) — mDNS Discovery, the 5-Layer Peer Lifecycle, and Mesh Deduplication.
* [SECURITY_AND_PROTOCOL.md](docs/architecture/SECURITY_AND_PROTOCOL.md) — Protocol Framing, X25519/ChaCha20 Cryptography, Replay Protection, and Trust pairing logic.
* [FILE_TRANSFERS.md](docs/architecture/FILE_TRANSFERS.md) — Adaptive Chunked pipelines, latency probes, and Resumability.
* [PLATFORM_INTEGRATION.md](docs/architecture/PLATFORM_INTEGRATION.md) — Deep dives into macOS App Nap prevention, Android JNI, and Windows Modern Standby hooks.

---

## 🤝 Contributing & License

Contributions are highly encouraged! 

* Read our [CONTRIBUTING.md](CONTRIBUTING.md) to understand our codebase structure, continuous integration pipeline, and security auditing requirements.
* Review our [CHANGELOG.md](CHANGELOG.md) for the latest release notes.
* Read our [SECURITY.md](SECURITY.md) for vulnerability disclosure guidelines.

### License

Deskdrop is open-source software licensed under the **GNU Affero General Public License v3.0 (AGPL-3.0)**. See [LICENSE](LICENSE) for full details.
