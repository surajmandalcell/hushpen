//! `hookctl`: talks to a running debug Hushpen over its test hook.

use hushpen_testhook::cli::{self, Env};
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = cli::run(
        &args,
        &Env::from_process(),
        &mut std::io::stdout(),
        &mut std::io::stderr(),
    );
    ExitCode::from(u8::try_from(code).unwrap_or(2))
}
