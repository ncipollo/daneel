use crate::feature::config::{Config, ProviderConfig};
use crate::feature::provider::ProviderError;

const DEFAULT_OLLAMA_URL: &str = "http://localhost:11434";
const KINDS: &str = "ollama, openai, anthropic, gemini, cohere, xai";

/// A fully resolved provider choice, ready for `build_provider`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderSpec {
    Ollama {
        base_url: String,
        model: String,
        keep_alive: Option<String>,
    },
    OpenAi {
        api_key: Option<String>,
        base_url: Option<String>,
        model: String,
    },
    Anthropic {
        api_key: Option<String>,
        model: String,
    },
    Gemini {
        api_key: Option<String>,
        model: String,
    },
    Cohere {
        api_key: Option<String>,
        model: String,
    },
    XAi {
        api_key: Option<String>,
        model: String,
    },
}

impl ProviderSpec {
    pub fn model(&self) -> &str {
        match self {
            Self::Ollama { model, .. }
            | Self::OpenAi { model, .. }
            | Self::Anthropic { model, .. }
            | Self::Gemini { model, .. }
            | Self::Cohere { model, .. }
            | Self::XAi { model, .. } => model,
        }
    }

    /// Resolves the default provider and model named in `settings` against `models`.
    pub fn from_config(config: &Config) -> Result<Self, ProviderError> {
        let name = config
            .settings
            .default_provider
            .as_deref()
            .ok_or_else(|| not_configured("no defaultProvider set in settings"))?;
        let provider = config
            .models
            .providers
            .get(name)
            .ok_or_else(|| not_configured(format!("provider '{name}' not found in models")))?;
        let model = pick_model(config, provider, name)?;
        let kind = provider.api.as_deref().unwrap_or(name);
        build_spec(kind, name, provider, model)
    }
}

fn not_configured(message: impl Into<String>) -> ProviderError {
    ProviderError::NotConfigured(message.into())
}

fn pick_model(
    config: &Config,
    provider: &ProviderConfig,
    name: &str,
) -> Result<String, ProviderError> {
    config
        .settings
        .default_model
        .clone()
        .or_else(|| provider.models.first().map(|m| m.id.clone()))
        .ok_or_else(|| not_configured(format!("no model for provider '{name}'")))
}

fn build_spec(
    kind: &str,
    name: &str,
    provider: &ProviderConfig,
    model: String,
) -> Result<ProviderSpec, ProviderError> {
    let api_key = provider.api_key.clone();
    let base_url = provider.base_url.clone();
    match kind.to_ascii_lowercase().as_str() {
        "ollama" => Ok(ProviderSpec::Ollama {
            base_url: base_url.unwrap_or_else(|| DEFAULT_OLLAMA_URL.to_string()),
            model,
            keep_alive: provider.keep_alive.clone(),
        }),
        "openai" => Ok(ProviderSpec::OpenAi {
            api_key,
            base_url,
            model,
        }),
        "anthropic" => Ok(ProviderSpec::Anthropic { api_key, model }),
        "gemini" => Ok(ProviderSpec::Gemini { api_key, model }),
        "cohere" => Ok(ProviderSpec::Cohere { api_key, model }),
        "xai" => Ok(ProviderSpec::XAi { api_key, model }),
        other => Err(not_configured(format!(
            "unknown provider kind '{other}' for '{name}' (expected one of: {KINDS})"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::feature::config::{ModelEntry, ModelsConfig, Settings};

    fn config(provider: Option<&str>, model: Option<&str>, entry: ProviderConfig) -> Config {
        let name = provider.unwrap_or("ollama").to_string();
        Config {
            settings: Settings {
                default_provider: provider.map(str::to_string),
                default_model: model.map(str::to_string),
                ..Settings::default()
            },
            models: ModelsConfig {
                providers: [(name, entry)].into(),
            },
        }
    }

    fn entry_with_model(id: &str) -> ProviderConfig {
        ProviderConfig {
            models: vec![ModelEntry {
                id: id.to_string(),
                name: None,
            }],
            ..ProviderConfig::default()
        }
    }

    #[test]
    fn ollama_uses_settings_model_and_defaults_url() {
        let cfg = config(Some("ollama"), Some("qwen"), ProviderConfig::default());
        let expected = ProviderSpec::Ollama {
            base_url: DEFAULT_OLLAMA_URL.to_string(),
            model: "qwen".to_string(),
            keep_alive: None,
        };
        assert_eq!(ProviderSpec::from_config(&cfg).unwrap(), expected);
    }

    #[test]
    fn ollama_carries_base_url_and_keep_alive() {
        let entry = ProviderConfig {
            base_url: Some("http://box:11434".to_string()),
            keep_alive: Some("5m".to_string()),
            ..entry_with_model("llama")
        };
        let spec = ProviderSpec::from_config(&config(Some("ollama"), None, entry)).unwrap();
        let expected = ProviderSpec::Ollama {
            base_url: "http://box:11434".to_string(),
            model: "llama".to_string(),
            keep_alive: Some("5m".to_string()),
        };
        assert_eq!(spec, expected);
    }

    #[test]
    fn falls_back_to_first_listed_model() {
        let cfg = config(Some("ollama"), None, entry_with_model("first"));
        let ProviderSpec::Ollama { model, .. } = ProviderSpec::from_config(&cfg).unwrap() else {
            panic!("expected ollama");
        };
        assert_eq!(model, "first");
    }

    #[test]
    fn api_field_overrides_the_provider_name() {
        let entry = ProviderConfig {
            api: Some("openai".to_string()),
            base_url: Some("http://vllm:8000/v1".to_string()),
            api_key: Some("k".to_string()),
            ..ProviderConfig::default()
        };
        let mut cfg = config(Some("my-vllm"), Some("m"), entry);
        cfg.settings.default_provider = Some("my-vllm".to_string());
        let expected = ProviderSpec::OpenAi {
            api_key: Some("k".to_string()),
            base_url: Some("http://vllm:8000/v1".to_string()),
            model: "m".to_string(),
        };
        assert_eq!(ProviderSpec::from_config(&cfg).unwrap(), expected);
    }

    #[test]
    fn hosted_kinds_resolve_by_name() {
        for name in ["anthropic", "gemini", "cohere", "xai", "openai"] {
            let cfg = config(Some(name), Some("m"), ProviderConfig::default());
            assert!(ProviderSpec::from_config(&cfg).is_ok(), "{name}");
        }
    }

    #[test]
    fn unknown_kind_lists_the_supported_ones() {
        let cfg = config(Some("mystery"), Some("m"), ProviderConfig::default());
        let err = ProviderSpec::from_config(&cfg).unwrap_err().to_string();
        assert!(err.contains("unknown provider kind 'mystery'"));
        assert!(err.contains("ollama"));
    }

    #[test]
    fn missing_default_provider_is_an_error() {
        let cfg = config(None, Some("m"), ProviderConfig::default());
        let err = ProviderSpec::from_config(&cfg).unwrap_err().to_string();
        assert!(err.contains("defaultProvider"));
    }

    #[test]
    fn provider_missing_from_models_is_an_error() {
        let mut cfg = config(Some("ollama"), Some("m"), ProviderConfig::default());
        cfg.settings.default_provider = Some("other".to_string());
        let err = ProviderSpec::from_config(&cfg).unwrap_err().to_string();
        assert!(err.contains("'other' not found"));
    }

    #[test]
    fn no_model_anywhere_is_an_error() {
        let cfg = config(Some("ollama"), None, ProviderConfig::default());
        let err = ProviderSpec::from_config(&cfg).unwrap_err().to_string();
        assert!(err.contains("no model"));
    }
}
