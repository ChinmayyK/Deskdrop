## 2026-08-07T15:49:12Z
<USER_REQUEST>
You are Worker 2 for Milestone M3 Iteration 2 (RPC Protocol & Dynamic Timeout Hardening).
Your working directory is /Users/chinmayk/Projects/Deskdrop/.agents/worker_m3_r2.

MANDATORY INTEGRITY WARNING:
DO NOT CHEAT. All implementations must be genuine. DO NOT hardcode test results, create dummy/facade implementations, or circumvent the intended task. A teamwork_preview_auditor will independently verify your work. Integrity violations WILL be detected and your work WILL be rejected.

Mission:
Implement the disconnect waiter drain fix in `deskdrop-core/src/engine/mod.rs` so that calling `Engine::disconnect_peer(device_id)`, `Engine::forget_device(device_id)`, or session disconnect cleanup immediately drains `remote_file_waiters` and `remote_thumb_waiters` with `"Peer disconnected"`.

Context & Explorer Reports:
- Explorer 4 Handoff: /Users/chinmayk/Projects/Deskdrop/.agents/explorer_m3_r2_1/handoff.md
- Explorer 4 Analysis: /Users/chinmayk/Projects/Deskdrop/.agents/explorer_m3_r2_1/analysis.md
- Challenger 2 Handoff: /Users/chinmayk/Projects/Deskdrop/.agents/challenger_m3_2/handoff.md
- Stress / Reproduction Test: deskdrop-core/tests/m3_challenger_stress_test.rs

Tasks:
1. Implement async helper `drain_remote_waiters(shared: &EngineShared, peer_id: Uuid)` in `deskdrop-core/src/engine/mod.rs` to drain both `remote_file_waiters` and `remote_thumb_waiters` matching `peer_id` and send fast-path error `"Peer disconnected"`.
2. Update `Engine::disconnect_peer` and `Engine::forget_device` in `engine/mod.rs` to call `drain_remote_waiters(&self.shared, device_id).await`.
3. Update session actor disconnect cleanup in `engine/mod.rs` to call `drain_remote_waiters(&shared, peer_id).await` unconditionally during session exit/cleanup.
4. Verify build and test suite:
   - Run `cargo check -p deskdrop-core`
   - Run `cargo test -p deskdrop-core --test remote_files_e2e_test`
   - Run `cargo test -p deskdrop-core --test m3_challenger_stress_test` (Verify `test_reproduce_disconnect_peer_waiter_leak` passes in < 1.0s returning "Peer disconnected").
   - Run `python3 scripts/test_remote_files_ipc.py` if available.
   Note: On macOS sandbox where TCP socket binding is restricted, use `BypassSandbox: true` for cargo test commands.
5. Write implementation report to `/Users/chinmayk/Projects/Deskdrop/.agents/worker_m3_r2/changes.md` and handoff report to `/Users/chinmayk/Projects/Deskdrop/.agents/worker_m3_r2/handoff.md`.
6. Notify parent via send_message when done.
</USER_REQUEST>
