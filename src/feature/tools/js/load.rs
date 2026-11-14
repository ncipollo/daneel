use std::fs;
use std::path::Path;
use std::sync::Arc;

use crate::feature::tools::AgentTool;
use crate::feature::tools::js::engine;
use crate::feature::tools::js::tool::JsTool;

/// The tools found in a directory, plus a message for each script that was skipped.
#[derive(Default)]
pub struct Loaded {
    pub tools: Vec<JsTool>,
    pub warnings: Vec<String>,
}

/// Loads every `*.js` file in `dir`. A broken script is skipped with a warning
/// and never stops the others; a missing directory just means no custom tools.
/// `reserved` names (the built-ins) cannot be replaced.
pub fn load_js_tools(
    dir: &Path,
    working_dir: &Path,
    shell: &Arc<dyn AgentTool>,
    reserved: &[String],
) -> Loaded {
    let mut loaded = Loaded::default();
    let Ok(entries) = fs::read_dir(dir) else {
        return loaded;
    };
    let mut paths: Vec<_> = entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
    paths.sort();
    for path in paths
        .iter()
        .filter(|p| p.extension().is_some_and(|e| e == "js"))
    {
        let name = path.file_stem().map(|s| s.to_string_lossy().into_owned());
        let Some(name) = name else { continue };
        match load_one(path, &name, working_dir, shell, reserved) {
            Ok(tool) => loaded.tools.push(tool),
            Err(why) => loaded
                .warnings
                .push(format!("skipped tool '{name}': {why}")),
        }
    }
    loaded
}

fn load_one(
    path: &Path,
    name: &str,
    working_dir: &Path,
    shell: &Arc<dyn AgentTool>,
    reserved: &[String],
) -> Result<JsTool, String> {
    if !valid_name(name) {
        return Err("tool names may only use letters, digits, '_' and '-'".to_string());
    }
    if reserved.iter().any(|r| r == name) {
        return Err("that name belongs to a built-in tool".to_string());
    }
    let source = fs::read_to_string(path).map_err(|e| format!("could not read the file: {e}"))?;
    let meta = engine::describe(&source, working_dir)?;
    Ok(JsTool {
        name: name.to_string(),
        description: meta
            .description
            .unwrap_or_else(|| format!("Custom tool '{name}'.")),
        schema: meta.parameters,
        output: meta.output,
        source: Arc::from(source),
        working_dir: working_dir.to_path_buf(),
        shell: Arc::clone(shell),
    })
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}
