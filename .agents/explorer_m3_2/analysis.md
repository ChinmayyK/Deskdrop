# Milestone M3: RPC Protocol & Dynamic Timeout Hardening Analysis

## 1. Executive Summary

This document provides a comprehensive technical investigation of the RPC protocol, waiter handling, and timeout mechanisms in `deskdrop-core` (`src/engine/mod.rs`, `src/ipc.rs`, and `src/bin/daemon.rs`). 

Currently, `query_remote_files_sync` supports a `timeout_secs: u64` parameter, but IPC request processing in both `ipc.rs` and `daemon.rs` omits `timeout_secs` from `IpcRequest::RemoteFilesQuery` and hardcodes a `12` second timeout when invoking `query_remote_files_sync`. 

To achieve dynamic, configurable timeouts, `IpcRequest::RemoteFilesQuery` must be updated to parse an optional `timeout_secs: Option<u64>` field, falling back to a default `10`s timeout when omitted or set to `0`. Furthermore, `query_remote_files_sync` in `engine/mod.rs` must enforce this fallback logic to protect against immediate expiration if `0` is passed. Peer disconnect fast-path error propagation is already implemented in `engine/mod.rs` via `remote_file_waiters` draining during peer session teardown.

---

## 2. Detailed Codebase Observations & Architectural Analysis

### A. Engine Waiter Registration & Timeout Handling (`deskdrop-core/src/engine/mod.rs`)

1. **Waiter State Definition** (line 578):
   ```rust
   pub(crate) remote_file_waiters: Arc<
       Mutex<
           std::collections::HashMap<
               uuid::Uuid,
               (
                   uuid::Uuid,
                   tokio::sync::oneshot::Sender<RemoteFilesResult>,
               ),
           >,
       >,
   >,
   ```
   - Key: `request_id: Uuid` (unique per RPC query).
   - Value: `(target_device: Uuid, tx: oneshot::Sender<RemoteFilesResult>)`.

2. **Query Execution (`query_remote_files_sync`, lines 2152–2214)**:
   - Generates `request_id = Uuid::new_v4()`.
   - Creates a oneshot channel `(tx, rx)`.
   - Registers `(target_device, tx)` in `remote_file_waiters`.
   - Transmits `AppMessage::RemoteFilesQuery` over TCP via `send_remote_files_query`.
   - If not connected, removes waiter and returns error immediately.
   - Awaits response with `tokio::time::timeout(Duration::from_secs(timeout_secs), rx)`:
     - `Ok(Ok(res))`: If `res.error` is `Some(err)`, bails with `err`. Otherwise returns `Ok(res)`. (Waiter was removed in response handler at line 5689).
     - `Ok(Err(_))`: Receiver dropped without response. Cleans up waiter from `remote_file_waiters` and returns `"Remote files query channel closed unexpectedly"`.
     - `Err(_)` (Tokio `Elapsed`): Query timed out. Cleans up waiter from `remote_file_waiters` and returns `"Remote files query timed out after {}s"`.

3. **Peer Disconnect Cleanup (Lines 5974–5995)**:
   - When a peer session terminates, `engine/mod.rs` acquires `remote_file_waiters.lock().await`.
   - It collects all `req_id` keys where `target == peer_id`.
   - For each matching waiter, it removes the entry and transmits:
     ```rust
     let _ = tx.send(RemoteFilesResult {
         summary: None,
         files: Vec::new(),
         total_matching: 0,
         error: Some("Peer disconnected".to_string()),
     });
     ```
   - This provides fast-path error propagation (`Err("Peer disconnected")`) to caller without waiting for full timeout.

4. **Default Timeout Fallback Vulnerability**:
   - Currently, if `timeout_secs = 0` is passed to `query_remote_files_sync`, `tokio::time::timeout(Duration::from_secs(0), rx)` expires immediately.
   - Defensive guard required: `let effective_timeout = if timeout_secs == 0 { 10 } else { timeout_secs };`.

