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
        Command::UpdateApply(args) => hushpen_app::update_apply::main(&args),
        Command::Restart(args) => restart(&args),
        Command::SwitcherGuard => hushpen_platform::keys::switcher_guard_main(),
        Command::Unknown(argument) => {
            eprintln!("hushpen: unknown argument '{argument}'. Try --version.");
            ExitCode::from(2)
        }
        Command::Run {
            hidden,
            show_window,
        } => hushpen_app::app::run(hidden, show_window),
    }
}

fn restart(args: &[String]) -> ExitCode {
    let (current, target, default) = match hushpen_platform::restart::wait_for_exit(args) {
        Ok(paths) => paths,
        Err(error) => {
            log::error!("The data-folder restart failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(error) = hushpen_store::data_move::finish_move_cleanup(&current, &target, &default) {
        log::warn!("The old data folder could not be cleared: {error}");
    }
    match hushpen_platform::restart::relaunch() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            log::error!("Could not relaunch Hushpen after the data-folder move: {error}");
            ExitCode::FAILURE
        }
    }
}
