//! The provider abstraction: one interface the agent loop uses to talk to any
//! model backend (Ollama, OpenAI, Anthropic, ...).

pub mod summary;

use std::future::Future;
use std::pin::Pin;

use thiserror::Error;

pub use summary::summarize_error;

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("provider error: {0}")]
    Provider(String),
    #[error("tool error: {0}")]
    Tool(String),
    #[error("provider not configured: {0}")]
    NotConfigured(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    User,
    Assistant,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatMessage {
    pub role: Role,
    pub content: String,
}

/// Per-call knobs. `max_turns: None` means no limit; a CLI override fills it in later.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ChatOptions {
    pub max_turns: Option<usize>,
}

pub struct ChatRequest<'a> {
    pub instructions: &'a str,
    pub prompt: &'a str,
    pub history: &'a [ChatMessage],
    pub tools: Vec<Box<dyn rig::tool::ToolDyn>>,
    pub options: ChatOptions,
}

impl<'a> ChatRequest<'a> {
    pub fn new(instructions: &'a str, prompt: &'a str) -> Self {
        Self {
            instructions,
            prompt,
            history: &[],
            tools: Vec::new(),
            options: ChatOptions::default(),
        }
    }
}

pub trait Provider: Send + Sync {
    fn chat<'a>(&'a self, request: ChatRequest<'a>)
    -> BoxFuture<'a, Result<String, ProviderError>>;
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    struct Echo;

    impl Provider for Echo {
        fn chat<'a>(
            &'a self,
            request: ChatRequest<'a>,
        ) -> BoxFuture<'a, Result<String, ProviderError>> {
            Box::pin(async move { Ok(format!("{}:{}", request.instructions, request.prompt)) })
        }
    }

    #[tokio::test]
    async fn provider_is_usable_behind_arc_dyn() {
        let provider: Arc<dyn Provider> = Arc::new(Echo);
        let reply = provider.chat(ChatRequest::new("be brief", "hi")).await;
        assert_eq!(reply.unwrap(), "be brief:hi");
    }

    #[test]
    fn default_options_have_no_turn_limit() {
        assert_eq!(ChatOptions::default().max_turns, None);
    }
}
