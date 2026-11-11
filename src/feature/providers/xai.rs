use rig::client::ProviderClient;
use rig::providers::xai;

use crate::feature::provider::{BoxFuture, ChatRequest, Provider, ProviderError};

use super::rig_chat::run_chat;

pub struct XaiProvider {
    pub api_key: Option<String>,
    pub model: String,
}

impl Provider for XaiProvider {
    fn chat<'a>(
        &'a self,
        request: ChatRequest<'a>,
    ) -> BoxFuture<'a, Result<String, ProviderError>> {
        Box::pin(async move {
            let client = match &self.api_key {
                Some(key) => xai::Client::from_val(key.clone()),
                None => xai::Client::from_env(),
            };
            run_chat(client, &self.model, request, None).await
        })
    }
}
