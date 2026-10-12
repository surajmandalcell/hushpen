//! Local language model worker. It talks to the app over stdin and stdout only.

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    hushpen_llm::entry::worker_main(&args)
}
