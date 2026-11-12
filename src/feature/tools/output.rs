pub const MAX_OUTPUT_CHARS: usize = 20_000;

/// Cuts `text` to `limit` characters and says how much was dropped.
pub fn truncate(text: String, limit: usize) -> String {
    let Some((cut, _)) = text.char_indices().nth(limit) else {
        return text;
    };
    let dropped = text[cut..].chars().count();
    format!(
        "{}\n[output truncated: {dropped} more characters]",
        &text[..cut]
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_text_is_unchanged() {
        assert_eq!(truncate("hello".to_string(), 10), "hello");
    }

    #[test]
    fn long_text_is_cut_with_a_marker() {
        let out = truncate("abcdef".to_string(), 4);
        assert_eq!(out, "abcd\n[output truncated: 2 more characters]");
    }

    #[test]
    fn cuts_on_character_boundaries() {
        let out = truncate("ééééé".to_string(), 2);
        assert!(out.starts_with("éé\n"));
    }
}
