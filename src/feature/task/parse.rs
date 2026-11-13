use crate::feature::task::{GENERIC_TYPE, Task};

const TYPE_KEY: &str = "type";
const DESCRIPTION_KEY: &str = "description";

/// Parses a `TYPE:` / `DESCRIPTION:` reply. Returns `None` when there is no usable
/// description. A missing type becomes the generic one. Small models add noise,
/// so unknown lines before the keys are ignored and the description may span lines.
pub fn parse_reply(reply: &str) -> Option<Task> {
    let mut task_type = String::new();
    let mut description: Option<String> = None;
    for line in reply.lines() {
        match split_key(line) {
            Some((TYPE_KEY, value)) if description.is_none() => task_type = value.to_string(),
            Some((DESCRIPTION_KEY, value)) => description = Some(value.to_string()),
            _ => append_continuation(&mut description, line),
        }
    }
    let description = description?.trim().to_string();
    if description.is_empty() {
        return None;
    }
    let task_type = clean_type(&task_type);
    Some(Task::new(task_type, description))
}

fn split_key(line: &str) -> Option<(&'static str, &str)> {
    let (key, value) = line.split_once(':')?;
    let key = key.trim().trim_matches(|c: char| !c.is_alphanumeric());
    [TYPE_KEY, DESCRIPTION_KEY]
        .into_iter()
        .find(|k| key.eq_ignore_ascii_case(k))
        .map(|k| {
            (
                k,
                value.trim_matches(|c: char| c == '*' || c == '_' || c.is_whitespace()),
            )
        })
}

fn append_continuation(description: &mut Option<String>, line: &str) {
    if let Some(text) = description {
        text.push('\n');
        text.push_str(line.trim_end());
    }
}

fn clean_type(raw: &str) -> String {
    let cleaned = raw.trim().trim_matches(|c: char| !c.is_alphanumeric());
    if cleaned.is_empty() {
        GENERIC_TYPE.to_string()
    } else {
        cleaned.to_lowercase()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_clean_reply() {
        let task = parse_reply("TYPE: code creation\nDESCRIPTION: write a parser").unwrap();
        assert_eq!(task, Task::new("code creation", "write a parser"));
    }

    #[test]
    fn ignores_noise_and_case() {
        let reply = "Sure! Here you go:\n\ntype: Query\n**Description:** find the config loader\n";
        assert_eq!(
            parse_reply(reply).unwrap(),
            Task::new("query", "find the config loader")
        );
    }

    #[test]
    fn description_may_span_lines() {
        let reply = "TYPE: refactor\nDESCRIPTION: split the module\nand add tests";
        assert_eq!(
            parse_reply(reply).unwrap().description,
            "split the module\nand add tests"
        );
    }

    #[test]
    fn missing_type_is_generic() {
        let task = parse_reply("DESCRIPTION: do it").unwrap();
        assert_eq!(task.task_type, "general");
    }

    #[test]
    fn missing_or_empty_description_is_none() {
        assert_eq!(parse_reply("TYPE: query"), None);
        assert_eq!(parse_reply("TYPE: query\nDESCRIPTION:   "), None);
        assert_eq!(parse_reply("no structure at all"), None);
        assert_eq!(parse_reply(""), None);
    }
}
