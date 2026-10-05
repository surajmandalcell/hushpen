//! Command line. `--version` is answered before anything touches the
//! display, the data folder, or the single-instance lock.

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Run,
    Version,
    Unknown(String),
}

pub fn parse(args: impl IntoIterator<Item = String>) -> Command {
    match args.into_iter().next().as_deref() {
        None => Command::Run,
        Some("--version" | "-V") => Command::Version,
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
