pub fn message() -> &'static str {
    "Greetings."
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_is_greeting() {
        assert_eq!(message(), "Greetings.");
    }
}
