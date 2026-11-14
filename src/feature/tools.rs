//! The agent's tools. Every call goes through `ToolRegistry::dispatch`, the single
//! point where custom tools and safety wrappers can later hook in.

pub mod agent_tool;
pub mod args;
pub mod edit_file;
pub mod find_files;
pub mod grep_files;
pub mod invoke_shell;
pub mod js;
pub mod list_files;
pub mod output;
pub mod read_file;
pub mod registry;
pub mod rig_adapter;
pub mod safety;
pub mod search;
pub mod write_file;

use std::path::{Path, PathBuf};
use std::sync::Arc;

pub use agent_tool::AgentTool;
pub use registry::ToolRegistry;
pub use rig_adapter::{into_rig, into_rig_observed};
pub use safety::Validator;

use edit_file::EditFile;
use find_files::FindFiles;
use grep_files::GrepFiles;
use invoke_shell::InvokeShell;
use list_files::ListFiles;
use read_file::ReadFile;
use write_file::WriteFile;

/// Every basic tool, all operating relative to `working_dir`.
pub fn builtin_registry(working_dir: &Path) -> ToolRegistry {
    phase_registry(working_dir, true)
}

/// What every tool set is built with: an optional safety reviewer and the folder
/// of custom JavaScript tools.
#[derive(Clone, Default)]
pub struct ToolSetup {
    pub safety: Option<Arc<dyn Validator>>,
    pub tools_dir: Option<PathBuf>,
}

/// The tools for one phase: the reading tools, plus the write and shell tools when
/// `allow_write` is set (behind a safety review when the setup has one), then
/// any custom scripts. Read-only phases get no shell, so no custom tools either.
pub fn assemble(working_dir: &Path, setup: &ToolSetup, allow_write: bool) -> ToolRegistry {
    let mut registry = phase_registry(working_dir, allow_write);
    if let Some(validator) = &setup.safety {
        safety::guard(&mut registry, validator, working_dir);
    }
    if let Some(dir) = &setup.tools_dir {
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

/// The read-only tools (`read_file` and the searches), plus the tools that change
/// things when `allow_write` is set.
pub fn phase_registry(working_dir: &Path, allow_write: bool) -> ToolRegistry {
    let dir = working_dir.to_path_buf();
    let mut registry = ToolRegistry::new();
    registry.register(ReadFile::new(dir.clone()));
    registry.register(ListFiles::new(dir.clone()));
    registry.register(FindFiles::new(dir.clone()));
    registry.register(GrepFiles::new(dir.clone()));
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
    fn read_only_phases_only_get_the_reading_tools() {
        let registry = phase_registry(Path::new("."), false);
        let names: Vec<_> = registry.names().collect();
        assert_eq!(
            names,
            ["find_files", "grep_files", "list_files", "read_file"]
        );
    }

    #[test]
    fn builtin_registry_has_every_tool() {
        let registry = builtin_registry(Path::new("."));
        let names: Vec<_> = registry.names().collect();
        assert_eq!(
            names,
            [
                "edit_file",
                "find_files",
                "grep_files",
                "invoke_shell",
                "list_files",
                "read_file",
                "write_file"
            ]
        );
    }
}
