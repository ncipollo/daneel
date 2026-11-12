use rig::client::Nothing;
use rig::providers::ollama;

use crate::feature::provider::{BoxFuture, ChatRequest, Provider, ProviderError};

use super::rig_chat::run_chat;

pub struct OllamaProvider {
    pub base_url: String,
    pub model: String,
    pub keep_alive: Option<String>,
}

fn keep_alive_params(keep_alive: &Option<String>) -> Option<serde_json::Value> {
    keep_alive
        .as_ref()
        .map(|ka| serde_json::json!({ "keep_alive": ka }))
}

impl Provider for OllamaProvider {
    fn chat<'a>(
        &'a self,
        request: ChatRequest<'a>,
    ) -> BoxFuture<'a, Result<String, ProviderError>> {
        Box::pin(async move {
            let client = ollama::Client::builder()
                .api_key(Nothing)
                .base_url(&self.base_url)
                .build()
                .map_err(|e| ProviderError::Provider(e.to_string()))?;
            run_chat(
                client,
                &self.model,
                request,
                keep_alive_params(&self.keep_alive),
            )
            .await
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keep_alive_params_none_when_unset() {
        assert_eq!(keep_alive_params(&None), None);
    }

    #[test]
    fn keep_alive_params_wraps_value_when_set() {
        assert_eq!(
            keep_alive_params(&Some("5m".to_string())),
            Some(serde_json::json!({ "keep_alive": "5m" }))
        );
    }
}
