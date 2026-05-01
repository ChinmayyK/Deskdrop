import re

with open("deskdrop-core/src/engine.rs", "r") as f:
    content = f.read()

# Replace the incorrect addrs logic
content = content.replace(
    "let addrs = peer.endpoints.into_iter().collect::<Vec<_>>();",
    "let addrs = peer.ips.into_iter().map(|ip| std::net::SocketAddr::new(ip, peer.port)).collect::<Vec<_>>();"
)

with open("deskdrop-core/src/engine.rs", "w") as f:
    f.write(content)
