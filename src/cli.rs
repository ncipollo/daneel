//! View layer. Parses arguments with clap and delegates to the feature layer.
//! No domain logic lives here.

pub mod prompt;
pub mod router;
pub mod tui;

use clap::{Parser, ValueEnum};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ModeArg {
    Text,
    Json,
}

#[derive(Parser, Debug)]
#[command(name = "daneel", version, about)]
pub struct Cli {
    /// Send a single prompt, print the response, and exit.
    #[arg(short = 'p', long, requires = "message")]
    pub prompt: bool,

    /// Output format for one-shot mode: plain text or newline-delimited JSON events.
    #[arg(long, value_enum, default_value_t = ModeArg::Text, requires = "prompt")]
    pub mode: ModeArg,

    /// The prompt to send (used with -p).
    pub message: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Cli, clap::Error> {
        Cli::try_parse_from(std::iter::once("daneel").chain(args.iter().copied()))
    }

    #[test]
    fn no_args_is_interactive() {
        let cli = parse(&[]).unwrap();
        assert!(!cli.prompt && cli.message.is_none());
    }

    #[test]
    fn one_shot_prompt() {
        let cli = parse(&["-p", "hi"]).unwrap();
        assert!(cli.prompt);
        assert_eq!(cli.message.as_deref(), Some("hi"));
        assert_eq!(cli.mode, ModeArg::Text);
    }

    #[test]
    fn json_mode_after_flag() {
        let cli = parse(&["-p", "--mode", "json", "hi"]).unwrap();
        assert_eq!(cli.mode, ModeArg::Json);
    }

    #[test]
    fn prompt_flag_requires_a_message() {
        assert!(parse(&["-p"]).is_err());
    }

    #[test]
    fn mode_requires_prompt_flag() {
        assert!(parse(&["--mode", "json"]).is_err());
    }
}
