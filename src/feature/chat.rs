//! A conversation with the configured model. Every message becomes a task and is
//! run through the strategy machine; the delivered result is the reply. Holds
//! history across turns and is shared by one-shot mode and the TUI.

mod history;
#[cfg(test)]
mod tests;

use std::path::PathBuf;
use std::sync::Arc;

use crate::feature::config::{self, Config};
use crate::feature::events::{Event, EventKind, EventSink};
use crate::feature::interaction::Environment;
use crate::feature::provider::{ChatMessage, Provider, ProviderError, Role};
use crate::feature::providers::{ProviderSpec, build_provider};
use crate::feature::session::{self, Session};
use crate::feature::strategy::load::{load_strategy_or_default, load_tactic_or_default};
use crate::feature::strategy::machine::{StrategyContext, run_strategy};
use crate::feature::strategy::{PhaseList, defaults, paths};
use crate::feature::task::transform::from_message;
use crate::feature::tools::safety::validator_from;
use crate::feature::tools::{ToolSetup, Validator};
use crate::feature::trace;

pub struct Chat {
    provider: Arc<dyn Provider>,
    working_dir: PathBuf,
    history: Vec<ChatMessage>,
    session: Option<Session>,
    tools: ToolSetup,
    strategy: PhaseList,
    tactic: PhaseList,
}

impl Chat {
    /// A chat using the built-in strategy and tactic.
    pub fn new(provider: Arc<dyn Provider>, working_dir: PathBuf) -> Self {
        Self {
            provider,
            working_dir,
            history: Vec::new(),
            session: None,
            tools: ToolSetup::default(),
            strategy: defaults::builtin_strategy(),
            tactic: defaults::builtin_tactic(),
        }
    }

    /// Builds a chat from the default provider and model named in `config`, with
    /// the strategy and tactic from `~/.daneel/strategies`. Also returns the
    /// model name for display.
    pub fn from_config(
        config: &Config,
        working_dir: PathBuf,
    ) -> Result<(Self, String), ProviderError> {
        let spec = ProviderSpec::from_config(config)?;
        let model = spec.model().to_string();
        let mut chat = Self::new(build_provider(spec), working_dir.clone());
        chat.session = session::start_default(&working_dir, &model);
        chat.tools.safety = validator_from(&config.settings);
        if let Ok(dir) = config::default_dir() {
            chat.tools.tools_dir = Some(dir.join("tools"));
            let name = paths::DEFAULT_NAME;
            let broken = |what: &str, e: config::ConfigError| {
                ProviderError::Run(format!("could not load the {what}: {e}"))
            };
            chat.strategy =
                load_strategy_or_default(&dir, name).map_err(|e| broken("strategy", e))?;
            chat.tactic = load_tactic_or_default(&dir, name).map_err(|e| broken("tactic", e))?;
        }
        Ok((chat, model))
    }

    pub fn with_session(mut self, session: Session) -> Self {
        self.session = Some(session);
        self
    }

    /// Puts write and shell tools behind a safety review by `validator`.
    pub fn with_safety(mut self, validator: Arc<dyn Validator>) -> Self {
        self.tools.safety = Some(validator);
        self
    }

    /// Loads custom JavaScript tools from `dir` on every turn.
    pub fn with_tools_dir(mut self, dir: PathBuf) -> Self {
        self.tools.tools_dir = Some(dir);
        self
    }

    pub fn with_phases(mut self, strategy: PhaseList, tactic: PhaseList) -> Self {
        self.strategy = strategy;
        self.tactic = tactic;
        self
    }

    fn session_id(&self) -> &str {
        self.session
            .as_ref()
            .map_or("none", |s| s.metadata().id.as_str())
    }

    fn record(&self, role: &str, content: &str) {
        if let Some(session) = &self.session {
            let _ = session.append_message(role, content);
        }
    }

    /// Sends one user message. It becomes a task, the strategy carries it to a
    /// delivered result, and progress and tool activity flow to `sink`. On
    /// success the exchange is added to the history.
    pub async fn send(&mut self, prompt: &str, sink: EventSink) -> Result<String, ProviderError> {
        self.record("user", prompt);
        tracing::debug!(
            session = self.session_id(),
            direction = "out",
            content = prompt,
            "model message"
        );
        let reply = match self.run(prompt, sink).await {
            Ok(reply) => reply,
            Err(err) => {
                tracing::error!(session = self.session_id(), error = %err, "model error");
                self.record("error", &err.to_string());
                return Err(err);
            }
        };
        tracing::debug!(
            session = self.session_id(),
            direction = "in",
            content = reply.as_str(),
            "model message"
        );
        self.record("assistant", &reply);
        self.history.push(message(Role::User, prompt));
        self.history.push(message(Role::Assistant, &reply));
        Ok(reply)
    }

    async fn run(&self, prompt: &str, sink: EventSink) -> Result<String, ProviderError> {
        let sink = match &self.session {
            Some(session) => trace::tee(session.clone(), sink),
            None => sink,
        };
        let provider: &dyn Provider = self.provider.as_ref();
        let task = from_message(provider, prompt).await;
        let env = Environment::new(provider, &self.working_dir, &self.tools)
            .with_session(self.session.as_ref())
            .with_sink(sink);
        let background = history::background(&self.history);
        let ctx = StrategyContext {
            env: &env,
            tactic: &self.tactic,
            background: &background,
        };
        let outcome = run_strategy(&ctx, &self.strategy, &task).await?;
        Ok(outcome.delivered)
    }
}

fn message(role: Role, content: &str) -> ChatMessage {
    ChatMessage {
        role,
        content: content.to_string(),
    }
}

/// The closing events for a finished turn: the reply and `done`, or an `error`.
pub fn completion_events(result: &Result<String, ProviderError>) -> Vec<Event> {
    match result {
        Ok(text) => vec![
            Event::new(EventKind::Message { text: text.clone() }),
            Event::new(EventKind::Done),
        ],
        Err(err) => vec![Event::new(EventKind::Error {
            message: err.to_string(),
        })],
    }
}
