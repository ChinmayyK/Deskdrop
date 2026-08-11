# Deskdrop Remote File Query E2E Test Suite — Technical Specifications & Analysis Report

**Author**: `e2e_explorer_1`  
**Date**: 2026-08-07  
**Target Repository**: `/Users/chinmayk/Projects/Deskdrop`  
**Focus Crate & Path**: `deskdrop-core/` and `scripts/`

---

## Executive Summary

This report provides comprehensive technical specifications for building an automated End-to-End (E2E) test suite for Deskdrop's **Remote File Query** system (Phase 3 Remote Media Explorer). It covers the full lifecycle of a remote file query—from client IPC initiation, wire protocol encoding and encryption, engine waiter resolution, event-driven request processing on host/peer platforms (macOS, Android MediaStore, Windows), down to mock peer orchestration and test harness design.

---

## 1. Codebase Structure Overview

The core networking, IPC, wire protocol, and engine event processing reside in the Rust workspace crate `deskdrop-core`.

```
deskdrop-core/
├── src/
│   ├── protocol.rs        # AppMessage wire protocol, RemoteFileCategory, RemoteFileEntry, etc.
│   ├── ipc.rs             # Local IPC request/response enums, Unix/Windows socket server & client.
│   ├── engine/
│   │   └── mod.rs         # Engine core, async waiter maps, query_remote_files_sync event loop.
│   ├── ffi.rs             # C FFI exports for macOS Swift / Windows C# native callers.
│   ├── jni_android.rs     # JNI exports for Android Kotlin service integration.
│   └── bin/
│       ├── daemon.rs      # Desktop daemon binary running IPC server & event handling loop.
│       └── scratch.rs / test_net.rs
├── tests/
│   ├── integration_test.rs# Real-TCP localhost two-engine integration test patterns.
│   └── e2e_test.rs        # SimNetwork in-process channel test patterns.
deskdrop-cli/              # Separate workspace crate CLI binary interfacing with IPC.
scripts/
├── query_ipc.py           # Basic Python Unix domain socket IPC test client.
├── test-windows-ipc.ps1   # PowerShell Windows Named Pipe test script.
└── build-*.sh             # Build scripts for macOS and Android.
```

### Key Source Files Summary
- **`deskdrop-core/src/ipc.rs`**: Defines `IpcRequest::RemoteFilesQuery` and `IpcResponse`. Handles Unix domain sockets (`/tmp/deskdrop-<uid>/deskdrop.sock` or `$XDG_RUNTIME_DIR/deskdrop.sock`) and Windows Named Pipes (`\\.\pipe\deskdrop_<username>`).
- **`deskdrop-core/src/protocol.rs`**: Defines wire enum `AppMessage::RemoteFilesQuery` and `AppMessage::RemoteFilesResponse`, along with `RemoteFileCategory`, `RemoteFileSource`, `RemoteFileEntry`, `RemoteFilesSummary`.
- **`deskdrop-core/src/engine/mod.rs`**: Manages `remote_file_waiters` (`HashMap<Uuid, oneshot::Sender<RemoteFilesResult>>`), exposes `query_remote_files_sync()`, dispatches `EngineEvent::RemoteFilesQueryReceived` and `EngineEvent::RemoteFilesResponseReceived`.
- **`deskdrop-core/src/ffi.rs`**: C FFI wrapper exposing `deskdrop_send_remote_files_query` and event accessors (`deskdrop_event_remote_files_json`, `deskdrop_event_remote_summary_json`).
- **`platforms/android/app/src/main/java/com/deskdrop/RemoteFileManager.kt`**: Queries Android `MediaStore.Files`, calculates `type_counts` and `source_counts`, and builds file list metadata.
- **`platforms/macos/Deskdrop/RemoteExplorerView.swift`**: macOS SwiftUI frontend invoking FFI/IPC queries.

---

## 2. Existing Test Infrastructure & Patterns

### 2.1 Real-TCP Engine Integration (`deskdrop-core/tests/integration_test.rs`)
The test `two_engines_exchange_text` in `integration_test.rs` demonstrates how to spawn two full, real `Engine` instances inside a Rust `#[tokio::test]`:
1. Creates `TempDir` for isolated identity keys (`IdentityStore`) and trust stores (`TrustStore`).
2. Generates ephemeral identities and establishes mutual trust via `trust_store.trust(peer_id, peer_name, pubkey)`.
3. Binds engines to `127.0.0.1:0` with `enable_discovery: false`.
4. Establishes TCP connection using `engine1.connect_to_peer("127.0.0.1", bound_port2)`.
5. Receives events via `tokio::sync::mpsc::Receiver<EngineEvent>`.

### 2.2 In-Process SimNetwork (`deskdrop-core/tests/e2e_test.rs`)
Uses `SimNetwork::pair("Alice", "Bob")` to test protocols without allocating OS socket descriptors or network interfaces. Ideal for fast unit testing of message serialization, dedup, and rate limiting.

---

