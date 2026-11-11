use rig::client::ProviderClient;
use rig::providers::anthropic;

use crate::feature::provider::{BoxFuture, ChatRequest, Provider, ProviderError};

use super::rig_chat::run_chat;

pub struct AnthropicProvider {
    pub api_key: Option<String>,
    pub model: String,
}

impl Provider for AnthropicProvider {
    fn chat<'a>(
        &'a self,
        request: ChatRequest<'a>,
    ) -> BoxFuture<'a, Result<String, ProviderError>> {
        Box::pin(async move {
            let client = match &self.api_key {
                Some(key) => anthropic::Client::from_val(key.clone()),
                None => anthropic::Client::from_env(),
            };
            run_chat(client, &self.model, request, None).await
        })
    }
}
