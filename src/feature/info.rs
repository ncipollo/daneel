//! Topic registry for `--info`, so AI agents can learn how to configure and use
//! daneel one topic at a time instead of reading one giant page.

mod config;
mod providers;
mod safe_tools;
mod settings;
mod strategies;
mod tools;
mod usage;

pub struct Topic {
    pub name: &'static str,
    pub summary: &'static str,
    render: fn() -> String,
}

pub const TOPICS: &[Topic] = &[
    Topic {
        name: "usage",
        summary: "Running the TUI and -p one-shot mode, JSON events, files daneel writes",
        render: usage::render,
    },
    Topic {
        name: "config",
        summary: "The ~/.daneel directory, file formats, $ENV_VAR values",
        render: config::render,
    },
    Topic {
        name: "providers",
        summary: "models file: provider kinds, local servers, API keys, picking a model",
        render: providers::render,
    },
    Topic {
        name: "settings",
        summary: "settings file keys",
        render: settings::render,
    },
    Topic {
        name: "safe-tools",
        summary: "Having an external agent review writes and shell commands",
        render: safe_tools::render,
    },
    Topic {
        name: "tools",
        summary: "Built-in tools and custom JavaScript tools",
        render: tools::render,
    },
    Topic {
        name: "strategies",
        summary: "Strategy and tactic phase files",
        render: strategies::render,
    },
];

/// The short page for a bare `--info`: one line per topic and how to open one.
pub fn directory() -> String {
    let mut page = String::from("TOPICS\n");
    for topic in TOPICS {
        page.push_str(&format!("  {:<12}{}\n", topic.name, topic.summary));
    }
    page.push_str("\nrun: daneel --info <topic>\n");
    page
}

/// The named topic's page, or `None` if there is no such topic.
pub fn topic(name: &str) -> Option<String> {
    TOPICS
        .iter()
        .find(|topic| topic.name == name)
        .map(|topic| (topic.render)())
}

pub fn names() -> Vec<&'static str> {
    TOPICS.iter().map(|topic| topic.name).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn topic_names_are_unique_and_lowercase() {
        let mut names = names();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), TOPICS.len());
        assert!(TOPICS.iter().all(|t| t.name == t.name.to_lowercase()));
    }

    #[test]
    fn every_topic_renders_a_page() {
        for topic in TOPICS {
            assert!(
                !(topic.render)().trim().is_empty(),
                "empty page: {}",
                topic.name
            );
        }
    }

    #[test]
    fn the_directory_lists_every_topic() {
        let page = directory();
        for topic in TOPICS {
            assert!(page.contains(topic.name) && page.contains(topic.summary));
        }
    }

    #[test]
    fn an_unknown_topic_has_no_page() {
        assert!(topic("bogus").is_none());
        assert!(topic("providers").is_some());
    }
}
