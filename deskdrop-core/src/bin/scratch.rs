use deskdrop_core::peer_manager::PeerRecord;
fn main() {
    let p = PeerRecord::default();
    println!("{}", serde_json::to_string(&p).unwrap());
}
