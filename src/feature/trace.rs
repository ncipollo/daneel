//! Tees the event stream into the session's `trace.jsonl`. The machines run one
//! phase at a time, so the latest `Phase` event names the scope and phase that
//! tool calls belong to.

use std::sync::{Arc, Mutex};

use crate::feature::events::{Event, EventKind, EventSink};
use crate::feature::session::Session;
use crate::feature::tools::output::truncate;

const MAX_TRACED_OUTPUT: usize = 500;

#[derive(Default)]
struct Position {
    scope: String,
    phase: String,
    /// Where the strategy was when a subtask started, restored when it ends.
    resume: Option<(String, String)>,
}

/// Records every event in `session`'s trace, then passes it on to `inner`.
/// Trace failures are ignored: a trace must never break a run.
pub fn tee(session: Session, inner: EventSink) -> EventSink {
    let position = Arc::new(Mutex::new(Position::default()));
    Arc::new(move |event: Event| {
        let mut here = position.lock().unwrap_or_else(|e| e.into_inner());
        if let EventKind::Phase { scope, name, .. } = &event.kind {
            here.scope = scope.clone();
            here.phase = name.clone();
        }
        if matches!(event.kind, EventKind::SubtaskStart { .. }) {
            here.resume = Some((here.scope.clone(), here.phase.clone()));
        }
        let _ = session.append_trace(&here.scope, &here.phase, &shortened(&event));
        if matches!(event.kind, EventKind::SubtaskEnd { .. })
            && let Some((scope, phase)) = here.resume.take()
        {
            (here.scope, here.phase) = (scope, phase);
        }
        drop(here);
        inner(event);
    })
}

fn shortened(event: &Event) -> Event {
    match &event.kind {
        EventKind::ToolResult { name, output } => Event::new(EventKind::ToolResult {
            name: name.clone(),
            output: truncate(output.clone(), MAX_TRACED_OUTPUT),
        }),
        _ => event.clone(),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;
    use std::time::UNIX_EPOCH;

    use serde_json::{Value, json};
    use tempfile::TempDir;

    use super::*;
    use crate::feature::events;

    fn trace_of(session: &Session) -> Vec<Value> {
        let text = fs::read_to_string(session.dir().join("trace.jsonl")).unwrap();
        text.lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }

    #[test]
    fn tool_events_get_the_scope_of_the_latest_phase() {
        let root = TempDir::new().unwrap();
        let session = Session::create(root.path(), UNIX_EPOCH, Path::new("/w"), "m").unwrap();
        let (inner, seen) = events::collector();
        let sink = tee(session.clone(), inner);
        sink(Event::new(EventKind::Phase {
            scope: "subtask 2".into(),
            name: "Execute".into(),
            number: 2,
            total: 4,
        }));
        sink(Event::new(EventKind::ToolCall {
            name: "read_file".into(),
            args: json!({ "path": "a" }),
        }));
        let lines = trace_of(&session);
        assert_eq!(lines[1]["event"]["type"], "tool_call");
        assert_eq!(lines[1]["scope"], "subtask 2");
        assert_eq!(lines[1]["phase"], "Execute");
        assert_eq!(seen.lock().unwrap().len(), 2);
    }

    #[test]
    fn long_tool_output_is_truncated_in_the_trace_only() {
        let root = TempDir::new().unwrap();
        let session = Session::create(root.path(), UNIX_EPOCH, Path::new("/w"), "m").unwrap();
        let (inner, seen) = events::collector();
        let sink = tee(session.clone(), inner);
        let output = "x".repeat(2_000);
        sink(Event::new(EventKind::ToolResult {
            name: "read_file".into(),
            output: output.clone(),
        }));
        let traced = trace_of(&session)[0]["event"]["output"]
            .as_str()
            .unwrap()
            .to_string();
        assert!(traced.contains("[output truncated"));
        assert!(
            matches!(&seen.lock().unwrap()[0], EventKind::ToolResult { output: o, .. } if *o == output)
        );
    }
}
