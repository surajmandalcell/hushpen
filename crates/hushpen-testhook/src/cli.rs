//! The `hookctl` command line.

use crate::client;
use crate::endpoint::{self, Endpoint};
use crate::protocol::{Request, Response};
use serde_json::{Value, json};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub const USAGE: &str = "usage: hookctl [--socket PATH | --tcp PORT] <command>
  tree                     element tree: id, role, text, bounds, enabled, focused
  state                    settings, window, pipeline, engine, tray, last insertion
  click <id>               click an element by its id
  action [name] [json]     list the registered actions, or run one
  wait <path>=<value> [ms] wait until a state value matches (default 5000 ms)
  events                   timestamped events
  net                      network request log: time, purpose, host, result
  feed-wav <file>          feed a WAV file into the capture path
  paths <file...>          answer the next file dialog
The hook is found through --socket, --tcp, HUSHPEN_TESTHOOK_SOCKET,
HUSHPEN_TESTHOOK_TCP, or HUSHPEN_DATA_DIR (run/hook.sock).";

const REPLY_TIMEOUT: Duration = Duration::from_secs(10);
const POLL: Duration = Duration::from_millis(50);
const DEFAULT_WAIT_MS: u64 = 5000;

#[derive(Debug, PartialEq)]
pub enum Command {
    Send(Request),
    Wait {
        path: String,
        expected: String,
        timeout: Duration,
    },
}

#[derive(Debug, Default, PartialEq)]
pub struct Options {
    pub socket: Option<String>,
    pub tcp: Option<String>,
}

/// The process environment, passed in so tests control it.
#[derive(Debug, Default, Clone)]
pub struct Env {
    pub socket: Option<String>,
    pub tcp: Option<String>,
    pub data_dir: Option<PathBuf>,
    pub in_container: bool,
    pub cwd: PathBuf,
}

impl Env {
    pub fn from_process() -> Self {
        Self {
            socket: std::env::var("HUSHPEN_TESTHOOK_SOCKET").ok(),
            tcp: std::env::var("HUSHPEN_TESTHOOK_TCP").ok(),
            data_dir: std::env::var_os("HUSHPEN_DATA_DIR")
                .filter(|dir| !dir.is_empty())
                .map(PathBuf::from),
            in_container: endpoint::in_container(),
            cwd: std::env::current_dir().unwrap_or_default(),
        }
    }
}

pub fn parse(args: &[String], cwd: &Path) -> Result<(Options, Command), String> {
    let mut options = Options::default();
    let mut rest = args;
    while let Some(first) = rest.first() {
        match first.as_str() {
            "--socket" => options.socket = Some(value_after(rest, "--socket")?),
            "--tcp" => options.tcp = Some(value_after(rest, "--tcp")?),
            _ => break,
        }
        rest = &rest[2..];
    }
    let (name, rest) = rest.split_first().ok_or("missing command")?;
    let command = match name.as_str() {
        "tree" | "state" | "events" | "net" => {
            no_more(rest, name)?;
            Command::Send(Request::new(name))
        }
        "click" => match rest {
            [id] => Command::Send(Request::new("click").with("id", id.as_str())),
            _ => return Err("usage: hookctl click <id>".into()),
        },
        "action" => match rest {
            [] => Command::Send(Request::new("action")),
            [name] => Command::Send(Request::new("action").with("name", name.as_str())),
            [name, json] => {
                let args: Value = serde_json::from_str(json)
                    .map_err(|error| format!("action arguments are not valid JSON: {error}"))?;
                Command::Send(
                    Request::new("action")
                        .with("name", name.as_str())
                        .with("args", args),
                )
            }
            _ => return Err("usage: hookctl action [name] [json]".into()),
        },
        "wait" => parse_wait(rest)?,
        "feed-wav" => match rest {
            [path] => {
                let absolute = cwd.join(path);
                Command::Send(Request::new("feed-wav").with("path", absolute.to_string_lossy()))
            }
            _ => return Err("usage: hookctl feed-wav <file>".into()),
        },
        "paths" => {
            if rest.is_empty() {
                return Err("usage: hookctl paths <file...>".into());
            }
            let paths: Vec<Value> = rest
                .iter()
                .map(|path| json!(cwd.join(path).to_string_lossy()))
                .collect();
            Command::Send(Request::new("paths").with("paths", paths))
        }
        other => return Err(format!("unknown command '{other}'")),
    };
    Ok((options, command))
}

