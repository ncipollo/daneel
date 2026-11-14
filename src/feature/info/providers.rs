//! The `providers` topic: the models file and picking a provider and model.

use crate::feature::providers::spec::KINDS;

const EXAMPLES: [Example; 3] = [
    Example {
        title: "Local Ollama",
        settings: r#"{ "defaultProvider": "ollama", "defaultModel": "qwen3:8b" }"#,
        models: r#"{ "providers": { "ollama": {
    "baseUrl": "http://localhost:11434",
    "keepAlive": "10m",
    "models": [{ "id": "qwen3:8b" }]
} } }"#,
    },
    Example {
        title: "Local OpenAI-compatible server (llama.cpp, LM Studio, vLLM)",
        settings: r#"{ "defaultProvider": "local" }"#,
        models: r#"{ "providers": { "local": {
    "api": "openai",
    "baseUrl": "http://localhost:1234/v1",
    "apiKey": "not-needed",
    "models": [{ "id": "my-model" }]
} } }"#,
    },
    Example {
        title: "Hosted Anthropic, key from the environment",
        settings: r#"{ "defaultProvider": "anthropic", "defaultModel": "claude-sonnet-5-5" }"#,
        models: r#"{ "providers": { "anthropic": {
    "apiKey": "$ANTHROPIC_API_KEY",
    "models": [{ "id": "claude-sonnet-5-5" }]
} } }"#,
    },
];

struct Example {
    title: &'static str,
    settings: &'static str,
    models: &'static str,
}

pub fn render() -> String {
    format!(
        "{}\n{}\n{}\n{}",
        shape_section(),
        kinds_section(),
        examples_section(),
        errors_section()
    )
}

fn shape_section() -> String {
    String::from(
        "MODELS FILE (~/.daneel/models.json or models.toml)\n\
         \"providers\" maps a name you choose to its settings:\n\
         \n\
         \x20 baseUrl    server URL (ollama; openai-compatible servers)\n\
         \x20 api        which kind of provider this is (see below)\n\
         \x20 apiKey     the key; use \"$VAR\" to read it from the environment\n\
         \x20 keepAlive  ollama only: how long the model stays loaded, e.g. \"10m\"\n\
         \x20 models     [{ \"id\": \"...\", \"name\": \"optional label\" }]\n\
         \n\
         CHOOSING WHAT RUNS\n\
         settings.defaultProvider names the entry in \"providers\".\n\
         settings.defaultModel is the model id; if unset, the first entry\n\
         of that provider's \"models\" is used. One of the two must exist.\n",
    )
}

fn kinds_section() -> String {
    format!(
        "PROVIDER KINDS\n\
         Kinds: {KINDS}.\n\
         The kind is the \"api\" value, or the provider's own name when\n\
         \"api\" is missing. So an entry named \"ollama\" needs no \"api\", but\n\
         an entry with any other name (\"local\", \"work\") must set it.\n\
         \n\
         \x20 ollama     baseUrl defaults to http://localhost:11434; no key\n\
         \x20 openai     baseUrl is optional; see the warning below\n\
         \x20 anthropic, gemini, cohere, xai   apiKey only\n\
         \n\
         API KEYS\n\
         If apiKey is unset, the SDK reads OPENAI_API_KEY, ANTHROPIC_API_KEY,\n\
         GEMINI_API_KEY, COHERE_API_KEY or XAI_API_KEY, and panics if that\n\
         variable is not set. Always set one or the other.\n\
         \n\
         WARNING (openai): baseUrl is only used when apiKey is set in the\n\
         file. For a local server that needs no key, set apiKey to any\n\
         placeholder string, as in the example below.\n"
    )
}

fn examples_section() -> String {
    let mut text = String::from("\nEXAMPLES\n");
    for example in &EXAMPLES {
        text.push_str(&format!(
            "\n{}\nsettings.json:\n{}\nmodels.json:\n{}\n",
            example.title, example.settings, example.models
        ));
    }
    text.push_str("\nTOML works too: the same keys, with [providers.<name>] tables.\n");
    text
}

fn errors_section() -> String {
    String::from(
        "\nCOMMON ERRORS\n\
         no defaultProvider set in settings   add defaultProvider\n\
         provider 'x' not found in models     add providers.x to models\n\
         no model for provider 'x'            set defaultModel or list models\n\
         unknown provider kind                \"api\" (or the name) is not a kind\n\
         connection or HTTP errors            check baseUrl and that the server runs\n",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::feature::config::{Config, ModelsConfig, Settings};
    use crate::feature::providers::ProviderSpec;

    fn load(example: &Example) -> Config {
        Config {
            settings: serde_json::from_str::<Settings>(example.settings).unwrap(),
            models: serde_json::from_str::<ModelsConfig>(example.models).unwrap(),
        }
    }

    #[test]
    fn every_example_resolves_to_a_provider() {
        let kinds: Vec<_> = EXAMPLES
            .iter()
            .map(|e| ProviderSpec::from_config(&load(e)).expect(e.title))
            .collect();
        assert!(matches!(kinds[0], ProviderSpec::Ollama { .. }));
        assert!(matches!(kinds[1], ProviderSpec::OpenAi { .. }));
        assert!(matches!(kinds[2], ProviderSpec::Anthropic { .. }));
    }

    #[test]
    fn the_local_server_example_keeps_its_base_url() {
        let spec = ProviderSpec::from_config(&load(&EXAMPLES[1])).unwrap();
        let ProviderSpec::OpenAi {
            api_key, base_url, ..
        } = spec
        else {
            panic!("expected openai");
        };
        assert!(api_key.is_some() && base_url.is_some());
    }

    #[test]
    fn every_kind_is_listed_on_the_page() {
        let page = render();
        for kind in KINDS.split(", ") {
            assert!(page.contains(kind), "missing kind: {kind}");
        }
    }
}
