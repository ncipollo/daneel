use std::process::ExitCode;

use clap::Parser;

use crate::cli::Cli;
use crate::feature::{config, greeting};

pub fn run() -> ExitCode {
    let _cli = Cli::parse();
    let _config = match config::load_default() {
        Ok(config) => config,
        Err(err) => {
            eprintln!("daneel: {err}");
            return ExitCode::FAILURE;
        }
    };
    println!("{}", greeting::message());
    ExitCode::SUCCESS
}
