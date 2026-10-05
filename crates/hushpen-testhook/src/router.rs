//! Maps hook commands to a [`Backend`]. The network log, the event log, and
//! the dialog answers live here because they need no UI thread.

use crate::protocol::{ActionInfo, ElementInfo, Request, Response};
use crate::server::Handler;
use crate::{dialogs, logs};
use serde_json::Value;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

/// What the app answers. Every method may run the reading on the UI thread
/// and wait for it.
pub trait Backend: Send + Sync + 'static {
    fn tree(&self) -> Result<Vec<ElementInfo>, String>;
    fn state(&self) -> Result<Value, String>;
    fn click(&self, id: &str) -> Result<Value, String>;
    fn actions(&self) -> Result<Vec<ActionInfo>, String>;
    fn run_action(&self, name: &str, args: Value) -> Result<Value, String>;
    /// Feeds a WAV file into the capture path as if it came from the mic.
    fn feed_wav(&self, path: &Path) -> Result<Value, String>;
}

pub struct Router<B> {
    backend: B,
}

impl<B: Backend> Router<B> {
    pub fn new(backend: B) -> Self {
        Self { backend }
    }

    fn dispatch(&self, request: &Request) -> Result<Value, String> {
        match request.cmd.as_str() {
            "ping" => Ok(Value::String("pong".into())),
            "tree" => Ok(Value::Array(
                self.backend
                    .tree()?
                    .iter()
                    .map(ElementInfo::to_json)
                    .collect(),
            )),
            "state" => self.backend.state(),
            "click" => self.backend.click(required(request, "id")?),
            "action" => match request.str_arg("name") {
                None => Ok(Value::Array(
                    self.backend
                        .actions()?
                        .iter()
                        .map(ActionInfo::to_json)
                        .collect(),
                )),
                Some(name) => self.backend.run_action(
                    name,
                    request.args.get("args").cloned().unwrap_or(Value::Null),
                ),
            },
            "net" => Ok(Value::Array(
                logs::net_entries()
                    .iter()
                    .map(logs::NetEntry::to_json)
                    .collect(),
            )),
            "events" => Ok(Value::Array(
                logs::event_entries()
                    .iter()
                    .map(logs::Event::to_json)
                    .collect(),
            )),
            "feed-wav" => {
                let path = PathBuf::from(required(request, "path")?);
                check_wav(&path)?;
                self.backend.feed_wav(&path)
            }
            "paths" => {
                let paths: Vec<PathBuf> = request
                    .args
                    .get("paths")
                    .and_then(Value::as_array)
                    .ok_or("\"paths\" must be an array of file paths")?
                    .iter()
                    .filter_map(Value::as_str)
                    .map(PathBuf::from)
                    .collect();
                if paths.is_empty() {
                    return Err("\"paths\" needs at least one file path".into());
                }
                let count = paths.len();
                dialogs::answer_next_dialog(paths);
                Ok(serde_json::json!({"queued": count}))
            }
            other => Err(format!(
                "unknown command '{other}'; try tree, state, click, action, net, events, feed-wav, paths"
            )),
        }
    }
}

impl<B: Backend> Handler for Router<B> {
    fn handle(&self, request: Request) -> Response {
        match self.dispatch(&request) {
            Ok(data) => Response::Ok(data),
            Err(error) => Response::Err(error),
        }
    }
}

fn required<'a>(request: &'a Request, key: &str) -> Result<&'a str, String> {
    request
        .str_arg(key)
        .ok_or_else(|| format!("`{}` needs a string \"{key}\"", request.cmd))
}

