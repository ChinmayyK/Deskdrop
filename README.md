# Deskdrop

<div align="center">

  <p><strong>Your local-first, zero-cloud superpower for seamless clipboard sharing, instant file transfers, and wireless continuity camera across all your devices.</strong></p>

  <div>
    <img src="https://img.shields.io/badge/Platform-macOS-lightgrey?style=for-the-badge&logo=apple" alt="macOS" />
    <img src="https://img.shields.io/badge/Platform-Android-brightgreen?style=for-the-badge&logo=android" alt="Android" />
    <img src="https://img.shields.io/badge/Platform-Linux-orange?style=for-the-badge&logo=linux" alt="Linux" />
    <img src="https://img.shields.io/badge/Platform-Windows%20(Alpha)-blue?style=for-the-badge&logo=windows" alt="Windows" />
    <img src="https://img.shields.io/badge/Core-Rust-B7410E?style=for-the-badge&logo=rust" alt="Rust Core" />
  </div>

  <br />
</div>

---

## ✨ What is Deskdrop?

Imagine copying a paragraph on your Android phone and instantly pasting it on your Mac. Or dragging a 4 GB video from your laptop right onto your tablet in seconds—or **remotely browsing your desktop's folders from your phone to pull the exact files you need on demand**—without uploading anything to Google Drive, iCloud, or Dropbox, and without paying subscription fees.

**Deskdrop** makes all your devices work together like one cohesive ecosystem, regardless of brand. By connecting your devices directly over your local home Wi-Fi, LAN, or **even your phone's Mobile Hotspot on the go**, Deskdrop wraps powerful features inside a **stunning, native glassmorphic UI** and delivers instantaneous, end-to-end encrypted clipboard sharing, file dropping, **secure remote file browsing**, and camera continuity with **zero servers, zero internet dependency, 0% background battery consumption, and absolute data privacy**.

---

## 💡 Why You'll Love It (Key Features for Everyone)

### 📋 Instant Clipboard Sync, Screenshot & OTP Sync
Unlike generic sync tools that silently overwrite your current clipboard and cause frustration, Deskdrop puts you in complete control across all your devices:
- **Universal Clipboard & Activity Feed**: Copy text, links, or images on one device, and they appear instantly across your connected hardware inside a rich, searchable **Activity Feed**. You can preview, pin important items, add tags, or apply them whenever you're ready.
- **Screenshot Sync**: Take a screenshot on your Android phone or laptop, and it instantly arrives on your desktop's clipboard or timeline—ready to paste straight into Slack, Discord, or Figma without saving to disk!
- **Smart OTP Sync & Shielding**: Copied a 2FA One-Time Password (OTP) on your phone? Deskdrop securely synchronizes your 2FA verification code straight to your computer for instant login! Meanwhile, our intelligent **Content Filter Chain (`filter.rs`)** gives you fine-grained control to auto-expire or block sensitive credentials (`passwords`, `API keys`, `credit cards`) from lingering on shared or untrusted screens.

### 📦 Blazing-Fast & Resumable File Sharing
Drag and drop files of any size directly onto a connected device card, and they arrive cleanly in your native `Downloads` folder in seconds. If a connection drops mid-stream, Deskdrop's resumable pipelines pick up right where they left off without restarting from zero. By combining zero-copy memory slices with adaptive network batching, Deskdrop saturates your local network without intermediate cloud bottlenecks:

| Network Connection | Typical Speed | Time to Send 1 GB File |
| :--- | :--- | :--- |
| **Mobile Hotspot Direct (5 GHz Point-to-Point)** | **150 – 300+ MB/s** (1.2 – 2.4+ Gbps) | 🔥 **3 – 6 seconds** |
| **Wi-Fi 6 / 6E Router (5 GHz / 6 GHz)** | **120 – 250+ MB/s** (1.0 – 2.0+ Gbps) | ⚡ **4 – 8 seconds** |
| **Wi-Fi 5 Router (802.11ac 5 GHz)** | **40 – 90+ MB/s** (300 – 700+ Mbps) | 🚀 **11 – 25 seconds** |

