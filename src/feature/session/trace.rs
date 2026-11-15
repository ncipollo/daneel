//! `trace.jsonl`: every event of a run in order, stamped with the time and the
//! scope (`strategy` or `subtask N`) and phase it happened in.

use std::io;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

use crate::feature::events::Event;
use crate::feature::session::Session;
use crate::feature::session::store::append_line;

const TRACE_FILE: &str = "trace.jsonl";

#[derive(Serialize)]
pub struct TraceLine<'a> {
    pub ts_ms: u128,
    pub scope: &'a str,
    pub phase: &'a str,
    pub event: &'a Event,
}

impl Session {
    pub fn append_trace(&self, scope: &str, phase: &str, event: &Event) -> io::Result<()> {
        let ts_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let line = TraceLine {
            ts_ms,
            scope,
            phase,
            event,
        };
        append_line(&self.dir().join(TRACE_FILE), &line)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use serde_json::Value;
    use tempfile::TempDir;

    use super::*;
    use crate::feature::events::EventKind;

    #[test]
    fn trace_lines_are_stamped_json() {
        let root = TempDir::new().unwrap();
        let session = Session::create(root.path(), UNIX_EPOCH, Path::new("/w"), "m").unwrap();
        let event = Event::new(EventKind::Done);
        session.append_trace("subtask 1", "Plan", &event).unwrap();
        session.append_trace("strategy", "Deliver", &event).unwrap();
        let text = fs::read_to_string(session.dir().join("trace.jsonl")).unwrap();
        let lines: Vec<Value> = text
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0]["scope"], "subtask 1");
        assert_eq!(lines[0]["phase"], "Plan");
        assert_eq!(lines[0]["event"]["type"], "done");
        assert!(lines[0]["ts_ms"].as_u64().unwrap() > 0);
    }
}
