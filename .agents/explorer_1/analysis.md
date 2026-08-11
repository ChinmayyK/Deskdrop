# Deskdrop Architecture & Remote Files Protocol Analysis Report

**Author**: Explorer 1 (Topology & Remote Files Protocol Explorer)  
**Date**: 2026-08-07  
**Workspace**: `/Users/chinmayk/Projects/Deskdrop`

---

## 1. Executive Summary & System Topology

Deskdrop is a local-first, zero-cloud desktop and mobile continuity engine for resumable file transfers, clipboard synchronization, continuous media streaming, and remote file browsing across mixed operating systems (macOS, Android, Windows, Linux).

### System Monorepo Breakdown
- **Core Engine (`deskdrop-core`)**: Asynchronous event-driven daemon written in Rust using `tokio` (multi-threaded async runtime). Compiled as a dynamic C library (`libdeskdrop_core.dylib` / `.so` / `.dll`), a static JNI target (`libdeskdrop_core.so` via `cargo-ndk` for Android), or a standalone binary (`daemon`).
- **macOS Client (`platforms/macos`)**: Native Swift 5 & SwiftUI application. Interfaces with `deskdrop-core` daemon via UNIX Domain Sockets (`DeskdropIPCClient.swift`).
- **Android Client (`platforms/android`)**: Native Kotlin & Jetpack Compose Android app (`:app`). Embeds `libdeskdrop_core.so` via JNI (`DeskdropJni.kt`). Uses background foreground service (`DeskdropService.kt`) and Android `MediaStore` ContentResolver (`RemoteFileManager.kt`).
- **Windows Client (`platforms/windows`)**: C# .NET 8 / WPF / WinUI 3 hybrid application (`Deskdrop.WinUI`). Interfaces via Windows Named Pipes and P/Invoke C-FFI (`NativeCore.cs`).
- **CLI Utility (`deskdrop-cli`)**: Rust CLI tool communicating directly with the daemon over local IPC domain sockets.

---

## 2. Network Protocol & Transport Layer Architecture

Deskdrop implements a custom length-prefixed, encrypted binary protocol operating over TCP/IP over local Wi-Fi / LAN, Wi-Fi Direct, or Mobile Hotspot connections.

### 2.1 Network & Socket Parameters
- **Default Port**: `47823` (`DEFAULT_PORT` in `protocol.rs`).
- **Protocol Version**: `4` (`PROTOCOL_VERSION` in `protocol.rs`).
- **Discovery**: Multi-layered discovery combining mDNS (`_deskdrop._tcp.local.`), UDP unicast/multicast beacons, and Android/iPhone hotspot gateway probing (`hotspot_probe.rs`).

### 2.2 Wire Framing
Every frame sent over a peer TCP socket follows a length-prefixed binary structure:
```text
┌───────────────────────────┬───────────────────────────────────────────┐
│ Length Prefix (4 bytes)   │ Payload (N bytes)                         │
│ u32 Little-Endian         │ Postcard-encoded plaintext (Handshake) OR │
│                           │ ChaCha20-Poly1305 AEAD Ciphertext (Data)  │
└───────────────────────────┴───────────────────────────────────────────┘
```
- **Plaintext Limit**: 8 KB during initial handshake (`EcdhFrame`).
- **Encrypted Frame Limit**: 40 MB (`MAX_FRAME_SIZE = 40 * 1024 * 1024` in `network.rs`) to accommodate up to 32 MB un-chunked image payloads.

### 2.3 Cryptography & Handshake Flow
1. **ECDH Key Exchange**: Initiator sends plaintext `EcdhFrame` containing ephemeral X25519 public key (32 bytes) and random nonce (16 bytes). Responder replies with its `EcdhFrame`.
2. **Session Key Derivation**: Both sides execute X25519 Diffie-Hellman and derive a 256-bit session key (`SessionKey` in `crypto.rs`) using HKDF-SHA256.
3. **Encrypted Identity Handshake**:
   - Initiator sends encrypted `AppMessage::Hello` containing `device_id` (UUID), `device_name`, `identity_pubkey`, and an Ed25519 identity proof.
   - Responder validates proof, checks trust store, and replies with encrypted `AppMessage::HelloAck`.
