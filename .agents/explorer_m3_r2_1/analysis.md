# Analysis Report: Disconnect Waiter Drain Defect & Fix Strategy

## Executive Summary

During Milestone M3 Iteration 1 verification, Challengers identified a defect where explicit peer disconnections (via `Engine::disconnect_peer(device_id)`) fail to drain pending RPC waiters in `remote_file_waiters` and `remote_thumb_waiters`. Consequently, in-flight RPC queries hang for their full timeout duration (~10 seconds) instead of returning an immediate fast-path error (`"Peer disconnected"`).

This report details the root cause analysis, evidence chain, exact proposed code modifications in `deskdrop-core/src/engine/mod.rs`, and the verification methodology.

---

## 1. Root Cause Analysis & Evidence Chain

### 1.1 Root Cause Explanation

1. **`disconnect_peer` Bypass**:
   When `Engine::disconnect_peer(device_id)` is called (or when `forget_device` is invoked):
   - It calls `shutdown_peer_session(device_id)`, which immediately removes `device_id` from `peer_manager.live`.
   - `disconnect_peer` does **NOT** drain `shared.remote_file_waiters` or `shared.remote_thumb_waiters`.

2. **Session Actor Cleanup Bypass**:
   - `disconnect_peer` sends a `SessionShutdown` signal to the peer's session actor loop.
   - The session actor loop terminates and calls `shared.peer_manager.mark_disconnected_if_current(peer_id, session_id, reason)`.
   - `mark_disconnected_if_current` checks `self.live.get(&device_id)`. Because `shutdown_peer_session` already removed `device_id` from `self.live`, `mark_disconnected_if_current` returns `Ok(None)`.
   - The `match` block in the session actor cleanup (lines 5934–6018 in `deskdrop-core/src/engine/mod.rs`) matches `Ok(None)` and bypasses the `Ok(Some(connected_at))` branch.
   - The waiter drain logic (lines 5975–6018) resides exclusively inside the `Ok(Some(connected_at))` branch.

3. **Consequence**:
   - Waiters in `shared.remote_file_waiters` and `shared.remote_thumb_waiters` remain orphaned in the HashMap until their dynamic timeout (e.g. 10s) expires.
   - Empirical test `test_reproduce_disconnect_peer_waiter_leak` in `deskdrop-core/tests/m3_challenger_stress_test.rs` panicked because the query took 9.95s to fail rather than failing immediately (< 500ms).

### 1.2 Evidence Chain

- **Observation 1**: Baseline test run `cargo test -p deskdrop-core --test m3_challenger_stress_test` fails with:
  `Query took 9.950199917s to fail after explicit disconnect_peer. Fast-path disconnect failed!`
- **Observation 2**: Code inspection of `Engine::disconnect_peer` (`deskdrop-core/src/engine/mod.rs:1908–1940`) shows no calls to drain waiter maps.
- **Observation 3**: Code inspection of `mark_disconnected_if_current` (`deskdrop-core/src/peer_manager.rs:574–593`) shows it returns `Ok(None)` when `device_id` is missing from `self.live`.
- **Observation 4**: Code inspection of session actor cleanup (`deskdrop-core/src/engine/mod.rs:5934–6018`) confirms waiter draining only occurs under `Ok(Some(connected_at))`.

---

## 2. Recommended Fix Strategy & Code Plan

To fix this defect robustly and prevent future race conditions, we propose:

1. **Standalone Helper Function**: Define `drain_remote_waiters(shared: &EngineShared, peer_id: Uuid)` in `deskdrop-core/src/engine/mod.rs`.
2. **Explicit Disconnect Fast-Path**: Call `drain_remote_waiters(&self.shared, device_id).await` directly inside `Engine::disconnect_peer` and `Engine::forget_device`.
3. **Session Actor Cleanup Guard**: Call `drain_remote_waiters(&shared, peer_id).await` in `Ok(Some)`, `Ok(None)`, and `Err(_)` branches of session actor disconnect cleanup.

### 2.1 Proposed Code Modifications

#### A. Helper Function `drain_remote_waiters`
Add to `deskdrop-core/src/engine/mod.rs`:

