use std::process::ExitCode;

use clap::Parser;

use crate::cli::{Cli, prompt};

pub fn run() -> ExitCode {
    let cli = Cli::parse();
    match (cli.prompt, cli.message.as_deref()) {
        (true, Some(message)) => prompt::run(&cli, message),
        _ => {
            eprintln!("daneel: the interactive TUI is not available yet; use `daneel -p \"...\"`");
            ExitCode::FAILURE
        }
    }
}