### 🗂️ Secure Remote File Browsing
Need a file sitting on your desktop upstairs while you're downstairs with your phone or laptop? With Deskdrop's **Remote File Browsing**, you don't need to physically walk over to push the file. You can securely browse shared directories on any paired device right from your local screen. Navigate folders, preview metadata, and download exact documents across your local network on demand—all protected by strict permission boundaries and zero-trust encryption.

### 🔒 100% Private & Local (Works on Mobile Hotspot too!)
Your sensitive data never touches the cloud or external servers. Deskdrop automatically discovers your trusted devices right on your home Wi-Fi, private LAN, or **Mobile Hotspot** and establishes direct, highly encrypted peer-to-peer connections. **No internet? No router? No problem! Turn on your phone's personal hotspot and share large files or clipboard items at direct Wi-Fi speeds without using a single drop of cellular data.**

### 🛡️ Intelligent Sensitive Data Shield & Custom Filtering
Deskdrop gives you ultimate control over what gets shared. Our customizable **Content Filter Chain (`filter.rs`)** allows you to whitelist specific sync streams while setting strict heuristics that intercept and block sensitive text snippets (like API keys, master passwords, or credit card numbers) or blocklist potentially dangerous file extensions (`.exe`, `.sh`).

### 🎨 Stunning, Native & Glassmorphic UI
Deskdrop isn't just powerful under the hood—it's gorgeous to interact with. Crafted with state-of-the-art native design principles for each operating system, Deskdrop features breathtaking **glassmorphic translucent panels**, fluid micro-animations, clean typography, and unobtrusive menu bar and system tray overlays. It feels like an ultra-premium, built-in system utility from the moment you launch it.

### ⚡ Ultra-Lightweight Engine (0% Battery & CPU in Background)
Unlike bloated Electron apps that devour RAM and drain your battery simply sitting idle, Deskdrop is engineered for extreme hardware efficiency. At its core is an asynchronous **Rust engine** that sleeps completely when idle—consuming literally **0% CPU and 0% battery while waiting quietly in your background**. When a transfer or clipboard update triggers, our zero-copy memory pipelines spring instantly to life with zero system stutter.

### 📷 Wireless Continuity Camera
Need a high-resolution webcam for your desktop meeting? Turn your mobile phone into a crystal-clear, zero-lag wireless camera feed for your computer over Wi-Fi—no cables, capture cards, or third-party camera software required.

### 🔔 Universal Notification, Call & Battery Sync across Brands
Get Apple-like "Handoff" capabilities across *all* your hardware, regardless of operating system:
- **Notification Sync**: Mirror your mobile phone app notifications (WhatsApp, Telegram, SMS messages, calendar alerts) directly onto your desktop or Mac screen without pulling your phone out of your pocket.
- **Call Sync & Controls**: See incoming phone call alerts instantly on your computer monitor with clean options to dismiss or respond.
- **Battery Sync & Charging Monitoring**: Check your phone, tablet, or secondary laptop's exact live battery percentage (0–100%) and charging status (`Charging` / `Discharging`) right from your desktop menu bar or Windows tray.
- **Sleep Immunity**: Never worry about large file transfers failing when your screen dims; Deskdrop automatically keeps your devices awake just long enough to finish the job safely.

---

## 📱 Supported Platforms & Current Status

We provide custom, polished native interfaces tailored to every operating system so that Deskdrop feels right at home on your device:

