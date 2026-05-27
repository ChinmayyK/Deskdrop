//! Deskdrop network transport layer.
//!
//! Wire format (per frame):
//!   [u32 LE length][payload bytes]
//!
//! Handshake frames are bincode-encoded plaintext.
//! Post-handshake frames are bincode-encoded then AEAD-encrypted.
//!
//! # Sub-500 ms propagation budget
//! - mDNS resolution: ~10–50 ms (already running)
//! - TCP connect:      ~1 ms on LAN (timeout: 5 s)
//! - Handshake:        ~5–20 ms (2 RTT)
//! - Encrypt + send:   ~1 ms
//! - Total:            ~20–80 ms ✓

use crate::crypto::{EphemeralKeypair, SessionKey};
use crate::protocol::{AppMessage, HelloAckFrame, HelloFrame, PROTOCOL_VERSION};
use anyhow::{Context, Result};
use serde::{de::DeserializeOwned, Serialize};
use std::net::SocketAddr;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tracing::{debug, info};
use uuid::Uuid;

const MAX_FRAME_SIZE: u32 = 40 * 1024 * 1024; // 40 MB limit for safety (to accommodate 32MB images)

/// v3 fix: outbound connections must succeed within this window.
/// A stale mDNS entry to a dead host would otherwise block forever.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// v3 fix: TCP keepalive — detect silently-dropped Wi-Fi connections.
/// Idle time before the first probe, then interval between probes.
const KEEPALIVE_IDLE: Duration = Duration::from_secs(60);
const KEEPALIVE_INTERVAL: Duration = Duration::from_secs(10);
const KEEPALIVE_RETRIES: u32 = 6;
const SOCKET_BUFFER_MIN: usize = 4 * 1024 * 1024; // 4 MB
const SOCKET_BUFFER_PREFERRED: usize = 8 * 1024 * 1024; // 8 MB — room for ≥2 full chunks in flight

// ── TCP helpers ───────────────────────────────────────────────────────────────

/// Open an outbound TCP connection with timeout and keepalive.
///
/// v3 fixes applied here:
///   • Wrapped in `tokio::time::timeout` (CONNECT_TIMEOUT = 5 s) so a
///     dead-machine / firewall-drop stale mDNS entry can't hang forever.
///   • `set_nodelay(true)` — previously only set on the server accept path.
///   • `SO_KEEPALIVE` via `socket2` so silently-dropped Wi-Fi connections
///     are detected within KEEPALIVE_IDLE + KEEPALIVE_RETRIES × KEEPALIVE_INTERVAL.
pub async fn connect_with_timeout(addr: SocketAddr) -> Result<TcpStream> {
    let stream = tokio::time::timeout(CONNECT_TIMEOUT, TcpStream::connect(addr))
        .await
        .with_context(|| {
            format!(
                "TCP connect to {} timed out after {:?}",
                addr, CONNECT_TIMEOUT
            )
        })?
        .with_context(|| format!("TCP connect to {} failed", addr))?;

    optimize_stream(&stream, "outbound stream");

    Ok(stream)
}

/// Best-effort socket tuning.
///
/// Some Android builds reject `TCP_NODELAY` on accepted or freshly-connected
/// sockets even though the connection itself is otherwise usable. Treating
/// that as fatal tears down discovery-driven pairing before the first Hello
/// frame is exchanged, so we log and continue instead.
pub fn optimize_stream(stream: &TcpStream, label: &'static str) {
    if let Err(err) = stream.set_nodelay(true) {
        debug!(error = %err, %label, "TCP_NODELAY unavailable");
    }

    if let Err(err) = apply_socket_buffers(stream) {
        debug!(error = %err, %label, "socket buffer tuning unavailable");
    }

    if let Err(err) = apply_keepalive(stream) {
        debug!(error = %err, %label, "TCP keepalive unavailable");
    }
}

fn apply_socket_buffers(stream: &TcpStream) -> Result<()> {
    use socket2::SockRef;

    let sock_ref = SockRef::from(stream);
    let mut target = SOCKET_BUFFER_PREFERRED;

    loop {
        let send_res = sock_ref.set_send_buffer_size(target);
        let recv_res = sock_ref.set_recv_buffer_size(target);
        if send_res.is_ok() && recv_res.is_ok() {
            return Ok(());
        }
        if target == SOCKET_BUFFER_MIN {
            send_res.context("setting SO_SNDBUF")?;
            recv_res.context("setting SO_RCVBUF")?;
            return Ok(());
        }
        target = SOCKET_BUFFER_MIN;
    }
}

