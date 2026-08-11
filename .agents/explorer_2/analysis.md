# Deskdrop Remote Files Query Timeout — Root Cause Analysis

## Executive Summary
The error `"Connection Interrupted - Remote files query timed out"` occurs during remote file browsing in Deskdrop due to two fundamental system flaws:
1. **Missing Desktop Response Handler (100% failure rate when querying macOS, Windows, or Linux remote nodes)**: The desktop daemon (`deskdrop-core/src/bin/daemon.rs`) receives `EngineEvent::RemoteFilesQueryReceived` from the network but ignores it (`_ => {}`). It performs no filesystem scanning and sends no `AppMessage::RemoteFilesResponse` back to the querying client. The client waits for the hardcoded 12-second timeout and fails.
2. **Unindexed Full Filesystem Scan on Android Remote Nodes**: On Android remote nodes, `RemoteFileManager.queryFiles` performs an unindexed full linear scan over all entries in `MediaStore.Files.getContentUri("external")` in Kotlin memory. On devices with large storage (10,000+ files), this blocking loop takes 10 to 25+ seconds, exceeding the client's 12-second socket timeout.

---

## 1. End-to-End Execution Flow

Below is the step-by-step trace of a remote file query (e.g., requesting "Images" from a remote node):

### Step 1: Client UI Request Initiation
* **macOS Client**: `RemoteExplorerView.swift:1251` calls `store.queryRemoteFiles(...)` -> `DeskdropStore.swift:878` -> `DeskdropIPCClient.swift:387` (`queryRemoteFiles`). `DeskdropIPCClient.swift` constructs JSON payload `{"cmd": "remote_files_query", "target_device": "...", "category": "Images", "offset": 0, "limit": 100}` and sends it over local IPC to `deskdrop-daemon`.
* **Windows Client**: `WindowsIpcClient.cs:241` (`RemoteFilesQueryAsync`) sends JSON IPC request `{"cmd": "remote_files_query", ...}` over Named Pipe / IPC socket to local `deskdrop-daemon`.
* **Android Client**: Invokes engine function `query_remote_files_sync` via JNI bridge.

### Step 2: Serialization & Transport Sending
* Local daemon receives `IpcRequest::RemoteFilesQuery` in `deskdrop-core/src/ipc.rs:1380` or `daemon.rs:1367`.
* Daemon calls `eng.query_remote_files_sync(target_uuid, summary_only, category, source, search_query, offset, limit, timeout_secs: 12)`.
* In `deskdrop-core/src/engine/mod.rs:2139–2187`:
  1. Generates `request_id = Uuid::new_v4()`.
  2. Creates `oneshot::channel()` and stores `(request_id, tx)` in `shared.remote_file_waiters`.
  3. Constructs `AppMessage::RemoteFilesQuery { request_id, origin_device, summary_only, category, source, search_query, offset, limit }`.
  4. Encrypts and transmits `AppMessage::RemoteFilesQuery` to `target_device` over network socket.
  5. Awaits response on `rx` channel using `tokio::time::timeout(Duration::from_secs(12), rx)`.

### Step 3: Remote Node Receiving & Processing
* **Case 3A — Target Remote Node is Desktop (macOS / Windows / Linux)**:
  - Remote engine receives `AppMessage::RemoteFilesQuery` (`engine/mod.rs:5611`).
  - Remote engine emits `EngineEvent::RemoteFilesQueryReceived`.
  - Remote daemon event loop (`daemon.rs:268–570`, line 566) matches `EngineEvent`: `_ => {}` (wildcard ignore).
  - **No filesystem scanning occurs and no `send_remote_files_response` is called.**
* **Case 3B — Target Remote Node is Android**:
  - Android engine receives `AppMessage::RemoteFilesQuery` (`engine/mod.rs:5611`).
  - Emits `EngineEvent::RemoteFilesQueryReceived`.
  - JNI maps event to code `30` (`CR_EVENT_REMOTE_FILES_QUERY`) in `jni_android.rs:420`.
  - `DeskdropService.kt:1484` receives event code 30 and dispatches to background task `executeInBackgroundWithWakeLock("RemoteFilesQuery")`.
  - `DeskdropService.kt:1509` calls `RemoteFileManager.queryFiles(...)`.
  - `RemoteFileManager.kt:64–129` queries `MediaStore.Files.getContentUri("external")` with NO SQL category filter (only `SIZE > 0`).
  - Synchronous `while (cursor.moveToNext())` loop iterates over ALL media/files on external storage in Kotlin memory to count categories and filter matching files.
  - Takes 10–25+ seconds on typical devices.

### Step 4: Network Transmission & Timeout Resolution
* If a response is produced, it is sent via `AppMessage::RemoteFilesResponse` back over TCP.
* On the client side, `engine/mod.rs:5644` receives `AppMessage::RemoteFilesResponse`, matches `request_id` in `shared.remote_file_waiters`, and sends the result down `tx`.
* **Timeout Failure**: When no response is returned within 12 seconds (either because Desktop ignored the query or Android was blocked scanning MediaStore), `tokio::time::timeout` in `query_remote_files_sync` expires.
* `engine/mod.rs:2184` bails with `"Remote files query timed out after 12s"`.

### Step 5: Client Response Parsing & UI Rendering
* `ipc.rs:1409` catches error and returns `IpcResponse::err("Remote files query timed out after 12s")`.
* macOS `RemoteExplorerView.swift:1267` catches error, sets `errorMessage = "Remote files query timed out after 12s"`.
* UI renders `errorStateView`:
  - Title: `"Connection Interrupted"`
  - Description: `"Remote files query timed out after 12s"`
  - Button: `"Retry Connection"`

