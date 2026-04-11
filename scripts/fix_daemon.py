import re
import sys

with open("deskdrop-core/src/engine.rs", "r") as f:
    content = f.read()

# Add peer_name to impl Engine
peer_name_fn = """    pub fn connected_peer_count(&self) -> usize {
        self.shared.peer_manager.connected_count()
    }

    pub fn peer_name(&self, device_id: Uuid) -> Option<String> {
        self.shared.peer_manager.get(device_id).map(|p| p.friendly_name)
    }"""

content = content.replace("""    pub fn connected_peer_count(&self) -> usize {
        self.shared.peer_manager.connected_count()
    }""", peer_name_fn)

with open("deskdrop-core/src/engine.rs", "w") as f:
    f.write(content)

with open("deskdrop-core/src/bin/daemon.rs", "r") as f:
    daemon_content = f.read()

daemon_content = daemon_content.replace("state.engine.active_file_transfers().await", "state.engine.active_transfers().await")

with open("deskdrop-core/src/bin/daemon.rs", "w") as f:
    f.write(daemon_content)