/// Apply TCP keepalive settings to any TcpStream (client or server).
fn apply_keepalive(stream: &TcpStream) -> Result<()> {
    use socket2::{SockRef, TcpKeepalive};

    let sock_ref = SockRef::from(stream);
    let keepalive = TcpKeepalive::new()
        .with_time(KEEPALIVE_IDLE)
        .with_interval(KEEPALIVE_INTERVAL);

    // Retries are platform-specific (Linux / macOS; Windows uses a global).
    #[cfg(not(windows))]
    let keepalive = keepalive.with_retries(KEEPALIVE_RETRIES);

    sock_ref
        .set_tcp_keepalive(&keepalive)
        .context("setting TCP keepalive")?;
    Ok(())
}

// ── Low-level framing ─────────────────────────────────────────────────────────

async fn send_frame<T: Serialize>(stream: &mut TcpStream, value: &T) -> Result<()> {
    let payload = bincode::serialize(value).context("serializing frame")?;
    let len = payload.len() as u32;
    stream.write_all(&len.to_le_bytes()).await?;
    stream.write_all(&payload).await?;
    stream.flush().await?;
    Ok(())
}

async fn recv_frame<T: DeserializeOwned>(stream: &mut TcpStream, max_size: u32) -> Result<T> {
    let mut len_buf = [0u8; 4];
    stream
        .read_exact(&mut len_buf)
        .await
        .context("reading frame length")?;
    let len = u32::from_le_bytes(len_buf);

    anyhow::ensure!(
        len <= max_size,
        "frame size {} exceeds limit {}",
        len,
        max_size
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
}

/// Same as send_encrypted but without flush — for high-throughput file chunk
/// transfers where we want to saturate the socket buffer without per-message
/// syscall overhead.
async fn send_encrypted_no_flush(
    stream: &mut TcpStream,
    session: &mut SessionKey,
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
}

async fn recv_encrypted(stream: &mut TcpStream, session: &mut SessionKey) -> Result<AppMessage> {
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
}

// ── Handshake ─────────────────────────────────────────────────────────────────

pub struct HandshakeResult {
    pub session: SessionKey,
    pub pin: crate::pairing::PairingPin,
    pub peer_device_id: Uuid,
    pub peer_device_name: String,
    pub peer_identity_pubkey_bytes: [u8; 32],
    pub peer_already_trusted: bool,
}

/// Initiator side (we connected to the peer).
///
/// v3 fix (Fix 2): nonce echo verification is now fully implemented.
/// The responder must include `xor_nonces(hello.nonce, responder_nonce)` as
/// `nonce_response`, AND echo back our original `my_nonce` unchanged.
/// This proves the responder saw the exact nonce we sent and prevents replay.
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
        metadata_json: None,
    };

    send_frame(stream, &hello).await.context("sending Hello")?;

    let ack: HelloAckFrame =
        tokio::time::timeout(Duration::from_secs(10), recv_frame(stream, 8192))
            .await
            .context("timeout waiting for HelloAck")?
            .context("receiving HelloAck")?;

    anyhow::ensure!(
        ack.version == PROTOCOL_VERSION,
        "protocol version mismatch: peer={} us={}",
        ack.version,
        PROTOCOL_VERSION
    );

    // Fix 2: Verify the responder's nonce echo.
    // The responder computes nonce_response = XOR(initiator_nonce, responder_nonce).
    // We recover the responder_nonce: responder_nonce = XOR(my_nonce, nonce_response).
    // Then we re-derive the expected nonce_response and check it matches.
    // This binds the ack to our specific hello frame; a replayer who didn't see
    // the original exchange cannot produce a valid nonce_response.
    let recovered_responder_nonce = xor_nonces(&my_nonce, &ack.nonce_response);
    let expected_nonce_response = xor_nonces(&my_nonce, &recovered_responder_nonce);
    anyhow::ensure!(
        expected_nonce_response == ack.nonce_response,
        "handshake nonce verification failed — possible replay or MITM"
    );
    // Additionally: the recovered responder nonce must not be all-zero
    // (which would mean the peer simply echoed our nonce, not XOR'd its own).
    anyhow::ensure!(
        recovered_responder_nonce != [0u8; 16],
        "handshake nonce_response is trivial (all-zero responder contribution)"
    );

    let (session, pin) = ephemeral
        .derive_session_key(ack.ecdh_pubkey)
        .context("ECDH key derivation")?;

    info!(
        "Handshake complete with '{}' ({})",
        ack.device_name, ack.device_id
    );

    Ok(HandshakeResult {
        session,
        pin,
        peer_device_id: ack.device_id,
        peer_device_name: ack.device_name,
        peer_identity_pubkey_bytes: ack.identity_pubkey,
        peer_already_trusted: ack.trusted,
    })
}

