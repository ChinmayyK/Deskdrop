//! Integration test: two in-process engines exchange clipboard content.

use cliprelay_core::engine::{Engine, EngineConfig, EngineEvent};
use cliprelay_core::identity::IdentityStore;
use cliprelay_core::protocol::ClipboardContent;
use cliprelay_core::trust::TrustStore;
use std::net::{IpAddr, Ipv4Addr};
use std::time::{Duration, Instant};
use tempfile::TempDir;
use tokio::sync::mpsc;
use tokio::time::timeout;
use uuid::Uuid;

#[tokio::test]
async fn two_engines_exchange_text() {
    let tmp = TempDir::new().unwrap();
    let (tx1, _rx1) = mpsc::channel(64);
    let (tx2, mut rx2) = mpsc::channel(64);
    let device_id_1 = Uuid::new_v4();
    let device_id_2 = Uuid::new_v4();
    let trust_path_1 = tmp.path().join("trust1.json");
    let trust_path_2 = tmp.path().join("trust2.json");
    let peer_path_1 = tmp.path().join("peers1.json");
    let peer_path_2 = tmp.path().join("peers2.json");
    let identity_path_1 = tmp.path().join("identity1.key");
    let identity_path_2 = tmp.path().join("identity2.key");

    let identity_1 = IdentityStore::new(&identity_path_1)
        .load_or_create()
        .unwrap();
    let identity_2 = IdentityStore::new(&identity_path_2)
        .load_or_create()
        .unwrap();

    let mut trust_1 = TrustStore::load(&trust_path_1).unwrap();
    trust_1
        .trust(device_id_2, "TestDevice2".into(), &identity_2.public_bytes)
        .unwrap();
    let mut trust_2 = TrustStore::load(&trust_path_2).unwrap();
    trust_2
        .trust(device_id_1, "TestDevice1".into(), &identity_1.public_bytes)
        .unwrap();

    let cfg1 = EngineConfig {
        device_id: device_id_1,
        device_name: "TestDevice1".into(),
        port: 47900,
        trust_store_path: trust_path_1,
        peer_store_path: peer_path_1,
        identity_path: identity_path_1,
        bind_ip: Some(IpAddr::V4(Ipv4Addr::LOCALHOST)),
        enable_discovery: false,
        ..EngineConfig::default()
    };

    let cfg2 = EngineConfig {
        device_id: device_id_2,
        device_name: "TestDevice2".into(),
        port: 47901,
        trust_store_path: trust_path_2,
        peer_store_path: peer_path_2,
        identity_path: identity_path_2,
        bind_ip: Some(IpAddr::V4(Ipv4Addr::LOCALHOST)),
        enable_discovery: false,
        ..EngineConfig::default()
    };

    let engine1 = Engine::start(cfg1, tx1).await.expect("engine1 start");
    let _engine2 = Engine::start(cfg2, tx2).await.expect("engine2 start");

    tokio::time::sleep(Duration::from_millis(100)).await;
    engine1
        .connect_to_peer("127.0.0.1".into(), 47901)
        .await
        .expect("manual connect");

    // Push from engine1.
    engine1
        .push_clipboard(ClipboardContent::Text("hello from device 1".into()))
        .await;

    // Engine2 should receive within 500 ms.
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let event = timeout(remaining, rx2.recv())
            .await
            .expect("timeout waiting for event")
            .expect("channel closed");

        match event {
            EngineEvent::ClipboardReceived {
                content: ClipboardContent::Text(t),
                ..
            } => {
                assert_eq!(t, "hello from device 1");
                break;
            }
            EngineEvent::PeerConnected { .. } | EngineEvent::ClipboardSynced { .. } => {}
            other => panic!("unexpected event: {:?}", other),
        }
    }
}
