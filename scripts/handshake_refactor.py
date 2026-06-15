import re

with open("deskdrop-core/src/network.rs", "r") as f:
    content = f.read()

# 1. Update HandshakeResult struct
handshake_result_old = """pub struct HandshakeResult {
    pub session: NoiseTransport,
    pub pin: crate::pairing::PairingPin,
    pub peer_device_id: Uuid,
    pub peer_device_name: String,
    pub peer_identity_pubkey_bytes: [u8; 32],
    pub peer_already_trusted: bool,
}"""
# No change needed actually.

# 2. Rewrite handshake_initiator
initiator_start = content.find("pub async fn handshake_initiator(")
initiator_end = content.find("pub async fn handshake_responder<", initiator_start)

new_initiator = """pub async fn handshake_initiator(
    stream: &mut TcpStream,
    my_device_id: Uuid,
    my_device_name: &str,
    my_identity_key: std::sync::Arc<std::sync::RwLock<crate::identity::IdentityKey>>,
) -> Result<HandshakeResult> {
    let my_static = my_identity_key.read().unwrap().secret_bytes();
    let builder = snow::Builder::new(NOISE_PARAMS.parse().unwrap());
    let mut noise = builder.local_private_key(&my_static).build_initiator().context("noise build initiator")?;

    let mut buf = vec![0u8; 65535];
    
    // Msg 1: -> e
    let len = noise.write_message(&[], &mut buf).context("noise write 1")?;
    let len_u32 = len as u32;
    stream.write_all(&len_u32.to_le_bytes()).await?;
    stream.write_all(&buf[..len]).await?;
    stream.flush().await?;

    // Msg 2: <- e, ee, s, es
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).await?;
    let len_u32 = u32::from_le_bytes(len_buf);
    anyhow::ensure!(len_u32 <= 65535, "noise message too large");
    let mut in_buf = vec![0u8; len_u32 as usize];
    stream.read_exact(&mut in_buf).await?;
    
    let mut payload = vec![0u8; 65535];
    noise.read_message(&in_buf, &mut payload).context("noise read 2")?;

    // Msg 3: -> s, se + Hello
    let hello = AppMessage::Hello {
        device_id: my_device_id,
        device_name: my_device_name.to_string(),
        metadata_json: None,
        fcm_token: None,
    };
    let hello_bytes = bincode::serialize(&hello).context("serialize hello")?;
    let len = noise.write_message(&hello_bytes, &mut buf).context("noise write 3")?;
    let len_u32 = len as u32;
    stream.write_all(&len_u32.to_le_bytes()).await?;
    stream.write_all(&buf[..len]).await?;
    stream.flush().await?;

    let handshake_hash = noise.get_handshake_hash().to_vec();
    
    let transport = noise.into_transport_mode().context("into transport")?;
    let mut session = NoiseTransport { transport };

    let ack_msg = tokio::time::timeout(Duration::from_secs(5), recv_encrypted(stream, &mut session))
        .await.context("timeout waiting for HelloAck")?.context("receiving HelloAck")?;

    let AppMessage::HelloAck {
        device_id,
        device_name,
        trusted,
        ..
    } = ack_msg else {
        anyhow::bail!("expected HelloAck");
    };

    let peer_static_pubkey = session.transport.get_remote_static().context("missing remote static")?;
    let mut peer_pubkey_bytes = [0u8; 32];
    peer_pubkey_bytes.copy_from_slice(peer_static_pubkey);

    let pin = crate::pairing::derive_pin(&handshake_hash, &handshake_hash);

    info!("Handshake complete with '{}' ({})", device_name, device_id);

    Ok(HandshakeResult {
        session,
        pin,
        peer_device_id: device_id,
        peer_device_name: device_name,
        peer_identity_pubkey_bytes: peer_pubkey_bytes,
        peer_already_trusted: trusted,
    })
}

"""

# 3. Rewrite handshake_responder
responder_start = content.find("pub async fn handshake_responder<", initiator_end)
responder_end = content.find("fn xor_nonces(", responder_start)

new_responder = """pub async fn handshake_responder<F, Fut>(
    stream: &mut TcpStream,
    my_device_id: Uuid,
    my_device_name: String,
    my_identity_key: std::sync::Arc<std::sync::RwLock<crate::identity::IdentityKey>>,
    check_trust: F,
) -> Result<HandshakeResult>
where
    F: FnOnce(Uuid, [u8; 32]) -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    let my_static = my_identity_key.read().unwrap().secret_bytes();
    let builder = snow::Builder::new(NOISE_PARAMS.parse().unwrap());
    let mut noise = builder.local_private_key(&my_static).build_responder().context("noise build responder")?;

    let mut buf = vec![0u8; 65535];

    // Msg 1: <- e
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).await?;
    let len_u32 = u32::from_le_bytes(len_buf);
    anyhow::ensure!(len_u32 <= 65535, "noise message too large");
    let mut in_buf = vec![0u8; len_u32 as usize];
    stream.read_exact(&mut in_buf).await?;
    
    let mut payload = vec![0u8; 65535];
    noise.read_message(&in_buf, &mut payload).context("noise read 1")?;

    // Msg 2: -> e, ee, s, es
    let len = noise.write_message(&[], &mut buf).context("noise write 2")?;
    let len_u32 = len as u32;
    stream.write_all(&len_u32.to_le_bytes()).await?;
    stream.write_all(&buf[..len]).await?;
    stream.flush().await?;

    // Msg 3: <- s, se + Hello
    stream.read_exact(&mut len_buf).await?;
    let len_u32 = u32::from_le_bytes(len_buf);
    anyhow::ensure!(len_u32 <= 65535, "noise message too large");
    let mut in_buf = vec![0u8; len_u32 as usize];
    stream.read_exact(&mut in_buf).await?;
    
    let payload_len = noise.read_message(&in_buf, &mut payload).context("noise read 3")?;
    let hello_bytes = &payload[..payload_len];
    
    let hello_msg: AppMessage = bincode::deserialize(hello_bytes).context("deserialize Hello")?;
    let AppMessage::Hello {
        device_id,
        device_name,
        ..
    } = hello_msg else {
        anyhow::bail!("expected Hello");
    };

    let handshake_hash = noise.get_handshake_hash().to_vec();

    let peer_static_pubkey = noise.get_remote_static().context("missing remote static")?;
    let mut peer_pubkey_bytes = [0u8; 32];
    peer_pubkey_bytes.copy_from_slice(peer_static_pubkey);

    let transport = noise.into_transport_mode().context("into transport")?;
    let mut session = NoiseTransport { transport };

    let peer_is_trusted = check_trust(device_id, peer_pubkey_bytes).await;
    
    let ack = AppMessage::HelloAck {
        device_id: my_device_id,
        device_name: my_device_name.to_string(),
        trusted: peer_is_trusted,
        metadata_json: None,
        fcm_token: None,
    };

    send_encrypted(stream, &mut session, &ack).await.context("sending HelloAck")?;

    let pin = crate::pairing::derive_pin(&handshake_hash, &handshake_hash);

    Ok(HandshakeResult {
        session,
        pin,
        peer_device_id: device_id,
        peer_device_name: device_name,
        peer_identity_pubkey_bytes: peer_pubkey_bytes,
        peer_already_trusted: peer_is_trusted,
    })
}

"""

new_content = content[:initiator_start] + new_initiator + new_responder + content[responder_end:]

with open("deskdrop-core/src/network.rs", "w") as f:
    f.write(new_content)