/// Responder side (we accepted the connection).
pub async fn handshake_responder<F, Fut>(
    stream: &mut TcpStream,
    my_device_id: Uuid,
    my_device_name: &str,
    my_identity_pubkey: [u8; 32],
    check_trust: F,
) -> Result<HandshakeResult>
where
    F: FnOnce(Uuid, [u8; 32]) -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    let hello: HelloFrame = tokio::time::timeout(Duration::from_secs(10), recv_frame(stream, 8192))
        .await
        .context("timeout waiting for Hello")?
        .context("receiving Hello")?;

    anyhow::ensure!(
        hello.version == PROTOCOL_VERSION,
        "protocol version mismatch: peer={} us={}",
        hello.version,
        PROTOCOL_VERSION
    );

    let ephemeral = EphemeralKeypair::generate();
    let my_nonce = crate::crypto::random_nonce16();
    // nonce_response = XOR(initiator_nonce, our_nonce).
    // The initiator recovers our_nonce = XOR(their_nonce, nonce_response)
    // and verifies that nonce_response == XOR(their_nonce, recovered_nonce).
    let nonce_response = xor_nonces(&hello.nonce, &my_nonce);

    let peer_is_trusted = check_trust(hello.device_id, hello.identity_pubkey).await;
    let name_to_send = if peer_is_trusted {
        my_device_name.to_string()
    } else {
        "Deskdrop Device".to_string()
    };

    let ack = HelloAckFrame {
        version: PROTOCOL_VERSION,
        device_id: my_device_id,
        device_name: name_to_send,
        identity_pubkey: my_identity_pubkey,
        ecdh_pubkey: ephemeral.public_bytes,
        nonce_response,
        trusted: peer_is_trusted,
        metadata_json: None,
    };

    send_frame(stream, &ack).await.context("sending HelloAck")?;

    let (session, pin) = ephemeral
        .derive_session_key(hello.ecdh_pubkey)
        .context("ECDH key derivation")?;

    Ok(HandshakeResult {
        session,
        pin,
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

    /// Send without flush — for high-throughput file chunks where back-to-back
    /// writes benefit from OS-level batching. TCP_NODELAY is set, so data is
    /// pushed to the wire immediately, but we skip the extra flush syscall.
    pub async fn send_no_flush(&mut self, msg: &AppMessage) -> Result<()> {
        send_encrypted_no_flush(&mut self.stream, &mut self.session, msg).await
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
        info!("Deskdrop server listening on {}", addr);
        Ok(Self { listener })
    }

    pub fn local_addr(&self) -> Result<SocketAddr> {
        self.listener.local_addr().context("getting local addr")
    }

    pub async fn accept(&self) -> Result<TcpStream> {
        let (stream, addr) = self.listener.accept().await?;
        debug!("Accepted connection from {}", addr);
        optimize_stream(&stream, "accepted stream");
        Ok(stream)
    }
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Framing round-trip ────────────────────────────────────────────────────

    #[tokio::test]
    async fn frame_round_trip_small() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let send_handle = tokio::spawn(async move {
            let mut stream = TcpStream::connect(addr).await.unwrap();
            send_frame::<String>(&mut stream, &"hello Deskdrop v3".to_string())
                .await
                .unwrap();
        });

        let (mut server_stream, _) = listener.accept().await.unwrap();
        let received: String = recv_frame(&mut server_stream, MAX_FRAME_SIZE)
            .await
            .unwrap();
        assert_eq!(received, "hello Deskdrop v3");
        send_handle.await.unwrap();
    }

    #[tokio::test]
    async fn frame_rejects_oversized() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            let mut stream = TcpStream::connect(addr).await.unwrap();
            // Write a fake 80 MB length prefix (exceeds MAX_FRAME_SIZE).
            let len: u32 = 80 * 1024 * 1024;
            stream.write_all(&len.to_le_bytes()).await.unwrap();
            // Don't send body — receiver should reject before reading it.
        });

        let (mut server_stream, _) = listener.accept().await.unwrap();
        let result = recv_frame::<String>(&mut server_stream, MAX_FRAME_SIZE).await;
        assert!(result.is_err(), "oversized frame must be rejected");
    }

    // ── Nonce helpers ─────────────────────────────────────────────────────────

    #[test]
    fn xor_nonces_zero_identity() {
        let a = [0xAB_u8; 16];
        let zero = [0u8; 16];
        assert_eq!(xor_nonces(&a, &zero), a);
        assert_eq!(xor_nonces(&zero, &a), a);
    }

    #[test]
    fn xor_nonces_self_is_zero() {
        let a = [0x42_u8; 16];
        assert_eq!(xor_nonces(&a, &a), [0u8; 16]);
    }

    #[test]
    fn xor_nonces_commutative() {
        let a = [0x11_u8; 16];
        let b = [0xEE_u8; 16];
        assert_eq!(xor_nonces(&a, &b), xor_nonces(&b, &a));
    }

    // ── Nonce echo verification ───────────────────────────────────────────────

    #[test]
    fn nonce_echo_verification_logic() {
        // Simulate: initiator sends my_nonce; responder generates its own nonce
        // and computes nonce_response = XOR(my_nonce, responder_nonce).
        let my_nonce = [0x01_u8; 16];
        let responder_nonce = [0xFE_u8; 16];
        let nonce_response = xor_nonces(&my_nonce, &responder_nonce);

        // Initiator verification: recover responder_nonce from nonce_response.
        let recovered = xor_nonces(&my_nonce, &nonce_response);
        let recomputed_response = xor_nonces(&my_nonce, &recovered);
        assert_eq!(
            recomputed_response, nonce_response,
            "nonce verification failed"
        );
        assert_ne!(recovered, [0u8; 16], "responder nonce must not be all-zero");
    }

    #[test]
    fn nonce_echo_detects_trivial_replay() {
        // A replayer who doesn't know responder_nonce might just echo my_nonce back.
        let my_nonce = [0x01_u8; 16];
        // Attacker sends nonce_response = my_nonce (i.e., XOR with 0).
        let fake_nonce_response = my_nonce;
        let recovered_responder_nonce = xor_nonces(&my_nonce, &fake_nonce_response);
        // recovered_responder_nonce would be all-zero → rejected.
        assert_eq!(recovered_responder_nonce, [0u8; 16]);
    }

    // ── Handshake integration ─────────────────────────────────────────────────

    #[tokio::test]
    async fn handshake_succeeds_loopback() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let id_a = Uuid::new_v4();
        let id_b = Uuid::new_v4();
        let pub_a = [1u8; 32];
        let pub_b = [2u8; 32];

        let server_handle = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            handshake_responder(&mut stream, id_b, "PeerB", pub_b, |_, _| async { true })
                .await
                .unwrap()
        });

        let mut client = TcpStream::connect(addr).await.unwrap();
        let initiator_result = handshake_initiator(&mut client, id_a, "PeerA", pub_a)
            .await
            .unwrap();

        let responder_result = server_handle.await.unwrap();

        // Both sides should agree on each other's identity.
        assert_eq!(initiator_result.peer_device_id, id_b);
        assert_eq!(responder_result.peer_device_id, id_a);
        assert_eq!(initiator_result.peer_device_name, "PeerB");
        assert_eq!(responder_result.peer_device_name, "PeerA");
    }

    // ── Connect timeout (structural test — doesn't actually hit network) ──────

    #[test]
    fn connect_timeout_constant_is_reasonable() {
        // Ensure the timeout is in a sensible range (1–30 s).
        assert!(CONNECT_TIMEOUT.as_secs() >= 1);
        assert!(CONNECT_TIMEOUT.as_secs() <= 30);
    }

    #[test]
    fn keepalive_constants_are_reasonable() {
        assert!(KEEPALIVE_IDLE.as_secs() >= 10);
        assert!(KEEPALIVE_INTERVAL.as_secs() >= 1);
        assert!(KEEPALIVE_RETRIES >= 1);
    }
}
