## 2026-08-07T15:40:39Z
<USER_REQUEST>
You are Worker 1 for Milestone M3 (RPC Protocol & Dynamic Timeout Hardening).
Your working directory is /Users/chinmayk/Projects/Deskdrop/.agents/worker_m3.

MANDATORY INTEGRITY WARNING:
DO NOT CHEAT. All implementations must be genuine. DO NOT hardcode test results, create dummy/facade implementations, or circumvent the intended task. A teamwork_preview_auditor will independently verify your work. Integrity violations WILL be detected and your work WILL be rejected.

Mission:
Implement dynamic timeout support for `IpcRequest::RemoteFilesQuery` in `deskdrop-core/src/ipc.rs`, `deskdrop-core/src/bin/daemon.rs`, and `deskdrop-core/src/engine/mod.rs`.

Context & Explorer Reports:
- Explorer 1 Handoff: /Users/chinmayk/Projects/Deskdrop/.agents/explorer_m3_1/handoff.md
- Explorer 2 Handoff: /Users/chinmayk/Projects/Deskdrop/.agents/explorer_m3_2/handoff.md
- Explorer 3 Handoff: /Users/chinmayk/Projects/Deskdrop/.agents/explorer_m3_3/handoff.md
- ORIGINAL_REQUEST: /Users/chinmayk/Projects/Deskdrop/.agents/ORIGINAL_REQUEST.md
- PROJECT: /Users/chinmayk/Projects/Deskdrop/PROJECT.md
- SCOPE: /Users/chinmayk/Projects/Deskdrop/.agents/sub_orch_m3/SCOPE.md

Detailed Requirements:
1. Update `deskdrop-core/src/ipc.rs`:
   - In `IpcRequest::RemoteFilesQuery` enum variant definition, add `#[serde(default)] timeout_secs: Option<u64>`.
   - In `handle_ipc_request` matching `IpcRequest::RemoteFilesQuery`, extract `timeout_secs` and call `query_remote_files_sync(..., timeout_secs.unwrap_or(10))` (instead of hardcoded 12).
2. Update `deskdrop-core/src/bin/daemon.rs`:
   - In `handle_request_inner` matching `IpcRequest::RemoteFilesQuery`, extract `timeout_secs` and call `query_remote_files_sync(..., timeout_secs.unwrap_or(10))` (instead of hardcoded 12).
3. Update `deskdrop-core/src/engine/mod.rs`:
   - In `query_remote_files_sync(..., timeout_secs: u64)`, compute `let effective_timeout = if timeout_secs == 0 { 10 } else { timeout_secs };` and use `effective_timeout` in the timeout duration `Duration::from_secs(effective_timeout)`.
   - Ensure clean timeout error message `"Remote files query timed out"` (or similar clean error string) is returned when expired.
4. Verify build and tests:
   - Run `cargo check -p deskdrop-core`
   - Run `cargo test -p deskdrop-core --test remote_files_e2e_test`
   - Run `python3 scripts/test_remote_files_ipc.py` if available.
   Note: If running on macOS in a sandbox environment where socket binding fails, use `BypassSandbox: true` for the cargo commands.
5. Write your implementation report to `/Users/chinmayk/Projects/Deskdrop/.agents/worker_m3/changes.md` and handoff report to `/Users/chinmayk/Projects/Deskdrop/.agents/worker_m3/handoff.md`.
6. Notify parent (Sub-Orchestrator) via send_message when complete.
</USER_REQUEST>
