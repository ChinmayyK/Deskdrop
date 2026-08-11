# Handoff Report — Forensic Integrity Audit (M3 R2)

## Verdict
**CLEAN**

## 1. Observation
- **Source Code Changes**:
  - `deskdrop-core/src/engine/mod.rs:6157–6213`: Added `pub(crate) async fn drain_remote_waiters(shared: &EngineShared, peer_id: Uuid)`. Atomically extracts and drains pending channels from `shared.remote_file_waiters` and `shared.remote_thumb_waiters` for a given `peer_id`, immediately sending `RemoteFilesResult { error: Some("Peer disconnected".to_string()), ... }` and `RemoteThumbnailResult { error: Some("Peer disconnected".to_string()), ... }`.
  - `deskdrop-core/src/engine/mod.rs:1915`: Called in `Engine::disconnect_peer(device_id)`.
  - `deskdrop-core/src/engine/mod.rs:2577`: Called in `Engine::forget_device(device_id)`.
  - `deskdrop-core/src/engine/mod.rs:5980, 6054, 6058`: Called in `register_session` actor disconnect cleanup across all completion branches (`Ok(Some)`, `Ok(None)`, and `Err(_)`).
  - `deskdrop-core/src/ipc.rs:415, 1407`: `IpcRequest::RemoteFilesQuery` updated to parse optional `timeout_secs` parameter and pass `timeout_secs.unwrap_or(10)` to `query_remote_files_sync`.
- **Test Suites**:
  - `deskdrop-core/tests/m3_challenger_stress_test.rs`: 2 test cases (`test_reproduce_disconnect_peer_waiter_leak`, `test_dynamic_timeouts_granularity`).
  - `deskdrop-core/tests/remote_files_e2e_test.rs`: 25 E2E integration test cases covering feature coverage, boundary conditions, pairwise parameter combinations, and real-world application scenarios.
  - `scripts/test_remote_files_ipc.py`: 3 end-to-end Python IPC protocol tests.
- **Empirical Execution Results**:
  - `cargo check -p deskdrop-core`: PASS (Exit code 0)
  - `cargo test -p deskdrop-core --test m3_challenger_stress_test`: PASS (2 passed, 0 failed, 0 ignored in 1.27s)
  - `cargo test -p deskdrop-core --test remote_files_e2e_test`: PASS (25 passed, 0 failed, 0 ignored in 10.96s)
  - `python3 scripts/test_remote_files_ipc.py`: PASS (3 passed in 0.172s)

## 2. Logic Chain
1. The bug causing hanging RPC requests during remote file queries was that pending oneshot receivers in `remote_file_waiters` and `remote_thumb_waiters` were not drained when a peer session closed or was explicitly disconnected. Receivers waited for the full timeout duration (10 seconds) before failing.
2. `drain_remote_waiters` directly targets this root cause by removing all waiter keys associated with `peer_id` from the shared state `HashMap` and sending fast-path `"Peer disconnected"` error responses to the oneshot senders.
3. In `query_remote_files_sync` and `request_remote_thumbnail_sync`, receiving this oneshot error immediately triggers `anyhow::bail!("{err}")`, unblocking the caller in ~1ms rather than 10,000ms.
4. `drain_remote_waiters` is hooked into all peer disconnection entry points: `disconnect_peer`, `forget_device`, and session actor termination (`register_session`).
5. Static inspection confirmed no hardcoded test outputs, facade logic, mock short-circuits, or pre-populated artifact files exist in the codebase.
6. The test assertions in `m3_challenger_stress_test.rs` and `remote_files_e2e_test.rs` are genuine, exercising real in-process `Engine` instances over local TCP sockets and asserting actual timing, errors, and payload structures.

## 3. Caveats
No caveats. Audit verified both implementation correctness and test suite integrity.

## 4. Conclusion
Final verdict: **CLEAN**.
The disconnect waiter drain fix and dynamic RPC timeout hardening in `deskdrop-core` meet all forensic integrity criteria.

## 5. Verification Method
Re-run the empirical verification suite:
```bash
cargo check -p deskdrop-core
cargo test -p deskdrop-core --test m3_challenger_stress_test
cargo test -p deskdrop-core --test remote_files_e2e_test
python3 scripts/test_remote_files_ipc.py
```
Invalidation conditions: Any compilation error, test failure, hardcoded result detection, or unhandled peer session termination branch that leaves RPC waiters in `remote_file_waiters`.
