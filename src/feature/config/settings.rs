use serde::Deserialize;

/// High-level preferences from `settings.{json,toml}`. Keys are camelCase, with
/// snake_case accepted so TOML files can use natural names.
#[derive(Debug, Default, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub theme: Option<String>,
    #[serde(alias = "default_provider")]
    pub default_provider: Option<String>,
    #[serde(alias = "default_model")]
    pub default_model: Option<String>,
    #[serde(alias = "default_thinking_level")]
    pub default_thinking_level: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_camel_case_keys() {
        let value = json!({ "theme": "dark", "defaultProvider": "ollama", "defaultModel": "qwen" });
        let settings: Settings = serde_json::from_value(value).unwrap();
        assert_eq!(settings.theme.as_deref(), Some("dark"));
        assert_eq!(settings.default_provider.as_deref(), Some("ollama"));
        assert_eq!(settings.default_model.as_deref(), Some("qwen"));
    }

    #[test]
    fn accepts_snake_case_aliases() {
        let value = json!({ "default_thinking_level": "high" });
        let settings: Settings = serde_json::from_value(value).unwrap();
        assert_eq!(settings.default_thinking_level.as_deref(), Some("high"));
    }

    #[test]
    fn empty_object_is_default() {
        let settings: Settings = serde_json::from_value(json!({})).unwrap();
        assert_eq!(settings, Settings::default());
    }
}
