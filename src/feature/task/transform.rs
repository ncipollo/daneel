use crate::feature::provider::{ChatRequest, Provider};
use crate::feature::task::Task;
use crate::feature::task::parse::parse_reply;

const INSTRUCTIONS: &str = "\
Turn the user's message into one task. Reply with exactly two lines and nothing else:
TYPE: <one or two words, such as query or code creation>
DESCRIPTION: <one sentence saying what will be done>

Example message: where is the config loaded?
TYPE: query
DESCRIPTION: Find where the configuration is loaded and explain it.

Example message: add a --verbose flag
TYPE: code creation
DESCRIPTION: Add a --verbose flag to the command line.";

/// Asks the model to turn a user message into a task. Never fails: a provider
/// error or an unusable reply falls back to a generic task wrapping the message.
pub async fn from_message(provider: &dyn Provider, message: &str) -> Task {
    let request = ChatRequest::new(INSTRUCTIONS, message);
    match provider.chat(request).await {
        Ok(reply) => parse_reply(&reply).unwrap_or_else(|| Task::generic(message)),
        Err(_) => Task::generic(message),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::feature::provider::{BoxFuture, ProviderError};

    struct Reply(Result<&'static str, &'static str>);

    impl Provider for Reply {
        fn chat<'a>(
            &'a self,
            request: ChatRequest<'a>,
        ) -> BoxFuture<'a, Result<String, ProviderError>> {
            Box::pin(async move {
                assert!(request.tools.is_empty() && request.history.is_empty());
                self.0
                    .map(str::to_string)
                    .map_err(|e| ProviderError::Provider(e.to_string()))
            })
        }
    }

    #[tokio::test]
    async fn good_reply_becomes_a_task() {
        let provider = Reply(Ok("TYPE: query\nDESCRIPTION: look it up"));
        let task = from_message(&provider, "where is it?").await;
        assert_eq!(task, Task::new("query", "look it up"));
    }

    #[tokio::test]
    async fn garbage_reply_falls_back_to_the_raw_message() {
        let task = from_message(&Reply(Ok("¯\\_(ツ)_/¯")), "do the thing").await;
        assert_eq!(task, Task::generic("do the thing"));
    }

    #[tokio::test]
    async fn provider_error_falls_back_to_the_raw_message() {
        let task = from_message(&Reply(Err("down")), "do the thing").await;
        assert_eq!(task, Task::generic("do the thing"));
    }
}
