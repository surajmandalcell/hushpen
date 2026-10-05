//! Wire format: one JSON object per line in each direction.
//!
//! Request: `{"cmd":"click","id":"sidebar.models"}`. Every key besides `cmd`
//! is an argument. Reply: `{"ok":true,"data":...}` or `{"ok":false,"error":"..."}`.

use serde_json::{Map, Value, json};

#[derive(Debug, Clone, PartialEq)]
pub struct Request {
    pub cmd: String,
    pub args: Map<String, Value>,
}

impl Request {
    pub fn new(cmd: &str) -> Self {
        Self {
            cmd: cmd.to_string(),
            args: Map::new(),
        }
    }

    pub fn with(mut self, key: &str, value: impl Into<Value>) -> Self {
        self.args.insert(key.to_string(), value.into());
        self
    }

    pub fn parse(line: &str) -> Result<Self, String> {
        let value: Value =
            serde_json::from_str(line).map_err(|error| format!("not valid JSON: {error}"))?;
        let Value::Object(mut args) = value else {
            return Err("a request must be a JSON object".to_string());
        };
        match args.remove("cmd") {
            Some(Value::String(cmd)) if !cmd.is_empty() => Ok(Self { cmd, args }),
            _ => Err("a request needs a string \"cmd\"".to_string()),
        }
    }

    pub fn to_line(&self) -> String {
        let mut object = self.args.clone();
        object.insert("cmd".to_string(), Value::String(self.cmd.clone()));
        Value::Object(object).to_string()
    }

    pub fn str_arg(&self, key: &str) -> Option<&str> {
        self.args.get(key).and_then(Value::as_str)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Response {
    Ok(Value),
    Err(String),
}

impl Response {
    pub fn error(message: impl Into<String>) -> Self {
        Response::Err(message.into())
    }

    pub fn to_line(&self) -> String {
        match self {
            Response::Ok(data) => json!({"ok": true, "data": data}).to_string(),
            Response::Err(error) => json!({"ok": false, "error": error}).to_string(),
        }
    }

    pub fn parse(line: &str) -> Result<Self, String> {
        let value: Value =
            serde_json::from_str(line).map_err(|error| format!("not valid JSON: {error}"))?;
        match value.get("ok").and_then(Value::as_bool) {
            Some(true) => Ok(Response::Ok(
                value.get("data").cloned().unwrap_or(Value::Null),
            )),
            Some(false) => Ok(Response::Err(
                value
                    .get("error")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown error")
                    .to_string(),
            )),
            None => Err("a reply needs a boolean \"ok\"".to_string()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Bounds {
    pub fn to_json(self) -> Value {
        json!({"x": self.x, "y": self.y, "width": self.width, "height": self.height})
    }
}

/// One element in the window tree.
#[derive(Debug, Clone, PartialEq)]
pub struct ElementInfo {
    /// Stable id, `<view>.<element>[.<index>]`.
    pub id: String,
    pub role: Option<String>,
    /// Visible text, or the accessibility label when that is all there is.
    pub text: String,
    /// Relative to the window's top-left corner.
    pub bounds: Bounds,
    /// Relative to the screen (window origin plus `bounds`).
    pub root_bounds: Bounds,
    pub enabled: bool,
    pub focused: bool,
    pub visible: bool,
}

impl ElementInfo {
    pub fn to_json(&self) -> Value {
        json!({
            "id": self.id,
            "role": self.role,
            "text": self.text,
            "bounds": self.bounds.to_json(),
            "root_bounds": self.root_bounds.to_json(),
            "enabled": self.enabled,
            "focused": self.focused,
            "visible": self.visible,
        })
    }
}

/// A named action that `hookctl action <name>` can run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionInfo {
    pub name: String,
    pub description: String,
}

impl ActionInfo {
    pub fn to_json(&self) -> Value {
        json!({"name": self.name, "description": self.description})
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_request_round_trips_through_a_line() {
        let request = Request::new("click").with("id", "sidebar.models");
        let parsed = Request::parse(&request.to_line()).unwrap();
        assert_eq!(parsed.cmd, "click");
        assert_eq!(parsed.str_arg("id"), Some("sidebar.models"));
    }

    #[test]
    fn bad_requests_are_refused_with_a_reason() {
        assert!(Request::parse("nope").unwrap_err().contains("JSON"));
        assert!(Request::parse("[1]").unwrap_err().contains("object"));
        assert!(Request::parse("{}").unwrap_err().contains("cmd"));
        assert!(Request::parse(r#"{"cmd":""}"#).unwrap_err().contains("cmd"));
    }

    #[test]
    fn replies_round_trip_for_data_and_errors() {
        let ok = Response::Ok(json!({"a": 1}));
        assert_eq!(Response::parse(&ok.to_line()).unwrap(), ok);
        let err = Response::error("no such element");
        assert_eq!(Response::parse(&err.to_line()).unwrap(), err);
        assert!(Response::parse(r#"{"data":1}"#).is_err());
    }

    #[test]
    fn an_element_serializes_every_field_the_contract_names() {
        let info = ElementInfo {
            id: "sidebar.home".into(),
            role: Some("Button".into()),
            text: "Home".into(),
            bounds: Bounds {
                x: 8.0,
                y: 48.0,
                width: 192.0,
                height: 36.0,
            },
            root_bounds: Bounds {
                x: 108.0,
                y: 148.0,
                width: 192.0,
                height: 36.0,
            },
            enabled: true,
            focused: false,
            visible: true,
        };
        let value = info.to_json();
        for key in [
            "id",
            "role",
            "text",
            "bounds",
            "root_bounds",
            "enabled",
            "focused",
            "visible",
        ] {
            assert!(value.get(key).is_some(), "missing {key}");
        }
        assert_eq!(value["bounds"]["width"], 192.0);
        assert_eq!(value["root_bounds"]["x"], 108.0);
    }
}