---

### B. IPC Layer & Serialization (`deskdrop-core/src/ipc.rs`)

1. **`IpcRequest::RemoteFilesQuery` Enum Variant (Lines 404–415)**:
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
   - **Gap**: Lacks `timeout_secs: Option<u64>` field. serde defaults missing fields to `None`.

2. **IPC Handler (`handle_ipc_request`, lines 1380–1405)**:
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
           12, // Hardcoded 12s timeout!
       )
       .await
   ```
   - **Gap**: Hardcodes `12` seconds. Should use `timeout_secs.unwrap_or(10)`.

---

### C. Daemon Request Routing (`deskdrop-core/src/bin/daemon.rs`)

1. **Daemon Dispatch (Lines 1710–1738)**:
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
       state.engine.query_remote_files_sync(
           target_uuid, summary_only, cat, src, search_query, offset, limit, 12
       ).await?;
   ```
   - **Gap**: Hardcodes `12` seconds. Must also be updated to extract `timeout_secs` and pass `timeout_secs.unwrap_or(10)`.

---

## 3. Concrete Implementation Strategy

### Step 1: Update `deskdrop-core/src/ipc.rs`
1. Add `#[serde(default)] timeout_secs: Option<u64>` to `IpcRequest::RemoteFilesQuery`.
2. In `handle_ipc_request`, update pattern match to destructure `timeout_secs`.
3. Compute `let effective_timeout = timeout_secs.unwrap_or(10);` and pass `effective_timeout` to `query_remote_files_sync`.

### Step 2: Update `deskdrop-core/src/bin/daemon.rs`
1. Update `IpcRequest::RemoteFilesQuery` pattern match in `daemon.rs` to destructure `timeout_secs`.
2. Compute `let effective_timeout = timeout_secs.unwrap_or(10);` and pass `effective_timeout` to `query_remote_files_sync`.

### Step 3: Update `deskdrop-core/src/engine/mod.rs`
1. In `query_remote_files_sync`, add fallback calculation:
   ```rust
   let effective_timeout = if timeout_secs == 0 { 10 } else { timeout_secs };
   ```
2. Pass `effective_timeout` to `tokio::time::timeout(std::time::Duration::from_secs(effective_timeout), rx)`.
3. Update error message on timeout to reflect `effective_timeout`: `"Remote files query timed out after {}s", effective_timeout`.

### Step 4: Verification & Test Coverage
1. Update/Add unit tests in `deskdrop-core/tests/remote_files_e2e_test.rs` validating custom `timeout_secs` values (e.g. 1s timeout fast failure, 10s default fallback).
2. Update `scripts/test_remote_files_ipc.py` to test JSON requests containing explicit `"timeout_secs": 15` and verifying JSON schema compatibility.

---

## 4. Summary Table of Files & Changes

| File Path | Component | Description of Edit |
|-----------|-----------|---------------------|
| `deskdrop-core/src/ipc.rs` | `IpcRequest::RemoteFilesQuery` | Add `#[serde(default)] timeout_secs: Option<u64>` field. |
| `deskdrop-core/src/ipc.rs` | `handle_ipc_request` | Destructure `timeout_secs`, fallback to 10s if `None`, pass to `query_remote_files_sync`. |
| `deskdrop-core/src/bin/daemon.rs` | `IpcRequest::RemoteFilesQuery` handler | Destructure `timeout_secs`, fallback to 10s if `None`, pass to `query_remote_files_sync`. |
| `deskdrop-core/src/engine/mod.rs` | `query_remote_files_sync` | Ensure `effective_timeout` defaults to 10s if `0` passed; use `effective_timeout` in `tokio::time::timeout`. |
| `deskdrop-core/tests/remote_files_e2e_test.rs` | E2E Tests | Add tests for custom timeout and default timeout behavior. |
| `scripts/test_remote_files_ipc.py` | Python IPC Test | Add schema test for optional `timeout_secs` field in IPC JSON requests. |
