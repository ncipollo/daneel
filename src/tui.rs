//! The interactive terminal UI. View-layer code: state, rendering, and the
//! terminal loop. Conversation logic lives in `feature::chat`.

pub mod app;
pub mod keys;
pub mod render;
pub mod runtime;

use std::io;

use thiserror::Error;

use crate::feature::chat::Chat;
use crate::feature::config::{self, ConfigError};
use crate::feature::provider::ProviderError;

#[derive(Debug, Error)]
pub enum TuiError {
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error(transparent)]
    Provider(#[from] ProviderError),
    #[error("terminal error: {0}")]
    Io(#[from] io::Error),
}

/// Loads the configuration and runs the interactive UI until the user quits.
pub fn start() -> Result<(), TuiError> {
    let config = config::load_default()?;
    let (chat, model) = Chat::from_config(&config, std::env::current_dir()?)?;
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(runtime::run(chat, model))?;
    Ok(())
}
