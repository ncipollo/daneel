use rig::client::CompletionClient;
use rig::completion::Chat;

use crate::feature::provider::{ChatRequest, ProviderError};
use crate::feature::providers::history_to_rig;

/// Rig defaults to zero tool turns, so "no limit" must be set explicitly. Not
/// `usize::MAX`: rig adds one to this value internally and would overflow.
pub const NO_TURN_LIMIT: usize = usize::MAX / 2;

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
    builder
        .build()
        .chat(request.prompt, history)
        .await
        .map_err(|e| ProviderError::Provider(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_turn_limit_leaves_headroom_for_rigs_internal_increment() {
        assert!(NO_TURN_LIMIT.checked_add(2).is_some());
    }
}
