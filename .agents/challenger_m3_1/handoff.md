# Handoff Report — Challenger 1 (Milestone M3: RPC Protocol & Dynamic Timeout Hardening)

## 1. Observation

1. **Suite Execution**:
   - `cargo test -p deskdrop-core --test remote_files_e2e_test`: **PASSED** (25/25 tests passed in 0.99s).
   - `python3 scripts/test_remote_files_ipc.py`: **PASSED** (3/3 tests passed in 0.17s).

2. **Empirical Verification of Dynamic Timeout Features**:
   - **Custom Timeout Expiry**: `timeout_secs = 1` expired in `1.00s` with error `"Remote files query timed out after 1s"`. `timeout_secs = 3` expired in `3.00s` with error `"Remote files query timed out after 3s"`.
   - **Fast-Path Completion under Custom Timeout**: Query with `timeout_secs = 5` completed in `<100ms` when responder answered immediately, without waiting 5s.
   - **Zero Timeout Fallback**: Query with `timeout_secs = 0` correctly evaluated `effective_timeout = if timeout_secs == 0 { 10 } else { timeout_secs }`, enforcing 10s default.
   - **IPC Request Schema Default**: IPC JSON payload omitting `timeout_secs` deserialized `timeout_secs` to `None`, defaulting to 10s via `timeout_secs.unwrap_or(10)`.

3. **Empirical Stress Test Failure (Fast-Path Disconnect Waiter Leak)**:
   - Command: `cargo test -p deskdrop-core --test m3_challenger_stress_test`
   - Test `test_reproduce_disconnect_peer_waiter_leak` failed with verbatim output:
     ```text
     Query result after disconnect_peer: Err(Remote files query timed out after 10s)
     Elapsed time after disconnect_peer: 9.951207041s
     thread 'test_reproduce_disconnect_peer_waiter_leak' panicked at deskdrop-core/tests/m3_challenger_stress_test.rs:136:5:
     Query took 9.951207041s to fail after explicit disconnect_peer. Fast-path disconnect failed!
     ```

---

## 2. Logic Chain

1. **Observations 1 & 2**: The worker's implementation of `timeout_secs` in `IpcRequest::RemoteFilesQuery` (`deskdrop-core/src/ipc.rs`), `daemon.rs`, and `query_remote_files_sync` (`deskdrop-core/src/engine/mod.rs`) successfully supports dynamic timeout parsing, custom 1s/3s expiry bounds, fast completion on success, 0s fallback to 10s, and default 10s fallback.

2. **Observation 3**: In `deskdrop-core/src/engine/mod.rs`, `register_session` drains `remote_file_waiters` and `remote_thumb_waiters` ONLY when `register_session`'s TCP read loop terminates normally.
   - When `Engine::disconnect_peer(target_device)` is invoked explicitly (or during concurrent disconnect scenarios), `disconnect_peer` unregisters `target_device` from `peer_manager` and aborts/drops the session task.
   - However, `disconnect_peer` **fails to drain or send error results to `shared.remote_file_waiters` or `shared.remote_thumb_waiters`**.
   - As a result, the oneshot receiver `rx` in `query_remote_files_sync` (and `request_remote_thumbnail_sync`) remains pending.
   - The query hangs for the full duration of `effective_timeout` (10 seconds), taking **9.95 seconds** to fail with a timeout error instead of immediately failing in **<1ms** with `"Peer disconnected"` or `"Target device ... is not connected"`.

---

## 3. Caveats

- **Scope Limits**: The core timeout parsing and serialization functionality in `ipc.rs` and `daemon.rs` works as designed. The rejection is strictly due to the unhandled waiter cleanup leak in `Engine::disconnect_peer` in `deskdrop-core/src/engine/mod.rs`.
- **Sandbox Requirement**: Execution of socket-binding Rust integration tests on macOS requires `BypassSandbox: true`.

---

## 4. Conclusion

**Verdict**: `REJECT`

While basic dynamic timeout configuration functions as intended, empirical stress testing revealed a **critical waiter leak on peer disconnect**. Calling `engine.disconnect_peer()` while an RPC query is in flight causes the caller to hang for 10 seconds until the timeout expires, violating fast-path error handling requirements.

### Required Actionable Mitigation:
In `deskdrop-core/src/engine/mod.rs`:
Update `Engine::disconnect_peer` (and any direct disconnect paths) to acquire `shared.remote_file_waiters` and `shared.remote_thumb_waiters` and drain all pending entries matching `peer_id`, sending `RemoteFilesResult { error: Some("Peer disconnected".to_string()), .. }` and `RemoteThumbnailResult { error: Some("Peer disconnected".to_string()), .. }`.

---

## 5. Verification Method

To independently reproduce and verify this finding:

1. **Run existing E2E and IPC test suites**:
   ```bash
   cargo test -p deskdrop-core --test remote_files_e2e_test
   python3 scripts/test_remote_files_ipc.py
   ```
2. **Run Challenger Stress Harness**:
   ```bash
   cargo test -p deskdrop-core --test m3_challenger_stress_test
   ```
   *Expected result*: `test_reproduce_disconnect_peer_waiter_leak` fails, showing a ~10-second delay instead of fast-path disconnection (< 500ms).
