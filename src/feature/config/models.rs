use std::collections::BTreeMap;

use serde::Deserialize;

/// Provider settings from `models.{json,toml}`.
#[derive(Debug, Default, Clone, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct ModelsConfig {
    pub providers: BTreeMap<String, ProviderConfig>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ProviderConfig {
    #[serde(alias = "base_url")]
    pub base_url: Option<String>,
    pub api: Option<String>,
    #[serde(alias = "api_key")]
    pub api_key: Option<String>,
    #[serde(alias = "keep_alive")]
    pub keep_alive: Option<String>,
    pub models: Vec<ModelEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ModelEntry {
    pub id: String,
    pub name: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_provider_with_models() {
        let value = json!({ "providers": { "ollama": {
            "baseUrl": "http://localhost:11434",
            "api": "openai-completions",
            "apiKey": "key",
            "models": [{ "id": "qwen", "name": "Qwen" }, { "id": "llama" }]
        } } });
        let config: ModelsConfig = serde_json::from_value(value).unwrap();
        let ollama = &config.providers["ollama"];
        assert_eq!(ollama.base_url.as_deref(), Some("http://localhost:11434"));
        assert_eq!(ollama.api_key.as_deref(), Some("key"));
        assert_eq!(ollama.models.len(), 2);
        assert_eq!(ollama.models[1].name, None);
    }

    #[test]
    fn accepts_snake_case_aliases() {
        let value = json!({ "providers": { "p": { "base_url": "u", "api_key": "k", "keep_alive": "5m" } } });
        let config: ModelsConfig = serde_json::from_value(value).unwrap();
        assert_eq!(config.providers["p"].base_url.as_deref(), Some("u"));
        assert_eq!(config.providers["p"].api_key.as_deref(), Some("k"));
        assert_eq!(config.providers["p"].keep_alive.as_deref(), Some("5m"));
    }
}
