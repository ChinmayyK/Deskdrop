# Handoff Report — Reviewer 1 (Milestone M1 Desktop Daemon & Core Remote Query Handling Review)

**Author**: Reviewer 1 (Reviewer & Critic)  
**Date**: 2026-08-07  
**Working Directory**: `/Users/chinmayk/Projects/Deskdrop/.agents/sub_orch_m1/reviewer_1`  
**Handoff Type**: Hard Handoff  

---

## 1. Observation

Direct observations from code review, static analysis, and test execution:

1. **Compilation Check (`cargo check -p deskdrop-core`)**:
   - Exit code: `0`
   - Result: Successful compilation with 2 minor compiler warnings.

2. **Test Suite Failure (`cargo test -p deskdrop-core`)**:
   - Command executed: `cargo test -p deskdrop-core`
   - Exit code: `101`
   - Failure detail:
     ```
     ---- test_tier4_scenario_device_reconnect_retry stdout ----
     thread 'test_tier4_scenario_device_reconnect_retry' panicked at deskdrop-core/tests/remote_files_e2e_test.rs:993:10:
     Retry query after reconnect should succeed: Remote files query timed out after 5s
     
     failures:
         test_tier4_scenario_device_reconnect_retry
     test result: FAILED. 23 passed; 1 failed; finished in 11.09s
     ```

3. **Silent Drop on Unconnected Peer in `send_remote_files_query`**:
   - File: `deskdrop-core/src/engine/mod.rs:2012–2019`
   - Snippet:
     ```rust
     let peers = self.shared.peer_manager.all_connected_senders();
     if let Some(tx) = peers
         .into_iter()
         .find(|(id, _)| *id == target_device)
         .map(|(_, tx)| tx)
     {
         let _ = tx.send(msg).await;
     }
     ```
   - Observation: When `target_device` is not currently in `all_connected_senders()`, `send_remote_files_query` silently drops `msg` and returns `()`. `query_remote_files_sync` (lines 2151–2168) has already registered a oneshot sender in `remote_file_waiters`. Because no message is ever transmitted to a peer, the waiter sits idle until `tokio::time::timeout` expires after 5s or 12s, bailing with `"Remote files query timed out after Ns"`.

4. **Global Waiter Drain on Single Peer Disconnect**:
   - File: `deskdrop-core/src/engine/mod.rs:5944–5962`
   - Snippet:
     ```rust
     let mut waiters = shared.remote_file_waiters.lock().await;
     for (_req_id, tx) in waiters.drain() {
         let _ = tx.send(RemoteFilesResult { ... error: Some("Peer disconnected".to_string()) });
     }
     ```
   - Observation: `remote_file_waiters` is a `HashMap<Uuid, oneshot::Sender<RemoteFilesResult>>` keyed solely by `request_id`. On `EngineEvent::PeerDisconnected { peer_id, .. }`, `.drain()` removes and fails ALL pending waiters across ALL peers, rather than filtering to fail only those queries targeted at `peer_id`.

5. **Daemon Event Handling & Filesystem Traversal (`daemon.rs`)**:
   - File: `deskdrop-core/src/bin/daemon.rs:571–920`
   - Observation: `EngineEvent::RemoteFilesQueryReceived` handling is properly offloaded via `tokio::task::spawn_blocking`. `scan_local_files_for_remote_query` correctly resolves standard user directories using `dirs`, categorizes extensions, maps MIME types, classifies sources, excludes hidden files (`.` prefix), bounds recursive depth (`max_depth = 3`), calculates aggregate summaries, filters, sorts by `date_modified` descending, and handles pagination offsets and limits.

---

## 2. Logic Chain

1. **Test Failure Cause**: `remote_files_e2e_test.rs::test_tier4_scenario_device_reconnect_retry` re-establishes a connection to a restarted peer node. When `query_remote_files_sync` is invoked immediately after `connect_to_peer`, the peer channel registration in `peer_manager.all_connected_senders()` has not completed or is momentarily absent.
2. `send_remote_files_query` finds no connected sender for `target_device` and silently drops the message without notifying the caller.
3. `query_remote_files_sync` enters `tokio::time::timeout(5s, rx)` and blocks for 5 full seconds before failing with a generic timeout error instead of failing fast with `"Target device not connected"`.
4. Making `send_remote_files_query` return a `bool` (or `Result<()>`) indicating whether the request was successfully dispatched to an active peer channel enables `query_remote_files_sync` to fail fast immediately if the target peer is unreachable.
5. In addition, when peer disconnect events occur, `remote_file_waiters` should store `(target_device, tx)` so `PeerDisconnected` cleans up only the waiters for that specific `peer_id`, avoiding cross-peer query cancellation.

---

## 3. Caveats

- **No Integrity Violations Found**: Code inspection confirmed no hardcoded test data, fake implementations, or bypassed requirements in `daemon.rs` or `engine/mod.rs`. The filesystem scanning logic is authentic and robust.
- **Scope of Changes**: The fix requires small, targeted adjustments in `deskdrop-core/src/engine/mod.rs` to return delivery status from query senders and filter disconnect drains by `peer_id`.

---

## 4. Conclusion

**Verdict**: **`REQUEST_CHANGES`**

### Required Action Items for Worker:

1. **Fix Fast-Fail on Unconnected Peer in `send_remote_files_query` (`deskdrop-core/src/engine/mod.rs`)**:
   - Update `send_remote_files_query` to return `bool` (`true` if peer sender was found and message queued, `false` otherwise).
   - In `query_remote_files_sync`, check the return value of `send_remote_files_query`. If `false`, remove `request_id` from `remote_file_waiters` and return `Err(anyhow::anyhow!("Target device {} is not connected", target_device))` immediately.
   - Apply the same pattern to `send_remote_thumbnail_request` and `request_remote_thumbnail_sync`.

2. **Scoped Waiter Draining on Disconnect (`deskdrop-core/src/engine/mod.rs`)**:
   - Change `remote_file_waiters` value type to store `(Uuid /* target_device */, oneshot::Sender<RemoteFilesResult>)`.
   - Change `remote_thumb_waiters` value type to store `(Uuid /* target_device */, oneshot::Sender<RemoteThumbnailResult>)`.
   - On `PeerDisconnected { peer_id, .. }`, retain waiters for other devices and drain/fail ONLY those where `target_device == peer_id`.

3. **Re-verify Test Suite**:
   - Run `cargo test -p deskdrop-core` to ensure 100% pass rate across all 24 E2E tests in `remote_files_e2e_test.rs`.

---

## 5. Verification Method

To independently verify the resolution:

1. Run `cargo check -p deskdrop-core`
2. Run `cargo test -p deskdrop-core`
3. Verify `test_tier4_scenario_device_reconnect_retry` passes without timing out.
