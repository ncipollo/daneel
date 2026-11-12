use std::process::ExitCode;

use clap::Parser;

use crate::cli::{Cli, prompt, tui};

pub fn run() -> ExitCode {
    let cli = Cli::parse();
    match cli.message.as_deref() {
        Some(message) if cli.prompt => prompt::run(&cli, message),
        _ => tui::run(),
    }
}
