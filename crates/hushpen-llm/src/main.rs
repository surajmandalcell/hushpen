//! Local language model worker. It talks to the app over stdin and stdout only.

fn main() {
    if std::env::args().nth(1).as_deref() == Some("--version") {
        println!("hushpen-llm {}", hushpen_core::VERSION);
    }
}
