//! Registries that features fill in. `C` is the context the handlers run
//! with; the app uses its GPUI `App`, so a hook action goes through the same
//! code as a click or a key.

use crate::protocol::ActionInfo;
use serde_json::{Map, Value};
use std::collections::BTreeMap;

type ActionFn<C> = Box<dyn Fn(&mut C, Value) -> Result<Value, String>>;
type SectionFn<C> = Box<dyn Fn(&mut C) -> Value>;

struct Action<C> {
    description: String,
    run: ActionFn<C>,
}

pub struct ActionRegistry<C> {
    actions: BTreeMap<String, Action<C>>,
}

impl<C> Default for ActionRegistry<C> {
    fn default() -> Self {
        Self {
            actions: BTreeMap::new(),
        }
    }
}

impl<C> ActionRegistry<C> {
    /// Names are lower-case words joined by hyphens, like `open-view`. A name
    /// can be registered once.
    pub fn register(
        &mut self,
        name: &str,
        description: &str,
        run: impl Fn(&mut C, Value) -> Result<Value, String> + 'static,
    ) -> Result<(), String> {
        if !valid_name(name) {
            return Err(format!(
                "action name '{name}' must be lower-case words joined by hyphens"
            ));
        }
        if self.actions.contains_key(name) {
            return Err(format!("action '{name}' is already registered"));
        }
        self.actions.insert(
            name.to_string(),
            Action {
                description: description.to_string(),
                run: Box::new(run),
            },
        );
        Ok(())
    }

    pub fn list(&self) -> Vec<ActionInfo> {
        self.actions
            .iter()
            .map(|(name, action)| ActionInfo {
                name: name.clone(),
                description: action.description.clone(),
            })
            .collect()
    }

    pub fn run(&self, cx: &mut C, name: &str, args: Value) -> Result<Value, String> {
        match self.actions.get(name) {
            Some(action) => (action.run)(cx, args),
            None => {
                let known: Vec<_> = self.actions.keys().map(String::as_str).collect();
                Err(format!(
                    "unknown action '{name}'; registered: {}",
                    if known.is_empty() {
                        "none".to_string()
                    } else {
                        known.join(", ")
                    }
                ))
            }
        }
    }
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('-')
        && !name.ends_with('-')
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// Named sections of `hookctl state`. A later feature replaces the section of
/// the same name with the real reading.
pub struct StateRegistry<C> {
    sections: BTreeMap<String, SectionFn<C>>,
}

impl<C> Default for StateRegistry<C> {
    fn default() -> Self {
        Self {
            sections: BTreeMap::new(),
        }
    }
}

impl<C> StateRegistry<C> {
    pub fn set_section(&mut self, name: &str, read: impl Fn(&mut C) -> Value + 'static) {
        self.sections.insert(name.to_string(), Box::new(read));
    }

    pub fn snapshot(&self, cx: &mut C) -> Value {
        let mut object = Map::new();
        for (name, read) in &self.sections {
            object.insert(name.clone(), read(cx));
        }
        Value::Object(object)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_registered_action_runs_with_the_context_and_arguments() {
        let mut registry = ActionRegistry::<Vec<String>>::default();
        registry
            .register("open-view", "Open a view", |seen, args| {
                seen.push(args["view"].as_str().unwrap_or("").to_string());
                Ok(json!({"opened": args["view"]}))
            })
            .unwrap();
        let mut seen = Vec::new();
        let reply = registry
            .run(&mut seen, "open-view", json!({"view": "models"}))
            .unwrap();
        assert_eq!(seen, ["models"]);
        assert_eq!(reply["opened"], "models");
    }

    #[test]
    fn listing_is_sorted_and_carries_descriptions() {
        let mut registry = ActionRegistry::<()>::default();
        registry
            .register("zeta", "Last", |_, _| Ok(Value::Null))
            .unwrap();
        registry
            .register("alpha", "First", |_, _| Ok(Value::Null))
            .unwrap();
        let names: Vec<_> = registry.list().into_iter().map(|a| a.name).collect();
        assert_eq!(names, ["alpha", "zeta"]);
        assert_eq!(registry.list()[0].description, "First");
    }

    #[test]
    fn an_unknown_action_names_the_registered_ones() {
        let mut registry = ActionRegistry::<()>::default();
        registry
            .register("alpha", "", |_, _| Ok(Value::Null))
            .unwrap();
        let error = registry.run(&mut (), "beta", Value::Null).unwrap_err();
        assert!(error.contains("unknown action 'beta'"), "{error}");
        assert!(error.contains("alpha"), "{error}");
    }

    #[test]
    fn duplicate_and_badly_named_actions_are_refused() {
        let mut registry = ActionRegistry::<()>::default();
        registry
            .register("alpha", "", |_, _| Ok(Value::Null))
            .unwrap();
        assert!(
            registry
                .register("alpha", "", |_, _| Ok(Value::Null))
                .is_err()
        );
        for bad in ["", "Alpha", "a b", "-a", "a-", "a_b"] {
            assert!(
                registry.register(bad, "", |_, _| Ok(Value::Null)).is_err(),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn an_action_error_reaches_the_caller() {
        let mut registry = ActionRegistry::<()>::default();
        registry
            .register("fail", "", |_, _| Err("no such view".to_string()))
            .unwrap();
        assert_eq!(
            registry.run(&mut (), "fail", Value::Null),
            Err("no such view".to_string())
        );
    }

    #[test]
    fn a_later_section_replaces_an_earlier_one() {
        let mut registry = StateRegistry::<u32>::default();
        registry.set_section("pipeline", |_| json!({"state": "idle"}));
        registry.set_section("count", |n| json!(*n));
        registry.set_section("pipeline", |_| json!({"state": "listening"}));
        let mut n = 7;
        let state = registry.snapshot(&mut n);
        assert_eq!(state["pipeline"]["state"], "listening");
        assert_eq!(state["count"], 7);
    }
}
