use std::time::Duration;
use tokio;

#[tokio::main]
async fn main() {
    let mut poll = tokio::time::interval(Duration::from_secs(1));
    let mut prev = None;
    for i in 0..10 {
        poll.tick().await;
        let snap = deskdrop_core::network_manager::resolve_snapshot(None, 47823).unwrap();
        if prev != Some(snap.clone()) {
            println!("Change detected at {i}: {:?}", snap);
            if let Some(p) = &prev {
                println!("Change event: {:?}", deskdrop_core::network_manager::detect_change(p, &snap));
            }
            prev = Some(snap);
        } else {
            println!("No change at {i}");
        }
    }
}
