//! Prints the outline of each file given, for comparing with the Python build:
//! `cargo run --example outline -- a.py b.m`.

fn main() {
    for path in std::env::args().skip(1) {
        let code = std::fs::read_to_string(&path).unwrap_or_default();
        let ext = std::path::Path::new(&path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        println!("=== {path}\n{}", codigest::outline::summarize(ext, &code));
    }
}
