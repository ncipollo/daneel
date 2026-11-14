use std::path::Path;

use crate::feature::events::{self, EventSink};
use crate::feature::provider::Provider;
use crate::feature::session::Session;
use crate::feature::tools::ToolSetup;

/// What a machine run needs from the outside world, shared by the strategy,
/// the tactic and every phase turn.
#[derive(Clone)]
pub struct Environment<'a> {
    pub provider: &'a dyn Provider,
    pub working_dir: &'a Path,
    pub session: Option<&'a Session>,
    pub tools: &'a ToolSetup,
    /// Receives progress and tool events as the run goes.
    pub sink: EventSink,
}

impl<'a> Environment<'a> {
    pub fn new(provider: &'a dyn Provider, working_dir: &'a Path, tools: &'a ToolSetup) -> Self {
        Self {
            provider,
            working_dir,
            session: None,
            tools,
            sink: events::discard(),
        }
    }

    pub fn with_session(mut self, session: Option<&'a Session>) -> Self {
        self.session = session;
        self
    }

    pub fn with_sink(mut self, sink: EventSink) -> Self {
        self.sink = sink;
        self
    }
}
