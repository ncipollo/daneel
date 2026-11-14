use std::process::ExitCode;

use clap::Parser;

use crate::cli::{Cli, prompt, tui};
use crate::feature::logging;
use crate::feature::strategy::defaults;

pub fn run() -> ExitCode {
    let cli = Cli::parse();
    if let Err(message) = defaults::ensure_default_dir() {
        eprintln!("daneel: {message}");
    }
    if let Err(message) = logging::init_default() {
        eprintln!("daneel: {message}");
    }
    match cli.message.as_deref() {
        Some(message) if cli.prompt => prompt::run(&cli, message),
        _ => tui::run(),
    }
}
