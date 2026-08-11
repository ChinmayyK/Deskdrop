# Implementation Changes Report — Worker 2 (M3 R2)

## Target File
- `deskdrop-core/src/engine/mod.rs`

## Modifications Made

1. **Implemented Async Helper `drain_remote_waiters(shared: &EngineShared, peer_id: Uuid)`**:
   - Location: `deskdrop-core/src/engine/mod.rs:6188–6243`
   - Purpose: Atomically extracts and drains all waiters from `shared.remote_file_waiters` and `shared.remote_thumb_waiters` matching `peer_id`.
   - Error Dispatch: Sends fast-path oneshot errors:
     - `RemoteFilesResult { error: Some("Peer disconnected".to_string()), ... }`
     - `RemoteThumbnailResult { error: Some("Peer disconnected".to_string()), ... }`

2. **Updated `Engine::disconnect_peer`**:
   - Location: `deskdrop-core/src/engine/mod.rs:1908–1940`
   - Change: Added `drain_remote_waiters(&self.shared, device_id).await;` immediately prior to session shutdown, ensuring explicit user/API disconnect requests immediately drain pending waiters.

3. **Updated `Engine::forget_device`**:
   - Location: `deskdrop-core/src/engine/mod.rs:2569–2586`
   - Change: Added `drain_remote_waiters(&self.shared, device_id).await;` when a device is forgotten and its trust is revoked.

4. **Updated Session Actor Disconnect Cleanup**:
   - Location: `deskdrop-core/src/engine/mod.rs:5974–6095`
   - Change: Refactored disconnect cleanup to call `drain_remote_waiters(&shared, peer_id).await;` across all session termination branches (`Ok(Some)`, `Ok(None)`, and `Err(_)`).
   - Rationale: Resolves race condition where `shutdown_peer_session` removes the peer from `peer_manager.live` prior to session actor exit, causing `mark_disconnected_if_current` to return `Ok(None)`. With this change, waiters are drained unconditionally.

## Verification Results
- `cargo check -p deskdrop-core`: PASS
- `cargo test -p deskdrop-core --test m3_challenger_stress_test`: PASS (2 passed; `test_reproduce_disconnect_peer_waiter_leak` resolved in ~1.57ms returning `Err("Peer disconnected")`)
- `cargo test -p deskdrop-core --test remote_files_e2e_test`: PASS (25 passed, 0 failed)
- `python3 scripts/test_remote_files_ipc.py`: PASS (3 passed in 0.177s)
