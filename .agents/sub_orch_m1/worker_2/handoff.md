# Handoff Report — Worker 2 (Engine Remote Query & Scoped Waiter Remediation)

**Author**: Worker 2 (Implementer & QA)  
**Date**: 2026-08-07  
**Working Directory**: `/Users/chinmayk/Projects/Deskdrop/.agents/sub_orch_m1/worker_2`  
**Handoff Type**: Hard Handoff  

---

## 1. Observation

Direct observations from code modification, build execution, and test results:

1. **Modified File**:
   - `deskdrop-core/src/engine/mod.rs`
     - Updated `EngineShared.remote_file_waiters` type signature to `Arc<Mutex<HashMap<Uuid, (Uuid, oneshot::Sender<RemoteFilesResult>)>>>` (lines 579–583).
     - Updated `EngineShared.remote_thumb_waiters` type signature to `Arc<Mutex<HashMap<Uuid, (Uuid, oneshot::Sender<RemoteThumbnailResult>)>>>` (lines 585–591).
     - Updated `send_remote_files_query` to return `bool` (`true` if target peer channel was found and message queued via `tx.send(msg).await.is_ok()`, `false` otherwise) (lines 1991–2020).
     - Updated `send_remote_thumbnail_request` to return `bool` (`true` if target peer channel was found and message queued via `tx.send(msg).await.is_ok()`, `false` otherwise) (lines 2048–2069).
     - Updated `query_remote_files_sync`: inserts `(target_device, tx)` into `remote_file_waiters`, checks return value of `send_remote_files_query`, and if `false`, removes `request_id` and immediately returns `Err(anyhow::anyhow!("Target device {} is not connected", target_device))` (lines 2140–2180).
     - Updated `request_remote_thumbnail_sync`: inserts `(target_device, tx)` into `remote_thumb_waiters`, checks return value of `send_remote_thumbnail_request`, and if `false`, removes `request_id` and immediately returns `Err(anyhow::anyhow!("Target device {} is not connected", target_device))` (lines 2194–2225).
     - Updated `RemoteFilesResponse` and `RemoteThumbnailResponse` incoming message handlers to unpack tuple `(_target, tx)` upon resolving `request_id` (lines 5657, 5711).
     - Updated `PeerDisconnected { peer_id, .. }` event handler: filters matching keys where `target == peer_id` and removes/drains ONLY those waiters, leaving waiters for other connected peers intact (lines 5942–5970).

2. **Compilation Checks**:
   - Command: `cargo check -p deskdrop-core`
     - Exit code: `0`
     - Result: Successful compilation with 0 errors.
   - Command: `cargo build --bin deskdrop-daemon`
     - Exit code: `0`
     - Result: Successful build.

3. **Test Results**:
   - Command: `cargo test -p deskdrop-core`
     - Exit code: `0`
     - Results: All tests passed across unit and integration test suites.
     - `remote_files_e2e_test.rs`: 24 passed; 0 failed (including `test_tier4_scenario_device_reconnect_retry` passing cleanly).

---

## 2. Logic Chain

1. **Problem Analysis**: Reviewer 1 identified that when a query was dispatched to an unconnected target device (or during temporary peer disconnect/reconnect phases), `send_remote_files_query` silently dropped the message while `query_remote_files_sync` had already inserted the request into `remote_file_waiters`. This caused queries to hang until timing out after 5s/12s. Additionally, on `PeerDisconnected`, `.drain()` failed all waiters globally regardless of target peer ID.
2. **Fast-Fail on Unconnected Device**: By returning a `bool` from `send_remote_files_query` and `send_remote_thumbnail_request` indicating whether the target sender exists and successfully queued the message, synchronous wrappers (`query_remote_files_sync` and `request_remote_thumbnail_sync`) fail fast immediately when the peer is not connected, removing `request_id` from the waiter map and returning `Target device <uuid> is not connected`.
3. **Scoped Disconnect Drain**: Storing `(target_device, tx)` in `remote_file_waiters` and `remote_thumb_waiters` allows `PeerDisconnected { peer_id }` to inspect target devices and drain only pending waiters targeted at `peer_id`. Queries aimed at other connected peers remain active and unaffected.
4. **Verification**: After applying these changes, `test_tier4_scenario_device_reconnect_retry` and the full suite of 24 remote file E2E tests pass 100% cleanly without timing out or hanging.

---

## 3. Caveats

- **Pre-existing Warnings**: `deskdrop-core` produces 2 pre-existing unused variable compiler warnings in `engine/mod.rs` (`unused variable: p` and `variable does not need to be mutable: ping`), which are unrelated to remote file query logic and were preserved to minimize non-scoped changes.
- No caveats for implementation correctness.

---

## 4. Conclusion

All remediation tasks requested by Reviewer 1 have been successfully implemented and verified. `deskdrop-core` now cleanly handles unconnected target device queries with immediate fast-fail errors and scopes disconnect waiter drains strictly to the affected peer.

---

## 5. Verification Method

To independently verify this implementation:

1. Run `cargo check -p deskdrop-core` — must compile with 0 errors.
2. Run `cargo build --bin deskdrop-daemon` — must build successfully.
3. Run `cargo test -p deskdrop-core` — must execute 283 unit tests and all test suites (including 24/24 in `remote_files_e2e_test`) with 100% pass rate.
