# Handoff Report — Explorer 4 (M3 Disconnect Waiter Drain Defect Analysis)

## 1. Observation

1. **Reproduction Test Execution**:
   - Command: `cargo test -p deskdrop-core --test m3_challenger_stress_test`
   - Result:
     ```text
     Query result after disconnect_peer: Err(Remote files query timed out after 10s)
     Elapsed time after disconnect_peer: 9.947838083s
     thread 'test_reproduce_disconnect_peer_waiter_leak' panicked at deskdrop-core/tests/m3_challenger_stress_test.rs:136:5:
     Query took 9.947838083s to fail after explicit disconnect_peer. Fast-path disconnect failed!
     ```

2. **Codebase Inspection (`deskdrop-core/src/engine/mod.rs` & `deskdrop-core/src/peer_manager.rs`)**:
   - `Engine::disconnect_peer` (lines 1908–1940): Calls `shutdown_peer_session(device_id)`, but does **NOT** drain `shared.remote_file_waiters` or `shared.remote_thumb_waiters`.
   - `Engine::forget_device` (lines 2569–2586): Calls `shutdown_peer_session(device_id)`, but does **NOT** drain waiter maps.
   - `PeerManager::shutdown_peer_session` (`peer_manager.rs:904–918`): Removes `device_id` from `self.live` synchronously.
   - Session actor disconnect cleanup (`engine/mod.rs:5934–6018`): Calls `mark_disconnected_if_current(peer_id, session_id, reason)`. Because `self.live` was already emptied by `shutdown_peer_session`, `mark_disconnected_if_current` returns `Ok(None)`.
   - Returning `Ok(None)` bypasses the `Ok(Some(connected_at))` match arm, skipping the waiter drain logic at lines 5975–6018.

---

## 2. Logic Chain

1. **Step 1**: In `test_reproduce_disconnect_peer_waiter_leak`, an RPC query is initiated while peer Node B is live. `query_remote_files_sync` inserts `(dev_b, tx)` into `shared.remote_file_waiters`.
2. **Step 2**: Node A calls `engine_a.disconnect_peer(dev_b)`.
3. **Step 3**: `disconnect_peer` calls `shutdown_peer_session(dev_b)`, removing `dev_b` from `peer_manager.live`, without draining `remote_file_waiters`.
4. **Step 4**: The session actor receives the shutdown signal, exits loop, and calls `mark_disconnected_if_current`.
5. **Step 5**: Because `self.live` no longer contains `dev_b`, `mark_disconnected_if_current` returns `Ok(None)`.
6. **Step 6**: The `match` statement in session actor cleanup skips `Ok(Some(connected_at))`, bypassing waiter map draining.
7. **Step 7**: The waiter entry `(dev_b, tx)` remains in `remote_file_waiters` until the 10-second timeout expires, returning `Err("Remote files query timed out after 10s")` instead of an immediate fast-path error (`"Peer disconnected"`).

---

## 3. Caveats

- **Execution Environment**: Integration tests that bind local TCP listeners require `BypassSandbox: true` on macOS.
- **Scope**: Analysis is focused on `remote_file_waiters` and `remote_thumb_waiters` in `deskdrop-core/src/engine/mod.rs`. No other waiter structures were found to be orphaned.

---

## 4. Conclusion

- **Assessment**: The defect is confirmed. `disconnect_peer` and session exit cleanup under `Ok(None)` / `Err(_)` bypass waiter draining.
- **Fix Strategy**:
  1. Add helper `drain_remote_waiters(shared: &EngineShared, peer_id: Uuid)` in `engine/mod.rs` to drain both `remote_file_waiters` and `remote_thumb_waiters` and send fast-path error `RemoteFilesResult { error: Some("Peer disconnected".to_string()), ... }` and `RemoteThumbnailResult { error: Some("Peer disconnected".to_string()), ... }`.
  2. Call `drain_remote_waiters(&self.shared, device_id).await` directly inside `Engine::disconnect_peer` and `Engine::forget_device`.
  3. Call `drain_remote_waiters(&shared, peer_id).await` in `Ok(Some)`, `Ok(None)`, and `Err(_)` branches of session actor disconnect cleanup.
- **Artifacts Created**:
  - `/Users/chinmayk/Projects/Deskdrop/.agents/explorer_m3_r2_1/analysis.md` (Detailed analysis and proposed diff snippets)
  - `/Users/chinmayk/Projects/Deskdrop/.agents/explorer_m3_r2_1/handoff.md` (Self-contained handoff report)

---

## 5. Verification Method

1. **Verify Baseline Failure**:
   ```bash
   cargo test -p deskdrop-core --test m3_challenger_stress_test
   ```
2. **Apply Proposed Fix** (Implementer step).
3. **Verify Stress Test Pass**:
   ```bash
   cargo test -p deskdrop-core --test m3_challenger_stress_test
   ```
   *Pass Criteria*: `test_reproduce_disconnect_peer_waiter_leak` completes in < 50ms returning `Err("Peer disconnected")`.
4. **Verify Integration Suite**:
   ```bash
   cargo test -p deskdrop-core --test remote_files_e2e_test
   ```
   *Pass Criteria*: 25/25 tests pass clean.
