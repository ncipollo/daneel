//! One-shot mode: send a single prompt, print the result, exit. Backs `daneel -p`.

pub mod output;

use std::path::Path;
use std::sync::Arc;

use thiserror::Error;

use crate::feature::config::{self, Config, ConfigError};
use crate::feature::events::{Event, EventKind, EventSink};
use crate::feature::provider::{ChatRequest, Provider, ProviderError};
use crate::feature::providers::{ProviderSpec, build_provider};
use crate::feature::tools::{builtin_registry, into_rig_observed};

pub use output::OutputMode;

const INSTRUCTIONS: &str =
    "You are daneel, a careful assistant. Use the available tools when they help.";

#[derive(Debug, Error)]
pub enum OneShotError {
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error(transparent)]
    Provider(#[from] ProviderError),
    #[error("could not start async runtime: {0}")]
    Runtime(String),
}

/// Runs one prompt against the configured provider and writes the result to stdout.
pub fn run_blocking(prompt: &str, mode: OutputMode) -> Result<(), OneShotError> {
    let config = config::load_default()?;
    let runtime =
        tokio::runtime::Runtime::new().map_err(|e| OneShotError::Runtime(e.to_string()))?;
    runtime.block_on(run_configured(&config, prompt, mode))
}

async fn run_configured(
    config: &Config,
    prompt: &str,
    mode: OutputMode,
) -> Result<(), OneShotError> {
    let provider = build_provider(ProviderSpec::from_config(config)?);
    let working_dir = std::env::current_dir().map_err(|e| OneShotError::Runtime(e.to_string()))?;
    let sink = output::sink_for(mode);
    let result = run(provider, prompt, &working_dir, sink.clone()).await;
    output::finish(mode, &sink, &result);
    result.map(|_| ()).map_err(OneShotError::from)
}

/// The provider/agent exchange for a single prompt. Tool activity flows to `sink` as it happens.
pub async fn run(
    provider: Arc<dyn Provider>,
    prompt: &str,
    working_dir: &Path,
    sink: EventSink,
) -> Result<String, ProviderError> {
    let mut request = ChatRequest::new(INSTRUCTIONS, prompt);
    request.tools = into_rig_observed(builtin_registry(working_dir), sink);
    provider.chat(request).await
}

pub(crate) fn error_event(err: &ProviderError) -> Event {
    Event::new(EventKind::Error {
        message: err.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use crate::feature::provider::BoxFuture;

    struct Fixed(Result<&'static str, &'static str>);

    impl Provider for Fixed {
        fn chat<'a>(
            &'a self,
            request: ChatRequest<'a>,
        ) -> BoxFuture<'a, Result<String, ProviderError>> {
            Box::pin(async move {
                assert_eq!(request.prompt, "hi");
                self.0
                    .map(str::to_string)
                    .map_err(|e| ProviderError::Provider(e.to_string()))
            })
        }
    }

    #[tokio::test]
    async fn returns_the_assistant_text() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&events);
        let sink: EventSink = Arc::new(move |e| log.lock().unwrap().push(e));
        let reply = run(Arc::new(Fixed(Ok("hello"))), "hi", Path::new("."), sink).await;
        assert_eq!(reply.unwrap(), "hello");
    }

    #[tokio::test]
    async fn provider_failure_is_an_error() {
        let provider = Arc::new(Fixed(Err("boom")));
        let reply = run(
            provider,
            "hi",
            Path::new("."),
            crate::feature::events::discard(),
        )
        .await;
        assert!(reply.unwrap_err().to_string().contains("boom"));
    }
}
