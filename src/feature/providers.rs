pub mod anthropic;
pub mod cohere;
pub mod gemini;
pub mod ollama;
pub mod openai;
pub mod rig_chat;
pub mod spec;
pub mod xai;

use std::sync::Arc;

use rig::message::Message;

use crate::feature::provider::{ChatMessage, Provider, Role};

pub use spec::ProviderSpec;

pub fn build_provider(spec: ProviderSpec) -> Arc<dyn Provider> {
    match spec {
        ProviderSpec::Ollama {
            base_url,
            model,
            keep_alive,
        } => Arc::new(ollama::OllamaProvider {
            base_url,
            model,
            keep_alive,
        }),
        ProviderSpec::OpenAi {
            api_key,
            base_url,
            model,
        } => Arc::new(openai::OpenAiProvider {
            api_key,
            base_url,
            model,
        }),
        ProviderSpec::Anthropic { api_key, model } => {
            Arc::new(anthropic::AnthropicProvider { api_key, model })
        }
        ProviderSpec::Gemini { api_key, model } => {
            Arc::new(gemini::GeminiProvider { api_key, model })
        }
        ProviderSpec::Cohere { api_key, model } => {
            Arc::new(cohere::CohereProvider { api_key, model })
        }
        ProviderSpec::XAi { api_key, model } => Arc::new(xai::XaiProvider { api_key, model }),
    }
}

pub fn history_to_rig(history: &[ChatMessage]) -> Vec<Message> {
    history
        .iter()
        .map(|m| match m.role {
            Role::User => Message::user(m.content.clone()),
            Role::Assistant => Message::assistant(m.content.clone()),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message(role: Role, content: &str) -> ChatMessage {
        ChatMessage {
            role,
            content: content.to_string(),
        }
    }

    #[test]
    fn history_maps_roles_in_order() {
        let history = [message(Role::User, "hi"), message(Role::Assistant, "hello")];
        let rig = history_to_rig(&history);
        assert_eq!(rig, vec![Message::user("hi"), Message::assistant("hello")]);
    }

    #[test]
    fn build_provider_accepts_every_spec() {
        let key = Some("key".to_string());
        let specs = [
            ProviderSpec::Ollama {
                base_url: "http://localhost:11434".to_string(),
                model: "m".to_string(),
                keep_alive: Some("5m".to_string()),
            },
            ProviderSpec::OpenAi {
                api_key: key.clone(),
                base_url: None,
                model: "m".to_string(),
            },
            ProviderSpec::Anthropic {
                api_key: key.clone(),
                model: "m".to_string(),
            },
            ProviderSpec::Gemini {
                api_key: key.clone(),
                model: "m".to_string(),
            },
            ProviderSpec::Cohere {
                api_key: key.clone(),
                model: "m".to_string(),
            },
            ProviderSpec::XAi {
                api_key: key,
                model: "m".to_string(),
            },
        ];
        for spec in specs {
            let _provider = build_provider(spec);
        }
    }
}
