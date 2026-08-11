# Implementation Changes — Worker M3 (Dynamic Timeout Support)

## Target Files Modified

1. **`deskdrop-core/src/ipc.rs`**:
   - Added `#[serde(default)] timeout_secs: Option<u64>` to `IpcRequest::RemoteFilesQuery` enum variant definition.
   - Updated `handle_ipc_request` for `IpcRequest::RemoteFilesQuery` to destructure `timeout_secs` and call `query_remote_files_sync` with `timeout_secs.unwrap_or(10)` instead of hardcoded `12`.

2. **`deskdrop-core/src/bin/daemon.rs`**:
   - Updated `handle_request_inner` match arm for `IpcRequest::RemoteFilesQuery` to destructure `timeout_secs` and call `query_remote_files_sync` with `timeout_secs.unwrap_or(10)` instead of hardcoded `12`.

3. **`deskdrop-core/src/engine/mod.rs`**:
   - Updated `query_remote_files_sync` method to compute `let effective_timeout = if timeout_secs == 0 { 10 } else { timeout_secs };`.
   - Used `effective_timeout` in `tokio::time::timeout(std::time::Duration::from_secs(effective_timeout), rx)`.
   - Updated error message on timeout expiration to `"Remote files query timed out after {}s", effective_timeout`.

4. **`deskdrop-core/tests/remote_files_e2e_test.rs`**:
   - Added `test_tier2_boundary_custom_timeout_expiry` to explicitly verify custom timeout duration behavior.

5. **`scripts/test_remote_files_ipc.py`**:
   - Added `timeout_secs: 15` validation to `test_ipc_serialization_schema_validation` to verify JSON socket framing for dynamic timeouts.

## Rationale & Design Decisions
- Keeping `timeout_secs` as `Option<u64>` with `#[serde(default)]` maintains complete backward compatibility for older JSON RPC callers where the parameter is omitted.
- Falling back to 10s default in `ipc.rs`, `daemon.rs`, and in `query_remote_files_sync` (`if timeout_secs == 0 { 10 }`) ensures safe operational defaults while allowing caller customization.

## Verification Results
- `cargo check -p deskdrop-core`: PASSED
- `cargo test -p deskdrop-core --test remote_files_e2e_test`: PASSED (25/25 tests passed)
- `python3 scripts/test_remote_files_ipc.py`: PASSED (3/3 tests passed)
