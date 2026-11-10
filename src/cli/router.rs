use std::process::ExitCode;

use clap::Parser;

use crate::cli::Cli;
use crate::feature::greeting;

pub fn run() -> ExitCode {
    let _cli = Cli::parse();
    println!("{}", greeting::message());
    ExitCode::SUCCESS
}
