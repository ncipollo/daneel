//! The `safe-tools` topic: reviewing risky tool calls with another agent.

use crate::feature::config::SafeToolsSettings;

pub fn render() -> String {
    let defaults = SafeToolsSettings::default();
    format!(
        "SAFE TOOLS\n\
         When enabled, every write_file, edit_file and invoke_shell call\n\
         (including shell calls made by custom tools) is first shown to a\n\
         separate agent process. Anything other than a clear \"safe\" verdict\n\
         blocks the call, and the model is told it was blocked. Reads are\n\
         never reviewed.\n\
         \n\
         SETTINGS (in the settings file, under \"safeTools\")\n\
         \x20 enabled      false by default; nothing is reviewed unless true\n\
         \x20 command      executable to run, default \"{command}\" (no shell)\n\
         \x20 args         arguments for it, default {args:?}\n\
         \x20 timeoutSecs  give up and block after this long, default {timeout}\n\
         \n\
         The review prompt is written to the command's stdin and its output\n\
         is read as the verdict. A missing command, a timeout or an\n\
         unclear answer all block the call.\n\
         \n\
         EXAMPLE\n\
         {{ \"safeTools\": {{ \"enabled\": true }} }}\n",
        command = defaults.command,
        args = defaults.args,
        timeout = defaults.timeout_secs,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::feature::config::Settings;

    #[test]
    fn the_example_enables_safe_tools_with_the_documented_defaults() {
        let settings: Settings =
            serde_json::from_str(r#"{ "safeTools": { "enabled": true } }"#).unwrap();
        let safe = settings.safe_tools.unwrap();
        assert!(safe.enabled);
        assert!(render().contains(&safe.command));
    }
}
