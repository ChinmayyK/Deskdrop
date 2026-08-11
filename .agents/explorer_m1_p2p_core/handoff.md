# P2P Core Architecture & Payload Transfer Analysis Report

## Summary
This survey maps out the P2P networking, pairing, and payload transfer engine across `deskdrop-core` (Rust) and Android platform bindings (`DeskdropJni.kt` and `DeskdropService.kt`).

---

## 1. Observation

### Code Base Inspection & File Locations
- **Rust Core Library**: `deskdrop-core/src/lib.rs` exports modules: `engine`, `discovery`, `discovery_manager`, `udp_discovery`, `pairing`, `trust`, `protocol`, `file_transfer`, `network`, `network_manager`, `dedup`, `chunked`, `compress`, `crypto`, `identity`, `ipc`, `jni_android`.
- **Android Platform Layer**:
  - `platforms/android/app/src/main/java/com/deskdrop/DeskdropJni.kt`: Kotlin external JNI declarations and event constants (`CR_EVENT_*`).
  - `platforms/android/app/src/main/java/com/deskdrop/DeskdropService.kt`: Android Foreground Service managing engine handle lifecycle (`activeEngineHandle`), Kotlin `NsdManager` mDNS discovery/registration, WakeLocks, and Intent handlers.
  - `platforms/android/app/src/main/java/com/deskdrop/TransferManager.kt`: Shared StateFlow objects (`activeTransfersFlow`, `activeSpeedTestsFlow`) powering Compose UI progress indicators.
  - `deskdrop-core/src/jni_android.rs`: Rust JNI exported C-ABI functions (`Java_com_deskdrop_DeskdropJni_*`).
- **Desktop Control Surface**: `deskdrop-cli/src/main.rs`, `deskdrop-core/src/bin/daemon.rs`, and `deskdrop-core/src/ipc.rs` providing Unix IPC socket communication (`IpcRequest` / `IpcResponse`).

---

## 2. Logic Chain

### 2.1 Native Rust Engine & JNI Bindings
1. **Engine Architecture**:
   - `Engine` in `deskdrop-core/src/engine/mod.rs` runs on a Tokio async runtime (`RT` static `OnceLock` in `jni_android.rs:23–26`).
   - Android engine lifecycle is initiated via `DeskdropJni.start(...)`, which invokes `Java_com_deskdrop_DeskdropJni_start` in `jni_android.rs:71–121`, allocating an `AndroidHandle` boxed raw pointer returned as a `jlong` to Kotlin (`engineHandle`).
   - Cleanup is executed via `DeskdropJni.stop(handle)`, which drops `Box::from_raw(handle as *mut AndroidHandle)`.

2. **JNI Thread Safety & Concurrency**:
   - In `DeskdropService.kt`, JNI calls are synchronized using `engineLock.readLock()` or handle checks to prevent dangling handle usage during service destruction.
   - Rust JNI exports in `jni_android.rs` guard against null/invalid handles (`if handle == 0 { return -1; }`).

---

### 2.2 P2P Discovery Subsystem
The engine employs a multi-tiered discovery subsystem unified by `DiscoveryManager` (`deskdrop-core/src/discovery_manager.rs`):

1. **mDNS-SD (DNS Service Discovery — Layer 1)**:
   - **Desktop Nodes** (`deskdrop-core/src/discovery.rs:25–260`): Uses `mdns_sd::ServiceDaemon` to advertise service type `_deskdrop._tcp.local.` on port `47823`. TXT records publish `id` (opaque UUID) and `v` (protocol version, currently `3`/`4`). Device friendly names are intentionally excluded from TXT records for privacy (TRU-06) and exchanged post-handshake in `Hello`/`HelloAck`.
   - **Android Nodes** (`platforms/android/app/src/main/java/com/deskdrop/DeskdropService.kt:2721–2918`): Rust `discovery.rs` is stubbed out on Android (`target_os = "android"`). Android uses Kotlin `NsdManager` (`startNsdDiscovery()`) to register service name `deskdrop-<uuid8>-<safeName>` and browse `_deskdrop._tcp.`. Discovered peers are resolved and fed to Rust via `DeskdropJni.reportDiscoveredPeer(handle, peerDeviceId, fallbackName, ip, port)`.

2. **UDP Broadcast (Layer 2)** (`deskdrop-core/src/udp_discovery.rs:263–395`):
   - Transmits to `255.255.255.255:47824` and interface subnet-directed broadcast addresses.
   - **AirDrop Startup Burst**: Sends 3 rapid beacons in 100ms intervals upon launch, then transitions to 15s intervals (`DEFAULT_BEACON_INTERVAL`).
   - **Payload Format**: `DESKDROP3:<uuid>:<port>:<fp_hex8>:<version>` (e.g. `DESKDROP3:550e8400-e29b-41d4-a716-446655440000:47823:a1b2c3d4e5f6a7b8:4`).

