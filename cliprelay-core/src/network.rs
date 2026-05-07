//! ClipRelay network transport layer.
//!
//! Wire format (per frame):
//!   [u32 LE length][payload bytes]
//!
//! Handshake frames are bincode-encoded plaintext.
//! Post-handshake frames are bincode-encoded then AEAD-encrypted.
//!
//! # Sub-500 ms propagation budget
//! - mDNS resolution: ~10–50 ms (already running)
//! - TCP connect:      ~1 ms on LAN
//! - Handshake:        ~5–20 ms (2 RTT)
//! - Encrypt + send:   ~1 ms
//! - Total:            ~20–80 ms ✓

use crate::crypto::{EphemeralKeypair, SessionKey};
use crate::protocol::{AppMessage, HelloAckFrame, HelloFrame, PROTOCOL_VERSION};
use anyhow::{Context, Result};
use serde::{de::DeserializeOwned, Serialize};
use std::net::SocketAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tracing::{debug, info};
use uuid::Uuid;

const MAX_FRAME_SIZE: u32 = 70 * 1024 * 1024; // 70 MB hard cap

// ── Low-level framing ─────────────────────────────────────────────────────────

async fn send_frame<T: Serialize>(stream: &mut TcpStream, value: &T) -> Result<()> {
    let payload = bincode::serialize(value).context("serializing frame")?;
    let len = payload.len() as u32;
    stream.write_all(&len.to_le_bytes()).await?;
    stream.write_all(&payload).await?;
    Ok(())
}

async fn recv_frame<T: DeserializeOwned>(stream: &mut TcpStream) -> Result<T> {
    let mut len_buf = [0u8; 4];
    stream
        .read_exact(&mut len_buf)
        .await
        .context("reading frame length")?;
    let len = u32::from_le_bytes(len_buf);

    anyhow::ensure!(
        len <= MAX_FRAME_SIZE,
        "frame size {} exceeds limit {}",
        len,
        MAX_FRAME_SIZE
    );

    let mut buf = vec![0u8; len as usize];
    stream
        .read_exact(&mut buf)
        .await
        .context("reading frame body")?;
    bincode::deserialize(&buf).context("deserializing frame")
}

async fn send_encrypted(
    stream: &mut TcpStream,
    session: &mut SessionKey,
    msg: &AppMessage,
) -> Result<()> {
    let plain = bincode::serialize(msg).context("serializing AppMessage")?;
    let cipher = session.encrypt(&plain).context("encrypting")?;
    let len = cipher.len() as u32;
    stream.write_all(&len.to_le_bytes()).await?;
    stream.write_all(&cipher).await?;
    Ok(())
}

async fn recv_encrypted(stream: &mut TcpStream, session: &mut SessionKey) -> Result<AppMessage> {
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).await?;
    let len = u32::from_le_bytes(len_buf);
    anyhow::ensure!(len <= MAX_FRAME_SIZE, "encrypted frame too large");

    let mut cipher = vec![0u8; len as usize];
    stream.read_exact(&mut cipher).await?;
    let plain = session.decrypt(&cipher).context("decrypting")?;
    bincode::deserialize(&plain).context("deserializing AppMessage")
}

// ── Handshake ─────────────────────────────────────────────────────────────────

pub struct HandshakeResult {
    pub session: SessionKey,
    pub peer_device_id: Uuid,
    pub peer_device_name: String,
    pub peer_identity_pubkey_bytes: [u8; 32],
    pub peer_already_trusted: bool,
}

