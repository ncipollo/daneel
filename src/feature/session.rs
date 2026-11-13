//! Sessions: every run is captured under `~/.daneel/sessions/<id>/` so users
//! and the agent can inspect what happened. Old sessions are pruned.

pub mod metadata;
pub mod prune;
pub mod store;

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use crate::feature::config;

pub use metadata::SessionMetadata;
pub use store::Session;

pub const RETENTION: Duration = Duration::from_secs(7 * 24 * 60 * 60);

pub fn default_root() -> Option<PathBuf> {
    config::default_dir().ok().map(|dir| dir.join("sessions"))
}

/// Prunes expired sessions, then starts a new one. Sessions are best-effort:
/// any storage failure yields `None` so a run is never blocked by them.
pub fn start_default(working_dir: &Path, model: &str) -> Option<Session> {
    let root = default_root()?;
    let now = SystemTime::now();
    let _ = prune::prune(&root, now, RETENTION);
    Session::create(&root, now, working_dir, model).ok()
}