fn check_wav(path: &Path) -> Result<(), String> {
    let mut header = [0u8; 12];
    File::open(path)
        .and_then(|mut file| file.read_exact(&mut header))
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    if &header[..4] == b"RIFF" && &header[8..] == b"WAVE" {
        Ok(())
    } else {
        Err(format!("{} is not a WAV file", path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::Bounds;
    use serde_json::json;
    use std::sync::Mutex;

    #[derive(Default)]
    struct Fake {
        clicked: Mutex<Vec<String>>,
        ran: Mutex<Vec<(String, Value)>>,
        fed: Mutex<Vec<PathBuf>>,
    }

    impl Backend for Fake {
        fn tree(&self) -> Result<Vec<ElementInfo>, String> {
            let bounds = Bounds {
                x: 8.0,
                y: 48.0,
                width: 192.0,
                height: 36.0,
            };
            Ok(vec![ElementInfo {
                id: "sidebar.home".into(),
                role: None,
                text: "Home".into(),
                bounds,
                root_bounds: bounds,
                enabled: true,
                focused: false,
                visible: true,
            }])
        }
        fn state(&self) -> Result<Value, String> {
            Ok(json!({"pipeline": {"state": "idle"}}))
        }
        fn click(&self, id: &str) -> Result<Value, String> {
            if id == "missing" {
                return Err("no element 'missing'".into());
            }
            self.clicked.lock().unwrap().push(id.into());
            Ok(Value::Null)
        }
        fn actions(&self) -> Result<Vec<ActionInfo>, String> {
            Ok(vec![ActionInfo {
                name: "open-view".into(),
                description: "Open a view".into(),
            }])
        }
        fn run_action(&self, name: &str, args: Value) -> Result<Value, String> {
            self.ran.lock().unwrap().push((name.into(), args));
            Ok(json!({"ran": name}))
        }
        fn feed_wav(&self, path: &Path) -> Result<Value, String> {
            self.fed.lock().unwrap().push(path.to_path_buf());
            Ok(Value::Null)
        }
    }

    fn router() -> Router<Fake> {
        Router::new(Fake::default())
    }

    fn data(response: Response) -> Value {
        match response {
            Response::Ok(data) => data,
            Response::Err(error) => panic!("unexpected error: {error}"),
        }
    }

    fn scratch_file(name: &str, bytes: &[u8]) -> PathBuf {
        let path = std::env::temp_dir().join(format!("hp-router-{}-{name}", std::process::id()));
        std::fs::write(&path, bytes).unwrap();
        path
    }

    #[test]
    fn tree_lists_elements_with_the_contract_fields() {
        let tree = data(router().handle(Request::new("tree")));
        assert_eq!(tree[0]["id"], "sidebar.home");
        assert_eq!(tree[0]["text"], "Home");
        assert_eq!(tree[0]["bounds"]["width"], 192.0);
    }

    #[test]
    fn state_comes_from_the_backend() {
        let state = data(router().handle(Request::new("state")));
        assert_eq!(state["pipeline"]["state"], "idle");
    }

    #[test]
    fn click_passes_the_id_and_reports_backend_errors() {
        let router = router();
        data(router.handle(Request::new("click").with("id", "sidebar.models")));
        assert_eq!(*router.backend.clicked.lock().unwrap(), ["sidebar.models"]);
        let failed = router.handle(Request::new("click").with("id", "missing"));
        assert_eq!(failed, Response::error("no element 'missing'"));
        assert!(matches!(
            router.handle(Request::new("click")),
            Response::Err(message) if message.contains("\"id\"")
        ));
    }

    #[test]
    fn action_without_a_name_lists_and_with_a_name_runs() {
        let router = router();
        let listed = data(router.handle(Request::new("action")));
        assert_eq!(listed[0]["name"], "open-view");
        let reply = data(
            router.handle(
                Request::new("action")
                    .with("name", "open-view")
                    .with("args", json!({"view": "models"})),
            ),
        );
        assert_eq!(reply["ran"], "open-view");
        assert_eq!(
            router.backend.ran.lock().unwrap()[0],
            ("open-view".to_string(), json!({"view": "models"}))
        );
    }

    #[test]
    fn net_prints_each_recorded_request_with_purpose_and_host() {
        logs::record_net("ModelDownload", "router-test.example", "ok");
        let entries = data(router().handle(Request::new("net")));
        let found = entries.as_array().unwrap().iter().find(|entry| {
            entry["host"] == "router-test.example" && entry["purpose"] == "ModelDownload"
        });
        let entry = found.expect("the recorded request is listed");
        assert_eq!(entry["result"], "ok");
        assert!(entry["time"].as_str().unwrap().ends_with('Z'));
    }

    #[test]
    fn events_list_recorded_transitions() {
        logs::record_event("router-test", "view=models");
        let events = data(router().handle(Request::new("events")));
        assert!(
            events.as_array().unwrap().iter().any(|event| {
                event["kind"] == "router-test" && event["detail"] == "view=models"
            })
        );
    }

    #[test]
    fn feed_wav_checks_the_file_before_the_backend_sees_it() {
        let router = router();
        let wav = scratch_file("ok.wav", b"RIFF\x24\x00\x00\x00WAVEfmt ");
        data(router.handle(Request::new("feed-wav").with("path", wav.to_str().unwrap())));
        assert_eq!(
            *router.backend.fed.lock().unwrap(),
            std::slice::from_ref(&wav)
        );

        let text = scratch_file("no.wav", b"hello world, not a wav");
        let refused = router.handle(Request::new("feed-wav").with("path", text.to_str().unwrap()));
        assert!(matches!(refused, Response::Err(m) if m.contains("not a WAV")));
        let missing = router.handle(Request::new("feed-wav").with("path", "/nonexistent/x.wav"));
        assert!(matches!(missing, Response::Err(m) if m.contains("cannot read")));
        assert_eq!(router.backend.fed.lock().unwrap().len(), 1);
        let _ = std::fs::remove_file(wav);
        let _ = std::fs::remove_file(text);
    }

    #[test]
    fn paths_queue_an_answer_for_the_next_dialog() {
        let _serial = dialogs::TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        while dialogs::take_answer().is_some() {}
        let reply = data(
            router()
                .handle(Request::new("paths").with("paths", json!(["/tmp/a.txt", "/tmp/b.txt"]))),
        );
        assert_eq!(reply["queued"], 2);
        assert_eq!(dialogs::take_answer().map(|paths| paths.len()), Some(2));
        assert!(matches!(
            router().handle(Request::new("paths").with("paths", json!([]))),
            Response::Err(_)
        ));
    }

    #[test]
    fn unknown_commands_list_the_known_ones() {
        let reply = router().handle(Request::new("explode"));
        assert!(
            matches!(reply, Response::Err(m) if m.contains("unknown command 'explode'") && m.contains("tree"))
        );
    }
}