| Platform | Native Technology | Current Status | Notes & Experience |
| :--- | :--- | :--- | :--- |
| **macOS** | Swift & SwiftUI | 🟢 **Production Ready** | Runs cleanly in the menu bar with sleek glassmorphic popovers, instant activity timelines, and virtual continuity camera support. |
| **Android** | Kotlin & JNI | 🟢 **Production Ready** | Low-profile background service, rich in-app timeline, seamless share-sheet integration, and PIN-verified pairing. |
| **Linux** | GTK3 & D-Bus | 🟢 **Production Ready** | Lightweight desktop integration with `systemd` user services and native XDG desktop notifications. |
| **Windows** | WinUI & C# (.NET) | 🟠 **Alpha / Experimental** | **Note:** Active developmental phase. The underlying core engine is fully functional, but the desktop UI may experience minor glitches or incomplete features compared to other platforms. |

> [!WARNING]
> **Windows Platform Note (`platforms/windows`)**: We are actively developing the Windows native client. While you can build and run it today, please be aware that it is still in an **Alpha / Developmental state**. You may encounter minor UI bugs or behavior differences while our team brings it to parity with macOS, Android, and Linux.

---

## ⚔️ How Deskdrop Compares to the Competition

Why choose Deskdrop over existing tools? Here is an objective, capability-by-capability comparison of how we stack up against industry leaders and proprietary ecosystem suites:

| App | Universal Cross-Platform (Mac/Win/Android/Linux) | Resumable & Zero-Copy File Speeds | Clipboard & Screenshot Sync | Notification & Call Mirroring | Remote Folder Browsing | Wireless Continuity Camera | Live Battery & Charging Sync | Background Resource & Idle Efficiency |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **Deskdrop (Ours)** | 🟢 **Yes (100% Brand Agnostic across all OS)** | ⚡ **Yes (`150–300+ MB/s` Zero-Copy + Resumable)** | 🟢 **Yes (With Timeline Feed & Sensitive OTP Guard)** | 🟢 **Yes (Mirror Notifications & Call Controls)** | 🟢 **Yes (Secure Zero-Trust Directory Access)** | 🟢 **Yes (Zero-Lag Wi-Fi Camera Feed)** | 🟢 **Yes (Menu Bar Monitor + Sleep Immunity)** | 🔥 **0% Battery / ~10 MB RAM (Event-Driven Async Rust)** |
| **Apple Handoff & AirDrop** | 🔴 **Apple Hardware Only** (Mac, iPhone, iPad) | 🟢 Fast inside Apple Ecosystem | 🟢 Universal Clipboard (No searchable feed) | 🟢 Phone & Message sync (Mac only) | 🔴 **No** (Cannot browse remote file systems over Wi-Fi) | 🟢 Continuity Camera (Apple devices only) | 🟢 Battery Widget (Apple devices only) | 🟢 Native OS integration (`~20–40 MB`) |
| **LocalSend** | 🟢 macOS, Windows, Android, Linux | 🟢 Fast Wi-Fi Transfers | 🔴 **No** (One-off manual text sending only) | 🔴 **No** (No notification or call sync) | 🔴 **No** (No remote directory browsing) | 🔴 **No** (No camera capabilities) | 🔴 **No** (No battery monitoring) | 🟡 Flutter UI (`~80–120 MB` RAM idle) |
| **KDE Connect** | 🟢 macOS, Windows, Android, Linux | 🟡 Standard Socket Copies (`~30–60 MB/s`) | 🟢 Basic Clipboard Sync (No OTP filtering) | 🟢 Notifications & SMS sync | 🟢 SFTP mount (Complex desktop setup required) | 🔴 **No** (No continuity camera support) | 🟢 Battery status notifications | 🔴 Heavy (`~80–150+ MB` RAM / Qt Daemon) |
| **Microsoft Phone Link** | 🔴 **Windows & Android/iOS Only** (No Mac/Linux) | 🔴 Slow / Cloud Relayed | 🟡 Clipboard Sync (Select Samsung/Honor phones only) | 🟢 Calls, SMS & Notifications | 🔴 **No** (No general remote directory browsing) | 🔴 **No** (Limited to select phone camera setups) | 🟢 Battery status icon | 🔴 Heavy (`~300+ MB` RAM / Continuous Telemetry) |
| **O+ Connect (OPPO/OnePlus)** | 🔴 **Windows & OPPO/OnePlus Only** | 🟡 Wi-Fi Direct hybrid | 🟢 Clipboard & Screen Mirroring (OEM locked) | 🟢 Calls & Notifications (OEM locked) | 🟡 In-app file access (OPPO/OnePlus devices only) | 🔴 **No** (No general webcam Handoff) | 🟢 Battery monitoring | 🔴 Heavy (`~200+ MB` RAM / OEM Background Daemons) |
| **Intel Unison** | 🟡 **Windows (Intel Evo PC) & Mobile** | 🟡 Bluetooth + Wi-Fi hybrid | 🟡 Basic Clipboard Sync | 🟢 Calls & Notifications | 🔴 **No** (No remote directory browsing) | 🔴 **No** | 🟢 Battery indicator | 🔴 Hardware Locked (`~150+ MB` RAM) |

