## 2026-08-07T10:44:40Z
You are Worker 1 for Milestone M1 in Deskdrop.
Working directory: /Users/chinmayk/Projects/Deskdrop/.agents/sub_orch_m1/worker_1

Your mission:
Implement local filesystem scanning & remote files query response in deskdrop-daemon (`deskdrop-core/src/bin/daemon.rs`) and pending waiter cleanup on peer disconnect in core engine (`deskdrop-core/src/engine/mod.rs`).

Context & Specification files:
- Read /Users/chinmayk/Projects/Deskdrop/.agents/ORIGINAL_REQUEST.md
- Read /Users/chinmayk/Projects/Deskdrop/PROJECT.md
- Read /Users/chinmayk/Projects/Deskdrop/.agents/sub_orch_m1/SCOPE.md
- Read /Users/chinmayk/Projects/Deskdrop/.agents/sub_orch_m1/explorer_1/handoff.md

Implementation Instructions:
1. Update `deskdrop-core/src/bin/daemon.rs`:
   - Handle `EngineEvent::RemoteFilesQueryReceived` in `async fn handle_event`.
   - Implement `scan_local_files_for_remote_query` helper using `tokio::task::spawn_blocking` and standard directory functions (`dirs::download_dir()`, `dirs::document_dir()`, `dirs::picture_dir()`, `dirs::video_dir()`, `dirs::audio_dir()`).
   - Categorize files (`Images`, `Videos`, `Audio`, `Documents`, `Apks`, `Archives`, `Other`) and set proper MIME types.
   - Filter by requested `category`, `source`, and `search_query`.
   - Compute `RemoteFilesSummary` (category counts & source counts).
   - Sort by `date_modified` descending, set `total_matching`, slice `[offset..offset+limit]` for `Vec<RemoteFileEntry>` (empty if `summary_only`), hash path into stable `file_id`.
   - Send `AppMessage::RemoteFilesResponse` using `engine.send_remote_files_response(...)`.

2. Update `deskdrop-core/src/engine/mod.rs`:
   - In `PeerDisconnected` cleanup logic (around line 5938), drain `shared.remote_file_waiters` and `shared.remote_thumb_waiters`.
   - Send error oneshot response (`error: Some("Peer disconnected".into())`) to all pending receivers so clients fail fast instead of hanging for 12 seconds.

3. Verification:
   - Run `cargo check -p deskdrop-core`
   - Run `cargo build --bin deskdrop-daemon`
   - Run `cargo test -p deskdrop-core`
   - Include exact build and test outputs in your handoff report.
