import re
import sys

with open("deskdrop-core/src/engine.rs", "r") as f:
    content = f.read()

# 1. Fix Deserialize derive
content = content.replace("#[derive(Debug, Clone, Serialize, Deserialize)]", "#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]")

# 2. Fix bg_shared.clone()
content = content.replace("read_outbound_chunks(bg_shared.clone(),", "read_outbound_chunks(std::sync::Arc::clone(&bg_shared),")

# 3. Remove FileTransferMessage from imports
content = content.replace("use crate::file_transfer::{default_save_dir, FileTransferManager, FileTransferMessage};", "use crate::file_transfer::{default_save_dir, FileTransferManager};")

with open("deskdrop-core/src/engine.rs", "w") as f:
    f.write(content)
