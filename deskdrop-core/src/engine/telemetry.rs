use super::*;

impl crate::engine::Engine {
    /// Push a phone call state change to all connected, trusted peers.
    /// Called by the Android JNI layer when PhoneStateListener fires.
    pub async fn push_call_state(&self, state: String, number: String, contact_name: String) {
        let msg = AppMessage::CallStateUpdate {
            state,
            number,
            contact_name,
            origin_device: self.shared.config.device_id,
            origin_device_name: self.shared.config.device_name.clone(),
        };

        let peers = self.shared.peer_manager.all_connected_senders();
        for (peer_id, tx) in peers {
            let Some(peer) = self.shared.peer_manager.get(peer_id) else {
                continue;
            };
            if !peer.trusted {
                continue;
            }
            let _ = tx.send(msg.clone()).await;
        }
    }

    /// Send an accept/decline call action to a specific Android peer.
    /// Called by the macOS IPC layer when the user taps Accept or Decline.
    pub async fn send_call_action(&self, action: String, target_device: Uuid) {
        tracing::info!(
            "send_call_action: action={}, target_device={}",
            action,
            target_device
        );
        let msg = AppMessage::CallAction {
            action,
            origin_device: self.shared.config.device_id,
        };

        let peers = self.shared.peer_manager.all_connected_senders();
        tracing::info!(
            "send_call_action: all connected peers count={}",
            peers.len()
        );
        for (peer_id, tx) in peers {
            tracing::info!("send_call_action: checking peer_id={}", peer_id);
            if peer_id != target_device {
                tracing::info!(
                    "send_call_action: peer_id mismatch (expected {}, got {})",
                    target_device,
                    peer_id
                );
                continue;
            }
            tracing::info!(
                "send_call_action: peer MATCHED! Sending call action message over socket..."
            );
            let _ = tx.send(msg.clone()).await;
        }
    }

    /// Get the current active phone call state, if any.
    /// Returns None when no call is in progress.
    pub async fn active_call(&self) -> Option<ActiveCallState> {
        self.shared.active_call.lock().await.clone()
    }

    /// Push this device's battery status to all connected trusted peers.
    pub async fn push_battery_status(&self, level: u8, charging: bool) {
        *self.shared.local_battery.lock().await = Some((level, charging));
        let msg = AppMessage::BatteryStatus {
            level,
            charging,
            origin_device: self.shared.config.device_id,
            origin_device_name: self.shared.config.device_name.clone(),
        };

        let peers = self.shared.peer_manager.all_trusted_senders();
        for (peer_id, tx) in peers {
            if self.is_trusted(peer_id).await {
                let _ = tx.send(msg.clone()).await;
            }
        }
    }

    /// Push this device's network status to all connected trusted peers.
    pub async fn push_network_status(&self, network_type: String) {
        *self.shared.local_network.lock().await = Some(network_type.clone());
        let msg = AppMessage::NetworkStatus {
            network_type,
            origin_device: self.shared.config.device_id,
            origin_device_name: self.shared.config.device_name.clone(),
        };

        let peers = self.shared.peer_manager.all_trusted_senders();
        for (peer_id, tx) in peers {
            if self.is_trusted(peer_id).await {
                let _ = tx.send(msg.clone()).await;
            }
        }
    }

    /// Relay a push notification to all connected, trusted peers.
    pub async fn push_notification(
        &self,
        id: String,
        package: String,
        title: String,
        text: String,
    ) {
        let msg = AppMessage::NotificationRelay {
            id,
            package,
            title,
            text,
            origin_device: self.shared.config.device_id,
            origin_device_name: self.shared.config.device_name.clone(),
        };

        let peers = self.shared.peer_manager.all_connected_senders();
        for (peer_id, tx) in peers {
            let Some(peer) = self.shared.peer_manager.get(peer_id) else {
                continue;
            };
            if !peer.trusted {
                continue;
            }
            let _ = tx.send(msg.clone()).await;
        }
    }

    pub async fn push_camera_frame(&self, data: Vec<u8>) {
        let msg = AppMessage::CameraFrame {
            origin_device: self.shared.config.device_id,
            data,
        };
        let peers = self.shared.peer_manager.all_connected_senders();
        for (peer_id, tx) in peers {
            let Some(peer) = self.shared.peer_manager.get(peer_id) else {
                continue;
            };
            if !peer.trusted {
                continue;
            }
            let _ = tx.send(msg.clone()).await;
        }
    }

    pub async fn stop_camera_stream(&self) {
        let msg = AppMessage::CameraStreamStop {
            origin_device: self.shared.config.device_id,
        };
        let peers = self.shared.peer_manager.all_connected_senders();
        for (peer_id, tx) in peers {
            let Some(peer) = self.shared.peer_manager.get(peer_id) else {
                continue;
            };
            if !peer.trusted {
                continue;
            }
            let _ = tx.send(msg.clone()).await;
        }
    }

    /// Get battery states for all peers that have reported their level.
    pub async fn peer_batteries(&self) -> Vec<PeerBatteryState> {
        self.shared
            .peer_batteries
            .lock()
            .await
            .values()
            .cloned()
            .collect()
    }

    /// Get network states for all peers that have reported their network.
    pub async fn peer_networks(&self) -> Vec<PeerNetworkState> {
        self.shared
            .peer_networks
            .lock()
            .await
            .values()
            .cloned()
            .collect()
    }

    pub async fn push_storage_status(
        &self,
        images_bytes: u64,
        videos_bytes: u64,
        apps_bytes: u64,
        free_bytes: u64,
        total_bytes: u64,
    ) {
        *self.shared.local_storage.lock().await = Some((
            images_bytes,
            videos_bytes,
            apps_bytes,
            free_bytes,
            total_bytes,
        ));

        let msg = AppMessage::StorageStatus {
            images_bytes,
            videos_bytes,
            apps_bytes,
            free_bytes,
            total_bytes,
            origin_device: self.shared.config.device_id,
            origin_device_name: self.shared.config.device_name.clone(),
        };

        let peers = self.shared.peer_manager.all_trusted_senders();
        for (peer_id, tx) in peers {
            if self.is_trusted(peer_id).await {
                let _ = tx.send(msg.clone()).await;
            }
        }
    }

    /// Get storage states for all peers that have reported their storage.
    pub async fn peer_storages(&self) -> Vec<PeerStorageState> {
        self.shared
            .peer_storage
            .lock()
            .await
            .values()
            .cloned()
            .collect()
    }
}
