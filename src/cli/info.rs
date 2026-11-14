use std::process::ExitCode;

use crate::feature::info;

pub fn run(topic: Option<&str>) -> ExitCode {
    match topic {
        None => {
            println!("{}", info::directory());
            ExitCode::SUCCESS
        }
        Some(name) => match info::topic(name) {
            Some(page) => {
                println!("{page}");
                ExitCode::SUCCESS
            }
            None => {
                eprintln!("{}", unknown_topic_error(name));
                ExitCode::FAILURE
            }
        },
    }
}

fn unknown_topic_error(name: &str) -> String {
    format!(
        "error: unknown topic \"{name}\": available topics: {}",
        info::names().join(", ")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_topic_error_names_available_topics() {
        let error = unknown_topic_error("bogus");
        assert!(error.contains("unknown topic \"bogus\""));
        for name in info::names() {
            assert!(error.contains(name), "missing topic name: {name}");
        }
    }
}