4. **AEAD Frame Protection**: All subsequent frames use ChaCha20-Poly1305 authenticated encryption. Each message includes an auto-incrementing 64-bit nonce counter for replay protection. Raw binary payloads (file transfer chunks, media thumbnails) append raw encrypted payload blocks (`expects_raw_payload()` in `protocol.rs`).

### 2.4 Connection Management & Socket Auto-Tuning
- **Session Registry**: `peer_manager.rs` manages `PeerRecord` instances and active peer senders (`mpsc::Sender<AppMessage>`). Each connected peer has a single persistent encrypted TCP socket.
- **TCP Options**: Sockets apply `set_nodelay(true)` (`TCP_NODELAY`) and geometric buffer tuning (`SO_SNDBUF` / `SO_RCVBUF` scaled from 16 MB down to 256 KB fallback).
- **TCP Keepalive**: `KEEPALIVE_IDLE` = 10s, `KEEPALIVE_INTERVAL` = 3s, `KEEPALIVE_RETRIES` = 3.
- **Outbound Connect Timeout**: 2 seconds (`CONNECT_TIMEOUT` in `network.rs`).

---

## 3. Remote File Browsing RPC & Data Models

Remote file browsing enables connected devices (e.g. macOS desktop or CLI) to navigate, query, filter, thumbnail, and pull files stored on remote paired devices (e.g. Android mobile).

### 3.1 Data Models (`protocol.rs`)
- `RemoteFileCategory`: `All`, `Images`, `Videos`, `Audio`, `Documents`, `Apks`, `Archives`, `Other`.
- `RemoteFileSource`: `All`, `WhatsApp`, `Downloads`, `Camera`, `Other`.
- `RemoteFileEntry`:
  ```rust
  pub struct RemoteFileEntry {
      pub file_id: u64,         // Android MediaStore _ID or FS inode
      pub display_name: String, // Filename
      pub size_bytes: u64,      // Size in bytes
      pub mime_type: String,    // MIME string
      pub date_modified: u64,   // Epoch timestamp (seconds)
      pub category: RemoteFileCategory,
      pub source: RemoteFileSource,
      pub content_uri: String, // Content URI (e.g. "content://media/external/file/1234")
  }
  ```
- `RemoteFilesSummary`: Summary object containing `type_counts` (`RemoteFileCategoryCounts`) and `source_counts` (`RemoteFileSourceCounts`).

### 3.2 Protocol Wire Messages (`AppMessage` enum in `protocol.rs`)
1. `AppMessage::RemoteFilesQuery`:
   - `request_id`: `Uuid` (unique correlation ID for the request)
   - `origin_device`: `Uuid`
   - `summary_only`: `bool` (if true, return summary counts without full file array)
   - `category`: `Option<RemoteFileCategory>`
   - `source`: `Option<RemoteFileSource>`
   - `search_query`: `Option<String>`
   - `offset`: `u32` (pagination start offset)
   - `limit`: `u32` (pagination page size)
2. `AppMessage::RemoteFilesResponse`:
   - `request_id`: `Uuid`
   - `summary`: `Option<RemoteFilesSummary>`
   - `files`: `Vec<RemoteFileEntry>`
   - `total_matching`: `u32`
   - `error`: `Option<String>`
3. `AppMessage::RemoteThumbnailRequest`:
   - `request_id`: `Uuid`
   - `origin_device`: `Uuid`
   - `file_id`: `u64`
   - `size_px`: `u32`
4. `AppMessage::RemoteThumbnailResponse`:
   - `request_id`: `Uuid`
   - `file_id`: `u64`
   - `data`: `Vec<u8>` (JPEG thumbnail bytes)
   - `error`: `Option<String>`
5. `AppMessage::RemoteFilePullRequest`:
   - `request_id`: `Uuid`
   - `origin_device`: `Uuid`
   - `file_id`: `u64`
6. `AppMessage::RemoteFileActionRequest`:
   - `action`: `String` ("delete", "rename")
   - `file_id`: `u64`
   - `new_name`: `Option<String>`

---

