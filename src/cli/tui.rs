use std::process::ExitCode;

use crate::tui;

pub fn run() -> ExitCode {
    match tui::start() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("daneel: {err}");
            ExitCode::FAILURE
        }
    }
}
