use deskdrop_core::engine::PeerBatteryState;
use uuid::Uuid;

fn main() {
    let state = PeerBatteryState {
        device_id: Uuid::new_v4(),
        device_name: "test".to_string(),
        level: 100,
        charging: true,
    };
    let json = serde_json::to_string(&state).unwrap();
    println!("{}", json);
}
