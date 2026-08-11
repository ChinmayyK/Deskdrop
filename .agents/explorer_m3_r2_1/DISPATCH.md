## 2026-08-07T15:47:41Z
You are Explorer 4 for Milestone M3 (Iteration 2).
Your working directory is /Users/chinmayk/Projects/Deskdrop/.agents/explorer_m3_r2_1.

Mission:
Investigate and formulate the fix strategy for the disconnect waiter drain defect discovered during Iteration 1 verification.

Failure Report & Context:
- Gate Status: /Users/chinmayk/Projects/Deskdrop/.agents/sub_orch_m3/GATE_STATUS.md
- Challenger 2 Handoff: /Users/chinmayk/Projects/Deskdrop/.agents/challenger_m3_2/handoff.md
- Reproduction Test: deskdrop-core/tests/m3_challenger_stress_test.rs

Defect Details:
1. When `Engine::disconnect_peer(device_id)` is called (or when peer session shuts down), `shutdown_peer_session(device_id)` removes `device_id` from `peer_manager.live`.
2. `disconnect_peer` does NOT drain `shared.remote_file_waiters` or `shared.remote_thumb_waiters`.
3. When the session actor loop exits and calls `mark_disconnected_if_current`, it returns `Ok(None)` (since `self.live` no longer has `device_id`), which skips the session actor's waiter drain block (engine/mod.rs:5975–6018).
4. As a result, `remote_file_waiters` and `remote_thumb_waiters` are not drained on explicit disconnect. RPC queries hang for their full timeout duration (~10s) instead of returning an immediate fast-path error `"Peer disconnected"`.

Tasks:
1. Inspect `deskdrop-core/src/engine/mod.rs` around `disconnect_peer` (lines 1908–1940) and session exit cleanup (lines 5934–6018).
2. Formulate the exact code changes needed in `Engine::disconnect_peer` and/or a helper function to drain both `remote_file_waiters` and `remote_thumb_waiters` for `device_id` on disconnect.
3. Verify how `deskdrop-core/tests/m3_challenger_stress_test.rs` and `remote_files_e2e_test.rs` will validate the fix.
4. Write your analysis and implementation plan to `/Users/chinmayk/Projects/Deskdrop/.agents/explorer_m3_r2_1/analysis.md` and handoff report to `/Users/chinmayk/Projects/Deskdrop/.agents/explorer_m3_r2_1/handoff.md`.
5. Notify parent (Sub-Orchestrator) via send_message when done.
