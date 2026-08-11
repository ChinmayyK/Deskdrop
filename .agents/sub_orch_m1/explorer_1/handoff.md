# Handoff Report — Explorer 1 (Milestone M1 Desktop Daemon & Core Remote Query Handling Specification)

**Author**: Explorer 1  
**Date**: 2026-08-07  
**Working Directory**: `/Users/chinmayk/Projects/Deskdrop/.agents/sub_orch_m1/explorer_1`  
**Handoff Type**: Hard Handoff  

---

## 1. Observation

Direct observations from codebase investigation:

1. **Ignored `EngineEvent::RemoteFilesQueryReceived` in `daemon.rs`**:
   - File: `deskdrop-core/src/bin/daemon.rs:566`
   - Snippet:
     ```rust
     EngineEvent::FileTransferPaused { .. } | EngineEvent::FileTransferResumed { .. } => {}
     _ => {} // EngineEvent::RemoteFilesQueryReceived falls into wildcard match and is silently ignored!
     ```
   - Observation: When a remote peer sends `AppMessage::RemoteFilesQuery` to a desktop daemon node, the engine receives the wire message and emits `EngineEvent::RemoteFilesQueryReceived`. However, `daemon.rs:handle_event` matches this event under the `_ => {}` wildcard and drops it without taking any action. No response is ever constructed or sent back to the querying peer.

2. **Existing Engine Response API**:
   - File: `deskdrop-core/src/engine/mod.rs:2022–2046`
   - Snippet:
     ```rust
     pub async fn send_remote_files_response(
         &self,
         target_device: Uuid,
         request_id: Uuid,
         summary: Option<crate::protocol::RemoteFilesSummary>,
         files: Vec<crate::protocol::RemoteFileEntry>,
         total_matching: u32,
         error: Option<String>,
     ) {
         let msg = AppMessage::RemoteFilesResponse {
             request_id,
             summary,
             files,
             total_matching,
             error,
         };
         let peers = self.shared.peer_manager.all_connected_senders();
         if let Some(tx) = peers
             .into_iter()
             .find(|(id, _)| *id == target_device)
             .map(|(_, tx)| tx)
         {
             let _ = tx.send(msg).await;
         }
     }
     ```
   - Observation: The core `Engine` already provides a public async method `send_remote_files_response` that formats an `AppMessage::RemoteFilesResponse` and sends it to the target peer over the active encrypted channel.

3. **Protocol Structs & Standard Directory Tools**:
   - File: `deskdrop-core/src/protocol.rs:215–271`
   - Protocol types: `RemoteFileCategory`, `RemoteFileSource`, `RemoteFileEntry`, `RemoteFileCategoryCounts`, `RemoteFileSourceCounts`, `RemoteFilesSummary`.
   - Dependency: `dirs = "5"` is included in `deskdrop-core/Cargo.toml:55`. The `dirs` crate provides cross-platform functions `dirs::download_dir()`, `dirs::document_dir()`, `dirs::picture_dir()`, `dirs::video_dir()`, `dirs::audio_dir()`, and `dirs::home_dir()`.

4. **Waiters Not Drained on Peer Disconnect**:
   - File: `deskdrop-core/src/engine/mod.rs:5913–5945`
   - Snippet:
     ```rust
     tracing::warn!("peer disconnected: peer_id={}, reason={:?}", peer_id, reason);
     let _ = shared.event_tx.send(EngineEvent::PeerDisconnected { ... }).await;
     shared.dedup.lock().await.remove_peer(peer_id);
     shared.file_transfers.lock().await.pause_all_for_device(peer_id);
     shared.camera_frames.remove(&peer_id);
     pump_transfer_queue(&shared).await;
     // Note: shared.remote_file_waiters and shared.remote_thumb_waiters are NOT cleared or notified here!
     ```
   - Observation: When a peer connection drops, `shared.remote_file_waiters` (defined at `engine/mod.rs:579`) and `shared.remote_thumb_waiters` (defined at `engine/mod.rs:585`) are not drained. Pending oneshot channels remain alive until `query_remote_files_sync` times out after 12 seconds.

