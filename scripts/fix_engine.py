import re
import sys

with open("deskdrop-core/src/engine.rs", "r") as f:
    content = f.read()

# 1. Fix EngineStatus
content = content.replace("#[derive(Debug, Clone, Serialize)]\npub struct EngineStatus", "#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]\npub struct EngineStatus")

# 2. Fix the three occurrences of all_sent logic
old_all_sent = """                    let all_sent = {
                        let mut mgr = bg_shared.file_transfers.lock().await;
                        mgr.get_outbound_mut(&bg_transfer_id)
                            .map(|transfer| transfer.is_all_sent())
                            .unwrap_or(false)
                    };
                    if all_sent {
                        let _ = bg_outbox
                            .send(AppMessage::FileTransferComplete {
                                transfer_id: bg_transfer_id,
                            })
                            .await;
                    }"""

new_all_sent = """                    let final_checksum = {
                        let mut mgr = bg_shared.file_transfers.lock().await;
                        mgr.get_outbound_mut(&bg_transfer_id)
                            .and_then(|transfer| if transfer.is_all_sent() { Some(transfer.finalize_checksum()) } else { None })
                    };
                    if let Some(sha256_checksum) = final_checksum {
                        let _ = bg_outbox
                            .send(AppMessage::FileTransferComplete {
                                transfer_id: bg_transfer_id,
                                sha256_checksum,
                            })
                            .await;
                    }"""

content = content.replace(old_all_sent, new_all_sent)

# 3. Fix pattern matching in rx task
old_rx_match = "Ok(AppMessage::FileTransferComplete { transfer_id }) => {"
new_rx_match = "Ok(AppMessage::FileTransferComplete { transfer_id, sha256_checksum }) => {"
content = content.replace(old_rx_match, new_rx_match)

with open("deskdrop-core/src/engine.rs", "w") as f:
    f.write(content)

