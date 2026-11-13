use std::process::ExitCode;

use crate::cli::{Cli, ModeArg};
use crate::feature::oneshot::{self, OutputMode};

pub fn run(cli: &Cli, message: &str) -> ExitCode {
    let mode = match cli.mode {
        ModeArg::Text => OutputMode::Text,
        ModeArg::Json => OutputMode::Json,
    };
    match oneshot::run_blocking(message, mode) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("daneel: {err}");
            ExitCode::FAILURE
        }
    }
}
