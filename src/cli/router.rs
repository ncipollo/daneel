use std::process::ExitCode;

use clap::Parser;

use crate::cli::{Cli, info, prompt, regenerate, tui};
use crate::feature::logging;
use crate::feature::strategy::defaults;

pub fn run() -> ExitCode {
    let cli = Cli::parse();
    if let Some(topic) = &cli.info {
        return info::run(topic.as_deref());
    }
    if cli.regenerate_strategy {
        return regenerate::run();
    }
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
