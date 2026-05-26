use hkdf::Hkdf;
use sha2::Sha256;
fn main() {
    let ikm = [0xAB_u8; 32];
    let hk = Hkdf::<Sha256>::new(None, &ikm);
    
    for info in ["deskdrop-v1-session", "deskdrop-v2-session", "deskdrop-v3-session"] {
        let mut okm = [0u8; 32];
        hk.expand(info.as_bytes(), &mut okm).unwrap();
        let hex: String = okm.iter().map(|b| format!("{:02x}", b)).collect();
        println!("Info: {}, OKM: {}", info, hex);
    }
}
