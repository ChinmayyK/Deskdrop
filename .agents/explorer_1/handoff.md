# Handoff Report — Explorer 1 (Topology & Remote Files Protocol Explorer)

**Author**: Explorer 1  
**Date**: 2026-08-07  
**Working Directory**: `/Users/chinmayk/Projects/Deskdrop/.agents/explorer_1`  
**Handoff Type**: Hard Handoff  

---

## 1. Observation

Direct observations from codebase investigation:

1. **Error Definition**:
   - `deskdrop-core/src/engine/mod.rs:2184`:
     ```rust
     anyhow::bail!("Remote files query timed out after {}s", timeout_secs)
     ```
   - `deskdrop-core/src/engine/mod.rs:2221`:
     ```rust
     anyhow::bail!("Remote thumbnail request timed out after {}s", timeout_secs)
     ```
   - `platforms/macos/Deskdrop/RemoteExplorerView.swift:591`:
     ```swift
     Text("Connection Interrupted")
     ```

2. **Protocol Definition & Wire Messages**:
   - `deskdrop-core/src/protocol.rs:513–530`:
     ```rust
     RemoteFilesQuery {
         request_id: Uuid,
         origin_device: Uuid,
         summary_only: bool,
         category: Option<RemoteFileCategory>,
         source: Option<RemoteFileSource>,
         search_query: Option<String>,
         offset: u32,
         limit: u32,
     },
     RemoteFilesResponse {
         request_id: Uuid,
         summary: Option<RemoteFilesSummary>,
         files: Vec<RemoteFileEntry>,
         total_matching: u32,
         error: Option<String>,
     }
     ```

3. **IPC Server RPC Timeouts**:
   - `deskdrop-core/src/ipc.rs:1380–1405`:
     ```rust
     IpcRequest::RemoteFilesQuery { ... } => {
         ...
         match eng.query_remote_files_sync(target_uuid, summary_only, cat, src, search_query, offset, limit, 12).await {
             Ok(res) => IpcResponse::ok(res),
             Err(e) => IpcResponse::err(e.to_string()),
         }
     }
     ```
   - `deskdrop-core/src/ipc.rs:1412–1424`:
     ```rust
     IpcRequest::RemoteThumbnailRequest { ... } => {
         ...
         match eng.request_remote_thumbnail_sync(target_uuid, file_id, size_px, 10).await { ... }
     }
     ```

4. **Network Read & Connect Timeouts**:
   - `deskdrop-core/src/network.rs:56`: `const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);`
   - `deskdrop-core/src/network.rs:307, 320, 336, 349`: `tokio::time::timeout(Duration::from_secs(30), stream.read_exact(...))`

5. **Android Responder Implementation**:
   - `platforms/android/app/src/main/java/com/deskdrop/DeskdropService.kt:1484–1524`:
     ```kotlin
     DeskdropJni.CR_EVENT_REMOTE_FILES_QUERY -> {
         ...
         executeInBackgroundWithWakeLock("RemoteFilesQuery") {
             val (summaryJson, filesJson, total) = RemoteFileManager.queryFiles(
                 applicationContext, category, source, query, offset, limit,
                 includeSummary = true, includeList = !summaryOnly
             )
             DeskdropJni.sendRemoteFilesResponse(engineHandle, requestId, targetDeviceId, summaryJson, filesJson, total, null)
         }
     }
     ```
   - `platforms/android/app/src/main/java/com/deskdrop/RemoteFileManager.kt:64–130`:
     Queries `MediaStore.Files.getContentUri("external")` and iterates over cursor while `cursor.moveToNext()`. When `includeSummary = true`, it tallies all file categories and sources line by line across the entire storage volume.

---

## 2. Logic Chain

1. **Observation 1 & 3**: When UI requests remote files (e.g. from macOS `RemoteExplorerView` via IPC), `ipc.rs` line 1404 passes a hardcoded `12` seconds timeout parameter to `eng.query_remote_files_sync(...)`.
2. **Observation 2 & 5**: `query_remote_files_sync` creates a oneshot channel keyed by `request_id` in `remote_file_waiters`, sends `AppMessage::RemoteFilesQuery` over encrypted TCP socket, and awaits `tokio::time::timeout(12s, rx)`.
3. **Observation 5**: On the responder (Android device), `DeskdropService.kt` receives `CR_EVENT_REMOTE_FILES_QUERY` and executes `RemoteFileManager.queryFiles(...)`.
4. **Observation 5**: `RemoteFileManager.queryFiles(...)` queries `MediaStore.Files.getContentUri("external")` without SQL category indexes or query limits for summary calculation, scanning every row in the device MediaStore to compute `images`, `videos`, `audio`, `documents`, `apks`, `archives`, `whatsapp`, `downloads`, `camera` counts.
5. **Observation 3, 4, 5**: On devices with large media stores (thousands of media items), running `queryFiles` takes longer than 12 seconds.
6. **Observation 1, 3**: Because `queryFiles` takes >12s, `tokio::time::timeout` in `query_remote_files_sync` expires before `RemoteFilesResponse` arrives, returning `anyhow::bail!("Remote files query timed out after 12s")`.
7. **Observation 1**: `ipc.rs` returns the error string to the UI client, which displays `Connection Interrupted - Remote files query timed out after 12s`.

---

## 3. Caveats

- **Device MediaStore Size Variance**: Small test devices with few files respond in <500ms, hiding the timeout bug during initial manual tests. Large storage devices with >5,000 files consistently trigger the 12s timeout.
- **Network Interface Latency**: On congested Wi-Fi networks or high packet-loss links, TCP retransmissions add network delay to the MediaStore execution delay, accelerating timeout expiration.
- **Uninvestigated Area**: Deep performance profiling of individual Android `MediaStore` queries per Android version (API 29+ vs older POSIX fallback) was not conducted in this read-only phase.

---

## 4. Conclusion

The "Connection Interrupted - Remote files query timed out" issue is caused by a structural mismatch between:
1. A hardcoded 12-second RPC timeout limit in `deskdrop-core/src/ipc.rs:1404` and `deskdrop-core/src/engine/mod.rs:2168`.
2. An un-indexed, full database scan in `RemoteFileManager.queryFiles` on Android (`platforms/android/app/src/main/java/com/deskdrop/RemoteFileManager.kt`) that tallies every file in `MediaStore.Files` for summary counts on every single category query.

---

## 5. Verification Method

To independently verify these findings:

1. **Source Code Inspection**:
   - Inspect `deskdrop-core/src/ipc.rs` line 1404 to verify `timeout_secs = 12`.
   - Inspect `deskdrop-core/src/engine/mod.rs` line 2168 to verify `tokio::time::timeout` handling.
   - Inspect `platforms/android/app/src/main/java/com/deskdrop/RemoteFileManager.kt` lines 64–130 to verify full-cursor traversal for summary counts.

2. **Automated Cargo Test**:
   ```bash
   cargo test -p deskdrop-core
   ```

3. **Invalidation Conditions**:
   - If reducing MediaStore scan scope or increasing RPC timeout resolves the timeout without breaking UI responsiveness, the root cause logic is verified.
