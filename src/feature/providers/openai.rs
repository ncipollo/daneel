use rig::client::ProviderClient;
use rig::providers::openai;

use crate::feature::provider::{BoxFuture, ChatRequest, Provider, ProviderError};

use super::rig_chat::run_chat;

pub struct OpenAiProvider {
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub model: String,
}

impl Provider for OpenAiProvider {
    fn chat<'a>(
        &'a self,
        request: ChatRequest<'a>,
    ) -> BoxFuture<'a, Result<String, ProviderError>> {
        Box::pin(async move {
            let client = match &self.api_key {
                None => openai::Client::from_env(),
                Some(key) => {
                    let mut builder = openai::Client::builder().api_key(key.as_str());
                    if let Some(url) = &self.base_url {
                        builder = builder.base_url(url.as_str());
                    }
                    builder
                        .build()
                        .map_err(|e| ProviderError::Provider(e.to_string()))?
                }
            };
            run_chat(client, &self.model, request, None).await
        })
    }
}
