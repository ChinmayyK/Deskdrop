# Handoff Report — Explorer 2 (Timeout Root Cause Analysis)

## 1. Observation
* **Error String in Client UI**:
  - File: `/Users/chinmayk/Projects/Deskdrop/platforms/macos/Deskdrop/RemoteExplorerView.swift:591`
    ```swift
    Text("Connection Interrupted")
    ```
  - File: `/Users/chinmayk/Projects/Deskdrop/platforms/macos/Deskdrop/RemoteExplorerView.swift:594`
    ```swift
    Text(err) // Renders error message from query response
    ```
* **Timeout Bail String in Engine**:
  - File: `/Users/chinmayk/Projects/Deskdrop/deskdrop-core/src/engine/mod.rs:2184`
    ```rust
    anyhow::bail!("Remote files query timed out after {}s", timeout_secs)
    ```
* **IPC Timeout Setting**:
  - File: `/Users/chinmayk/Projects/Deskdrop/deskdrop-core/src/ipc.rs:1404`
  - File: `/Users/chinmayk/Projects/Deskdrop/deskdrop-core/src/bin/daemon.rs:1385`
    ```rust
    eng.query_remote_files_sync(target_uuid, summary_only, cat, src, search_query, offset, limit, 12).await
    ```
* **Desktop Daemon Event Loop Ignoring `RemoteFilesQueryReceived`**:
  - File: `/Users/chinmayk/Projects/Deskdrop/deskdrop-core/src/bin/daemon.rs:566`
    ```rust
    EngineEvent::FileTransferPaused { .. } | EngineEvent::FileTransferResumed { .. } => {}
    _ => {} // EngineEvent::RemoteFilesQueryReceived falls into wildcard ignore!
    ```
* **Lack of C FFI `send_remote_files_response` Export**:
  - File: `/Users/chinmayk/Projects/Deskdrop/deskdrop-core/src/ffi.rs:1036` contains `deskdrop_send_remote_files_query`, but there is no corresponding `deskdrop_send_remote_files_response` function exported for C/Swift/C# FFI handlers.
* **Android Synchronous Unindexed Full MediaStore Scan**:
  - File: `/Users/chinmayk/Projects/Deskdrop/platforms/android/app/src/main/java/com/deskdrop/RemoteFileManager.kt:60–80`
    ```kotlin
    val uri = MediaStore.Files.getContentUri("external")
    val selection = "${MediaStore.Files.FileColumns.SIZE} > 0"
    context.contentResolver.query(uri, projection, selection, null, "${MediaStore.Files.FileColumns.DATE_MODIFIED} DESC")?.use { cursor ->
        while (cursor.moveToNext()) { ... }
    }
    ```
* **Waiters Not Cleared on Disconnect**:
  - File: `/Users/chinmayk/Projects/Deskdrop/deskdrop-core/src/engine/mod.rs:430` (`PeerDisconnected` handler) does not notify or drain `shared.remote_file_waiters`.

---

## 2. Logic Chain
1. **Observation 1 & 2**: Client UI renders `"Connection Interrupted"` with the localized error message returned when `query_remote_files_sync` bails with `"Remote files query timed out after 12s"`.
2. **Observation 3 & 4**: When a client sends `AppMessage::RemoteFilesQuery` to a desktop target node (macOS, Windows, Linux), the desktop daemon receives `EngineEvent::RemoteFilesQueryReceived`. However, `daemon.rs` handles `EngineEvent` with `_ => {}`, silently dropping the query. No response is ever constructed or sent back over the network. Therefore, the querying client is guaranteed to wait for the entire 12-second socket timeout and fail every single time when target is a desktop node.
3. **Observation 5**: C FFI bindings (`ffi.rs`) lack `deskdrop_send_remote_files_response`, preventing desktop GUI applications using FFI from responding to queries if they bypassed the daemon.
4. **Observation 6**: When a client sends `AppMessage::RemoteFilesQuery` to an Android target node, Android receives event code 30 and calls `RemoteFileManager.queryFiles`. `queryFiles` queries `MediaStore.Files.getContentUri("external")` with no category or MIME type SQL filter (`SIZE > 0`). It loops through every file on the device synchronously in Kotlin. On media-heavy devices (10,000+ files), this blocking loop takes 10 to 25+ seconds, exceeding the 12s client timeout and causing `"Remote files query timed out after 12s"`.
5. **Observation 7**: When a peer disconnects mid-query, `PeerDisconnected` does not clear `remote_file_waiters`. The client hangs for the full 12s before reporting an error instead of failing fast.

---

## 3. Caveats
- No code modifications were performed in this turn (read-only investigation per role constraints).
- Real device execution time on Android was deduced from code logic (`MediaStore.Files.getContentUri("external")` full cursor iteration with in-memory matching); actual latency depends on total files on test device.

---

## 4. Conclusion
The `"Connection Interrupted - Remote files query timed out"` issue is caused by:
1. **Complete absence of a desktop response handler** in `deskdrop-core/src/bin/daemon.rs` for `EngineEvent::RemoteFilesQueryReceived`.
2. **Unindexed, full-cursor synchronous MediaStore scan** on Android in `RemoteFileManager.kt`.
3. **Inelastic 12s socket timeout** without dynamic feedback or connection-drop cancellation in `engine/mod.rs`.

To permanently fix the issue across all platforms (macOS, Windows, Android):
- Implement local directory scanning and response handling in `deskdrop-core/src/bin/daemon.rs` for desktop nodes.
- Push SQL `selection` filters (`MIME_TYPE`) and pagination down to MediaStore in `RemoteFileManager.kt` on Android, and cache category summaries.
- Clean up pending waiters in `engine/mod.rs` on `PeerDisconnected`.

---

## 5. Verification Method
1. **Files to Inspect**:
   - `/Users/chinmayk/Projects/Deskdrop/deskdrop-core/src/bin/daemon.rs` lines 268–570 (verify `RemoteFilesQueryReceived` handling).
   - `/Users/chinmayk/Projects/Deskdrop/deskdrop-core/src/engine/mod.rs` lines 2139–2187 (verify `query_remote_files_sync` timeout and waiter handling).
   - `/Users/chinmayk/Projects/Deskdrop/platforms/android/app/src/main/java/com/deskdrop/RemoteFileManager.kt` lines 60–129 (verify MediaStore query selection and pagination).
2. **Build and Test Verification**:
   - Run `cargo test -p deskdrop-core` to verify existing core tests pass.
   - When implementation is completed by Implementer, run test queries between desktop<->desktop and desktop<->android to verify remote folder contents ("Images") load within <1s without timeout.
