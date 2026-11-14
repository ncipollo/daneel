//! Runs tool scripts on boa. Every evaluation gets a fresh `Context`, so tools
//! cannot leak state into each other or between calls.

use std::cell::RefCell;
use std::path::Path;
use std::sync::Arc;

use boa_engine::object::JsObject;
use boa_engine::{
    Context, JsArgs, JsError, JsNativeError, JsResult, JsString, JsValue, NativeFunction, Source,
    js_string,
};
use serde_json::{Value, json};
use tokio::runtime::Handle;

use crate::feature::tools::AgentTool;

/// Stops a runaway `while (true)` instead of hanging the agent.
const LOOP_LIMIT: u64 = 5_000_000;

/// How a script's `shell(command)` reaches the (possibly safety-wrapped) shell tool.
pub struct ShellAccess {
    pub tool: Arc<dyn AgentTool>,
    pub handle: Handle,
}

thread_local! {
    static SHELL: RefCell<Option<ShellAccess>> = const { RefCell::new(None) };
}

/// What a script declares about itself.
pub struct Meta {
    pub description: Option<String>,
    pub parameters: Value,
    pub output: Option<Value>,
}

/// Loads `source` and reads `parameters()`, `output()` and `description()`.
pub fn describe(source: &str, working_dir: &Path) -> Result<Meta, String> {
    let mut context = load(source, working_dir)?;
    let parameters = call_optional(&mut context, "parameters", &[])?
        .ok_or("the script has no parameters() function")?;
    if function(&mut context, "run")?.is_none() {
        return Err("the script has no run(args) function".to_string());
    }
    let description = call_optional(&mut context, "description", &[])?;
    Ok(Meta {
        description: description.and_then(|v| v.as_str().map(str::to_string)),
        parameters,
        output: call_optional(&mut context, "output", &[])?,
    })
}

/// Calls the script's `run(args)` with `shell` available to it.
pub fn run(
    source: &str,
    args: &Value,
    working_dir: &Path,
    shell: ShellAccess,
) -> Result<Value, String> {
    SHELL.with(|cell| *cell.borrow_mut() = Some(shell));
    let result = run_loaded(source, args, working_dir);
    SHELL.with(|cell| *cell.borrow_mut() = None);
    result
}

fn run_loaded(source: &str, args: &Value, working_dir: &Path) -> Result<Value, String> {
    let mut context = load(source, working_dir)?;
    let arg = JsValue::from_json(args, &mut context).map_err(js_error)?;
    let value = call_optional(&mut context, "run", &[arg])?;
    value.ok_or_else(|| "run() returned nothing; it must return a value".to_string())
}

fn load(source: &str, working_dir: &Path) -> Result<Context, String> {
    let mut context = Context::default();
    context
        .runtime_limits_mut()
        .set_loop_iteration_limit(LOOP_LIMIT);
    let shell = NativeFunction::from_fn_ptr(host_shell);
    context
        .register_global_builtin_callable(JsString::from("shell"), 1, shell)
        .map_err(js_error)?;
    let info = json!({ "workingDir": working_dir.display().to_string() });
    let info = JsValue::from_json(&info, &mut context).map_err(js_error)?;
    let global = context.global_object();
    global
        .set(js_string!("context"), info, true, &mut context)
        .map_err(js_error)?;
    context
        .eval(Source::from_bytes(source))
        .map_err(|e| format!("script error: {}", js_error(e)))?;
    Ok(context)
}

fn function(context: &mut Context, name: &str) -> Result<Option<JsObject>, String> {
    let global = context.global_object();
    let value = global
        .get(JsString::from(name), context)
        .map_err(js_error)?;
    Ok(value.as_callable())
}

/// Calls the global function `name`; `None` when it is missing or returns undefined.
fn call_optional(
    context: &mut Context,
    name: &str,
    args: &[JsValue],
) -> Result<Option<Value>, String> {
    let Some(function) = function(context, name)? else {
        return Ok(None);
    };
    let result = function
        .call(&JsValue::undefined(), args, context)
        .map_err(|e| format!("{name}() failed: {}", js_error(e)))?;
    result.to_json(context).map_err(js_error)
}

/// `shell(command)`: runs the command through the shell tool and returns its output text.
fn host_shell(_this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let command = args
        .get_or_undefined(0)
        .to_string(context)?
        .to_std_string_escaped();
    let output = SHELL.with(|cell| match &*cell.borrow() {
        None => Err("shell() is not available here".to_string()),
        Some(access) => {
            let call = access.tool.call(json!({ "command": command }));
            access.handle.block_on(call)
        }
    });
    let text = output.map_err(|message| JsNativeError::error().with_message(message))?;
    Ok(JsValue::from(JsString::from(text)))
}

fn js_error(error: JsError) -> String {
    error.to_string()
}
