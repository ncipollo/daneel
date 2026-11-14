use crate::feature::interaction::brief;
use crate::feature::provider::{ChatMessage, Role};

/// Messages from earlier exchanges that are carried into a new run (3 exchanges).
const KEPT_MESSAGES: usize = 6;

/// Short notes about the recent conversation. Each run starts fresh, so these
/// are how the model hears about what was said before.
pub fn background(history: &[ChatMessage]) -> Vec<String> {
    let start = history.len().saturating_sub(KEPT_MESSAGES);
    history[start..]
        .iter()
        .map(|message| match message.role {
            Role::User => format!("Earlier you were asked: {}", brief(&message.content)),
            Role::Assistant => format!("You answered: {}", brief(&message.content)),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message(role: Role, content: &str) -> ChatMessage {
        ChatMessage {
            role,
            content: content.to_string(),
        }
    }

    #[test]
    fn exchanges_become_short_notes() {
        let history = vec![
            message(Role::User, "fix it"),
            message(Role::Assistant, "done"),
        ];
        assert_eq!(
            background(&history),
            ["Earlier you were asked: fix it", "You answered: done"]
        );
    }

    #[test]
    fn only_the_recent_exchanges_are_kept() {
        let history: Vec<_> = (0..10)
            .map(|i| message(Role::User, &format!("q{i}")))
            .collect();
        let notes = background(&history);
        assert_eq!(notes.len(), KEPT_MESSAGES);
        assert!(notes[0].ends_with("q4"));
    }

    #[test]
    fn long_messages_are_shortened() {
        let history = vec![message(Role::Assistant, &"x".repeat(1000))];
        assert!(background(&history)[0].len() < 400);
    }

    #[test]
    fn no_history_means_no_notes() {
        assert!(background(&[]).is_empty());
    }
}
