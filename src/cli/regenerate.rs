use std::io::{self, BufRead, Write};
use std::process::ExitCode;

use crate::feature::strategy::defaults;

pub fn run() -> ExitCode {
    eprint!(
        "This overwrites the default strategy and tactic files, discarding any edits. Are you sure? [y/N] "
    );
    let _ = io::stderr().flush();
    let mut answer = String::new();
    if io::stdin().lock().read_line(&mut answer).is_err() || !is_yes(&answer) {
        eprintln!("Cancelled; nothing was changed.");
        return ExitCode::FAILURE;
    }
    match defaults::regenerate_default_dir() {
        Ok(paths) => {
            for path in paths {
                println!("wrote {}", path.display());
            }
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("daneel: {message}");
            ExitCode::FAILURE
        }
    }
}

fn is_yes(answer: &str) -> bool {
    matches!(answer.trim().to_lowercase().as_str(), "y" | "yes")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_an_explicit_yes_confirms() {
        assert!(is_yes("y\n") && is_yes(" YES "));
        assert!(!is_yes("\n") && !is_yes("n") && !is_yes("yep"));
    }
}