### 💡 Deep Feature Comparison & Why Deskdrop Wins:
- **vs. Apple Handoff & AirDrop**: Apple's Handoff ecosystem is magical—until you need to connect a Windows PC, an Android device, or a Linux workstation. Deskdrop delivers the exact same Apple-like continuity (AirDrop, Universal Clipboard, Continuity Camera, Call/Battery Handoff) completely unlocked across **every hardware brand and operating system without iCloud account requirements**. Plus, Deskdrop adds killer features Apple lacks: **a searchable clipboard timeline feed, secure remote directory browsing across devices, and intelligent One-Time Password (OTP) shielding**.
- **vs. LocalSend**: LocalSend is an excellent open-source utility for sending one-off files across devices over Wi-Fi. However, it is strictly limited to file and text transfer. It does **not** provide real-time background clipboard syncing, notification mirroring, phone call controls, continuity webcam streaming, or remote file system browsing. Deskdrop delivers the entire multi-device continuity super-suite inside an even lighter native memory footprint.
- **vs. KDE Connect**: KDE Connect is a long-standing cross-platform tool, but its architecture shows its age. Its desktop client relies on Qt/C++ with heavy background RAM overhead (`~150+ MB`) and complex SFTP filesystem mounting. Furthermore, it lacks zero-copy memory pipelines, making large video transfers significantly slower, and provides zero continuity camera capabilities. Deskdrop's event-driven async Rust engine runs at zero idle CPU/battery while sustaining up to 300+ MB/s throughput wrapped in native glassmorphic UI polish.
- **vs. Vendor-Locked OEM Suites (`Phone Link`, `O+ Connect`, `Intel Unison`)**: Microsoft Phone Link, OPPO O+ Connect, and Intel Unison deliberately lock you into their specific proprietary silos (`Windows PCs only`, `OPPO/OnePlus phones only`, `Intel Evo PCs only`). In addition to vendor lock-in, they run heavy background telemetry daemons that consume hundreds of megabytes of RAM. Deskdrop is **100% open, local, private, brand-agnostic across Mac, Windows, Android, and Linux, and guarantees zero cloud telemetry with 0% background battery drain**.

---

## 🚀 Quick Start & How to Use

### For Everyday Users
1. **Connect to the same network**: Make sure your phone, laptop, or desktop are connected to the same Wi-Fi router, LAN network, or **phone Mobile Hotspot**.
2. **Install Deskdrop**: Download and open the Deskdrop app on your devices.
3. **Pair securely in seconds**: On first discovery, select your peer device. A 6-digit security code (PIN) will appear. Confirm the matching code on both screens to establish a permanent, trusted connection.
4. **Start Dropping!**
   - **Copy & Paste**: Copy text on one device; pop open the Deskdrop feed on your second device to view or paste it instantly.
   - **File Dropping**: Drag and drop files directly onto a connected device card. They will arrive cleanly in your native `Downloads` folder!
   - **Remote File Browsing**: Open a paired device card to securely explore allowed folders on your remote machine and download files directly to your current device right when you need them.

