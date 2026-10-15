//! Test hook for the Import view: the `import` state section and the actions that add files,
//! cancel a job, and tidy the queue like the view's controls do.
//! Compiled only with the `test-automation` feature.

use super::{register_action, set_state_section};
use crate::import::Import;
use gpui_kit::{App, Entity};
use serde_json::{Value, json};
use std::path::PathBuf;

fn id_of(args: &Value) -> Result<u64, String> {
    args.get("id")
        .and_then(Value::as_u64)
        .ok_or_else(|| "needs args like {\"id\": 1}".to_string())
}

pub fn attach(cx: &mut App, import: Entity<Import>) {
    set_state_section(cx, "import", {
        let import = import.clone();
        move |cx| import.read(cx).state_json(cx)
    });
    let _ = register_action(
        cx,
        "import-add",
        "Add files like a drop on the Import view. Args: {\"paths\": [\"/path/a.mp3\"]}. A path \
         that is not a regular file is refused with a notice and is never opened. Returns how \
         many files were queued.",
        {
            let import = import.clone();
            move |cx, args| {
                let paths: Vec<PathBuf> = args
                    .get("paths")
                    .and_then(Value::as_array)
                    .map(|paths| {
                        paths
                            .iter()
                            .filter_map(Value::as_str)
                            .map(PathBuf::from)
                            .collect()
                    })
                    .ok_or("needs args like {\"paths\": [\"/path/a.mp3\"]}")?;
                let queued = import.update(cx, |import, cx| import.add_paths(paths, cx));
                Ok(json!({"queued": queued}))
            }
        },
    );
    let _ = register_action(
        cx,
        "import-choose",
        "Press Choose files. The picker answer comes from the dialog answer of the test hook.",
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
        "import-cancel",
        "Cancel a queued or running item like its Cancel button. Args: {\"id\": 1}. The ids are \
         in `hookctl state` section `import`.",
        {
            let import = import.clone();
            move |cx, args| {
                let id = id_of(&args)?;
                import.update(cx, |import, cx| import.cancel(id, cx))?;
                Ok(json!({"id": id}))
            }
        },
    );
    let _ = register_action(
        cx,
        "import-remove",
        "Take a finished item out of the list like its Remove button. Args: {\"id\": 1}.",
        {
            let import = import.clone();
            move |cx, args| {
                let id = id_of(&args)?;
                import.update(cx, |import, cx| import.remove(id, cx))?;
                Ok(json!({"id": id}))
            }
        },
    );
    let _ = register_action(cx, "import-clear", "Press Clear finished.", move |cx, _| {
        import.update(cx, |import, cx| import.clear_finished(cx));
        Ok(Value::Null)
    });
}