## 3. IPC & Wire Message Specifications

### 3.1 Local IPC Layer (Client/CLI/GUI ↔ Daemon)

#### Socket Paths
- **Unix (macOS/Linux)**: `$XDG_RUNTIME_DIR/deskdrop.sock` or fallback `/tmp/deskdrop-<uid>/deskdrop.sock` (mode `0o700` directory, atomic `0o600` socket creation).
- **Windows**: Named Pipe `\\.\pipe\deskdrop_<username>`.

#### IPC Format
Newline-delimited (`\n`) JSON string over raw stream socket.

#### Request Schema (`IpcRequest::RemoteFilesQuery`)
```json
{
  "cmd": "remote_files_query",
  "target_device": "550e8400-e29b-41d4-a716-446655440000",
  "summary_only": false,
  "category": "Images",
  "source": "Downloads",
  "search_query": "photo",
  "offset": 0,
  "limit": 50
}
```

#### Response Schema (`IpcResponse::Ok`)
```json
{
  "status": "ok",
  "data": {
    "summary": {
      "type_counts": {
        "images": 120,
        "videos": 15,
        "audio": 42,
        "documents": 8,
        "apks": 2,
        "archives": 1
      },
      "source_counts": {
        "whatsapp": 45,
        "downloads": 80,
        "camera": 63
      }
    },
    "files": [
      {
        "file_id": 1001,
        "display_name": "IMG_20260807_120000.jpg",
        "size_bytes": 2458291,
        "mime_type": "image/jpeg",
        "date_modified": 1770450000,
        "category": "Images",
        "source": "Camera",
        "content_uri": "content://media/external/file/1001"
      }
    ],
    "total_matching": 120,
    "error": null
  }
}
```

#### Response Schema (`IpcResponse::Error`)
```json
{
  "status": "error",
  "message": "Remote files query timed out after 12s"
}
```

---

### 3.2 Wire Protocol Layer (`AppMessage` Peer ↔ Peer)

- **Transport**: Encrypted TCP session.
- **Framing**: 4-byte LE length prefix + `postcard` binary serialization + AES-256-GCM AEAD encryption.

#### `AppMessage::RemoteFilesQuery` (Wire Message)
```rust
AppMessage::RemoteFilesQuery {
    request_id: Uuid,        // Random v4 UUID generated by requestor
    origin_device: Uuid,     // Local device UUID
    summary_only: bool,      // If true, peer returns only count summaries
    category: Option<RemoteFileCategory>,
    source: Option<RemoteFileSource>,
    search_query: Option<String>,
    offset: u32,
    limit: u32,
}
```

#### `AppMessage::RemoteFilesResponse` (Wire Message)
```rust
AppMessage::RemoteFilesResponse {
    request_id: Uuid,        // Matches request_id from query
    summary: Option<RemoteFilesSummary>,
    files: Vec<RemoteFileEntry>,
    total_matching: u32,
    error: Option<String>,
}
```

---

## 4. Query Parameters & Category Enums

| Parameter | Type / Enum | Default | Parsing / String Values |
|---|---|---|---|
| `target_device` | `Uuid` (String) | Required | Validated via `uuid::Uuid::parse_str` |
| `summary_only` | `bool` | `false` | `true` for counts-only, `false` for full file list |
| `category` | `Option<RemoteFileCategory>` | `None` (All) | `"Images"`, `"Videos"`, `"Audio"`, `"Documents"`, `"Apks"`, `"Archives"`, `"Other"` (Case-insensitive) |
| `source` | `Option<RemoteFileSource>` | `None` (All) | `"WhatsApp"`, `"Downloads"`, `"Camera"`, `"Other"`, `"All"` (Case-insensitive) |
| `search_query` | `Option<String>` | `None` | Case-insensitive substring filter on file name |
| `offset` | `u32` | `0` | Paging offset |
| `limit` | `u32` | `50` | Default via `default_remote_files_limit()` in `ipc.rs` |
| `timeout_secs` | `u64` | `12` | Default RPC timeout in `query_remote_files_sync()` |

---

## 5. End-to-End Test Architecture Blueprint

To verify remote file queries end-to-end without failing due to unhandled network events or timeouts, tests should operate at three distinct verification tiers:

```
+-------------------------------------------------------------------------+
|                         TIER 1: In-Process Dual-Engine                  |
|  [Test Node A (Requestor)] <--- Real TCP (127.0.0.1) ---> [Mock Node B] |
|   Calls query_remote_files_sync()                        Responds to   |
|   Verifies RemoteFilesResult                              QueryEvent    |
+-------------------------------------------------------------------------+
                                    |
                                    v
+-------------------------------------------------------------------------+
|                         TIER 2: IPC Daemon Smoke Test                   |
|  [Python/Rust IPC Client] <--- Unix/Pipe Socket ---> [Deskdrop Daemon]  |
|   Sends JSON RemoteFilesQuery                         Dispatches RPC    |
+-------------------------------------------------------------------------+
                                    |
                                    v
+-------------------------------------------------------------------------+
|                         TIER 3: Live Android ADB Test                   |
|  [Desktop Node (macOS)] <--- Wi-Fi / ADB Socket ---> [Android Device]   |
|   Queries "Images" folder                            MediaStore query   |
+-------------------------------------------------------------------------+
```

