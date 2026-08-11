# Handoff Report — Explorer 2 (Milestone M3: RPC Protocol & Dynamic Timeout Hardening)

## 1. Observation

1. **`deskdrop-core/src/ipc.rs` struct definition** (lines 404–415):
   ```rust
   RemoteFilesQuery {
       target_device: String,
       #[serde(default)]
       summary_only: bool,
       category: Option<String>,
       source: Option<String>,
       search_query: Option<String>,
       #[serde(default)]
       offset: u32,
       #[serde(default = "default_remote_files_limit")]
       limit: u32,
   },
   ```
   - **Finding**: The `timeout_secs` field is missing from `IpcRequest::RemoteFilesQuery`.

2. **`deskdrop-core/src/ipc.rs` IPC handler** (lines 1380–1405):
   ```rust
   IpcRequest::RemoteFilesQuery {
       target_device,
       summary_only,
       category,
       source,
       search_query,
       offset,
       limit,
   } => {
       let target_uuid = match uuid::Uuid::parse_str(&target_device) { ... };
       let cat = category.as_deref().and_then(parse_remote_file_category);
       let src = source.as_deref().and_then(parse_remote_file_source);
       match eng
           .query_remote_files_sync(
               target_uuid,
               summary_only,
               cat,
               src,
               search_query,
               offset,
               limit,
               12,
           )
           .await
   ```
   - **Finding**: Invocation of `query_remote_files_sync` hardcodes `12` seconds as the timeout value.

3. **`deskdrop-core/src/bin/daemon.rs` IPC handler** (lines 1710–1738):
   ```rust
   IpcRequest::RemoteFilesQuery {
       target_device,
       summary_only,
       category,
       source,
       search_query,
       offset,
       limit,
   } => {
       ...
       state
           .engine
           .query_remote_files_sync(
               target_uuid,
               summary_only,
               cat,
               src,
               search_query,
               offset,
               limit,
               12,
           )
           .await?;
   ```
   - **Finding**: The daemon request router also hardcodes `12` seconds as the timeout value.

4. **`deskdrop-core/src/engine/mod.rs` `query_remote_files_sync`** (lines 2152–2214):
   - Registers oneshot channel `(tx, rx)` into `self.shared.remote_file_waiters` under key `request_id`.
   - Sends `AppMessage::RemoteFilesQuery` over TCP.
   - Awaits `tokio::time::timeout(std::time::Duration::from_secs(timeout_secs), rx)`.
   - If `timeout_secs` is passed as `0`, it expires immediately without waiting.

5. **`deskdrop-core/src/engine/mod.rs` Disconnect Cleanup** (lines 5974–5995):
   - `remote_file_waiters` are drained on peer disconnect, returning `RemoteFilesResult { error: Some("Peer disconnected") }`.
   - Receiver receives this fast-path error payload and bails immediately with `"Peer disconnected"`.

6. **Test Suite Verification**:
   - `cargo check -p deskdrop-core` succeeded with code 0.
   - `cargo test -p deskdrop-core --test remote_files_e2e_test` passed all 24 tests in 10.92s.

---

## 2. Logic Chain

1. **Observation 1 & 2**: `IpcRequest::RemoteFilesQuery` currently lacks `timeout_secs`, causing JSON IPC requests sent from clients (or `scripts/test_remote_files_ipc.py`) to omit dynamic timeout configuration.
2. **Observation 2 & 3**: Both `ipc.rs` and `daemon.rs` hardcode a `12` second timeout when forwarding requests to `query_remote_files_sync`.
3. **Observation 4**: Passing `0` for `timeout_secs` causes `tokio::time::timeout` to expire instantly. Defaulting to `10`s when `timeout_secs` is `None` (or `0`) ensures robust fallback behavior.
4. **Observation 5**: Disconnect cleanup in `engine/mod.rs` already contains fast-path error reporting for disconnected peers.
5. **Conclusion**: To complete Milestone M3 scope, `IpcRequest::RemoteFilesQuery` must be extended with `#[serde(default)] timeout_secs: Option<u64>`, `ipc.rs` and `daemon.rs` must compute `let effective_timeout = timeout_secs.unwrap_or(10);`, and `query_remote_files_sync` must safeguard `0` values with `if timeout_secs == 0 { 10 } else { timeout_secs }`.

---

## 3. Caveats

- **Scope Boundary**: This analysis covers core engine RPC, IPC deserialization/routing, and waiter handling. Implementation of native UI client changes (e.g. Swift or C# optional timeout parameters) is outside `deskdrop-core` but natively backwards-compatible because `timeout_secs` is serde-defaulted.
- **Assumptions**: 10 seconds is the target default timeout specified in Milestone M3 SCOPE.md and PROJECT.md contract specs.

---

## 4. Conclusion

The exact code changes needed for Milestone M3 implementation are:
1. `deskdrop-core/src/ipc.rs`:
   - Add `#[serde(default)] timeout_secs: Option<u64>` to `IpcRequest::RemoteFilesQuery`.
   - Update `handle_ipc_request` to destructure `timeout_secs` and call `query_remote_files_sync(..., timeout_secs.unwrap_or(10))`.
2. `deskdrop-core/src/bin/daemon.rs`:
   - Update `IpcRequest::RemoteFilesQuery` match to destructure `timeout_secs` and call `query_remote_files_sync(..., timeout_secs.unwrap_or(10))`.
3. `deskdrop-core/src/engine/mod.rs`:
   - Enforce `let effective_timeout = if timeout_secs == 0 { 10 } else { timeout_secs };` inside `query_remote_files_sync`.
4. Tests:
   - Add unit tests for dynamic timeout and default fallback in `remote_files_e2e_test.rs` and `test_remote_files_ipc.py`.

Full concrete implementation plan is documented in `/Users/chinmayk/Projects/Deskdrop/.agents/explorer_m3_2/analysis.md`.

---

## 5. Verification Method

To verify implementation:
1. Run compilation check:
   ```bash
   cargo check -p deskdrop-core
   ```
2. Run automated E2E tests:
   ```bash
   cargo test -p deskdrop-core --test remote_files_e2e_test
   ```
3. Run Python IPC socket tests:
   ```bash
   python3 scripts/test_remote_files_ipc.py
   ```
4. **Invalidation Conditions**: Any failure in `IpcRequest::RemoteFilesQuery` deserialization when `timeout_secs` is omitted, or failure to timeout at the requested duration.
