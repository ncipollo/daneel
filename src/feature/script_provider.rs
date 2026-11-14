//! Test-only provider that plays back scripted tool calls, so the machines can
//! be exercised without a model.

use std::collections::VecDeque;
use std::sync::Mutex;

use serde_json::Value;

use crate::feature::provider::{BoxFuture, ChatRequest, Provider, ProviderError};

/// One scripted tool call: the tool's name and its arguments.
pub type Call = (String, Value);

pub fn call(name: &str, args: Value) -> Call {
    (name.to_string(), args)
}

/// Each `chat` pops the next step and makes its calls through the request's
/// tools. When the script runs out the model "says" something and calls nothing.
pub struct ScriptProvider {
    steps: Mutex<VecDeque<Vec<Call>>>,
    prompts: Mutex<Vec<String>>,
    tool_names: Mutex<Vec<Vec<String>>>,
}

impl ScriptProvider {
    pub fn new(steps: Vec<Vec<Call>>) -> Self {
        Self {
            steps: Mutex::new(steps.into()),
            prompts: Mutex::new(Vec::new()),
            tool_names: Mutex::new(Vec::new()),
        }
    }

    pub fn prompts(&self) -> Vec<String> {
        self.prompts.lock().unwrap().clone()
    }

    pub fn tool_names(&self) -> Vec<Vec<String>> {
        self.tool_names.lock().unwrap().clone()
    }
}

impl Provider for ScriptProvider {
    fn chat<'a>(
        &'a self,
        request: ChatRequest<'a>,
    ) -> BoxFuture<'a, Result<String, ProviderError>> {
        Box::pin(async move {
            self.prompts
                .lock()
                .unwrap()
                .push(request.prompt.to_string());
            let names = request.tools.iter().map(|tool| tool.name()).collect();
            self.tool_names.lock().unwrap().push(names);
            let step = self.steps.lock().unwrap().pop_front().unwrap_or_default();
            for (name, args) in step {
                let tool = request.tools.iter().find(|tool| tool.name() == name);
                let tool = tool.unwrap_or_else(|| panic!("no tool named {name}"));
                let _ = tool.call(args.to_string()).await;
            }
            Ok("scripted reply".to_string())
        })
    }
}