3. **UDP Multicast (Layer 3)** (`deskdrop-core/src/udp_discovery.rs:397–570`):
   - Transmits to `239.255.77.77:47825` with `TTL=1` (link-local).
   - On Android, execution requires acquiring `WifiManager.MulticastLock`.

4. **DiscoveryManager Aggregation** (`deskdrop-core/src/discovery_manager.rs`):
   - Merges findings from all layers, deduplicating by `device_id` (not IP).
   - Enforces per-source staleness timeouts: mDNS (30s), UDP Beacon (10s), Hotspot Probe (6s).

---

### 2.3 Pairing & Trust Model
- **Handshake & Encryption**: Ephemeral X25519 ECDH + HKDF key exchange yields AES-256-GCM encrypted session frames (`deskdrop-core/src/protocol.rs:276–300`).
- **Identity & Fingerprints**: Devices persist Ed25519/X25519 keys in `identity.key`. Peer trust records are saved in `trust.json`.
- **Pairing Modes**:
  1. **TOFU / Auto-Trust**: Pre-trusted peers in `trust.json` bypass prompts.
  2. **6-Digit PIN Code Pairing** (`deskdrop-core/src/pairing.rs:34–200`):
     - PIN is derived via `HKDF-SHA256(shared_secret, info="deskdrop-pin") % 1_000_000` and formatted as `"XXX YYY"` (e.g., `"482 917"`).
     - 30-second expiry (`PAIRING_TIMEOUT`).
     - Bounded queue (max 5 pending prompts) to prevent UI DoS attacks.
     - Flow: `PairingRequest` wire message -> `EngineEvent::PairingRequested` -> User approves -> Kotlin/CLI calls `trustPeer` / `respondToPairing` -> `PairingResponse(accepted=true)` -> peer stored in `trust.json`.
  3. **QR Code Pairing** (`deskdrop-core/src/engine/mod.rs:509–512, 1964–1975, 5240–5280`):
     - Host calls `generate_qr_token()` (produces short-lived token).
     - Client scans QR code, calls `trustPeerFromQr(handle, deviceId, token)`.
     - Client sends `AppMessage::QrAuth { token }`. Host verifies token and establishes mutual trust.

---

### 2.4 Payload Handling Breakdown

#### A. Text Exchange (Clipboard Sync & Messages)
- **Wire Message**: `AppMessage::ClipboardPush` with `ClipboardContent::Text(String)`.
- **Limits**: `MAX_TEXT_BYTES` = 4 MB (`protocol.rs:10`).
- **Android Flow**:
  - Outgoing: `DeskdropJni.pushText(engineHandle, text)` -> Rust `push_clipboard(ClipboardContent::Text)`.
  - Incoming: If `auto_applied=true`, service writes directly to Android `ClipboardManager`. If timeline-first mode is active, it emits `CR_EVENT_CLIPBOARD_AVAILABLE` and adds entry to `ActivityFeed` for manual application via `applyClipboardByHash`.
- **Desktop Flow**: `deskdrop-cli push <text>` or `deskdrop-cli send <target> <text>` via IPC `PushText` / `PushTextTo`.

#### B. File Exchange (Binary Data Streams & Transfers)
- **Pipeline**: Dedicated chunked transfer manager (`FileTransferManager` in `file_transfer.rs`).
- **Limits**: Up to 1 TB (`MAX_FILE_BYTES` = 1 TB).
- **Wire Messages**:
  1. `AppMessage::FileTransferAnnounce { meta: FileTransferMetadata }`
  2. Prompt emitted: `EngineEvent::FileTransferIncoming`
  3. Receiver response: `AppMessage::FileTransferAccept { transfer_id, accepted: true, resume_from_chunk }`
  4. Stream chunks: `AppMessage::FileChunk { transfer_id, chunk_index, total_chunks, data, compressed }` (256KB or 1MB chunk size; optional zstd compression).
  5. Flow control: `AppMessage::FileChunkAck { transfer_id, last_confirmed_chunk }`.
  6. Finalization: `AppMessage::FileTransferComplete { transfer_id, sha256_checksum }` + `FileTransferCompleteAck`.
- **Save Location**: Default save directory (`Downloads/Deskdrop` or custom `file_save_dir`).