fn value_after(args: &[String], flag: &str) -> Result<String, String> {
    args.get(1)
        .cloned()
        .ok_or_else(|| format!("{flag} needs a value"))
}

fn no_more(rest: &[String], name: &str) -> Result<(), String> {
    if rest.is_empty() {
        Ok(())
    } else {
        Err(format!("`{name}` takes no arguments"))
    }
}

fn parse_wait(rest: &[String]) -> Result<Command, String> {
    let usage = "usage: hookctl wait <path>=<value> [timeout-ms]";
    let (predicate, timeout) = match rest {
        [predicate] => (predicate, DEFAULT_WAIT_MS),
        [predicate, ms] => (
            predicate,
            ms.parse()
                .map_err(|_| format!("'{ms}' is not a number of milliseconds"))?,
        ),
        _ => return Err(usage.into()),
    };
    let (path, expected) = predicate.split_once('=').ok_or(usage)?;
    if path.is_empty() {
        return Err(usage.into());
    }
    Ok(Command::Wait {
        path: path.to_string(),
        expected: expected.to_string(),
        timeout: Duration::from_millis(timeout),
    })
}

/// Reads a dotted path out of the state. A path that is not found at the top
/// level is tried inside `pipeline`, so `state=Listening` works.
pub fn lookup(state: &Value, path: &str) -> Option<String> {
    let find = |prefix: &[&str]| {
        let mut node = state;
        for key in prefix.iter().copied().chain(path.split('.')) {
            node = node.get(key)?;
        }
        Some(match node {
            Value::String(text) => text.clone(),
            other => other.to_string(),
        })
    };
    find(&[]).or_else(|| find(&["pipeline"]))
}

pub fn matches(state: &Value, path: &str, expected: &str) -> bool {
    lookup(state, path).is_some_and(|actual| actual.eq_ignore_ascii_case(expected))
}

pub fn resolve_endpoint(options: &Options, env: &Env) -> Result<Endpoint, String> {
    endpoint::resolve(
        options.socket.as_deref().or(env.socket.as_deref()),
        options.tcp.as_deref().or(env.tcp.as_deref()),
        env.in_container,
        env.data_dir.as_deref(),
    )
}

/// Runs a command line. Returns the exit code: 0 done, 1 the hook refused or
/// a wait timed out, 2 usage or no hook to talk to.
pub fn run(args: &[String], env: &Env, out: &mut impl Write, err: &mut impl Write) -> i32 {
    let (options, command) = match parse(args, &env.cwd) {
        Ok(parsed) => parsed,
        Err(message) => {
            let _ = writeln!(err, "hookctl: {message}\n{USAGE}");
            return 2;
        }
    };
    let endpoint = match resolve_endpoint(&options, env) {
        Ok(endpoint) => endpoint,
        Err(message) => {
            let _ = writeln!(err, "hookctl: {message}");
            return 2;
        }
    };
    match command {
        Command::Send(request) => match ask(&endpoint, &request) {
            Ok(data) => print_json(out, &data),
            Err(CtlError::Hook(message)) => {
                let _ = writeln!(err, "hookctl: {message}");
                1
            }
            Err(CtlError::Connect(message)) => {
                let _ = writeln!(err, "hookctl: {message}");
                2
            }
        },
        Command::Wait {
            path,
            expected,
            timeout,
        } => wait(&endpoint, &path, &expected, timeout, out, err),
    }
}

enum CtlError {
    Hook(String),
    Connect(String),
}

fn ask(endpoint: &Endpoint, request: &Request) -> Result<Value, CtlError> {
    match client::send(endpoint, request, REPLY_TIMEOUT) {
        Ok(Response::Ok(data)) => Ok(data),
        Ok(Response::Err(message)) => Err(CtlError::Hook(message)),
        Err(error) => Err(CtlError::Connect(format!(
            "cannot reach the hook at {endpoint}: {error}"
        ))),
    }
}

