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

## ⚡ Key Value Propositions

Unlike generic clipboard managers or complex sync utilities, Deskdrop focuses on three core pillars:

1. **Absolute Local-First Privacy (Zero Cloud)**: Your data never contacts an external server. Discovery, handshake, and payload transfers are completely peer-to-peer over the local network using mDNS and secure sockets.
2. **Timeline-First Clipboard UX**: Say goodbye to remote copies silently hijacking your local clipboard. Deskdrop lands incoming text, images, and files in a rich **Activity Feed** first. You preview, tag, pin, and explicitly apply items when you are ready.
3. **Premium Hardware Continuity**: Experience high-end ecosystem features (like Apple's Universal Clipboard and Handoff) on *all* your devices—including call continuity controls, battery status telemetry, and resumable file sharing.

---

## 🏗️ System Architecture

Deskdrop employs a modular, decoupled architecture centered around a high-performance **Rust Engine Library** (`deskdrop-core`). Native platforms interface with the core via direct C FFI, JNI (Android), or Named Pipe IPC (CLI & Daemons).

```text
 ┌───────────────────────────────────────────────────────────────────────────────────────────┐
 │                                       PLATFORM LAYER                                      │
 │   macOS (Swift UI)       Android (Kotlin/JNI)       Windows (C#/WinUI)       Linux (GTK)      │
 └───────────┬───────────────────────┬─────────────────────────┬──────────────────────┬──────┘
             │                       │                         │                      │
             │ Native C FFI          │ JNI Direct              │ Named Pipe IPC       │ Unix IPC
             ▼                       ▼                         ▼                      ▼
 ┌───────────────────────────────────────────────────────────────────────────────────────────┐
 │                                  DESKDROP-CORE (RUST)                                    │
 │                                                                                           │
 │   ┌───────────────────────┐   ┌───────────────────────┐   ┌───────────────────────────┐   │
 │   │       COORDINATOR     │   │      NETWORKING       │   │       SECURITY SUITE      │   │
 │   │   • engine.rs         │   │   • network.rs        │   │   • crypto.rs             │   │
 │   │   • engine_support.rs │   │   • network_manager.rs│   │     - X25519 Ephemeral    │   │
 │   └───────────┬───────────┘   │   • discovery.rs      │   │     - HKDF-SHA256         │   │
 │               │               │     - mDNS Private    │   │     - ChaCha20-Poly1305   │   │
 │               │               └───────────┬───────────┘   │   • pairing.rs            │   │
 │               ▼                           ▼               │     - PINmod10^6          │   │
 │   ┌───────────────────────┐   ┌───────────────────────┐   │   • trust.rs (TOFU)       │   │
 │   │      DATA STORES      │   │     SYNC CONTEXT      │   └───────────────────────────┘   │
 │   │   • history.rs        │   │   • sync_controller.rs│   ┌───────────────────────────┐   │
 │   │   • activity.rs       │   │   • dedup.rs          │   │      FILE TRANSFER        │   │
 │   │   • settings.rs       │   │   • filter.rs         │   │   • file_transfer.rs      │   │
 │   └───────────────────────┘   │   • throttle.rs       │   │   • chunked.rs (256KB)    │   │
 │                               └───────────────────────┘   │   • probe.rs (Adaptive)   │   │
 │                                                           └───────────────────────────┘   │
 └───────────────────────────────────────────────────────────────────────────────────────────┘
```

### Protocol Frame Lifecycle
1. **Discovery**: Nodes advertise on `_deskdrop._tcp.local.` using **mDNS**.
2. **Handshake (`HelloFrame` / `HelloAckFrame`)**: Peers establish identity and platform metadata. Ephemeral **X25519 ECDH** keys are exchanged in plaintext.
3. **Session Secret**: Diffie-Hellman secret is combined via **HKDF-SHA256** to derive a symmetric session key.
4. **Encrypted Tunnel**: Subsequent frames (`ClipboardPush`, `FileChunk`, `CallStateUpdate`, `BatteryStatus`) are encrypted with **ChaCha20-Poly1305** using strictly monotonic counter nonces.

---

## 🌟 Technical Feature Highlights

### 🔒 Cryptographic Security Model
* **End-to-End Encryption**: Every session is locked down with standard Curve25519 ECDH key exchange and ChaCha20-Poly1305 AEAD. 
* **Zeroization (`CRIT-02`)**: Memory containing Diffie-Hellman shared secret bytes is explicitly zeroized in RAM immediately after HKDF expansion to eliminate cold-boot and dump vulnerabilities.
* **Strict Replay Protection**: Each frame relies on a strictly monotonic, 64-bit big-endian message counter. Replaying captured packets is rejected immediately at the protocol level.
* **PIN-Based Secure Pairing**: Combats active Man-in-the-Middle (MITM) attacks by displaying a commutative numeric PIN (`PairingPin`) derived dynamically:
  $$\text{PIN} = \text{HKDF-SHA256}(\text{shared\_secret}, \text{"deskdrop-pin"}) \pmod{10^6}$$
  Displayed in two split 3-digit groups (e.g., `048 291`) with uniform distribution.
* **mDNS Privacy Enforcer (`TRU-06`)**: Friendly device names are never broadcast in plain-text mDNS TXT records. Only opaque UUIDs are published. Canonical device names are exchanged only *after* a successful encrypted handshake.

### 📦 Resumable Chunked File Transfers
* **High-Speed Chunking**: Files are broken down into standard $256\text{ KB}$ chunks (`FILE_CHUNK_SIZE`).
* **Resumable Pipelines**: If a transfer drops mid-stream, the receiver stores a progressive chunk acknowledgment state. On reconnect, the transfer resumes starting at index `last_confirmed_chunk + 1`, skipping completed data.
* **Adaptive Chunk Sizing (`HIGH-03`)**: Dynamic round-trip telemetry monitors connection quality (pings/pongs) using latency probes, scaling transfer buffer allocations to optimize throughput.
* **Path Traversal Shielding (`MED-04`)**: Senders' file names are strictly stripped of traversal characters (e.g. `../`), separators, and leading dots (`.`). They are bound securely within a dedicated `Deskdrop/` downloads directory.
* **Integrity Guarantee**: Finalization requires SHA-256 validation of the re-assembled file in temp storage. Corrupted hashes prompt immediate deletion.

### 📞 Ecosystem Continuity
* **Cross-Platform Call Continuity**: Android mobile devices track incoming calls via `TelecomManager` APIs and relay call states (`ringing`, `offhook`, `idle`) alongside contact details to peers. The macOS client triggers a premium glassmorphic notification banner letting users accept or decline the call remotely.
* **Telemetry & Battery Sync**: Passive battery monitoring synchronizes device charge levels ($0\text{--}100\%$) and charging states across peers periodically or on $\ge 5\%$ shifts.
* **Sensitive Text Filtering**: Configurable regular expression ignore patterns, payload limit debouncers, and secret filtering block OTPs, API keys, or passwords from propagating.

---

## 💻 Command-Line Interface (CLI)

The `deskdrop-cli` binary communicates with the background Rust daemon over local IPC channels, offering a powerful admin utility.

```bash
# Start the core background daemon
cargo run -p deskdrop-core --bin deskdrop-daemon

# Inspect status, network interfaces, and peer health
cargo run -p deskdrop-cli -- status
cargo run -p deskdrop-cli -- ping
cargo run -p deskdrop-cli -- metrics

# Query, search, tag, and export activity history
cargo run -p deskdrop-cli -- history --last 20
cargo run -p deskdrop-cli -- history pin <id>
cargo run -p deskdrop-cli -- history tag <id> "work"
cargo run -p deskdrop-cli -- history export csv
cargo run -p deskdrop-cli -- history export json
cargo run -p deskdrop-cli -- history stats

# Control peer trust settings and synchronization bounds
cargo run -p deskdrop-cli -- devices list
cargo run -p deskdrop-cli -- devices trust <device-id>
cargo run -p deskdrop-cli -- devices peer-settings <device-id> pause

# Manage reusable clipboard templates
cargo run -p deskdrop-cli -- template list
cargo run -p deskdrop-cli -- template push "billing_address"
```

---

## 📂 Configuration & Data Layout

Deskdrop organizes its persistent stores according to platform-native user directory standards:

| Platform | Base Config & Data Directory |
| :--- | :--- |
| **macOS** | `~/Library/Application Support/deskdrop/` |
| **Linux** | `~/.config/deskdrop/` |
| **Windows** | `%APPDATA%\deskdrop\` |

### Primary Files
* `settings.json`: Global configuration, regular expression filters, payload thresholds, and clipboard behavior.
* `trust.json`: Device trust registries, pairing dates, and long-term cryptographical identity public keys.
* `peers.json`: Discovery tables, connection details, and friendly name mappings.
* `history.json`: Bounded activity logs (defaults to a $500\text{--}\text{entry}$ ring-buffer history).
* `identity.json`: Stores the stable 32-byte X25519 identity key pair. 

> [!CAUTION]
> The private identity scalar in `identity.json` must be kept strictly secure (mode `0600` on Unix-like environments). Never share or commit these key files.

---

## 🛠️ Building & Platform Notes

### Prerequisites
* Rust toolchain (MSRV 1.75+)
* CMake (for building cryptographic and hashing libraries)

### Core Compilation
Build the Rust daemon workspace:
```bash
cargo build --release --workspace
```

### Native Client Platforms

> [!NOTE]
> The shared Rust core must be compiled into target libraries (static/dynamic `.a`, `.dylib`, `.so`, or `.dll`) before compiling the platform wrappers.

* **macOS (`platforms/macos`)**: Open the project folder in Xcode. Written in native Swift, featuring a status bar manager, glassmorphic dashboards, and real-time continuity overlays.
* **Android (`platforms/android`)**: Open the project folder in Android Studio. Leverages JNI to bridge `jni_android.rs` to Kotlin services, managing background sync tasks and call handlers.
* **Linux (`platforms/linux`)**: Built on top of the GTK graphical framework. Run the Linux wrapper directly:
  ```bash
  cargo run -p deskdrop-linux
  ```
* **Windows (`platforms/windows`)**: Open the C# project in Visual Studio, leveraging native WinUI/WPF integration.

---

## 🤝 Contributing

Contributions are highly encouraged! Please read our [CONTRIBUTING.md](CONTRIBUTING.md) to understand our codebase structure, continuous integration pipeline, and security auditing requirements.

## 📄 License

Deskdrop is open-source software licensed under the **MIT License**. See [LICENSE](LICENSE) for full details.