### 5.1 Tier 1: In-Process Rust Integration Harness Code Pattern

Below is the concrete code pattern for a complete remote file query test in Rust:

```rust
#[tokio::test]
async fn test_e2e_remote_files_query_success() {
    let tmp = TempDir::new().unwrap();
    let (tx_a, mut rx_a) = mpsc::channel(64);
    let (tx_b, mut rx_b) = mpsc::channel(64);

    let dev_a = Uuid::new_v4();
    let dev_b = Uuid::new_v4();

    // Setup identities & mutual trust
    let id_a = IdentityStore::new(&tmp.path().join("id_a")).load_or_create().unwrap();
    let id_b = IdentityStore::new(&tmp.path().join("id_b")).load_or_create().unwrap();

    let mut trust_a = TrustStore::load(&tmp.path().join("trust_a.json")).unwrap();
    trust_a.trust(dev_b, "NodeB".into(), &id_b.public_bytes).unwrap();

    let mut trust_b = TrustStore::load(&tmp.path().join("trust_b.json")).unwrap();
    trust_b.trust(dev_a, "NodeA".into(), &id_a.public_bytes).unwrap();

    // Configure and start engines
    let cfg_a = EngineConfig { device_id: dev_a, port: 0, bind_ip: Some(IpAddr::V4(Ipv4Addr::LOCALHOST)), enable_discovery: false, ..Default::default() };
    let cfg_b = EngineConfig { device_id: dev_b, port: 0, bind_ip: Some(IpAddr::V4(Ipv4Addr::LOCALHOST)), enable_discovery: false, ..Default::default() };

    let engine_a = Engine::start(cfg_a, tx_a).await.unwrap();
    let engine_b = Engine::start(cfg_b, tx_b).await.unwrap();

    let port_b = engine_b.bound_port().await;
    engine_a.connect_to_peer("127.0.0.1".into(), port_b).await.unwrap();

    // Spawn mock responder on Node B
    let engine_b_clone = engine_b.clone();
    tokio::spawn(async move {
        while let Some(event) = rx_b.recv().await {
            if let EngineEvent::RemoteFilesQueryReceived { request_id, from_device, category, .. } = event {
                let mock_files = vec![RemoteFileEntry {
                    file_id: 42,
                    display_name: "test_photo.jpg".into(),
                    size_bytes: 1024,
                    mime_type: "image/jpeg".into(),
                    date_modified: 1770000000,
                    category: category.unwrap_or(RemoteFileCategory::Images),
                    source: RemoteFileSource::Camera,
                    content_uri: "content://media/external/file/42".into(),
                }];
                engine_b_clone.send_remote_files_response(from_device, request_id, None, mock_files, 1, None).await;
            }
        }
    });

    // Node A performs synchronous remote files query
    let result = engine_a.query_remote_files_sync(
        dev_b,
        false,
        Some(RemoteFileCategory::Images),
        None,
        None,
        0,
        50,
        5, // 5s timeout
    ).await.expect("query should succeed");

    assert_eq!(result.total_matching, 1);
    assert_eq!(result.files[0].display_name, "test_photo.jpg");
}
```

---

## 6. Helper Scripts & Utilities in `scripts/`

- **`scripts/query_ipc.py`**:
  Demonstrates connecting to `/tmp/deskdrop.sock` (or `/tmp/deskdrop-<uid>/deskdrop.sock`) via Python `socket`. Can be extended to issue `remote_files_query` commands and validate JSON output.
- **`scripts/test-windows-ipc.ps1`**:
  Demonstrates connecting to Windows named pipe `\\.\pipe\DeskdropIPC`.
- **`scripts/build-macos.sh` / `scripts/build-android.sh`**:
  Used to compile native shared libraries (`libdeskdrop_core.dylib` / `libdeskdrop_so`) for native E2E test runs.

---

## 7. Caveats & Requirements for E2E Test Verification

1. **Peer Trust Verification**:
   `AppMessage::RemoteFilesQuery` will be silently dropped with `continue` if the requesting peer is not marked as `trusted: true` in the receiving engine's `PeerManager` (see `deskdrop-core/src/engine/mod.rs` line 5624). E2E tests must pre-populate `TrustStore` for both nodes.
2. **Channel Waiter Cleanup**:
   If a query times out or fails, the entry in `shared.remote_file_waiters` must be removed to avoid memory leaks (currently handled properly in `query_remote_files_sync`).
3. **Android MediaStore Permissions**:
   On real Android devices, `READ_MEDIA_IMAGES` / `READ_EXTERNAL_STORAGE` permissions are required for `RemoteFileManager.kt` to populate the `Cursor`.

---
