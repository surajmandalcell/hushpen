//! Hushpen desktop app.

fn main() {
    if std::env::args().nth(1).as_deref() == Some("--version") {
        println!("hushpen {}", hushpen_core::VERSION);
    }
}