```rust
/// Drain pending remote file waiters and remote thumbnail waiters for a given peer
/// and notify oneshot receivers with an immediate fast-path error ("Peer disconnected").
pub(crate) async fn drain_remote_waiters(shared: &EngineShared, peer_id: Uuid) {
    let waiters_to_notify: Vec<tokio::sync::oneshot::Sender<RemoteFilesResult>> = {
        let mut waiters = shared.remote_file_waiters.lock().await;
        let matching_keys: Vec<Uuid> = waiters
            .iter()
            .filter_map(|(req_id, (target, _))| {
                if *target == peer_id {
                    Some(*req_id)
                } else {
                    None
                }
            })
            .collect();
        matching_keys
            .into_iter()
            .filter_map(|req_id| waiters.remove(&req_id).map(|(_, tx)| tx))
            .collect()
    };

    for tx in waiters_to_notify {
        let _ = tx.send(RemoteFilesResult {
            summary: None,
            files: Vec::new(),
            total_matching: 0,
            error: Some("Peer disconnected".to_string()),
        });
    }

    let thumb_waiters_to_notify: Vec<tokio::sync::oneshot::Sender<RemoteThumbnailResult>> = {
        let mut thumb_waiters = shared.remote_thumb_waiters.lock().await;
        let matching_keys: Vec<Uuid> = thumb_waiters
            .iter()
            .filter_map(|(req_id, (target, _))| {
                if *target == peer_id {
                    Some(*req_id)
                } else {
                    None
                }
            })
            .collect();
        matching_keys
            .into_iter()
            .filter_map(|req_id| thumb_waiters.remove(&req_id).map(|(_, tx)| tx))
            .collect()
    };

    for tx in thumb_waiters_to_notify {
        let _ = tx.send(RemoteThumbnailResult {
            file_id: 0,
            data: Vec::new(),
            error: Some("Peer disconnected".to_string()),
        });
    }
}
```

#### B. `Engine::disconnect_peer` Update
Modify `Engine::disconnect_peer` in `deskdrop-core/src/engine/mod.rs` (lines 1908–1940):

```rust
    pub async fn disconnect_peer(&self, device_id: Uuid) -> Result<bool> {
        let _ = self
            .shared
            .peer_manager
            .set_explicit_disconnect(device_id, true)?;

        // Drain pending RPC waiters immediately on explicit disconnect
        drain_remote_waiters(&self.shared, device_id).await;

        let session = self.shared.peer_manager.shutdown_peer_session(device_id)?;
        if let Some(session) = session {
            if let Some(shutdown_tx) = session.shutdown_tx {
                let _ = shutdown_tx.send(SessionShutdown {
                    reason: "manually disconnected".to_string(),
                    send_bye: true,
                    explicit_disconnect: true,
                });
            }
            let _ = self
                .shared
                .event_tx
                .send(EngineEvent::PeerDisconnected {
                    device_id,
                    device_name: self
                        .shared
                        .peer_manager
                        .get(device_id)
                        .map(|peer| Some(peer.friendly_name))
                        .unwrap_or(None),
                    reason: Some("manually disconnected".into()),
                })
                .await;
            return Ok(true);
        }
        Ok(false)
    }
```

#### C. `Engine::forget_device` Update
Modify `Engine::forget_device` in `deskdrop-core/src/engine/mod.rs` (lines 2569–2586):

```rust
    pub async fn forget_device(&self, device_id: Uuid) -> Result<bool> {
        let found = self.shared.peer_manager.forget_device(device_id)?;
        if found {
            let _ = self.shared.trust.lock().await.revoke_peer(device_id);
            // Drain pending RPC waiters for forgotten device
            drain_remote_waiters(&self.shared, device_id).await;
            // Disconnect the session — device will not auto-reconnect
            let session = self.shared.peer_manager.shutdown_peer_session(device_id)?;
            if let Some(session) = session {
                if let Some(shutdown_tx) = session.shutdown_tx {
                    let _ = shutdown_tx.send(crate::peer_manager::SessionShutdown {
                        reason: "device forgotten".to_string(),
                        send_bye: true,
                        explicit_disconnect: false,
                    });
                }
            }
        }
        Ok(found)
    }
```

#### D. Session Actor Cleanup Update
Modify lines 5975–6018 in `deskdrop-core/src/engine/mod.rs`:

```rust
                // Drain pending remote file waiters and notify oneshot receivers with error fast-path
                drain_remote_waiters(&shared, peer_id).await;
```
And add to `Ok(None)` and `Err(_)` branches:

```rust
            Ok(None) => {
                // Ensure waiters are drained even if peer was already removed from live sessions
                drain_remote_waiters(&shared, peer_id).await;
            }
            Err(_) => {
                drain_remote_waiters(&shared, peer_id).await;
            }
```

---

## 3. Verification Plan

### 3.1 Verification Commands
1. **Stress/Challenger Test**:
   ```bash
   cargo test -p deskdrop-core --test m3_challenger_stress_test
   ```
   *Expected Result*: `test_reproduce_disconnect_peer_waiter_leak` passes in < 50ms returning `Err("Peer disconnected")`.

2. **Integration Suite**:
   ```bash
   cargo test -p deskdrop-core --test remote_files_e2e_test
   ```
   *Expected Result*: All 25 tests pass without regressions.

3. **Full Workspace Check**:
   ```bash
   cargo test -p deskdrop-core
   ```
   *Expected Result*: All tests pass clean.