fn print_json(out: &mut impl Write, value: &Value) -> i32 {
    let text = serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string());
    match writeln!(out, "{text}") {
        Ok(()) => 0,
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => 0,
        Err(_) => 2,
    }
}

fn wait(
    endpoint: &Endpoint,
    path: &str,
    expected: &str,
    timeout: Duration,
    out: &mut impl Write,
    err: &mut impl Write,
) -> i32 {
    let deadline = Instant::now() + timeout;
    loop {
        let seen = match ask(endpoint, &Request::new("state")) {
            Ok(state) => {
                let seen = lookup(&state, path);
                if seen
                    .as_deref()
                    .is_some_and(|actual| actual.eq_ignore_ascii_case(expected))
                {
                    let _ = writeln!(out, "{path}={}", seen.unwrap_or_default());
                    return 0;
                }
                seen
            }
            Err(CtlError::Connect(message)) => {
                let _ = writeln!(err, "hookctl: {message}");
                return 2;
            }
            Err(CtlError::Hook(message)) => {
                let _ = writeln!(err, "hookctl: {message}");
                return 1;
            }
        };
        if Instant::now() >= deadline {
            let _ = writeln!(
                err,
                "hookctl: timed out after {} ms waiting for {path}={expected}; last value {}",
                timeout.as_millis(),
                seen.as_deref().unwrap_or("(not set)")
            );
            return 1;
        }
        std::thread::sleep(POLL);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::{Handler, HookServer};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn words(line: &str) -> Vec<String> {
        line.split_whitespace().map(String::from).collect()
    }

    fn parsed(line: &str) -> Command {
        parse(&words(line), Path::new("/work")).unwrap().1
    }

    #[test]
    fn simple_commands_become_requests() {
        assert_eq!(parsed("tree"), Command::Send(Request::new("tree")));
        assert_eq!(
            parsed("click sidebar.models"),
            Command::Send(Request::new("click").with("id", "sidebar.models"))
        );
        assert_eq!(parsed("net"), Command::Send(Request::new("net")));
    }

    #[test]
    fn action_lists_runs_and_carries_json_arguments() {
        assert_eq!(parsed("action"), Command::Send(Request::new("action")));
        assert_eq!(
            parsed("action open-view"),
            Command::Send(Request::new("action").with("name", "open-view"))
        );
        let with_args = parse(
            &[
                "action".into(),
                "open-view".into(),
                r#"{"view":"models"}"#.into(),
            ],
            Path::new("/work"),
        )
        .unwrap()
        .1;
        assert_eq!(
            with_args,
            Command::Send(
                Request::new("action")
                    .with("name", "open-view")
                    .with("args", json!({"view": "models"}))
            )
        );
        assert!(parse(&words("action x {bad"), Path::new("/")).is_err());
    }

    #[test]
    fn file_arguments_are_made_absolute() {
        assert_eq!(
            parsed("feed-wav speech.wav"),
            Command::Send(Request::new("feed-wav").with("path", "/work/speech.wav"))
        );
        assert_eq!(
            parsed("paths /abs/a.txt b.txt"),
            Command::Send(
                Request::new("paths").with("paths", json!(["/abs/a.txt", "/work/b.txt"]))
            )
        );
    }

    #[test]
    fn wait_takes_a_predicate_and_an_optional_timeout() {
        assert_eq!(
            parsed("wait state=Listening 2000"),
            Command::Wait {
                path: "state".into(),
                expected: "Listening".into(),
                timeout: Duration::from_millis(2000)
            }
        );
        assert!(
            matches!(parsed("wait view=models"), Command::Wait { timeout, .. } if timeout == Duration::from_millis(5000))
        );
        assert!(parse(&words("wait nothing"), Path::new("/")).is_err());
        assert!(parse(&words("wait a=b soon"), Path::new("/")).is_err());
    }

    #[test]
    fn flags_come_before_the_command() {
        let (options, _) = parse(&words("--socket /x/hook.sock state"), Path::new("/")).unwrap();
        assert_eq!(options.socket.as_deref(), Some("/x/hook.sock"));
        assert!(parse(&words("--socket"), Path::new("/")).is_err());
    }

    #[test]
    fn bad_command_lines_are_refused() {
        for line in ["", "bogus", "click", "click a b", "tree extra", "paths"] {
            assert!(parse(&words(line), Path::new("/")).is_err(), "{line:?}");
        }
    }

    #[test]
    fn lookup_reads_dotted_paths_and_falls_back_to_the_pipeline() {
        let state = json!({"view": "models", "window": {"width": 780, "resizable": false},
                           "pipeline": {"state": "listening"}, "engine": {"pid": null}});
        assert_eq!(lookup(&state, "view").as_deref(), Some("models"));
        assert_eq!(lookup(&state, "window.width").as_deref(), Some("780"));
        assert_eq!(lookup(&state, "window.resizable").as_deref(), Some("false"));
        assert_eq!(lookup(&state, "engine.pid").as_deref(), Some("null"));
        assert_eq!(lookup(&state, "state").as_deref(), Some("listening"));
        assert_eq!(lookup(&state, "nope"), None);
        assert!(matches(&state, "state", "Listening"));
        assert!(!matches(&state, "state", "idle"));
    }

    struct Counting(AtomicUsize);
    impl Handler for Counting {
        fn handle(&self, request: Request) -> Response {
            let n = self.0.fetch_add(1, Ordering::SeqCst);
            match request.cmd.as_str() {
                "state" => Response::Ok(
                    json!({"pipeline": {"state": if n >= 2 { "idle" } else { "listening" }}}),
                ),
                "tree" => Response::Ok(json!([{"id": "sidebar.home"}])),
                _ => Response::error("not today"),
            }
        }
    }

    struct Dir(PathBuf);
    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// A running hook. Fields drop in order: the server first, then the folder.
    struct Live {
        env: Env,
        _server: HookServer,
        _dir: Dir,
    }

    fn live(name: &str) -> Live {
        let dir = std::env::temp_dir().join(format!("hp-ctl-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let socket = dir.join("hook.sock");
        let server = HookServer::start(
            Endpoint::Unix(socket.clone()),
            Arc::new(Counting(AtomicUsize::new(0))),
        )
        .unwrap();
        Live {
            env: Env {
                socket: Some(socket.to_string_lossy().into_owned()),
                cwd: dir.clone(),
                ..Env::default()
            },
            _server: server,
            _dir: Dir(dir),
        }
    }

    fn run_line(line: &str, env: &Env) -> (i32, String, String) {
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = run(&words(line), env, &mut out, &mut err);
        (
            code,
            String::from_utf8(out).unwrap(),
            String::from_utf8(err).unwrap(),
        )
    }

    #[test]
    fn a_command_prints_the_reply_data_as_json() {
        let hook = live("print");
        let (code, out, _) = run_line("tree", &hook.env);
        assert_eq!(code, 0);
        assert_eq!(
            serde_json::from_str::<Value>(&out).unwrap()[0]["id"],
            "sidebar.home"
        );
    }

    #[test]
    fn a_refusal_from_the_hook_exits_1_with_its_message() {
        let hook = live("refuse");
        let (code, out, err) = run_line("click sidebar.home", &hook.env);
        assert_eq!((code, out.as_str()), (1, ""));
        assert!(err.contains("not today"), "{err}");
    }

    #[test]
    fn wait_polls_until_the_value_matches() {
        let hook = live("wait");
        let (code, out, _) = run_line("wait state=Idle 3000", &hook.env);
        assert_eq!(code, 0);
        assert_eq!(out.trim(), "state=idle");
    }

    #[test]
    fn wait_times_out_and_says_what_it_last_saw() {
        let hook = live("timeout");
        let (code, _, err) = run_line("wait state=recording 150", &hook.env);
        assert_eq!(code, 1);
        assert!(err.contains("timed out"), "{err}");
        assert!(err.contains("last value"), "{err}");
    }

    #[test]
    fn no_hook_to_reach_exits_2() {
        let env = Env {
            socket: Some("/nonexistent/hook.sock".into()),
            ..Env::default()
        };
        let (code, _, err) = run_line("state", &env);
        assert_eq!(code, 2);
        assert!(err.contains("cannot reach the hook"), "{err}");
        let (code, _, err) = run_line("state", &Env::default());
        assert_eq!(code, 2);
        assert!(err.contains("HUSHPEN_DATA_DIR"), "{err}");
    }
}
