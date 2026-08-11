## 2026-08-07T10:53:13Z

You are Worker 2 for Milestone M1 in Deskdrop.
Working directory: /Users/chinmayk/Projects/Deskdrop/.agents/sub_orch_m1/worker_2

Your mission:
Remediate the edge-case test failure and scoped disconnect waiter drain in `deskdrop-core/src/engine/mod.rs` requested by Reviewer 1.

Context & Review files:
- Read /Users/chinmayk/Projects/Deskdrop/.agents/ORIGINAL_REQUEST.md
- Read /Users/chinmayk/Projects/Deskdrop/PROJECT.md
- Read /Users/chinmayk/Projects/Deskdrop/.agents/sub_orch_m1/SCOPE.md
- Read /Users/chinmayk/Projects/Deskdrop/.agents/sub_orch_m1/reviewer_1/handoff.md

Remediation Tasks in `deskdrop-core/src/engine/mod.rs`:
1. Update `send_remote_files_query` to return `bool` (`true` if target peer sender was found and message queued, `false` otherwise).
2. In `query_remote_files_sync`, check the return value of `send_remote_files_query`. If `false`, immediately remove `request_id` from `shared.remote_file_waiters` and return `Err(anyhow::anyhow!("Target device {} is not connected", target_device))`.
3. Apply the same return boolean check for `send_remote_thumbnail_request` and `request_remote_thumbnail_sync`.
4. Update `shared.remote_file_waiters` and `shared.remote_thumb_waiters` to store `(target_device: Uuid, tx)` as values. On `PeerDisconnected { peer_id, .. }` (around line 5940), retain waiters for other devices and drain/fail ONLY those where `target_device == peer_id`.
5. Verification:
   - Run `cargo check -p deskdrop-core`
   - Run `cargo build --bin deskdrop-daemon`
   - Run `cargo test -p deskdrop-core` (verify 100% tests pass, including `test_tier4_scenario_device_reconnect_retry`).

MANDATORY INTEGRITY WARNING: DO NOT CHEAT. All implementations must be genuine. DO NOT hardcode test results, create dummy/facade implementations, or circumvent the intended task. A teamwork_preview_auditor will independently verify your work.

Write your handoff report to /Users/chinmayk/Projects/Deskdrop/.agents/sub_orch_m1/worker_2/handoff.md and send a message when complete.
