//! Hushpen desktop app.

use hushpen_app::cli::{self, Command};
use std::process::ExitCode;

fn main() -> ExitCode {
    match cli::parse(std::env::args().skip(1)) {
        Command::Version => {
            println!("{}", cli::version_line());
            ExitCode::SUCCESS
        }
        Command::Engine(args) => hushpen_engine::engine_main(&args),
        Command::Unknown(argument) => {
            eprintln!("hushpen: unknown argument '{argument}'. Try --version.");
            ExitCode::from(2)
        }
        Command::Run { hidden } => hushpen_app::app::run(hidden),
    }
}
