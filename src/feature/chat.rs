//! A conversation with the configured model. Holds history across turns and is
//! shared by one-shot mode and the TUI.

use std::path::PathBuf;
use std::sync::Arc;

use crate::feature::config::Config;
use crate::feature::events::{Event, EventKind, EventSink};
use crate::feature::provider::{ChatMessage, ChatRequest, Provider, ProviderError, Role};
use crate::feature::providers::{ProviderSpec, build_provider};
use crate::feature::tools::{builtin_registry, into_rig_observed};

const INSTRUCTIONS: &str =
    "You are daneel, a careful assistant. Use the available tools when they help.";

pub struct Chat {
    provider: Arc<dyn Provider>,
    working_dir: PathBuf,
    history: Vec<ChatMessage>,
}

impl Chat {
    pub fn new(provider: Arc<dyn Provider>, working_dir: PathBuf) -> Self {
        Self {
            provider,
            working_dir,
            history: Vec::new(),
        }
    }

    /// Builds a chat from the default provider and model named in `config`.
    /// Also returns the model name for display.
    pub fn from_config(
        config: &Config,
        working_dir: PathBuf,
    ) -> Result<(Self, String), ProviderError> {
        let spec = ProviderSpec::from_config(config)?;
        let model = spec.model().to_string();
        Ok((Self::new(build_provider(spec), working_dir), model))
    }

    /// Sends one user prompt. Tool activity flows to `sink`; on success the
    /// exchange is added to the history.
    pub async fn send(&mut self, prompt: &str, sink: EventSink) -> Result<String, ProviderError> {
        let mut request = ChatRequest::new(INSTRUCTIONS, prompt);
        request.history = &self.history;
        request.tools = into_rig_observed(builtin_registry(&self.working_dir), sink);
        let reply = self.provider.chat(request).await?;
        self.history.push(message(Role::User, prompt));
        self.history.push(message(Role::Assistant, &reply));
        Ok(reply)
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

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use crate::feature::events;
    use crate::feature::provider::BoxFuture;

    struct Recorder(Mutex<Vec<usize>>);

    impl Provider for Recorder {
        fn chat<'a>(
            &'a self,
            request: ChatRequest<'a>,
        ) -> BoxFuture<'a, Result<String, ProviderError>> {
            Box::pin(async move {
                self.0.lock().unwrap().push(request.history.len());
                Ok(format!("re:{}", request.prompt))
            })
        }
    }

    #[tokio::test]
    async fn history_grows_across_turns() {
        let provider = Arc::new(Recorder(Mutex::new(Vec::new())));
        let mut chat = Chat::new(provider.clone(), PathBuf::from("."));
        chat.send("a", events::discard()).await.unwrap();
        let reply = chat.send("b", events::discard()).await.unwrap();
        assert_eq!(reply, "re:b");
        assert_eq!(*provider.0.lock().unwrap(), vec![0, 2]);
    }

    #[test]
    fn completion_events_cover_success_and_failure() {
        let ok = completion_events(&Ok("hi".to_string()));
        assert_eq!(ok.len(), 2);
        let err = completion_events(&Err(ProviderError::Provider("x".into())));
        assert!(matches!(err[0].kind, EventKind::Error { .. }));
    }
}
