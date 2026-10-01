//! Two engines on localhost with seeded trust, shared by the pairing tests.

#![allow(dead_code)]

use deskdrop_core::engine::{Engine, EngineConfig, EngineEvent};
use deskdrop_core::identity::IdentityStore;
use deskdrop_core::trust::TrustStore;
use std::net::{IpAddr, Ipv4Addr};
use std::time::{Duration, Instant};
use tempfile::TempDir;
use tokio::sync::mpsc;
use tokio::time::timeout;
use uuid::Uuid;

pub struct Node {
    pub engine: Engine,
    pub events: mpsc::Receiver<EngineEvent>,
    pub id: Uuid,
}

/// Starts two engines. `a_trusts_b` / `b_trusts_a` seed each trust store.
pub async fn start_pair(tmp: &TempDir, a_trusts_b: bool, b_trusts_a: bool) -> (Node, Node) {
    let ids = [Uuid::new_v4(), Uuid::new_v4()];
    let names = ["NodeA", "NodeB"];
    let identities: Vec<_> = (0..2)
        .map(|i| {
            IdentityStore::new(tmp.path().join(format!("identity{i}.key")))
                .load_or_create()
                .unwrap()
        })
        .collect();
    let seed = [a_trusts_b, b_trusts_a];

    let mut nodes = Vec::new();
    for i in 0..2 {
        let other = 1 - i;
        let trust_path = tmp.path().join(format!("trust{i}.json"));
        let mut trust = TrustStore::load(&trust_path).unwrap();
        if seed[i] {
            trust
                .trust(
                    ids[other],
                    names[other].into(),
                    &identities[other].public_bytes,
                )
                .unwrap();
        }
        let (tx, rx) = mpsc::channel(256);
        let cfg = EngineConfig {
            device_id: ids[i],
            device_name: names[i].into(),
            port: 0,
            trust_store_path: trust_path,
            peer_store_path: tmp.path().join(format!("peers{i}.json")),
            identity_path: tmp.path().join(format!("identity{i}.key")),
            data_dir: tmp.path().join(format!("data{i}")),
            bind_ip: Some(IpAddr::V4(Ipv4Addr::LOCALHOST)),
            enable_discovery: false,
            ..EngineConfig::default()
        };
        let engine = Engine::start(cfg, tx).await.expect("engine start");
        nodes.push(Node {
            engine,
            events: rx,
            id: ids[i],
        });
    }
    let b = nodes.pop().unwrap();
    let a = nodes.pop().unwrap();
    (a, b)
}

pub async fn wait_for<F: Fn(&EngineEvent) -> bool>(
    rx: &mut mpsc::Receiver<EngineEvent>,
    what: &str,
    pred: F,
) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let event = timeout(remaining, rx.recv())
            .await
            .unwrap_or_else(|_| panic!("timeout waiting for {what}"))
            .expect("channel closed");
        if pred(&event) {
            return;
        }
    }
}
