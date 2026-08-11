# Milestone M3: RPC Protocol & Dynamic Timeout Hardening Analysis

## 1. Executive Summary & Scope Overview
Milestone M3 focuses on upgrading the Deskdrop Remote File RPC layer in `deskdrop-core` to support dynamic, configurable timeouts, clean error responses on expiration/disconnect, and resilient pagination handling.

Currently:
- `IpcRequest::RemoteFilesQuery` in `deskdrop-core/src/ipc.rs` lacks the `timeout_secs` field, and `handle_ipc_request` hardcodes a 12-second timeout (`eng.query_remote_files_sync(..., 12)`).
- `query_remote_files_sync` in `deskdrop-core/src/engine/mod.rs` takes `timeout_secs: u64`, but does not guard against `0` timeout (which would cause instantaneous failure).
- The wire protocol (`AppMessage::RemoteFilesQuery` in `protocol.rs`) remains client-agnostic and does not need protocol wire format changes, as timeout is purely a local client-side wait budget.

---

## 2. Technical Codebase Deep-Dive

### A. IPC Layer (`deskdrop-core/src/ipc.rs`)
1. **Request Variant Definition** (lines 404–415):
   Currently:
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
   }
   ```
   Proposed Change:
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
       #[serde(default)]
       timeout_secs: Option<u64>,
   }
   ```

2. **IPC Handler Function** (`handle_ipc_request` in `ipc.rs` lines 1380–1411):
   Currently:
   ```rust
   match eng
       .query_remote_files_sync(
           target_uuid,
           summary_only,
           cat,
           src,
           search_query,
           offset,
           limit,
           12, // Hardcoded 12s timeout
       )
       .await
   ```
   Proposed Change:
   ```rust
   let timeout = match timeout_secs {
       Some(0) | None => 10,
       Some(t) => t,
   };
   match eng
       .query_remote_files_sync(
           target_uuid,
           summary_only,
           cat,
           src,
           search_query,
           offset,
           limit,
           timeout,
       )
       .await
   ```

### B. Core Engine Layer (`deskdrop-core/src/engine/mod.rs`)
1. **`query_remote_files_sync`** (lines 2152–2214):
   - Accepts `timeout_secs: u64`.
   - Effective timeout resolution:
     ```rust
     let effective_timeout = if timeout_secs == 0 { 10 } else { timeout_secs };
     ```
   - Registers oneshot waiter `(target_device, tx)` in `shared.remote_file_waiters`.
   - Calls `send_remote_files_query`. If target peer is not connected (`!sent`), immediately removes waiter and returns `Err("Target device {} is not connected")`.
   - Awaits response with `tokio::time::timeout(Duration::from_secs(effective_timeout), rx)`.
   - On timeout (`Err(_)`), removes waiter from `remote_file_waiters` and returns `Err("Remote files query timed out after {}s", effective_timeout)`.
   - On channel closure (`Ok(Err(_))`), removes waiter and returns `Err("Remote files query channel closed unexpectedly")`.
   - On response with remote error (`Ok(Ok(res))` where `res.error.is_some()`), bails with the remote error string.

2. **Peer Disconnect Handling** (lines 5973–5996):
   - When `PeerDisconnected` event fires, `shared.remote_file_waiters` is locked.
   - All waiters matching `target_device == peer_id` are drained and sent a `RemoteFilesResult` with `error: Some("Peer disconnected")`.
   - This provides a fast error path so active queries fail immediately upon peer disconnect instead of hanging until timeout expiry.

---

## 3. Pagination & Dynamic Timeout Interaction Analysis

1. **Pagination Parameter Breakdown**:
   - `offset`: Zero-indexed entry offset (u32).
   - `limit`: Maximum entries per page (u32, default 50).
   - `summary_only = true`: File listing pagination is bypassed, and category/source summary counts are aggregated.
   - `summary_only = false`:
     - `limit = 0`: Returns `total_matching` count without returning file entries (lightweight count check).
     - `offset >= total_matching`: Returns `total_matching` count with empty file vector (end-of-directory reached).
     - Multi-page infinite scroll: Sequentially querying `offset = 0, 50, 100...` with `limit = 50`.

2. **Dynamic Timeout Interaction**:
   - Short/Interactive Queries (e.g. quick count checks or subsequent page loads): Clients can specify `timeout_secs: Some(5)` to fail fast if network latency spikes.
   - Heavy Initial Scans (e.g. initial full-library query on large MediaStore): Clients can request higher budgets like `timeout_secs: Some(25)`.
   - Backward Compatibility: Clients omitting `timeout_secs` get the default fallback of 10 seconds.

---

## 4. Comprehensive Edge Cases Matrix

| Edge Case Scenario | Inputs / Conditions | Engine & IPC Behavior | Expected Result |
|---|---|---|---|
| Omitted timeout_secs | `"timeout_secs"` field missing in IPC JSON | IPC deserializes to `None`, defaults to 10s | Query runs with 10s timeout budget |
| Zero timeout_secs | `"timeout_secs": 0` in IPC JSON or `timeout_secs = 0` in Rust API | Engine maps `effective_timeout = 10` | Default 10s timeout applied, preventing instant 0s timeout |
| Short timeout expiry | `timeout_secs: Some(1)`, responder delayed >1s | `tokio::time::timeout` expires, waiter cleaned up | Returns `Err("Remote files query timed out after 1s")` |
| Peer disconnect mid-query | Remote peer drops connection during query | `PeerDisconnected` drains `remote_file_waiters` | Returns `Err("Peer disconnected")` fast without waiting for timeout |
| Unreachable target peer | Query target is not in `all_connected_senders()` | `send_remote_files_query` returns `false` | Bails fast with `Err("Target device ... is not connected")` |
| Zero limit pagination | `limit: 0` | Responder returns `total_matching` with empty `files` list | Returns `Ok(RemoteFilesResult { files: [], total_matching: N })` |
| Out-of-bounds offset | `offset: 9999` (greater than dataset length) | Responder returns `total_matching` with empty `files` list | Returns `Ok(RemoteFilesResult { files: [], total_matching: N })` |

---

## 5. Implementation Roadmap for Milestone M3

1. **`deskdrop-core/src/ipc.rs` Updates**:
   - Add `#[serde(default)] timeout_secs: Option<u64>` to `IpcRequest::RemoteFilesQuery`.
   - Update `handle_ipc_request` for `IpcRequest::RemoteFilesQuery` to resolve `timeout_secs` with fallback to 10s.

2. **`deskdrop-core/src/engine/mod.rs` Updates**:
   - Ensure `query_remote_files_sync` converts `timeout_secs == 0` to `10`.
   - Verify error messages include the resolved timeout seconds.

3. **Build & Test Verification**:
   - Run `cargo check -p deskdrop-core`.
   - Run `cargo test -p deskdrop-core --test remote_files_e2e_test` with `BypassSandbox`.
