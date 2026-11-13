//! How one-shot results reach stdout. Text mode prints only the final reply;
//! json mode streams events, one per line.

use std::sync::Arc;

use crate::feature::chat::completion_events;
use crate::feature::events::{self, Event, EventSink};
use crate::feature::provider::ProviderError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutputMode {
    #[default]
    Text,
    Json,
}

pub fn sink_for(mode: OutputMode) -> EventSink {
    match mode {
        OutputMode::Text => events::discard(),
        OutputMode::Json => Arc::new(|event: Event| println!("{}", event.to_line())),
    }
}

/// Emits the closing output once the run is over.
pub fn finish(mode: OutputMode, sink: &EventSink, result: &Result<String, ProviderError>) {
    match (mode, result) {
        (OutputMode::Text, Ok(text)) => println!("{text}"),
        (OutputMode::Text, Err(_)) => {}
        (OutputMode::Json, result) => completion_events(result).into_iter().for_each(|e| sink(e)),
    }
}
