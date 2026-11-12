use rig::client::ProviderClient;
use rig::providers::gemini;

use crate::feature::provider::{BoxFuture, ChatRequest, Provider, ProviderError};

use super::rig_chat::run_chat;

pub struct GeminiProvider {
    pub api_key: Option<String>,
    pub model: String,
}

impl Provider for GeminiProvider {
    fn chat<'a>(
        &'a self,
        request: ChatRequest<'a>,
    ) -> BoxFuture<'a, Result<String, ProviderError>> {
        Box::pin(async move {
            let client = match &self.api_key {
                Some(key) => gemini::Client::builder()
                    .api_key(key.as_str())
                    .build()
                    .map_err(|e| ProviderError::Provider(e.to_string()))?,
                None => gemini::Client::from_env(),
            };
            run_chat(client, &self.model, request, None).await
        })
    }
}
