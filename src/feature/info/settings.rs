//! The `settings` topic: the keys of the settings file.

pub fn render() -> String {
    String::from(
        "SETTINGS FILE (~/.daneel/settings.json or settings.toml)\n\
         \n\
         \x20 defaultProvider        name of an entry in the models file\n\
         \x20 defaultModel           model id to use (see --info providers)\n\
         \x20 safeTools              external review of risky calls\n\
         \x20                        (see --info safe-tools)\n\
         \x20 theme                  accepted but not used yet\n\
         \x20 defaultThinkingLevel   accepted but not used yet\n\
         \n\
         Unknown keys are ignored. Every key is optional, but daneel needs\n\
         a defaultProvider (and a model) before it can answer anything.\n\
         \n\
         EXAMPLE\n\
         { \"defaultProvider\": \"ollama\", \"defaultModel\": \"qwen3:8b\" }\n",
    )
}
