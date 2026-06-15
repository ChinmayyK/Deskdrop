import re

with open("deskdrop-core/src/network.rs", "r") as f:
    content = f.read()

# Replace HandshakeResult
content = content.replace("pub session: NoiseTransport,", "pub session: std::sync::Arc<std::sync::Mutex<NoiseTransport>>,")

# Update handshake returns
content = content.replace("let mut session = NoiseTransport { transport };", "let session = std::sync::Arc::new(std::sync::Mutex::new(NoiseTransport { transport }));")
# Wait, for handshake_initiator, recv_encrypted needs &session, but session is now Arc<Mutex>.
# We can change recv_encrypted to take `session: &std::sync::Arc<std::sync::Mutex<NoiseTransport>>`

# Update send_encrypted signature and body
content = re.sub(
    r"async fn send_encrypted\(\s*stream:\s*&mut\s*\(impl AsyncWriteExt \+ Unpin\),\s*session:\s*&mut NoiseTransport,\s*msg:\s*&AppMessage,\s*\)\s*->\s*Result<\(\)>\s*\{",
    """async fn send_encrypted(
    stream: &mut (impl AsyncWriteExt + Unpin),
    session: &std::sync::Arc<std::sync::Mutex<NoiseTransport>>,
    msg: &AppMessage,
) -> Result<()> {""",
    content
)

content = content.replace("let ct = session.encrypt(&buffer).context(\"encrypting\")?;", "let ct = session.lock().unwrap().encrypt(&buffer).context(\"encrypting\")?;")

# Update send_encrypted_no_flush
content = re.sub(
    r"async fn send_encrypted_no_flush\(\s*stream:\s*&mut\s*\(impl AsyncWriteExt \+ Unpin\),\s*session:\s*&mut NoiseTransport,\s*msg:\s*&AppMessage,\s*\)\s*->\s*Result<\(\)>\s*\{",
    """async fn send_encrypted_no_flush(
    stream: &mut (impl AsyncWriteExt + Unpin),
    session: &std::sync::Arc<std::sync::Mutex<NoiseTransport>>,
    msg: &AppMessage,
) -> Result<()> {""",
    content
)

# Update recv_encrypted
content = re.sub(
    r"async fn recv_encrypted\(\s*stream:\s*&mut\s*\(impl AsyncReadExt \+ Unpin\),\s*session:\s*&mut NoiseTransport,\s*\)\s*->\s*Result<AppMessage>\s*\{",
    """async fn recv_encrypted(
    stream: &mut (impl AsyncReadExt + Unpin),
    session: &std::sync::Arc<std::sync::Mutex<NoiseTransport>>,
) -> Result<AppMessage> {""",
    content
)

content = content.replace("let pt = session.decrypt(&cipher_buffer).context(\"decrypting\")?;", "let pt = session.lock().unwrap().decrypt(&cipher_buffer).context(\"decrypting\")?;")

# Update PeerSession structures
content = content.replace("pub session: NoiseTransport,", "pub session: std::sync::Arc<std::sync::Mutex<NoiseTransport>>,")

# Remove mut from session passing
content = content.replace("&mut session", "&session")

with open("deskdrop-core/src/network.rs", "w") as f:
    f.write(content)
