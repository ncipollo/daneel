/// Wrapper prefixes that add nothing for a reader; the root cause follows them.
const NOISE_PREFIXES: [&str; 4] = [
    "provider error:",
    "CompletionError:",
    "HttpError:",
    "Http client error:",
];

/// Shortens an error for display: drops redundant wrapper prefixes and joins
/// lines. The full text should be kept elsewhere for debugging.
pub fn summarize_error(full: &str) -> String {
    let mut rest = full.trim();
    while let Some(stripped) = strip_noise(rest) {
        rest = stripped;
    }
    let joined = rest.split_whitespace().collect::<Vec<_>>().join(" ");
    if joined.is_empty() {
        full.trim().to_string()
    } else {
        joined
    }
}

fn strip_noise(text: &str) -> Option<&str> {
    NOISE_PREFIXES
        .iter()
        .find_map(|prefix| text.strip_prefix(prefix))
        .map(str::trim_start)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_the_ollama_connection_error_chain() {
        let full = "provider error: CompletionError: HttpError: Http client error: error sending request for url (http://127.0.0.1:9/api/chat)";
        assert_eq!(
            summarize_error(full),
            "error sending request for url (http://127.0.0.1:9/api/chat)"
        );
    }

    #[test]
    fn collapses_newlines_and_spaces() {
        assert_eq!(
            summarize_error("provider error: bad\n  gateway\n"),
            "bad gateway"
        );
    }

    #[test]
    fn short_messages_pass_through() {
        assert_eq!(summarize_error("tool error: nope"), "tool error: nope");
    }

    #[test]
    fn prefix_only_input_is_kept_whole() {
        assert_eq!(summarize_error("provider error:"), "provider error:");
        assert_eq!(summarize_error(""), "");
    }
}
