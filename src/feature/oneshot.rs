//! One-shot mode: send a single prompt, print the result, exit. Backs `daneel -p`.

pub mod output;

use thiserror::Error;

use crate::feature::chat::Chat;
use crate::feature::config::{self, Config, ConfigError};
use crate::feature::provider::ProviderError;

pub use output::OutputMode;

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
    let working_dir = std::env::current_dir().map_err(|e| OneShotError::Runtime(e.to_string()))?;
    let (mut chat, _model) = Chat::from_config(config, working_dir)?;
    let sink = output::sink_for(mode);
    let result = chat.send(prompt, sink.clone()).await;
    output::finish(mode, &sink, &result);
    result.map(|_| ()).map_err(OneShotError::from)
}
