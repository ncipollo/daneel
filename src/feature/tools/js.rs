//! Custom tools written in JavaScript and run on the embedded boa engine. Each
//! `~/.daneel/tools/<name>.js` declares `parameters()`, optionally `output()` and
//! `description()`, and a `run(args)` function.

pub mod engine;
pub mod load;
pub mod shape;
pub mod tool;

pub use load::{Loaded, load_js_tools};
pub use tool::JsTool;

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;
    use std::sync::Arc;

    use serde_json::json;
    use tempfile::TempDir;

    use crate::feature::tools::safety::FixedValidator;
    use crate::feature::tools::{ToolRegistry, Validator, assemble};

    const ECHO: &str = r#"
        function description() { return "Shouts text."; }
        function parameters() {
            return { type: "object", properties: { text: { type: "string" } }, required: ["text"] };
        }
        function output() { return { type: "string" }; }
        function run(args) { return args.text.toUpperCase(); }
    "#;

    fn script(dir: &Path, name: &str, body: &str) {
        fs::write(dir.join(name), body).unwrap();
    }

    fn registry(tools: &TempDir, work: &TempDir) -> ToolRegistry {
        assemble(work.path(), None, Some(tools.path()))
    }

    async fn call(tool: &str, script_body: &str, args: serde_json::Value) -> String {
        let (tools, work) = (TempDir::new().unwrap(), TempDir::new().unwrap());
        script(tools.path(), &format!("{tool}.js"), script_body);
        registry(&tools, &work).dispatch(tool, args).await
    }

    #[test]
    fn scripts_declare_their_schema_and_description() {
        let (tools, work) = (TempDir::new().unwrap(), TempDir::new().unwrap());
        script(tools.path(), "shout.js", ECHO);
        let registry = registry(&tools, &work);
        let definition = registry
            .definitions()
            .into_iter()
            .find(|d| d.name == "shout")
            .unwrap();
        assert_eq!(definition.description, "Shouts text.");
        assert_eq!(definition.parameters["required"], json!(["text"]));
    }

    #[tokio::test]
    async fn run_returns_its_result() {
        assert_eq!(call("shout", ECHO, json!({ "text": "hi" })).await, "HI");
    }

    #[tokio::test]
    async fn non_string_results_come_back_as_json() {
        let body =
            "function parameters() { return {}; } function run() { return { n: 2, ok: true }; }";
        assert_eq!(call("obj", body, json!({})).await, r#"{"n":2,"ok":true}"#);
    }

    #[tokio::test]
    async fn output_is_checked_against_the_declared_shape() {
        let body = r#"function parameters() { return {}; }
            function output() { return { type: "string" }; }
            function run() { return 5; }"#;
        let out = call("bad", body, json!({})).await;
        assert!(out.starts_with("error: output did not match"), "{out}");
        assert!(out.contains("should be string but is number"));
    }

    #[tokio::test]
    async fn thrown_errors_become_tool_errors() {
        let body =
            r#"function parameters() { return {}; } function run() { throw new Error("kaput"); }"#;
        let out = call("boom", body, json!({})).await;
        assert!(out.starts_with("error:") && out.contains("kaput"), "{out}");
    }

    #[tokio::test]
    async fn runaway_loops_are_stopped() {
        let body = "function parameters() { return {}; } function run() { while (true) {} }";
        let out = call("spin", body, json!({})).await;
        assert!(out.starts_with("error:"), "{out}");
    }

    #[tokio::test]
    async fn the_working_directory_is_injected() {
        let body =
            "function parameters() { return {}; } function run() { return context.workingDir; }";
        let (tools, work) = (TempDir::new().unwrap(), TempDir::new().unwrap());
        script(tools.path(), "where.js", body);
        let out = registry(&tools, &work).dispatch("where", json!({})).await;
        assert_eq!(out, work.path().display().to_string());
    }

    #[tokio::test]
    async fn shell_runs_commands_in_the_working_directory() {
        let body = r#"function parameters() { return {}; } function run() { return shell("echo from-shell"); }"#;
        let out = call("sh1", body, json!({})).await;
        assert!(
            out.contains("from-shell") && out.contains("exit code: 0"),
            "{out}"
        );
    }

    #[tokio::test]
    async fn shell_goes_through_the_safety_review() {
        let body = r#"function parameters() { return {}; } function run() { return shell("touch made"); }"#;
        let (tools, work) = (TempDir::new().unwrap(), TempDir::new().unwrap());
        script(tools.path(), "sneaky.js", body);
        let validator: Arc<dyn Validator> = Arc::new(FixedValidator::unsafe_because("no touching"));
        let registry = assemble(work.path(), Some(&validator), Some(tools.path()));
        let out = registry.dispatch("sneaky", json!({})).await;
        assert!(out.contains("blocked by safety review"), "{out}");
        assert!(!work.path().join("made").exists());
    }

    #[test]
    fn broken_scripts_are_skipped_not_fatal() {
        let (tools, work) = (TempDir::new().unwrap(), TempDir::new().unwrap());
        script(tools.path(), "good.js", ECHO);
        script(tools.path(), "syntax.js", "function (");
        script(
            tools.path(),
            "norun.js",
            "function parameters() { return {}; }",
        );
        script(tools.path(), "read_file.js", ECHO);
        script(tools.path(), "notes.txt", "not a tool");
        let shell = assemble(work.path(), None, None)
            .get("invoke_shell")
            .unwrap();
        let reserved = vec!["read_file".to_string()];
        let loaded = super::load_js_tools(tools.path(), work.path(), &shell, &reserved);
        let names: Vec<_> = loaded.tools.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, ["good"]);
        assert_eq!(loaded.warnings.len(), 3, "{:?}", loaded.warnings);
        assert!(loaded.warnings.iter().any(|w| w.contains("built-in")));
    }

    #[test]
    fn a_missing_directory_means_no_custom_tools() {
        let work = TempDir::new().unwrap();
        let shell = assemble(work.path(), None, None)
            .get("invoke_shell")
            .unwrap();
        let loaded = super::load_js_tools(&work.path().join("nope"), work.path(), &shell, &[]);
        assert!(loaded.tools.is_empty() && loaded.warnings.is_empty());
    }
}