---

## 2. Identified Root Causes

### Root Cause 1: Missing Query Response Handler in Desktop Daemon (Critical Flaw)
* **Location**: `deskdrop-core/src/bin/daemon.rs:566`
* **Details**: In `daemon.rs`, `handle_event` handles incoming engine events. `EngineEvent::RemoteFilesQueryReceived` is not handled and falls through to `_ => {}`. Furthermore, `deskdrop-core/src/ffi.rs` has no C FFI export for sending remote file responses (`deskdrop_send_remote_files_response`).
* **Impact**: 100% failure rate when querying any macOS, Windows, or Linux remote device.

### Root Cause 2: Unindexed MediaStore Full Scan on Android
* **Location**: `platforms/android/app/src/main/java/com/deskdrop/RemoteFileManager.kt:60–129`
* **Details**: `RemoteFileManager.queryFiles` queries `MediaStore.Files.getContentUri("external")` with selection `"SIZE > 0"`. It evaluates categories and search filters in Kotlin code by iterating every single record on the device's storage.
* **Impact**: On devices with >10,000 files, the query takes 10 to 25+ seconds, exceeding the client's 12s socket timeout.

### Root Cause 3: Recalculating Full Category Summaries on Paginated Requests
* **Location**: `platforms/android/app/src/main/java/com/deskdrop/RemoteFileManager.kt:89–103`
* **Details**: `includeSummary` triggers full storage recount on every page query (`offset`, `limit`), multiplying performance overhead on page navigation.
* **Impact**: Rapid timeouts when scrolling or switching tabs.

### Root Cause 4: Low Fixed Socket Wait Timeout (12s) Without Dynamic Adaptation
* **Location**: `deskdrop-core/src/ipc.rs:1404` and `deskdrop-core/src/bin/daemon.rs:1385`
* **Details**: `query_remote_files_sync` hardcodes a 12-second timeout.
* **Impact**: 12s is too short for large initial disk/MediaStore indexing and high-latency Wi-Fi networks, but too long for responsive error handling when target nodes drop offline.

### Root Cause 5: Peer Disconnection Does Not Fail Pending Query Waiters
* **Location**: `deskdrop-core/src/engine/mod.rs:430` (`PeerDisconnected` handler)
* **Details**: When a peer disconnects, pending channels in `shared.remote_file_waiters` are NOT notified or removed until the 12s timeout expires.
* **Impact**: Unnecessary 12-second hang when peer drops mid-request.

### Root Cause 6: Unbounded Concurrent Thumbnail Request Storm
* **Location**: `platforms/macos/Deskdrop/RemoteExplorerView.swift:1294–1299`
* **Details**: UI spawns an unthrottled `Task` for every image/video item in view to request thumbnails (`request_remote_thumbnail_sync`). Each has a 10s timeout (`daemon.rs:1398`).
* **Impact**: Dozens of concurrent IPC/network requests saturate the engine transport channel, starving query response messages and triggering timeout errors.

---

## 3. Recommended Fix Strategies & Architectural Adjustments

### Recommendation 1: Implement Desktop Local File Scanning in Rust Core / Daemon
* Implement a cross-platform file scanner in `deskdrop-core` (or `daemon.rs`) that indexes/scans user directories (`Downloads`, `Pictures`, `Documents`, `Desktop`, `Videos`, `Music`).
* In `daemon.rs`, handle `EngineEvent::RemoteFilesQueryReceived`:
  - Perform category/source filtering on local directory roots.
  - Call `state.engine.send_remote_files_response(...)`.
* In `ffi.rs`, expose `deskdrop_send_remote_files_response`.

### Recommendation 2: Optimize Android MediaStore Queries with Push-Down SQL Selection & Caching
* In `RemoteFileManager.kt`:
  - Push category filters down to MediaStore SQL queries using `MIME_TYPE` selections (e.g. `MediaStore.Images.Media.EXTERNAL_CONTENT_URI` or `MIME_TYPE LIKE 'image/%'`).
  - Cache category summary counts (`images`, `videos`, `docs`, etc.) with a 30-second TTL in memory so paginated requests do not recount storage.
  - Apply `QUERY_ARG_LIMIT` and `QUERY_ARG_OFFSET` to MediaStore queries on Android 8+ (API 26+).

### Recommendation 3: Protocol & Connection Lifecycle Enhancements
* In `deskdrop-core/src/engine/mod.rs`:
  - When `PeerDisconnected` event occurs, drain matching request IDs from `remote_file_waiters` and `remote_thumb_waiters`, returning an immediate `"Peer disconnected"` error.
  - Allow configurable timeout parameters (e.g., 25s for initial cold queries, 10s for cached/paged queries).

### Recommendation 4: Thumbnail Request Throttling & Concurrency Control
* Implement thumbnail request queuing / semaphore on the client side (limit to 4 concurrent thumbnail requests).
* Ensure remote thumbnail generation uses fast downsampling (`ContentResolver.loadThumbnail` or `BitmapFactory.Options.inSampleSize`).

### Recommendation 5: User Experience & Error Handling Improvements
* Distinguish between network disconnects, storage permission denials, and scan timeouts in UI.
* Display clear guidance when storage permissions are missing on Android or desktop.