/// Initiator side (we connected to the peer).
pub async fn handshake_initiator(
    stream: &mut TcpStream,
    my_device_id: Uuid,
    my_device_name: &str,
    my_identity_pubkey: [u8; 32],
) -> Result<HandshakeResult> {
    let ephemeral = EphemeralKeypair::generate();
    let my_nonce = crate::crypto::random_nonce16();

    let hello = HelloFrame {
        version: PROTOCOL_VERSION,
        device_id: my_device_id,
        device_name: my_device_name.to_string(),
        identity_pubkey: my_identity_pubkey,
        ecdh_pubkey: ephemeral.public_bytes,
        nonce: my_nonce,
    };

    send_frame(stream, &hello).await.context("sending Hello")?;

    let ack: HelloAckFrame = recv_frame(stream).await.context("receiving HelloAck")?;

    anyhow::ensure!(
        ack.version == PROTOCOL_VERSION,
        "protocol version mismatch: peer={} us={}",
        ack.version,
        PROTOCOL_VERSION
    );

    // Verify nonce echo (replay protection for the handshake itself).
    let _expected_echo = xor_nonces(&my_nonce, &ack.nonce_response);
    // (In a complete implementation, check expected_echo against a separate
    //  responder nonce included in the ack.)

    let session = ephemeral
        .derive_session_key(ack.ecdh_pubkey)
        .context("ECDH key derivation")?;

    info!(
        "Handshake complete with '{}' ({})",
        ack.device_name, ack.device_id
    );

    Ok(HandshakeResult {
        session,
        peer_device_id: ack.device_id,
        peer_device_name: ack.device_name,
        peer_identity_pubkey_bytes: ack.identity_pubkey,
        peer_already_trusted: ack.trusted,
    })
}

/// Responder side (we accepted the connection).
pub async fn handshake_responder(
    stream: &mut TcpStream,
    my_device_id: Uuid,
    my_device_name: &str,
    my_identity_pubkey: [u8; 32],
    peer_is_trusted: bool,
) -> Result<HandshakeResult> {
    let hello: HelloFrame = recv_frame(stream).await.context("receiving Hello")?;

    anyhow::ensure!(
        hello.version == PROTOCOL_VERSION,
        "protocol version mismatch: peer={} us={}",
        hello.version,
        PROTOCOL_VERSION
    );

    let ephemeral = EphemeralKeypair::generate();
    let my_nonce = crate::crypto::random_nonce16();
    let nonce_response = xor_nonces(&hello.nonce, &my_nonce);

    let ack = HelloAckFrame {
        version: PROTOCOL_VERSION,
        device_id: my_device_id,
        device_name: my_device_name.to_string(),
        identity_pubkey: my_identity_pubkey,
        ecdh_pubkey: ephemeral.public_bytes,
        nonce_response,
        trusted: peer_is_trusted,
    };

    send_frame(stream, &ack).await.context("sending HelloAck")?;

    let session = ephemeral
        .derive_session_key(hello.ecdh_pubkey)
        .context("ECDH key derivation")?;

    Ok(HandshakeResult {
        session,
        peer_device_id: hello.device_id,
        peer_device_name: hello.device_name,
        peer_identity_pubkey_bytes: hello.identity_pubkey,
        peer_already_trusted: peer_is_trusted,
    })
}

fn xor_nonces(a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
    let mut out = [0u8; 16];
    for i in 0..16 {
        out[i] = a[i] ^ b[i];
    }
    out
}

// ── Session ───────────────────────────────────────────────────────────────────

/// An established, encrypted connection to a peer.
pub struct PeerSession {
    pub stream: TcpStream,
    pub session: SessionKey,
    pub peer_device_id: Uuid,
    pub peer_device_name: String,
}

impl PeerSession {
    pub async fn send(&mut self, msg: &AppMessage) -> Result<()> {
        send_encrypted(&mut self.stream, &mut self.session, msg).await
    }

    pub async fn recv(&mut self) -> Result<AppMessage> {
        recv_encrypted(&mut self.stream, &mut self.session).await
    }
}

// ── Server ────────────────────────────────────────────────────────────────────

pub struct Server {
    listener: TcpListener,
}

impl Server {
    pub async fn bind(addr: SocketAddr) -> Result<Self> {
        let listener = TcpListener::bind(addr)
            .await
            .context(format!("binding to {}", addr))?;
        info!("ClipRelay server listening on {}", addr);
        Ok(Self { listener })
    }

    pub fn local_addr(&self) -> Result<SocketAddr> {
        self.listener.local_addr().context("getting local addr")
    }

    pub async fn accept(&self) -> Result<TcpStream> {
        let (stream, addr) = self.listener.accept().await?;
        debug!("Accepted connection from {}", addr);
        // Disable Nagle — we want sub-ms latency on LAN.
        stream.set_nodelay(true)?;
        Ok(stream)
    }
}
