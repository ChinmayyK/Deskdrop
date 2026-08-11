# Handoff Report — Challenger 2 (Milestone M3: RPC Protocol & Dynamic Timeout Hardening)

## 1. Observation

1. **Standard Integration Suite Execution**:
   - Command: `cargo test -p deskdrop-core --test remote_files_e2e_test`
   - Result: 25 passed; 0 failed. All standard feature and boundary tests in `remote_files_e2e_test.rs` passed.

2. **Empirical Adversarial Disconnect Fast-Path Test**:
   - Test File: `deskdrop-core/tests/m3_challenger_stress_test.rs`
   - Test Case: `test_reproduce_disconnect_peer_waiter_leak`
   - Action: Initiated `query_remote_files_sync` with a 10s dynamic timeout to Node B, followed immediately (50ms) by `engine_a.disconnect_peer(dev_b)`.
   - Result:
     ```text
     Query result after disconnect_peer: Err(Remote files query timed out after 10s)
     Elapsed time after disconnect_peer: 9.950199917s

     thread 'test_reproduce_disconnect_peer_waiter_leak' panicked at deskdrop-core/tests/m3_challenger_stress_test.rs:136:5:
     Query took 9.950199917s to fail after explicit disconnect_peer. Fast-path disconnect failed!
     ```

3. **Codebase Inspection (`deskdrop-core/src/engine/mod.rs` & `deskdrop-core/src/peer_manager.rs`)**:
   - **`disconnect_peer`** (`deskdrop-core/src/engine/mod.rs:1908–1940`):
     ```rust
     pub async fn disconnect_peer(&self, device_id: Uuid) -> Result<bool> {
         let _ = self.shared.peer_manager.set_explicit_disconnect(device_id, true)?;
         let session = self.shared.peer_manager.shutdown_peer_session(device_id)?;
         ...
     ```
     `disconnect_peer` calls `shutdown_peer_session(device_id)`, but does **NOT** drain `shared.remote_file_waiters` or `shared.remote_thumb_waiters`.
   - **`shutdown_peer_session`** (`deskdrop-core/src/peer_manager.rs:904–918`):
     ```rust
     pub fn shutdown_peer_session(&self, device_id: Uuid) -> Result<Option<ReplacedSession>> {
         let removed = self.live.remove(&device_id);
         ...
     ```
     Removes `device_id` from `self.live`.
   - **Session actor disconnect cleanup** (`deskdrop-core/src/engine/mod.rs:5934–6018`):
     ```rust
     match shared
         .peer_manager
         .mark_disconnected_if_current(peer_id, session_id, reason.clone())
     {
         Ok(Some(connected_at)) => {
             ...
             // Drain pending remote file waiters and notify oneshot receivers with error fast-path
             let mut waiters = shared.remote_file_waiters.lock().await;
             ...
     ```
   - **`mark_disconnected_if_current`** (`deskdrop-core/src/peer_manager.rs:574–593`):
     ```rust
     if let Some(current) = self.live.get(&device_id) { ... } else { return Ok(None); }
     ```
     Because `shutdown_peer_session` already removed `device_id` from `self.live`, `mark_disconnected_if_current` returns `Ok(None)`. This bypasses the `Ok(Some(connected_at))` branch, skipping the waiter drain logic entirely.

---

## 2. Logic Chain

1. **Observation 1 & 2**: Standard integration tests pass because they test process drop/crash rather than explicit UI/engine `disconnect_peer` calls or concurrent disconnect races. However, empirical testing with `disconnect_peer` revealed that in-flight RPC queries take ~10 seconds to fail with `"Remote files query timed out after 10s"` instead of failing immediately (< 100ms) with `"Peer disconnected"`.
2. **Observation 3**:
   - `disconnect_peer` calls `shutdown_peer_session`, which removes the peer from `peer_manager.live`.
   - `disconnect_peer` itself does not drain `remote_file_waiters` or `remote_thumb_waiters`.
   - When the session actor thread receives the shutdown signal and runs cleanup, `mark_disconnected_if_current` returns `Ok(None)` because `self.live` was already emptied by `shutdown_peer_session`.
   - Returning `Ok(None)` bypasses the block at `engine/mod.rs:5975`, leaving all pending oneshot channels in `remote_file_waiters` orphaned until their dynamic timeout expires.
3. **Conclusion**: Explicit disconnects and raced disconnects leave waiter maps orphaned and fail to provide fast-path error handling.

---

## 3. Caveats

- macOS sandbox execution of `cargo test` requires `BypassSandbox: true` due to TCP socket binding (`127.0.0.1:0`).
- No other caveats.

---

## 4. Conclusion

Verdict: **`REJECT`**

**Reasoning**:
Peer disconnect fast-path handling is broken when `disconnect_peer` is called or when session shutdown races with cleanup. Waiters in `remote_file_waiters` and `remote_thumb_waiters` are not drained upon explicit disconnect, causing RPC queries to hang for their full timeout duration (e.g., 10s) rather than resolving immediately with an error fast-path.

**Required Action**:
1. In `disconnect_peer` (`deskdrop-core/src/engine/mod.rs`), or inside `shutdown_peer_session` / shared waiter cleanup helper, ensure `remote_file_waiters` and `remote_thumb_waiters` for `device_id` are explicitly drained and sent `RemoteFilesResult { error: Some("Peer disconnected".to_string()), ... }`.
2. Ensure session disconnect cleanup handles both `Ok(Some)` and `Ok(None)` / explicit disconnect paths so that waiter maps never leak.

---

## 5. Verification Method

1. **Run target integration test suite**:
   ```bash
   cargo test -p deskdrop-core --test remote_files_e2e_test
   ```
2. **Run empirical challenger stress / reproduction test suite**:
   ```bash
   cargo test -p deskdrop-core --test m3_challenger_stress_test
   ```
   *Verification criteria for pass*: `test_reproduce_disconnect_peer_waiter_leak` must resolve in < 1.0s and return `Err("Peer disconnected")`.
