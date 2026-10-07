//! Test hook for the model library: the `models` state section and the actions that
//! download, cancel, delete, and select a model like the Models view does.
//! Compiled only with the `test-automation` feature.

use super::{register_action, set_state_section};
use crate::models::Models;
use gpui_kit::{App, Entity};
use serde_json::{Value, json};

fn model_id(args: &Value) -> Result<String, String> {
    args.get("id")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| "needs args like {\"id\": \"base.en\"}".to_string())
}

pub fn attach(cx: &mut App, models: Entity<Models>) {
    set_state_section(cx, "models", {
        let models = models.clone();
        move |cx| models.read(cx).state_json()
    });

    let _ = register_action(
        cx,
        "model-download",
        "Start or resume a model download like the Download button. Args: {\"id\": \"base.en\"}. \
         Progress is `models.models[].progress` in `hookctl state`.",
        {
            let models = models.clone();
            move |cx, args| {
                let id = model_id(&args)?;
                models.update(cx, |models, cx| models.download(&id, cx))?;
                Ok(json!({"id": id}))
            }
        },
    );
    let _ = register_action(
        cx,
        "model-cancel",
        "Cancel a running download like the Cancel button. Args: {\"id\": \"base.en\"}.",
        {
            let models = models.clone();
            move |cx, args| {
                let id = model_id(&args)?;
                models.update(cx, |models, cx| models.cancel(&id, cx))?;
                Ok(json!({"id": id}))
            }
        },
    );
    let _ = register_action(
        cx,
        "model-delete",
        "Delete a downloaded model like the Delete button. The active model is refused with \
         MODEL_IN_USE. Args: {\"id\": \"base.en\"}.",
        {
            let models = models.clone();
            move |cx, args| {
                let id = model_id(&args)?;
                models.update(cx, |models, cx| models.delete(&id, cx))?;
                Ok(json!({"id": id}))
            }
        },
    );
    let _ = register_action(
        cx,
        "model-select",
        "Make a verified model the active one like the Use button. A model that failed \
         verification is refused. Args: {\"id\": \"base.en\"}.",
        move |cx, args| {
            let id = model_id(&args)?;
            models.update(cx, |models, cx| models.select(&id, cx))?;
            Ok(json!({"id": id}))
        },
    );
}
