//! Opening a folder, a file, or a web page on Linux through `xdg-open`.
//!
//! The program is started and left to run: the caller never waits for it, because a desktop
//! that is slow to answer must not stall a frame.

use std::ffi::OsStr;
use std::io::{self, ErrorKind};
use std::process::{Command, Stdio};

pub(super) const PROGRAM: &str = "xdg-open";

/// The command that opens `target`. No standard stream is shared with the app.
pub(super) fn command(target: &OsStr) -> Command {
    let mut command = Command::new(PROGRAM);
    command
        .arg(target)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command
}

/// Only a web page may go to `xdg-open` as a URL: it hands any other scheme to whatever
/// program the desktop has registered for it.
pub(super) fn check_url(url: &str) -> io::Result<()> {
    let web = ["https://", "http://"].iter().any(|scheme| {
        url.get(..scheme.len())
            .is_some_and(|head| head.eq_ignore_ascii_case(scheme))
    });
    if web {
        Ok(())
    } else {
        Err(io::Error::new(
            ErrorKind::InvalidInput,
            "only an http or https address can be opened",
        ))
    }
}

/// Hands the command for `target` to `launch`. A desktop with no `xdg-open` is a normal
/// state (a bare container, a window manager alone): it logs a warning and succeeds, so the
/// click does not turn into an error notice.
pub(super) fn open(
    target: &OsStr,
    launch: impl FnOnce(Command) -> io::Result<()>,
) -> io::Result<()> {
    match launch(command(target)) {
        Err(error) if error.kind() == ErrorKind::NotFound => {
            log::warn!("{PROGRAM} is not installed, so nothing was opened");
            Ok(())
        }
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::path::Path;

    fn launched(target: &OsStr) -> (String, Vec<String>) {
        let seen = RefCell::new(None);
        open(target, |command| {
            *seen.borrow_mut() = Some((
                command.get_program().to_string_lossy().into_owned(),
                command
                    .get_args()
                    .map(|arg| arg.to_string_lossy().into_owned())
                    .collect(),
            ));
            Ok(())
        })
        .unwrap();
        seen.into_inner().unwrap()
    }

    #[test]
    fn a_path_is_the_one_argument_of_xdg_open() {
        let (program, args) = launched(Path::new("/home/me/.local/share/hushpen/logs").as_os_str());
        assert_eq!(program, "xdg-open");
        assert_eq!(args, ["/home/me/.local/share/hushpen/logs"]);
    }

    #[test]
    fn a_web_address_is_the_one_argument_of_xdg_open() {
        let (program, args) = launched(OsStr::new("https://github.com/notes"));
        assert_eq!(program, "xdg-open");
        assert_eq!(args, ["https://github.com/notes"]);
    }

    #[test]
    fn a_missing_xdg_open_succeeds_with_a_warning() {
        let result = open(OsStr::new("/tmp"), |_| {
            Err(io::Error::from(ErrorKind::NotFound))
        });
        assert!(result.is_ok());
    }

    #[test]
    fn another_launch_failure_is_still_an_error() {
        let result = open(OsStr::new("/tmp"), |_| {
            Err(io::Error::from(ErrorKind::PermissionDenied))
        });
        assert_eq!(result.unwrap_err().kind(), ErrorKind::PermissionDenied);
    }

    #[test]
    fn only_web_addresses_pass_the_url_check() {
        assert!(check_url("https://github.com/surajmandalcell/asrpro").is_ok());
        assert!(check_url("HTTP://localhost:8080/x").is_ok());
        for refused in [
            "file:///etc/passwd",
            "x-apple.systempreferences:com.apple.preference",
            "--help",
            "ftp://example.com",
            "",
        ] {
            assert_eq!(
                check_url(refused).unwrap_err().kind(),
                ErrorKind::InvalidInput,
                "{refused}"
            );
        }
    }
}