---

## 2. Logic Chain

1. **Observation 1 & 2**: The client UI timeout ("Connection Interrupted - Remote files query timed out after 12s") when querying desktop targets is directly caused by `daemon.rs:566` ignoring `EngineEvent::RemoteFilesQueryReceived`.
2. **Observation 2 & 3**: Desktop nodes have access to `DaemonState` (which wraps `state.engine`) and the `dirs` crate. They can scan system folders (`Downloads`, `Documents`, `Pictures`, `Videos`, `Music`) to satisfy remote file queries.
3. **Observation 1, 2, 3**: Adding a dedicated match arm for `EngineEvent::RemoteFilesQueryReceived` in `daemon.rs:handle_event` that spawns a filesystem scanning task using `tokio::task::spawn_blocking`, constructs `RemoteFilesSummary` and `Vec<RemoteFileEntry>`, and calls `state.engine.send_remote_files_response(...)` will resolve remote file queries from desktop nodes.
4. **Observation 4**: When a remote peer disconnects during an active query, `query_remote_files_sync` (lines 2168–2185) awaits `rx` until the full 12s timeout expires.
5. **Observation 4**: Draining `shared.remote_file_waiters` and `shared.remote_thumb_waiters` inside the peer disconnect logic (`engine/mod.rs:5938`) and sending a failure result (`error: Some("Peer disconnected".into())`) causes pending waiters to fail fast immediately without waiting for the 12s timeout.

---

## 3. Caveats

- **Disk Scan Latency**: Filesystem scans with thousands of files can take up to a few hundred milliseconds. Wrapping the scan in `tokio::task::spawn_blocking` avoids blocking the daemon's async reactor.
- **Recursive Directory Depth**: Filesystem iteration must limit traversal depth (e.g. max depth 3) and skip hidden directories/files (starting with `.`) to avoid performance bottlenecks or infinite recursion from symlinks.
- **Platform Path Resolution**: `dirs` crate automatically resolves `~/Downloads`, `~/Documents`, `~/Pictures`, `~/Movies`/`~/Videos`, `~/Music` across macOS, Windows, and Linux.

---

## 4. Conclusion

### Implementation Specification for Milestone M1 Worker

#### Task A: Implement `EngineEvent::RemoteFilesQueryReceived` in `deskdrop-core/src/bin/daemon.rs`

1. **Location**: `deskdrop-core/src/bin/daemon.rs` line 566 (inside `async fn handle_event`).
2. **Event Handling Code**:
   ```rust
   EngineEvent::RemoteFilesQueryReceived {
       request_id,
       from_device,
       summary_only,
       category,
       source,
       search_query,
       offset,
       limit,
   } => {
       let engine = state.engine.clone();
       tokio::spawn(async move {
           let res = tokio::task::spawn_blocking(move || {
               scan_local_files_for_remote_query(summary_only, category, source, search_query, offset, limit)
           })
           .await;

           match res {
               Ok(Ok((summary, files, total))) => {
                   engine
                       .send_remote_files_response(from_device, request_id, summary, files, total, None)
                       .await;
               }
               Ok(Err(e)) => {
                   engine
                       .send_remote_files_response(from_device, request_id, None, Vec::new(), 0, Some(e.to_string()))
                       .await;
               }
               Err(e) => {
                   engine
                       .send_remote_files_response(from_device, request_id, None, Vec::new(), 0, Some(format!("Task failed: {e}")))
                       .await;
               }
           }
       });
   }
   ```

