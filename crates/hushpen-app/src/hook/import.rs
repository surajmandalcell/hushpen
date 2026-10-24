//! Test hook for the Files view in Transcribe mode: the `files` and `import` state sections and
//! the actions that drop files, switch the mode, and drive the queue like the view's controls do.
//! Compiled only with the `test-automation` feature.

use super::{register_action, set_state_section};
use crate::files::{Files, Mode};
use crate::import::Import;
use gpui_kit::{App, Context, Entity};
use hushpen_core::export::Format;
use serde_json::{Value, json};
use std::path::PathBuf;

type Action = fn(&mut Import, u64, &mut Context<Import>) -> Result<(), String>;

fn id_of(args: &Value) -> Result<u64, String> {
    args.get("id")
        .and_then(Value::as_u64)
        .ok_or_else(|| "needs args like {\"id\": 1}".to_string())
}

fn paths_of(args: &Value) -> Result<Vec<PathBuf>, String> {
    args.get("paths")
        .and_then(Value::as_array)
        .map(|paths| {
            paths
                .iter()
                .filter_map(Value::as_str)
                .map(PathBuf::from)
                .collect()
        })
        .ok_or_else(|| "needs args like {\"paths\": [\"/path/a.mp3\"]}".to_string())
}

pub fn attach(cx: &mut App, files: Entity<Files>, import: Entity<Import>) {
    set_state_section(cx, "files", {
        let files = files.clone();
        move |cx| files.read(cx).state_json(cx)
    });
    set_state_section(cx, "import", {
        let import = import.clone();
        move |cx| import.read(cx).state_json(cx)
    });
    let _ = register_action(
        cx,
        "files-mode",
        "Pick a mode in the Files view. Args: {\"mode\": \"transcribe\" | \"readAloud\"}.",
        {
            let files = files.clone();
            move |cx, args| {
                let mode = args
                    .get("mode")
                    .and_then(Value::as_str)
                    .and_then(Mode::from_key)
                    .ok_or("needs args like {\"mode\": \"readAloud\"}")?;
                files.update(cx, |files, cx| files.set_mode(mode, cx))?;
                Ok(json!({"mode": mode.key()}))
            }
        },
    );
    let _ = register_action(
        cx,
        "files-drop",
        "Drop files on the window. Args: {\"paths\": [\"/path/a.mp3\"]}. Text files open in Read \
         aloud; anything else goes in the queue.",
        move |cx, args| {
            let paths = paths_of(&args)?;
            files.update(cx, |files, cx| files.drop_paths(paths, cx));
            Ok(Value::Null)
        },
    );
    let _ = register_action(
        cx,
        "import-add",
        "Add files like a drop on the drop zone. Args: {\"paths\": [\"/path/a.mp3\"]}. A path \
         that is not a regular file is refused with a notice and is never opened. Returns how \
         many files were queued.",
        {
            let import = import.clone();
            move |cx, args| {
                let paths = paths_of(&args)?;
                let queued = import.update(cx, |import, cx| import.add_paths(paths, cx));
                Ok(json!({"queued": queued}))
            }
        },
    );
    let _ = register_action(
        cx,
        "import-choose",
        "Press Choose Files. The picker answer comes from the dialog answer of the test hook.",
        {
            let import = import.clone();
            move |cx, _| {
                import.update(cx, |import, cx| import.choose(cx))?;
                Ok(Value::Null)
            }
        },
    );
    let _ = register_action(
        cx,
        "import-language",
        "Pick the transcription language. Args: {\"code\": \"de\"} or {\"code\": \"auto\"}.",
        {
            let import = import.clone();
            move |cx, args| {
                let code = args
                    .get("code")
                    .and_then(Value::as_str)
                    .ok_or("needs args like {\"code\": \"de\"}")?
                    .to_owned();
                import.update(cx, |import, cx| import.set_language(&code, cx))?;
                Ok(json!({"code": code}))
            }
        },
    );
    let _ = register_action(
        cx,
        "import-translate",
        "Turn Translate to English on or off. Args: {\"on\": true}. Refused for English-only and \
         turbo models.",
        {
            let import = import.clone();
            move |cx, args| {
                let on = args
                    .get("on")
                    .and_then(Value::as_bool)
                    .ok_or("needs args like {\"on\": true}")?;
                import.update(cx, |import, cx| import.set_translate(on, cx))?;
                Ok(json!({"on": on}))
            }
        },
    );
    let _ = register_action(
        cx,
        "import-export",
        "Export a finished item like its Export menu. Args: {\"id\": 1, \"format\": \"srt\"}. \
         The save dialog answer comes from the dialog answer of the test hook.",
        {
            let import = import.clone();
            move |cx, args| {
                let id = id_of(&args)?;
                let format = args
                    .get("format")
                    .and_then(Value::as_str)
                    .and_then(Format::from_key)
                    .ok_or("needs args like {\"id\": 1, \"format\": \"srt\"}")?;
                import.update(cx, |import, cx| import.export(id, format, cx))?;
                Ok(json!({"id": id, "format": format.extension()}))
            }
        },
    );
    let actions: [(&str, &str, Action); 5] = [
        (
            "import-cancel",
            "Cancel a queued or running item like its Cancel button. Args: {\"id\": 1}. The ids \
             are in `hookctl state` section `import`.",
            Import::cancel,
        ),
        (
            "import-remove",
            "Take a finished item out of the list like its Remove button. Args: {\"id\": 1}.",
            Import::remove,
        ),
        (
            "import-retry",
            "Queue a failed item again like its Retry button. Args: {\"id\": 1}.",
            Import::retry,
        ),
        (
            "import-copy",
            "Copy the transcript of a finished item like its Copy button. Args: {\"id\": 1}.",
            Import::copy,
        ),
        (
            "import-show",
            "Open a finished item in History like its Show in History button. Args: {\"id\": 1}.",
            Import::show_in_history,
        ),
    ];
    for (name, description, action) in actions {
        let import = import.clone();
        let _ = register_action(cx, name, description, move |cx, args| {
            let id = id_of(&args)?;
            import.update(cx, |import, cx| action(import, id, cx))?;
            Ok(json!({"id": id}))
        });
    }
    let _ = register_action(cx, "import-clear", "Press Clear Finished.", move |cx, _| {
        import.update(cx, |import, cx| import.clear_finished(cx));
        Ok(Value::Null)
    });
}
