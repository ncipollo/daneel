use std::path::{Path, PathBuf};

pub const DEFAULT_NAME: &str = "default";

pub fn strategies_dir(config_dir: &Path) -> PathBuf {
    config_dir.join("strategies")
}

pub fn tactics_dir(config_dir: &Path) -> PathBuf {
    strategies_dir(config_dir).join("tactics")
}

pub fn strategy_file(config_dir: &Path, name: &str) -> PathBuf {
    strategies_dir(config_dir).join(format!("{name}.toml"))
}

pub fn tactic_file(config_dir: &Path, name: &str) -> PathBuf {
    tactics_dir(config_dir).join(format!("{name}.toml"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tactics_live_inside_the_strategies_folder() {
        let root = Path::new("/h/.daneel");
        assert_eq!(
            tactic_file(root, "default"),
            Path::new("/h/.daneel/strategies/tactics/default.toml")
        );
        assert_eq!(
            strategy_file(root, "x"),
            Path::new("/h/.daneel/strategies/x.toml")
        );
    }
}
