//! Command line. `--version` and `engine` are answered before anything touches the
//! display, the data folder, or the single-instance lock.

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Run,
    Version,
    /// The speech engine child, with the arguments after `engine`.
    Engine(Vec<String>),
    Unknown(String),
}

pub fn parse(args: impl IntoIterator<Item = String>) -> Command {
    let mut args = args.into_iter();
    match args.next().as_deref() {
        None => Command::Run,
        Some("--version" | "-V") => Command::Version,
        Some("engine") => Command::Engine(args.collect()),
        Some(other) => Command::Unknown(other.to_string()),
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

    #[test]
    fn no_arguments_start_the_app() {
        assert_eq!(parse_args(&[]), Command::Run);
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
    fn unknown_arguments_are_reported() {
        assert_eq!(parse_args(&["--bogus"]), Command::Unknown("--bogus".into()));
    }

    #[test]
    fn version_line_names_the_binary_and_the_build_version() {
        assert_eq!(
            version_line(),
            format!("hushpen {}", hushpen_core::BUILD_VERSION)
        );
        assert!(version_line().starts_with("hushpen 2.0.0"));
    }
}
