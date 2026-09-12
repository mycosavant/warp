// Prints its own path so the caller can tell which copy ran.
fn main() {
    let me = std::env::current_exe()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "?".into());
    println!("FAKE RAN {me}");
}
