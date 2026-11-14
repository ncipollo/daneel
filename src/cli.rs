//! View layer. Parses arguments with clap and delegates to the feature layer.
//! No domain logic lives here.

pub mod info;
pub mod prompt;
pub mod regenerate;
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
    #[arg(short = 'p', long, requires = "message", conflicts_with = "info")]
    pub prompt: bool,

    /// Output format for one-shot mode: plain text or newline-delimited JSON events.
    #[arg(long, value_enum, default_value_t = ModeArg::Text, requires = "prompt")]
    pub mode: ModeArg,

    /// The prompt to send (used with -p).
    pub message: Option<String>,

    /// Print documentation for agents and exit. Bare --info lists the topics;
    /// --info <TOPIC> prints one.
    #[arg(long, value_name = "TOPIC", num_args = 0..=1, conflicts_with = "message")]
    pub info: Option<Option<String>>,

    /// Overwrite the default strategy and tactic files with the built-in ones
    /// (asks for confirmation first) and exit.
    #[arg(long, conflicts_with_all = ["info", "message", "prompt"])]
    pub regenerate_strategy: bool,
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

    #[test]
    fn bare_info_lists_topics() {
        assert_eq!(parse(&["--info"]).unwrap().info, Some(None));
    }

    #[test]
    fn info_takes_a_topic() {
        let cli = parse(&["--info", "providers"]).unwrap();
        assert_eq!(cli.info, Some(Some("providers".to_string())));
        assert!(cli.message.is_none());
    }

    #[test]
    fn regenerate_strategy_stands_alone() {
        assert!(
            parse(&["--regenerate-strategy"])
                .unwrap()
                .regenerate_strategy
        );
        assert!(parse(&["--regenerate-strategy", "-p", "hi"]).is_err());
        assert!(parse(&["--regenerate-strategy", "--info"]).is_err());
    }

    #[test]
    fn info_conflicts_with_prompt_mode() {
        assert!(parse(&["--info", "-p", "hi"]).is_err());
        assert!(parse(&["-p", "--info", "hi"]).is_err());
    }
}