## 4. End-to-End Remote File Query Execution Flow

```text
[ macOS / Desktop UI ]
         │ (Swift/JSON IPC over UNIX Socket)
         ▼
[ deskdrop-core IPC Server (`ipc.rs`) ]
         │ calls eng.query_remote_files_sync(..., timeout_secs = 12)
         ▼
[ Rust Engine (`engine/mod.rs`) ]
         │ 1. Generates request_id (Uuid)
         │ 2. Registers oneshot channel in remote_file_waiters HashMap
         │ 3. Sends AppMessage::RemoteFilesQuery over peer TCP channel
         │ 4. Awaits tokio::time::timeout(12s, rx)
         ▼
[ Encrypted TCP Socket Transport ]
         │ length-prefixed + ChaCha20-Poly1305 AEAD
         ▼
[ Remote Peer Receiver (`deskdrop-core` socket reader) ]
         │ Decrypts AppMessage::RemoteFilesQuery
         │ Emits EngineEvent::RemoteFilesQueryReceived via FFI / JNI
         ▼
[ Android Service (`DeskdropService.kt` / `RemoteFileManager.kt`) ]
         │ Runs executeInBackgroundWithWakeLock("RemoteFilesQuery")
         │ Calls RemoteFileManager.queryFiles(...) -> ContentResolver query MediaStore
         │ Calls DeskdropJni.sendRemoteFilesResponse(...)
         ▼
[ Responder `deskdrop-core` Engine ]
         │ Sends AppMessage::RemoteFilesResponse back over TCP socket
         ▼
[ Encrypted TCP Socket Transport ]
         ▼
[ Initiator `deskdrop-core` Socket Reader (`engine/mod.rs`) ]
         │ Receives AppMessage::RemoteFilesResponse
         │ Resolves oneshot tx from remote_file_waiters HashMap
         ▼
[ query_remote_files_sync in `engine/mod.rs` ]
         │ Returns RemoteFilesResult
         ▼
[ IPC Server (`ipc.rs`) ] -> [ Desktop UI ]
```

---

## 5. Socket Timeouts & Timeout Bottleneck Mapping

| Component / Layer | Location | Configured Timeout | Description / Function |
|---|---|---|---|
| **RPC Query Timeout** | `deskdrop-core/src/ipc.rs:1404` | **12 seconds** | Hardcoded timeout passed to `query_remote_files_sync(...)` for remote file queries. |
| **RPC Thumbnail Timeout** | `deskdrop-core/src/ipc.rs:1422` | **10 seconds** | Hardcoded timeout passed to `request_remote_thumbnail_sync(...)` for remote thumbnail queries. |
| **Engine Waiter Timeout** | `deskdrop-core/src/engine/mod.rs:2168` | `timeout_secs` (12s) | `tokio::time::timeout` awaiting oneshot channel `rx` from `remote_file_waiters`. |
| **Socket Frame Read** | `deskdrop-core/src/network.rs:307, 320` | **30 seconds** | `tokio::time::timeout` waiting for frame length and frame body reads in `recv_encrypted`. |
| **TCP Outbound Connect** | `deskdrop-core/src/network.rs:56` | **2 seconds** | `CONNECT_TIMEOUT` when connecting to peer IP. |
| **TCP Keepalive Idle** | `deskdrop-core/src/network.rs:60` | **10 seconds** | `KEEPALIVE_IDLE` before sending TCP keepalive probes. |
| **Android Query Execution** | `RemoteFileManager.kt:64` | Unbounded | `ContentResolver.query` scans full `MediaStore.Files` database on device. |

### Identified Root-Cause Bottleneck Mechanism
1. On Android, `RemoteFileManager.queryFiles(...)` performs an un-indexed full linear scan over all rows in `MediaStore.Files.getContentUri("external")` whenever `includeSummary = true` (which is true on category queries).
2. On devices with large media libraries (thousands of files/photos), iterating over every MediaStore record, resolving categories/sources via path heuristics, and constructing JSON strings takes longer than 12 seconds.
3. Because the RPC timeout in `ipc.rs` is hardcoded to 12 seconds, `query_remote_files_sync` in Rust core times out first, triggering `anyhow::bail!("Remote files query timed out after 12s")`.
4. The IPC layer converts this error into JSON response, which `DeskdropIPCClient` converts to `serverError("Remote files query timed out after 12s")`, prompting `RemoteExplorerView.swift` to display **"Connection Interrupted - Remote files query timed out after 12s"**.

