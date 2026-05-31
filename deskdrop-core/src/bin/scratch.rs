fn main() {
    let raw = "ChinmayK’s MacBook Air".to_string();
    let patterns = ["'s ", "’s ", "s' ", "s’ "];
    let mut result = raw.as_str();
    for p in patterns.iter() {
        if let Some(idx) = result.rfind(p) {
            result = &result[idx + p.len()..];
        }
    }
    let cleaned = result.trim().to_string();
    println!("Cleaned name is: {}", cleaned);
}