#### C. Image Exchange (Media & Gallery Integration)
- **Wire Message**: `AppMessage::ClipboardPush` with `ClipboardContent::Image { mime, data }`.
- **Limits**: `MAX_IMAGE_BYTES` = 32 MB (`protocol.rs:11`).
- **Android Flow**:
  - Outgoing: `DeskdropJni.pushImage(handle, mimeType, byteArray)`.
  - Incoming: Emits `CR_EVENT_CLIPBOARD_IMAGE`. Saved to cache/media storage, registered in MediaStore or copied as content URI to clipboard where supported.

---

## 3. Caveats
- Android mDNS discovery relies on system `NsdManager`. On certain OEM Android devices with aggressive power management or multicast filtering, fallback to UDP Broadcast/Multicast requires active Wi-Fi connection and `MulticastLock`.
- IPv6 link-local addresses (`fe80::/10`) are filtered out during NSD resolution on Android because link-local scopes require interface index suffixes not directly provided by standard socket resolution without scope IDs. IPv4 is prioritized (`prefer_ipv4`).

---

## 4. Conclusion
The P2P core in `deskdrop-core` and platform bindings in `platforms/android` form a complete end-to-end transport layer supporting automatic discovery (mDNS + UDP broadcast/multicast), secure pairing (PIN, QR, auto-accept), and payload transfers for text, images, and chunked files.

---

## 5. Verification Method

### 5.1 Verification Commands Overview

#### 1. Android Build & Deployment
```bash
cd /Users/chinmayk/Projects/Deskdrop/platforms/android
./gradlew installDebug
```

#### 2. Desktop Core Build & Tests
```bash
cd /Users/chinmayk/Projects/Deskdrop
cargo test --workspace
cargo build --bin daemon --bin deskdrop-cli
```

---

### 5.2 Programmatic & Interactive P2P Test Scenarios

#### Scenario A: Desktop -> Android Text Transfer
1. **Start Desktop Daemon**:
   ```bash
   cargo run --bin daemon
   ```
2. **Start Android App & Foreground Service**:
   ```bash
   adb shell am startservice -n com.deskdrop.debug/com.deskdrop.DeskdropService -a com.deskdrop.START
   ```
3. **Connect Desktop to Android Node**:
   ```bash
   cargo run --bin deskdrop-cli -- connect <ANDROID_IP> 47823
   ```
4. **Verify Connection**:
   ```bash
   cargo run --bin deskdrop-cli -- peers
   ```
5. **Send Text Payload from Desktop**:
   ```bash
   cargo run --bin deskdrop-cli -- push "Test P2P Message from Desktop"
   ```
6. **Verify Reception on Android**:
   ```bash
   adb logcat -d -s Deskdrop | grep "ClipboardReceived"
   ```

#### Scenario B: Android -> Desktop Text Transfer
1. **Trigger Text Push via ADB Service Intent**:
   ```bash
   adb shell am startservice -n com.deskdrop.debug/com.deskdrop.DeskdropService -a com.deskdrop.PUSH_TEXT --es text "Test P2P Message from Android"
   ```
2. **Verify Reception on Desktop**:
   ```bash
   cargo run --bin deskdrop-cli -- history --last 5
   ```

#### Scenario C: File Transfer Verification (Desktop -> Android)
1. **Send File via IPC / CLI**:
   ```bash
   cargo run --bin deskdrop-cli -- send-file --path /path/to/testfile.pdf --target <ANDROID_DEVICE_ID>
   ```
   *(Or trigger via IPC `SendFilePath` socket request)*
2. **Accept Incoming File on Android**:
   ```bash
   adb shell am startservice -n com.deskdrop.debug/com.deskdrop.DeskdropService -a com.deskdrop.ACCEPT_FILE_TRANSFER --es transfer_id <TRANSFER_ID_HEX>
   ```
3. **Verify File Saved on Android**:
   ```bash
   adb shell ls -la /sdcard/Download/Deskdrop/
   ```

#### Scenario D: Image Exchange Verification
1. **Push Image from Android**:
   - Use UI "Share" sheet to share an image file to Deskdrop app, or invoke `ACTION_PUSH_SHARED_URI` with an image URI:
   ```bash
   adb shell am startservice -n com.deskdrop.debug/com.deskdrop.DeskdropService -a com.deskdrop.PUSH_SHARED_URI --es shared_uri "content://media/external/images/media/1000000001"
   ```
2. **Verify Desktop Activity Feed**:
   ```bash
   cargo run --bin deskdrop-cli -- events --last 5
   ```

---

### Invalidation Conditions
- Any panic or null pointer dereference in JNI layer (`jni_android.rs`).
- Failure of `cargo test` in `deskdrop-core`.
- Failure of Android `NsdManager` or UDP beacon to discover collocated nodes within 15 seconds.
