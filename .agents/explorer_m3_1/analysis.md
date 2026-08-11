# Technical Analysis & Fix Strategy for Milestone M3 — RPC Protocol & Dynamic Timeout Hardening

## 1. Executive Summary

Milestone M3 focuses on upgrading the Deskdrop RPC protocol and engine query primitives to support configurable dynamic timeouts, fast error recovery, and robust pagination handling.

During this investigation, we examined `deskdrop-core/src/ipc.rs`, `deskdrop-core/src/bin/daemon.rs`, `deskdrop-core/src/engine/mod.rs`, `deskdrop-core/tests/remote_files_e2e_test.rs`, and `scripts/test_remote_files_ipc.py`.

### Primary Discoveries:
1. **Missing `timeout_secs` in `IpcRequest::RemoteFilesQuery`**:
   In `ipc.rs` (lines 404–415), `IpcRequest::RemoteFilesQuery` lacks the `timeout_secs: Option<u64>` field specified in `PROJECT.md` interface contracts.
2. **Hardcoded Query Timeouts in IPC / Daemon Handlers**:
   Both `handle_ipc_request` in `ipc.rs` (line 1404) and `handle_request_inner` in `daemon.rs` (line 1736) invoke `query_remote_files_sync` with a hardcoded timeout of `12` seconds instead of parsing `timeout_secs` (or defaulting to 10s if `None`).
3. **Engine Timeout Handling (`query_remote_files_sync`)**:
   In `engine/mod.rs` (lines 2152–2214), `query_remote_files_sync` accepts `timeout_secs: u64`. If `0` is passed, `tokio::time::timeout` would expire instantly (0s). It should fallback to a default of 10s if `0` is provided.
4. **Peer Disconnect Error Fast-Path**:
   `engine/mod.rs` (lines 5975–5995) already correctly drains `remote_file_waiters` when a peer disconnects, sending an immediate `RemoteFilesResult` with `error: Some("Peer disconnected".to_string())`. This avoids blocking until timeout expiry when a connection breaks.

---

## 2. Detailed Findings & Code Mapping

### A. IPC Request Definition & Parsing (`deskdrop-core/src/ipc.rs`)

**Location**: `deskdrop-core/src/ipc.rs`, lines 404–415
```rust
    /// Query remote files or summary from a connected Android peer.
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
**Defect**: `timeout_secs` is absent.
**Fix Plan**: Add `#[serde(default)] timeout_secs: Option<u64>` to `RemoteFilesQuery`.

---

### B. IPC Server Request Handlers (`ipc.rs` & `daemon.rs`)

**Location 1**: `deskdrop-core/src/ipc.rs`, lines 1380–1411 (`handle_ipc_request`)
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
            match eng
                .query_remote_files_sync(
                    target_uuid,
                    summary_only,
                    cat,
                    src,
                    search_query,
                    offset,
                    limit,
                    12, // <--- Hardcoded 12s timeout
                )
                ...
```

**Location 2**: `deskdrop-core/src/bin/daemon.rs`, lines 1710–1740 (`handle_request_inner`)
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
            let res = state
                .engine
                .query_remote_files_sync(
                    target_uuid,
                    summary_only,
                    cat,
                    src,
                    search_query,
                    offset,
                    limit,
                    12, // <--- Hardcoded 12s timeout
                )
                ...
```
**Fix Plan**:
In both places, destructure `timeout_secs` from `IpcRequest::RemoteFilesQuery`, compute `let timeout = timeout_secs.unwrap_or(10);`, and pass `timeout` to `query_remote_files_sync`.

---

### C. Engine Query Implementation (`deskdrop-core/src/engine/mod.rs`)

