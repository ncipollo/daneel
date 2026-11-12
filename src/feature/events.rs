//! Newline-delimited JSON events emitted by `--mode json`. Every line carries the
//! schema version `v` so consumers can detect changes.

use std::sync::Arc;

use serde::Serialize;
use serde_json::Value;

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EventKind {
    ToolCall { name: String, args: Value },
    ToolResult { name: String, output: String },
    Message { text: String },
    Done,
    Error { message: String },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Event {
    pub v: u32,
    #[serde(flatten)]
    pub kind: EventKind,
}

impl Event {
    pub fn new(kind: EventKind) -> Self {
        Self {
            v: SCHEMA_VERSION,
            kind,
        }
    }

    pub fn to_line(&self) -> String {
        serde_json::to_string(self).expect("events are always serializable")
    }
}

/// Receives events as they happen. Shared with tool wrappers, hence `Arc`.
pub type EventSink = Arc<dyn Fn(Event) + Send + Sync>;

pub fn discard() -> EventSink {
    Arc::new(|_| {})
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn message_line_has_version_and_type() {
        let line = Event::new(EventKind::Message { text: "hi".into() }).to_line();
        assert_eq!(line, r#"{"v":1,"type":"message","text":"hi"}"#);
    }

    #[test]
    fn tool_call_carries_structured_args() {
        let event = Event::new(EventKind::ToolCall {
            name: "read_file".into(),
            args: json!({ "path": "a" }),
        });
        let value: Value = serde_json::from_str(&event.to_line()).unwrap();
        assert_eq!(value["type"], "tool_call");
        assert_eq!(value["args"]["path"], "a");
    }

    #[test]
    fn done_has_no_payload() {
        assert_eq!(
            Event::new(EventKind::Done).to_line(),
            r#"{"v":1,"type":"done"}"#
        );
    }
}
