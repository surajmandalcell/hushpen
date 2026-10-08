//! Which display session the app runs in. Global keys and paste need X11 or macOS.

use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Session {
    X11,
    /// `XDG_SESSION_TYPE=wayland`, or `WAYLAND_DISPLAY` set and `DISPLAY` not set.
    Wayland,
    /// Neither a display nor a Wayland session variable.
    NoDisplay,
}

/// The environment, as a lookup so tests need no process state. Empty values count as unset.
pub type Env<'a> = &'a dyn Fn(&str) -> Option<String>;

fn var(env: Env<'_>, name: &str) -> Option<String> {
    env(name).filter(|value| !value.is_empty())
}

pub fn detect_with(env: Env<'_>) -> Session {
    let wayland_type = var(env, "XDG_SESSION_TYPE").is_some_and(|kind| kind == "wayland");
    let wayland_display = var(env, "WAYLAND_DISPLAY").is_some();
    let display = var(env, "DISPLAY").is_some();
    if wayland_type || (wayland_display && !display) {
        Session::Wayland
    } else if display {
        Session::X11
    } else {
        Session::NoDisplay
    }
}

pub fn detect() -> Session {
    detect_with(&|name| std::env::var(name).ok())
}

/// Why the window system cannot start, or `None` when a display server can be reached.
/// A `WAYLAND_DISPLAY` with no compositor behind it counts as unreachable, and so does a
/// missing `DISPLAY`: the toolkit would panic instead of failing.
pub fn display_problem_with(
    env: Env<'_>,
    socket_exists: &dyn Fn(&Path) -> bool,
) -> Option<Session> {
    let wayland_ready = var(env, "WAYLAND_DISPLAY").is_some_and(|name| {
        let path = Path::new(&name);
        if path.is_absolute() {
            return socket_exists(path);
        }
        var(env, "XDG_RUNTIME_DIR").is_some_and(|dir| socket_exists(&Path::new(&dir).join(&name)))
    });
    if wayland_ready || var(env, "DISPLAY").is_some() {
        return None;
    }
    Some(match detect_with(env) {
        Session::X11 => Session::NoDisplay,
        other => other,
    })
}

pub fn display_problem() -> Option<Session> {
    display_problem_with(&|name| std::env::var(name).ok(), &|path| path.exists())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn env(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }

    fn session(pairs: &[(&str, &str)]) -> Session {
        let vars = env(pairs);
        detect_with(&|name| vars.get(name).cloned())
    }

    #[test]
    fn a_session_type_of_wayland_is_wayland_even_with_a_display() {
        assert_eq!(
            session(&[("XDG_SESSION_TYPE", "wayland"), ("DISPLAY", ":99")]),
            Session::Wayland
        );
    }

    #[test]
    fn a_wayland_display_without_an_x_display_is_wayland() {
        assert_eq!(
            session(&[("WAYLAND_DISPLAY", "wayland-0")]),
            Session::Wayland
        );
    }

    #[test]
    fn a_wayland_display_with_an_x_display_is_x11() {
        assert_eq!(
            session(&[("WAYLAND_DISPLAY", "wayland-0"), ("DISPLAY", ":0")]),
            Session::X11
        );
    }

    #[test]
    fn x11_needs_a_display_and_empty_values_count_as_unset() {
        assert_eq!(
            session(&[("XDG_SESSION_TYPE", "x11"), ("DISPLAY", ":99")]),
            Session::X11
        );
        assert_eq!(session(&[("DISPLAY", "")]), Session::NoDisplay);
        assert_eq!(session(&[]), Session::NoDisplay);
    }

    #[test]
    fn a_wayland_socket_that_does_not_exist_is_a_display_problem() {
        let vars = env(&[
            ("WAYLAND_DISPLAY", "wayland-0"),
            ("XDG_SESSION_TYPE", "wayland"),
            ("XDG_RUNTIME_DIR", "/run/user/1000"),
        ]);
        let lookup = |name: &str| vars.get(name).cloned();
        assert_eq!(
            display_problem_with(&lookup, &|_| false),
            Some(Session::Wayland)
        );
        assert_eq!(
            display_problem_with(&lookup, &|path| path
                == Path::new("/run/user/1000/wayland-0")),
            None
        );
    }

    #[test]
    fn an_x_display_is_always_reachable_and_no_variables_are_a_problem() {
        let vars = env(&[("DISPLAY", ":99"), ("XDG_SESSION_TYPE", "wayland")]);
        assert_eq!(
            display_problem_with(&|name| vars.get(name).cloned(), &|_| false),
            None
        );
        assert_eq!(
            display_problem_with(&|_| None, &|_| false),
            Some(Session::NoDisplay)
        );
    }
}
