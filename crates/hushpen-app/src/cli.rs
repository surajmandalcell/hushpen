//! Command line. `--version` and `engine` are answered before anything touches the
//! display, the data folder, or the single-instance lock.

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    /// `hidden` comes from `--hidden`, the flag the login item carries: the
    /// app starts in the tray without a window. `show_window` comes from
    /// `--show-window`, which the update helper adds when the window was on screen before the
    /// update: the new copy puts it up again, behind the front app.
    Run {
        hidden: bool,
        show_window: bool,
    },
    Version,
    /// The speech engine child, with the arguments after `engine`.
    Engine(Vec<String>),
    /// The update helper that swaps a staged version in after the app quit.
    UpdateApply(Vec<String>),
    /// The child that turns the macOS app switcher on again if the app ends without doing it.
    SwitcherGuard,
    Unknown(String),
}

pub fn parse(args: impl IntoIterator<Item = String>) -> Command {
    let mut args = args.into_iter();
    let (mut hidden, mut show_window) = (false, false);
    loop {
        match args.next().as_deref() {
            None => {
                return Command::Run {
                    hidden,
                    show_window,
                };
            }
            Some("--hidden") => hidden = true,
            Some("--show-window") => show_window = true,
            Some("--version" | "-V") => return Command::Version,
            Some("engine") => return Command::Engine(args.collect()),
            Some("update-apply") => return Command::UpdateApply(args.collect()),
            Some(hushpen_platform::keys::SWITCHER_GUARD_ARG) => return Command::SwitcherGuard,
            Some(other) => return Command::Unknown(other.to_owned()),
        }
    }
}

pub fn version_line() -> String {
    format!("hushpen {}", hushpen_core::BUILD_VERSION)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_args(args: &[&str]) -> Command {
        parse(args.iter().map(|a| a.to_string()))
    }

    fn run(hidden: bool, show_window: bool) -> Command {
        Command::Run {
            hidden,
            show_window,
        }
    }

    #[test]
    fn no_arguments_start_the_app() {
        assert_eq!(parse_args(&[]), run(false, false));
    }

    #[test]
    fn the_hidden_flag_starts_the_app_without_a_window() {
        assert_eq!(parse_args(&["--hidden"]), run(true, false));
        assert_eq!(
            parse_args(&["--hidden", "extra"]),
            Command::Unknown("extra".into())
        );
    }

    #[test]
    fn the_show_window_flag_puts_the_window_up_on_a_background_start() {
        assert_eq!(parse_args(&["--show-window"]), run(false, true));
        assert_eq!(parse_args(&["--show-window", "--hidden"]), run(true, true));
    }

    #[test]
    fn version_flags_print_the_version() {
        assert_eq!(parse_args(&["--version"]), Command::Version);
        assert_eq!(parse_args(&["-V"]), Command::Version);
    }

    #[test]
    fn the_engine_subcommand_keeps_its_own_arguments() {
        assert_eq!(parse_args(&["engine"]), Command::Engine(vec![]));
        assert_eq!(
            parse_args(&["engine", "--smoke", "a.wav"]),
            Command::Engine(vec!["--smoke".into(), "a.wav".into()])
        );
    }

    #[test]
    fn the_update_apply_subcommand_keeps_its_own_arguments() {
        assert_eq!(
            parse_args(&["update-apply", "--pid", "7"]),
            Command::UpdateApply(vec!["--pid".into(), "7".into()])
        );
    }

    #[test]
    fn the_switcher_guard_subcommand_starts_the_guard() {
        assert_eq!(parse_args(&["switcher-guard"]), Command::SwitcherGuard);
    }

    #[test]
    fn unknown_arguments_are_reported() {
        assert_eq!(parse_args(&["--bogus"]), Command::Unknown("--bogus".into()));
    }

    #[test]
    fn version_line_names_the_binary_and_the_build_version() {
        assert_eq!(
            version_line(),
            format!("hushpen {}", hushpen_core::BUILD_VERSION)
        );
        assert!(version_line().starts_with("hushpen 2.2.0"));
    }
}