3. **Local Filesystem Scanning Helper**:
   Create `fn scan_local_files_for_remote_query` in `daemon.rs`:
   - **Root Directories**:
     - Downloads: `dirs::download_dir()`
     - Documents: `dirs::document_dir()`
     - Pictures: `dirs::picture_dir()`
     - Videos: `dirs::video_dir()`
     - Audio: `dirs::audio_dir()`
   - **Categorization Rules**:
     - `Images`: `.jpg`, `.jpeg`, `.png`, `.gif`, `.bmp`, `.webp`, `.heic`, `.svg`
     - `Videos`: `.mp4`, `.mkv`, `.mov`, `.avi`, `.wmv`, `.flv`, `.webm`, `.m4v`
     - `Audio`: `.mp3`, `.wav`, `.flac`, `.aac`, `.ogg`, `.m4a`, `.wma`
     - `Documents`: `.pdf`, `.doc`, `.docx`, `.txt`, `.rtf`, `.xls`, `.xlsx`, `.ppt`, `.pptx`, `.csv`, `.md`
     - `Apks`: `.apk`
     - `Archives`: `.zip`, `.tar`, `.gz`, `.7z`, `.rar`, `.bz2`, `.xz`
     - `Other`: everything else
   - **MIME Type Mapping**: Map extensions to standard MIME strings (e.g. `image/jpeg`, `application/pdf`, `video/mp4`, etc.).
   - **Source Classification**:
     - Path contains `"WhatsApp"` -> `RemoteFileSource::WhatsApp`
     - Path inside `picture_dir()` or contains `"Camera"`/`"DCIM"` -> `RemoteFileSource::Camera`
     - Path inside `download_dir()` -> `RemoteFileSource::Downloads`
     - Other -> `RemoteFileSource::Other`
   - **Filtering**:
     - Skip hidden files (starting with `.`).
     - If `category` specified (and `!= All`), retain only matching categories.
     - If `source` specified (and `!= All`), retain only matching sources.
     - If `search_query` specified (`Some(q)`), match case-insensitively against file display name.
   - **Summary Calculation**:
     Calculate `RemoteFileCategoryCounts` and `RemoteFileSourceCounts` across scanned files and construct `RemoteFilesSummary`.
   - **Sorting & Pagination**:
     Sort matching entries by `date_modified` descending.
     Set `total_matching = entries.len() as u32`.
     If `summary_only` is `true`, return empty `files` list.
     Else slice `entries[offset..(offset + limit)]` and build `Vec<RemoteFileEntry>`.
     Generate stable 64-bit `file_id` by hashing canonical file path string.

#### Task B: Clean Up Pending Waiters on Disconnect in `deskdrop-core/src/engine/mod.rs`

1. **Location**: `deskdrop-core/src/engine/mod.rs` line 5938 (inside peer disconnect cleanup block).
2. **Cleanup Code**:
   ```rust
   // Drain pending remote file waiters and notify oneshot receivers with error fast-path
   {
       let mut waiters = shared.remote_file_waiters.lock().await;
       for (_req_id, tx) in waiters.drain() {
           let _ = tx.send(RemoteFilesResult {
               summary: None,
               files: Vec::new(),
               total_matching: 0,
               error: Some("Peer disconnected".to_string()),
           });
       }

       let mut thumb_waiters = shared.remote_thumb_waiters.lock().await;
       for (_req_id, tx) in thumb_waiters.drain() {
           let _ = tx.send(RemoteThumbnailResult {
               file_id: 0,
               data: Vec::new(),
               error: Some("Peer disconnected".to_string()),
           });
       }
   }
   ```

---

## 5. Verification Method

1. **Inspect Target Files**:
   - `deskdrop-core/src/bin/daemon.rs` (lines 566+)
   - `deskdrop-core/src/engine/mod.rs` (lines 5938+)
2. **Build and Unit Tests**:
   - `cargo check -p deskdrop-core`
   - `cargo test -p deskdrop-core`
3. **Invalidation Conditions**:
   - If desktop target node drops `RemoteFilesQueryReceived`, desktop remote browsing queries will time out.
   - If `remote_file_waiters` are not drained on disconnect, client queries to disconnected peers will hang for 12 seconds instead of failing fast.