**Location**: `deskdrop-core/src/engine/mod.rs`, lines 2152–2214
```rust
    #[allow(clippy::too_many_arguments)]
    pub async fn query_remote_files_sync(
        &self,
        target_device: Uuid,
        summary_only: bool,
        category: Option<crate::protocol::RemoteFileCategory>,
        source: Option<crate::protocol::RemoteFileSource>,
        search_query: Option<String>,
        offset: u32,
        limit: u32,
        timeout_secs: u64,
    ) -> Result<RemoteFilesResult> {
        let request_id = Uuid::new_v4();
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.shared
            .remote_file_waiters
            .lock()
            .await
            .insert(request_id, (target_device, tx));
        ...
        let effective_timeout = if timeout_secs == 0 { 10 } else { timeout_secs };
        match tokio::time::timeout(std::time::Duration::from_secs(effective_timeout), rx).await {
            Ok(Ok(res)) => {
                if let Some(err) = res.error {
                    anyhow::bail!("{err}");
                }
                Ok(res)
            }
            Ok(Err(_)) => {
                self.shared
                    .remote_file_waiters
                    .lock()
                    .await
                    .remove(&request_id);
                anyhow::bail!("Remote files query channel closed unexpectedly")
            }
            Err(_) => {
                self.shared
                    .remote_file_waiters
                    .lock()
                    .await
                    .remove(&request_id);
                anyhow::bail!("Remote files query timed out after {}s", effective_timeout)
            }
        }
    }
```
**Observations**:
- Adding `let effective_timeout = if timeout_secs == 0 { 10 } else { timeout_secs };` ensures valid timeouts regardless of input.
- Waiter cleanup on timeout/error is cleanly executed via `.remove(&request_id)`.
- Peer disconnect handling in `engine/mod.rs` (lines 5975–5995) drains `remote_file_waiters` and responds with `"Peer disconnected"`, satisfying the fast-path disconnect requirement.

---

### D. Integration & E2E Testing Coverage

**Location**: `deskdrop-core/tests/remote_files_e2e_test.rs` & `scripts/test_remote_files_ipc.py`
Existing tests in `remote_files_e2e_test.rs` already exercise:
- Timeout expiry (`test_tier2_boundary_timeout_expiry`, line 675)
- Disconnect cleanup (`test_tier2_boundary_disconnect_cleanup`, line 698)
- Dynamic timeout with disconnect (`test_tier3_pairwise_timeout_with_disconnect`, line 784)

**New Test Additions Proposed for M3**:
1. Unit test in `ipc.rs` testing `IpcRequest::RemoteFilesQuery` deserialization with `timeout_secs` provided vs omitted.
2. IPC test in `scripts/test_remote_files_ipc.py` testing custom `timeout_secs` in JSON request payload.
3. Integration test in `remote_files_e2e_test.rs` verifying `query_remote_files_sync` with dynamic custom timeout (e.g. 2s timeout firing accurately after 2s).

---

## 3. Concrete Implementation Plan

### Step 1: Update `deskdrop-core/src/ipc.rs`
- Edit `IpcRequest::RemoteFilesQuery` enum variant to include `#[serde(default)] timeout_secs: Option<u64>`.
- Update `handle_ipc_request` for `IpcRequest::RemoteFilesQuery` to destructure `timeout_secs`, evaluate `let timeout = timeout_secs.unwrap_or(10);`, and pass `timeout` to `query_remote_files_sync`.

### Step 2: Update `deskdrop-core/src/bin/daemon.rs`
- Update `handle_request_inner` for `IpcRequest::RemoteFilesQuery` to destructure `timeout_secs`, evaluate `let timeout = timeout_secs.unwrap_or(10);`, and pass `timeout` to `query_remote_files_sync`.

### Step 3: Update `deskdrop-core/src/engine/mod.rs`
- In `query_remote_files_sync`, add `let effective_timeout = if timeout_secs == 0 { 10 } else { timeout_secs };` and use `effective_timeout` for duration and error reporting.

### Step 4: Add Unit & Integration Tests
- Add IPC JSON unit test for `RemoteFilesQuery` with `timeout_secs`.
- Add test case in `scripts/test_remote_files_ipc.py`.
- Add dynamic timeout test in `remote_files_e2e_test.rs`.

---

## 4. Verification Method

1. **Compilation Check**:
   ```bash
   cargo check -p deskdrop-core
   ```
2. **E2E Test Suite Execution**:
   ```bash
   cargo test -p deskdrop-core --test remote_files_e2e_test
   ```
3. **Python IPC Test Suite Execution**:
   ```bash
   python3 scripts/test_remote_files_ipc.py
   ```
