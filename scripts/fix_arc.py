import sys

with open("deskdrop-core/src/engine.rs", "r") as f:
    content = f.read()

# Fix signature
content = content.replace("shared: std::sync::Arc<crate::engine::EngineShared>,", "shared: crate::engine::EngineShared,")

# Fix calls
content = content.replace("read_outbound_chunks(std::sync::Arc::clone(&bg_shared),", "read_outbound_chunks(bg_shared.clone(),")

with open("deskdrop-core/src/engine.rs", "w") as f:
    f.write(content)
