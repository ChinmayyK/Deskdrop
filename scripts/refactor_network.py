import re
import sys

with open("deskdrop-core/src/network.rs", "r") as f:
    content = f.read()

# Replace imports
content = content.replace("use crate::crypto::{EphemeralKeypair, SessionKey};", "use crate::crypto::{NoiseTransport, NOISE_PARAMS};")
content = content.replace("use crate::protocol::{AppMessage, EcdhFrame, PROTOCOL_VERSION};", "use crate::protocol::{AppMessage, PROTOCOL_VERSION};")

# Replace SessionKey with NoiseTransport
content = content.replace("SessionKey", "NoiseTransport")

# Rewrite send_encrypted and recv_encrypted to use NoiseTransport's encrypt/decrypt which allocate
# send_encrypted:
send_encrypted_old = """async fn send_encrypted(
    stream: &mut (impl AsyncWriteExt + Unpin),
    session: &mut NoiseTransport,
    msg: &AppMessage,
) -> Result<()> {
    let mut buffer = bincode::serialize(msg).context("serializing AppMessage")?;
    let nonce = session
        .encrypt_in_place(&mut buffer)
        .context("encrypting")?;
    let len = (12 + buffer.len()) as u32;
    stream.write_all(&len.to_le_bytes()).await?;
    stream.write_all(nonce.as_slice()).await?;
    stream.write_all(&buffer).await?;
    stream.flush().await?;
    Ok(())
}"""

send_encrypted_new = """async fn send_encrypted(
    stream: &mut (impl AsyncWriteExt + Unpin),
    session: &mut NoiseTransport,
    msg: &AppMessage,
) -> Result<()> {
    let buffer = bincode::serialize(msg).context("serializing AppMessage")?;
    let ct = session.encrypt(&buffer).context("encrypting")?;
    let len = ct.len() as u32;
    stream.write_all(&len.to_le_bytes()).await?;
    stream.write_all(&ct).await?;
    stream.flush().await?;
    Ok(())
}"""
content = content.replace(send_encrypted_old, send_encrypted_new)

# send_encrypted_no_flush
send_encrypted_no_flush_old = """async fn send_encrypted_no_flush(
    stream: &mut (impl AsyncWriteExt + Unpin),
    session: &mut NoiseTransport,
    msg: &AppMessage,
) -> Result<()> {
    let mut buffer = bincode::serialize(msg).context("serializing AppMessage")?;
    let nonce = session
        .encrypt_in_place(&mut buffer)
        .context("encrypting")?;
    let len = (12 + buffer.len()) as u32;
    stream.write_all(&len.to_le_bytes()).await?;
    stream.write_all(nonce.as_slice()).await?;
    stream.write_all(&buffer).await?;
    Ok(())
}"""

send_encrypted_no_flush_new = """async fn send_encrypted_no_flush(
    stream: &mut (impl AsyncWriteExt + Unpin),
    session: &mut NoiseTransport,
    msg: &AppMessage,
) -> Result<()> {
    let buffer = bincode::serialize(msg).context("serializing AppMessage")?;
    let ct = session.encrypt(&buffer).context("encrypting")?;
    let len = ct.len() as u32;
    stream.write_all(&len.to_le_bytes()).await?;
    stream.write_all(&ct).await?;
    Ok(())
}"""
content = content.replace(send_encrypted_no_flush_old, send_encrypted_no_flush_new)

# recv_encrypted
recv_encrypted_old = """async fn recv_encrypted(
    stream: &mut (impl AsyncReadExt + Unpin),
    session: &mut NoiseTransport,
) -> Result<AppMessage> {
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).await?;
    let len = u32::from_le_bytes(len_buf);
    anyhow::ensure!(len <= MAX_FRAME_SIZE, "encrypted frame too large");

    let mut cipher_buffer = vec![0u8; len as usize];
    stream.read_exact(&mut cipher_buffer).await?;
    session
        .decrypt_in_place(&mut cipher_buffer)
        .context("decrypting")?;
    bincode::deserialize(&cipher_buffer).context("deserializing AppMessage")
}"""

recv_encrypted_new = """async fn recv_encrypted(
    stream: &mut (impl AsyncReadExt + Unpin),
    session: &mut NoiseTransport,
) -> Result<AppMessage> {
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).await?;
    let len = u32::from_le_bytes(len_buf);
    anyhow::ensure!(len <= MAX_FRAME_SIZE, "encrypted frame too large");

    let mut cipher_buffer = vec![0u8; len as usize];
    stream.read_exact(&mut cipher_buffer).await?;
    let pt = session.decrypt(&cipher_buffer).context("decrypting")?;
    bincode::deserialize(&pt).context("deserializing AppMessage")
}"""
content = content.replace(recv_encrypted_old, recv_encrypted_new)

with open("deskdrop-core/src/network.rs", "w") as f:
    f.write(content)