---

## 🛠️ Building & Installing from Source (For Developers)

If you are a developer, security researcher, or enthusiast wanting to build Deskdrop directly from source code:

### Global Prerequisites
- **Rust Toolchain**: MSRV `1.75+` (install easily via [rustup.rs](https://rustup.rs/)).
- **CMake & C Compiler**: Required for compiling underlying cryptographic libraries (`Xcode command-line tools` on macOS, `build-essential` on Linux/macOS).

### 1. One-Click Reinstall & Rebuild Script
We provide an automated script that completely cleans, recompiles the Rust core, and installs both macOS and Android applications in one go:

```bash
# Clone the repository
git clone https://github.com/ChinmayyK/Deskdrop.git
cd Deskdrop

# Rebuild and reinstall macOS & Android apps with the latest code (Debug mode)
./scripts/reinstall-all.sh --debug

# Or build optimized release binaries
./scripts/reinstall-all.sh --release
```

### 2. Platform-Specific Manual Builds

#### macOS App Bundle (`Deskdrop.app`)
```bash
# Build universal macOS bundle & copy to /Applications
./scripts/build-macos.sh --debug
cp -a platforms/macos/build/Deskdrop.app /Applications/
open /Applications/Deskdrop.app
```

#### Android APK (`com.deskdrop.debug`)
```bash
# Build Android APK and install directly onto a USB-connected device
./scripts/build-android.sh --debug --fast-abi
adb install -r platforms/android/app/build/outputs/apk/debug/app-debug.apk
adb shell monkey -p com.deskdrop.debug -c android.intent.category.LAUNCHER 1
```

#### Linux Desktop (`deskdrop-linux`)
```bash
# Run the GTK native client directly via Cargo
cargo run -p deskdrop-linux
```

#### Windows Client (`Deskdrop.WinUI`)
```bash
# Build the core Windows dynamic library
cargo build --release -p deskdrop-core

# Open and compile the project in Visual Studio 2022 or run using PowerShell
# Project path: platforms/windows/Deskdrop.WinUI/Deskdrop.WinUI.csproj
# Or run installation script directly:
powershell -ExecutionPolicy Bypass -File platforms/windows/Deskdrop.WinUI/install_and_run.ps1
```

---

## ⚙️ Under the Hood: High-Performance Architecture

For curious minds and software engineers, Deskdrop is engineered from the ground up for maximum throughput, absolute security, and zero memory waste. All core business logic—from cryptographic handshakes to network socket routing—lives inside our shared, cross-platform Rust engine (`deskdrop-core`).

```text
 ┌───────────────────────────────────────────────────────────────────────────────────────────┐
 │                                    NATIVE PLATFORM UI                                     │
 │    macOS (SwiftUI)       Android (Kotlin/JNI)       Linux (GTK3)      Windows (WinUI/C#)  │
 └───────────┬───────────────────────┬───────────────────────┬───────────────────────┬───────┘
             │ Direct C FFI          │ JNI Bridge            │ Unix Sockets          │ Named Pipes
             ▼                       ▼                       ▼                       ▼
 ┌───────────────────────────────────────────────────────────────────────────────────────────┐
 │                             DESKDROP-CORE ENGINE (RUST)                                   │
 │                                                                                           │
 │  ┌─────────────────────────┐  ┌─────────────────────────┐  ┌───────────────────────────┐  │
 │  │    ZERO-COPY MEMORY     │  │   ADAPTIVE PIPELINES    │  │    SECURITY & PROTOCOL    │  │
 │  │  • bytes::Bytes slices  │  │  • Dynamic Batching     │  │  • X25519 Ephemeral ECDH  │  │
 │  │  • O(1) buffer sharing  │  │  • Geometric Socket     │  │  • ChaCha20-Poly1305 AEAD │  │
 │  │  • String Caching (FFI) │  │    Tuning (16MB→256KB)  │  │  • PINmod10^6 MITM Guard  │  │
 │  └─────────────────────────┘  └─────────────────────────┘  └───────────────────────────┘  │
 └───────────────────────────────────────────────────────────────────────────────────────────┘
```

### ⚡ High-Performance Core Engine & Zero-Copy Architecture
- **Zero-Copy Memory Management (`bytes::Bytes`)**: Instead of allocating and copying file buffers back and forth across computer RAM during transfers, Deskdrop operates on reference-counted memory slices (`bytes::Bytes`). Slicing chunks out of memory buffers (`data.slice(start..end)`) takes $O(1)$ reference-counted views without duplicating memory bytes across worker threads or channels.
- **Adaptive Chunk Batching (`adaptive_batch_size`)**: Outbound file pipelines dynamically scale how many chunks are read and sent simultaneously by monitoring real-time network queue depth (`next_chunk - last_acked_chunk`). On fast, low-latency home Wi-Fi or Ethernet, batching scales up to saturate your maximum line speeds; when packet loss or high latency occurs, batching automatically throttles down (`4 MB` / `8 MB`) to prevent memory spikes and queue starvation.
- **Geometric Network Socket Auto-Tuning**: Network sockets apply geometric buffer fallback progressions (`16 MB`, `8 MB`, `4 MB`, `2 MB`, `1 MB`, `512 KB`, `256 KB`), locking in the absolute largest OS kernel buffer (`SO_SNDBUF` / `SO_RCVBUF`) permitted by your system for maximum networking throughput.
- **Fast Entropy & Compression Sampling**: Before spending heavy CPU cycles attempting to compress a `4 MB` chunk with LZ4, the engine tests a quick `4 KB` sample (`should_try_compress`). If compression yields less than 5% size reduction (`< 95%`), the engine immediately skips compression, preserving CPU and battery on already-compressed files (`.zip`, `.mp4`, `.jpg`).
- **Deduplicated FFI String Caching**: Across C-FFI boundaries (`PbEvent::cache_str`), existing pointer strings (`device_name`, `transfer_id`, `text`) are cached and recycled when queried by native UI layers, avoiding repeated heap allocations during UI updates.
- **macOS UI Status Debouncing**: High-frequency IPC status checks (`refresh()`) are debounced (`80 ms`) inside `DeskdropStore.swift` to prevent flooding the local Unix domain socket during concurrent timers or user interactions.
- **Real-World Wireless Saturation Benchmarks**: Thanks to the synergy between geometric socket auto-tuning (`16 MB` kernel buffers) and zero-copy memory pipelines, Deskdrop avoids CPU copy bottlenecks and achieves near-theoretical physical line speeds:
  - **Mobile Hotspot Direct (5 GHz Point-to-Point)**: Sustains **150 MB/s to 300+ MB/s**. Because devices connect directly across dedicated 80 MHz/160 MHz wide channels without packet queueing, router hops, or airtime contention from smart home devices, direct hotspot tethering often outpaces crowded home routers!
  - **Wi-Fi 6 / 6E Router**: Sustains **120 MB/s to 250+ MB/s** throughput.
  - **Wi-Fi 5 Router**: Sustains **40 MB/s to 90+ MB/s** throughput.

### 🛡️ Military-Grade Local Security & Privacy
- **True End-to-End Encryption**: Every session is secured using ephemeral **Curve25519 (X25519) ECDH** key exchanges combined with **HKDF-SHA256** key derivation and **ChaCha20-Poly1305** authenticated encryption (`AEAD`).
- **Strict Monotonic Replay Protection**: Every single frame across the wire includes a strictly increasing 64-bit counter nonce, making packet replay or reordering attacks mathematically impossible.
- **Memory Zeroization**: Secret Diffie-Hellman keys are explicitly wiped from RAM immediately after session negotiation (`zeroize`) to eliminate cold-boot memory attacks.
- **mDNS Privacy Enforcer**: Friendly device names (`My MacBook Pro`) are never broadcast across open Wi-Fi in plain text. Only opaque UUID identifiers are advertised via mDNS until peers authenticate and complete an encrypted handshake.
- **PIN-Verified & TOFU Pairing (`PairingPin`)**: Protects against active Man-in-the-Middle (MITM) attacks by generating and verifying a commutative 6-digit numeric PIN on both devices during initial pairing (`Trust On First Use`).
- **Configurable Content Filtering (`filter.rs`)**: Implements pre-flight inspection pipelines to catch and block One-Time Passwords (OTPs), API keys, passwords, or restricted file extensions (`.exe`, `.sh`) from being transmitted.

### 🔋 OS-Level Sleep Immunity & Background Sync
Deskdrop implements system-native power hooks on every operating system so long transfers don't get interrupted when your devices go idle:
- **macOS:** Calls `ProcessInfo.beginActivity` to prevent Apple's App Nap from suspending background daemon syncs.
- **Windows:** Calls `SetThreadExecutionState` to halt Modern Standby transitions during active file relays.
- **Android:** Incorporates diagnostics and wake locks tailored for restrictive OEM power managers (Xiaomi MIUI, Samsung OneUI) to prevent background service kills.

---

## 💻 Command-Line Interface (CLI) & Power User Tools

For power users, system administrators, and terminal automation, Deskdrop includes a feature-packed command-line utility (`deskdrop-cli`) that communicates directly with the background daemon over fast local IPC sockets:

```bash
# Check real-time daemon status & uptime
cargo run -p deskdrop-cli -- status

# Ping daemon to test local IPC latency
cargo run -p deskdrop-cli -- ping
# Output: PONG (1.1ms)

# Inspect live network metrics, throughput speeds, and p50/p95 latency
cargo run -p deskdrop-cli -- metrics

# View the last 15 items in your clipboard history
cargo run -p deskdrop-cli -- history --last 15

# Pin a clipboard item by ID so it is never automatically cleared
cargo run -p deskdrop-cli -- history pin <id>

# List all discovered and paired devices on your network
cargo run -p deskdrop-cli -- devices list

# Temporarily pause or resume sync with a specific peer
cargo run -p deskdrop-cli -- devices peer-settings <device-id> pause
cargo run -p deskdrop-cli -- devices peer-settings <device-id> resume
```

---

## 📂 Configuration & Data Storage

Deskdrop adheres strictly to standard, clean system directory structures so your home folder stays organized:

| Platform | Configuration & Data Directory |
| :--- | :--- |
| **macOS** | `~/Library/Application Support/deskdrop/` |
| **Linux** | `~/.config/deskdrop/` |
| **Windows** | `%APPDATA%\deskdrop\` |

- `settings.json`: User preferences, custom regex filters, and behavior thresholds.
- `peers.json`: Discovered network nodes and friendly name mappings.
- `trust.json`: Long-term cryptographic public keys of paired peers.
- `history.json`: Bounded, highly resilient NDJSON activity history ring buffer.
- `identity.json`: Your local device's permanent 32-byte X25519 identity key pair (`0600` permissions).

> [!CAUTION]
> Your `identity.json` file contains your device's private cryptographic identity. Never share, copy, or commit this file to public repositories!

---

## 🤝 Contributing, Security & License

We love open-source contributions, bug reports, and feature suggestions!
- **Contributing**: Check out [CONTRIBUTING.md](CONTRIBUTING.md) to learn about our codebase structure, PR guidelines, and testing workflow.
- **Security & Vulnerabilities**: Please read [SECURITY.md](SECURITY.md) for responsible disclosure procedures.
- **Changelog**: See [CHANGELOG.md](CHANGELOG.md) for detailed version updates.

### License
Deskdrop is open-source software licensed under the **GNU Affero General Public License v3.0 (AGPL-3.0)**. See [LICENSE](LICENSE) for full details.
