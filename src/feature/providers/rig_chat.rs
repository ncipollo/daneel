use rig::client::CompletionClient;
use rig::completion::{Chat, CompletionError, PromptError};

use crate::feature::provider::{ChatRequest, ProviderError};
use crate::feature::providers::history_to_rig;

/// Rig defaults to zero tool turns, so "no limit" must be set explicitly. Not
/// `usize::MAX`: rig adds one to this value internally and would overflow.
pub const NO_TURN_LIMIT: usize = usize::MAX / 2;
/// What rig reports when the model's message has neither text nor tool calls.
const EMPTY_RESPONSE: &str = "No content provided";

pub async fn run_chat<C>(
    client: C,
    model: &str,
    request: ChatRequest<'_>,
    additional_params: Option<serde_json::Value>,
) -> Result<String, ProviderError>
where
    C: CompletionClient,
    C::CompletionModel: 'static,
{
    let max_turns = request.options.max_turns.unwrap_or(NO_TURN_LIMIT);
    let mut builder = client
        .agent(model)
        .preamble(request.instructions)
        .tools(request.tools)
        .default_max_turns(max_turns);
    if let Some(params) = additional_params {
        builder = builder.additional_params(params);
    }
    let history = history_to_rig(request.history);
    let result = builder.build().chat(request.prompt, history).await;
    reply_or_error(result)
}

/// Some models (thinking ones especially) end a turn with no text and no tool
/// call. That is an empty reply for the phase to nudge, not a failed run.
fn reply_or_error(result: Result<String, PromptError>) -> Result<String, ProviderError> {
    match result {
        Err(PromptError::CompletionError(CompletionError::ResponseError(message)))
            if message == EMPTY_RESPONSE =>
        {
            tracing::warn!("model returned no content and no tool call");
            Ok(String::new())
        }
        other => other.map_err(|e| ProviderError::Provider(e.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn response_error(message: &str) -> Result<String, PromptError> {
        Err(CompletionError::ResponseError(message.into()).into())
    }

    #[test]
    fn an_empty_model_response_becomes_an_empty_reply() {
        assert_eq!(reply_or_error(response_error(EMPTY_RESPONSE)).unwrap(), "");
    }

    #[test]
    fn other_errors_still_fail_and_replies_pass_through() {
        assert!(reply_or_error(response_error("bad json")).is_err());
        assert_eq!(reply_or_error(Ok("hi".into())).unwrap(), "hi");
    }

    #[test]
    fn no_turn_limit_leaves_headroom_for_rigs_internal_increment() {
        assert!(NO_TURN_LIMIT.checked_add(2).is_some());
    }
}
