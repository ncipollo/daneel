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
    #[serde(alias = "safe_tools")]
    pub safe_tools: Option<SafeToolsSettings>,
}

/// Safe-tools mode: a separate agent process reviews risky tool calls before they run.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SafeToolsSettings {
    pub enabled: bool,
    /// The validator executable, run without a shell; the review prompt goes to its stdin.
    pub command: String,
    pub args: Vec<String>,
    #[serde(alias = "timeout_secs")]
    pub timeout_secs: u64,
}

impl Default for SafeToolsSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            command: "claude".to_string(),
            args: vec!["-p".to_string()],
            timeout_secs: 120,
        }
    }
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
    fn safe_tools_parse_with_defaults() {
        let value = json!({ "safeTools": { "enabled": true } });
        let settings: Settings = serde_json::from_value(value).unwrap();
        let safe = settings.safe_tools.unwrap();
        assert!(safe.enabled);
        assert_eq!(safe.command, "claude");
        assert_eq!(safe.args, ["-p"]);
        assert_eq!(safe.timeout_secs, 120);
    }

    #[test]
    fn safe_tools_accept_snake_case_and_overrides() {
        let value = json!({ "safe_tools": { "enabled": true, "command": "v", "args": [], "timeout_secs": 5 } });
        let safe = serde_json::from_value::<Settings>(value)
            .unwrap()
            .safe_tools
            .unwrap();
        assert_eq!((safe.command.as_str(), safe.timeout_secs), ("v", 5));
        assert!(safe.args.is_empty());
    }

    #[test]
    fn safe_tools_are_off_by_default() {
        assert_eq!(Settings::default().safe_tools, None);
        assert!(!SafeToolsSettings::default().enabled);
    }

    #[test]
    fn empty_object_is_default() {
        let settings: Settings = serde_json::from_value(json!({})).unwrap();
        assert_eq!(settings, Settings::default());
    }
}
