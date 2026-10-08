//! Test hook for the personal dictionary: the `dictionary` state section and the actions that
//! add, edit, and delete an entry like the Dictionary view does.
//! Compiled only with the `test-automation` feature.

use super::{register_action, set_state_section};
use crate::dictionary::Dictionary;
use gpui_kit::{App, Entity};
use serde_json::{Value, json};

fn text<'a>(args: &'a Value, key: &str) -> &'a str {
    args.get(key).and_then(Value::as_str).unwrap_or("")
}

fn id_of(args: &Value) -> Result<i64, String> {
    args.get("id")
        .and_then(Value::as_i64)
        .ok_or_else(|| "needs args like {\"id\": 1, \"phrase\": \"Vixen\"}".to_string())
}

pub fn attach(cx: &mut App, dictionary: Entity<Dictionary>) {
    set_state_section(cx, "dictionary", {
        let dictionary = dictionary.clone();
        move |cx| dictionary.read(cx).state_json(cx)
    });
    let _ = register_action(
        cx,
        "dictionary-add",
        "Save a dictionary entry like the Add button. A word has no heard_as. Args: \
         {\"phrase\": \"Foxtrel\", \"heard_as\": \"fox\"}. An empty or repeated phrase is refused \
         with the same message the view shows.",
        {
            let dictionary = dictionary.clone();
            move |cx, args| {
                dictionary.update(cx, |dictionary, cx| {
                    dictionary.add(text(&args, "phrase"), text(&args, "heard_as"), cx)
                })?;
                Ok(Value::Null)
            }
        },
    );
    let _ = register_action(
        cx,
        "dictionary-edit",
        "Change an entry like the Save button after Edit. Args: {\"id\": 1, \"phrase\": \"Vixen\", \
         \"heard_as\": \"fox\"}. The ids are in `hookctl state` section `dictionary`.",
        {
            let dictionary = dictionary.clone();
            move |cx, args| {
                let id = id_of(&args)?;
                dictionary.update(cx, |dictionary, cx| {
                    dictionary.update(id, text(&args, "phrase"), text(&args, "heard_as"), cx)
                })?;
                Ok(json!({"id": id}))
            }
        },
    );
    let _ = register_action(
        cx,
        "dictionary-delete",
        "Delete an entry like the Delete button. Args: {\"id\": 1}.",
        move |cx, args| {
            let id = id_of(&args)?;
            dictionary.update(cx, |dictionary, cx| dictionary.remove(id, cx))?;
            Ok(json!({"id": id}))
        },
    );
}
