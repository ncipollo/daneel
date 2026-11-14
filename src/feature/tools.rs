//! The agent's tools. Every call goes through `ToolRegistry::dispatch`, the single
//! point where custom tools and safety wrappers can later hook in.

pub mod agent_tool;
pub mod args;
pub mod edit_file;
pub mod invoke_shell;
pub mod js;
pub mod output;
pub mod read_file;
pub mod registry;
pub mod rig_adapter;
pub mod safety;
pub mod write_file;

use std::path::Path;
use std::sync::Arc;

pub use agent_tool::AgentTool;
pub use registry::ToolRegistry;
pub use rig_adapter::{into_rig, into_rig_observed};
pub use safety::Validator;

use edit_file::EditFile;
use invoke_shell::InvokeShell;
use read_file::ReadFile;
use write_file::WriteFile;

/// The four basic tools, all operating relative to `working_dir`.
pub fn builtin_registry(working_dir: &Path) -> ToolRegistry {
    phase_registry(working_dir, true)
}

/// The tools a chat turn gets: the built-ins (write and shell tools behind a
/// safety review when `safety` is set), then any custom scripts in `tools_dir`.
pub fn assemble(
    working_dir: &Path,
    safety: Option<&Arc<dyn Validator>>,
    tools_dir: Option<&Path>,
) -> ToolRegistry {
    let mut registry = builtin_registry(working_dir);
    if let Some(validator) = safety {
        safety::guard(&mut registry, validator, working_dir);
    }
    if let Some(dir) = tools_dir {
        add_custom_tools(&mut registry, dir, working_dir);
    }
    registry
}

/// Custom scripts get the registry's own shell tool, so a safety review applies to them too.
fn add_custom_tools(registry: &mut ToolRegistry, dir: &Path, working_dir: &Path) {
    let Some(shell) = registry.get("invoke_shell") else {
        return;
    };
    let reserved: Vec<String> = registry.names().map(str::to_string).collect();
    let loaded = js::load_js_tools(dir, working_dir, &shell, &reserved);
    for warning in &loaded.warnings {
        tracing::warn!("{warning}");
    }
    for tool in loaded.tools {
        registry.register(tool);
    }
}

/// `read_file`, plus the tools that change things when `allow_write` is set.
pub fn phase_registry(working_dir: &Path, allow_write: bool) -> ToolRegistry {
    let dir = working_dir.to_path_buf();
    let mut registry = ToolRegistry::new();
    registry.register(ReadFile::new(dir.clone()));
    if allow_write {
        registry.register(WriteFile::new(dir.clone()));
        registry.register(EditFile::new(dir.clone()));
        registry.register(InvokeShell::new(dir));
    }
    registry
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_only_phases_only_get_read_file() {
        let registry = phase_registry(Path::new("."), false);
        assert_eq!(registry.names().collect::<Vec<_>>(), ["read_file"]);
    }

    #[test]
    fn builtin_registry_has_the_four_tools() {
        let registry = builtin_registry(Path::new("."));
        let names: Vec<_> = registry.names().collect();
        assert_eq!(
            names,
            ["edit_file", "invoke_shell", "read_file", "write_file"]
        );
    }
}
