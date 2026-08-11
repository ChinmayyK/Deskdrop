# Scope: Milestone M3 — RPC Protocol & Dynamic Timeout Hardening

## Objective
Expose configurable RPC timeouts in `IpcRequest::RemoteFilesQuery` and update `engine/mod.rs` and `ipc.rs` to support dynamic timeouts, clean error responses on expiration/disconnect, and proper pagination parameters.

## Work Items
1. **Explorer Phase**: Analyze `deskdrop-core/src/ipc.rs` and `deskdrop-core/src/engine/mod.rs` (and any related test files or protocol definitions) to determine the exact changes needed for dynamic timeout support and parsing.
2. **Worker Phase**:
   a. Update `deskdrop-core/src/ipc.rs`: Ensure `IpcRequest::RemoteFilesQuery` parses optional `timeout_secs` and passes it to `query_remote_files_sync`.
   b. Update `deskdrop-core/src/engine/mod.rs`: Ensure `query_remote_files_sync` utilizes the requested `timeout_secs` parameter (or default 10s if not specified), returning clean timeout errors if expired.
   c. Build and test verification (`cargo check -p deskdrop-core`, `cargo test -p deskdrop-core --test remote_files_e2e_test`).
3. **Reviewer Phase**: Two independent code quality and correctness reviews.
4. **Challenger Phase**: Two independent adversarial verifications / stress testing.
5. **Auditor Phase**: Forensic integrity audit.
6. **Gate Evaluation & Completion**: Record gate status in `GATE_STATUS.md`, update `PROJECT.md`, write `handoff.md`, notify parent.
