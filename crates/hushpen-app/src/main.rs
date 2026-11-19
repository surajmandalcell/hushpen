//! Hushpen desktop app.

use hushpen_app::cli::{self, Command};
use std::process::ExitCode;

fn main() -> ExitCode {
    match cli::parse(std::env::args().skip(1)) {
        Command::Version => {
            println!("{}", cli::version_line());
            ExitCode::SUCCESS
        }
        #[cfg(all(target_os = "macos", feature = "test-automation"))]
        Command::LiveTestCheckAccessibility => check_live_test_accessibility(),
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
        } => {
            #[cfg(all(target_os = "macos", feature = "test-automation"))]
            if hushpen_platform::permissions::live_test_mode()
                && check_live_test_accessibility() != ExitCode::SUCCESS
            {
                return ExitCode::FAILURE;
            }
            hushpen_app::app::run(hidden, show_window)
        }
    }
}

#[cfg(all(target_os = "macos", feature = "test-automation"))]
fn check_live_test_accessibility() -> ExitCode {
    use hushpen_platform::permissions::{accessibility_trusted, live_test_mode};

    if !live_test_mode() {
        eprintln!("HUSHPEN_LIVE_TEST Run the check from the app.hushpen.livetest bundle.");
        return ExitCode::from(2);
    }
    let (message, code) = if accessibility_trusted() {
        (
            "HUSHPEN_LIVE_TEST Accessibility granted for app.hushpen.livetest.",
            ExitCode::SUCCESS,
        )
    } else {
        (
            "HUSHPEN_LIVE_TEST Accessibility missing for app.hushpen.livetest. Exiting.",
            ExitCode::FAILURE,
        )
    };
    eprintln!("{message}");
    // A Launch Services run has no terminal and cannot redirect to the external drive.
    let _ = std::process::Command::new("/usr/bin/logger")
        .args(["-t", "hushpen-livetest", message])
        .status();
    code
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