---

## 6. Comprehensive Module & Source File Inventory

| Module / Path | Language | Subsystem / Role | Key Responsibilities & Functions |
|---|---|---|---|
| `deskdrop-core/src/protocol.rs` | Rust | Wire Protocol | Structs (`RemoteFileEntry`, `RemoteFilesSummary`, `RemoteFileCategory`, `RemoteFileSource`) and `AppMessage` enum (`RemoteFilesQuery`, `RemoteFilesResponse`, `RemoteThumbnailRequest`, `RemoteThumbnailResponse`, etc.). |
| `deskdrop-core/src/network.rs` | Rust | Network Transport | Framing (`send_frame`, `recv_frame`), encryption (`send_encrypted`, `recv_encrypted` with 30s timeout), TCP connection setup (`connect_with_timeout`), keepalive, and socket buffer tuning. |
| `deskdrop-core/src/engine/mod.rs` | Rust | Core Engine | Engine state (`EngineShared`), `remote_file_waiters` HashMap, `query_remote_files_sync` (12s timeout), `request_remote_thumbnail_sync` (10s timeout), incoming socket message router. |
| `deskdrop-core/src/ipc.rs` | Rust | IPC Server | IPC request router handling `IpcRequest::RemoteFilesQuery` (passes 12s timeout) and `IpcRequest::RemoteThumbnailRequest` (passes 10s timeout). |
| `deskdrop-core/src/ffi.rs` | Rust | C-FFI Bridge | C exports (`deskdrop_send_remote_files_query`, `deskdrop_send_remote_thumbnail_request`, `deskdrop_send_remote_file_pull_request`, event polling C API). |
| `deskdrop-core/src/jni_android.rs` | Rust | Android JNI Bridge | JNI C functions exporting engine methods to Kotlin (`Java_com_deskdrop_DeskdropJni_...`). |
| `deskdrop-core/src/peer_manager.rs` | Rust | Session Manager | Maintains `PeerRecord`, connected socket senders, device lifecycle state. |
| `platforms/android/app/src/main/java/com/deskdrop/RemoteFileManager.kt` | Kotlin | Android MediaStore | Scans Android MediaStore, computes category/source counts, extracts thumbnails (`getThumbnail`), resolves file paths. |
| `platforms/android/app/src/main/java/com/deskdrop/DeskdropService.kt` | Kotlin | Android Service | Foreground service receiving `CR_EVENT_REMOTE_FILES_QUERY` events, running background coroutine, invoking `RemoteFileManager` and sending response back via JNI. |
| `platforms/android/app/src/main/java/com/deskdrop/DeskdropJni.kt` | Kotlin | Android JNI Interface | Kotlin object defining native JNI methods (`sendRemoteFilesResponse`, `sendRemoteThumbnailResponse`, etc.). |
| `platforms/macos/Deskdrop/RemoteExplorerView.swift` | Swift | macOS UI | SwiftUI interface for remote file browser, handles query states, filtering, date grouping, inspector, and "Connection Interrupted" error presentation. |
| `platforms/macos/Deskdrop/DeskdropIPCClient.swift` | Swift | macOS IPC Client | Swift IPC client sending JSON commands (`remote_files_query`, `remote_thumbnail_request`) to Rust daemon over UNIX domain socket. |
| `platforms/macos/Deskdrop/DeskdropStore.swift` | Swift | macOS State Store | Main UI state store coordinating remote files querying, caching, and transfers. |
| `platforms/windows/Deskdrop.WinUI/Views/RemoteExplorerView.xaml.cs` | C# | Windows UI | WinUI 3 XAML code-behind for remote file browsing. |
| `platforms/windows/Deskdrop.WinUI/WindowsIpcClient.cs` | C# | Windows IPC Client | C# client sending IPC requests to Rust daemon over Windows Named Pipes. |
