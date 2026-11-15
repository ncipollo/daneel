//! The `config` topic: the config directory and its file conventions.

pub fn render() -> String {
    String::from(
        "CONFIG DIRECTORY\n\
         Everything lives in ~/.daneel. daneel runs without it; a missing\n\
         file just means defaults.\n\
         \n\
         \x20 settings.json|toml     preferences, safe tools (--info settings)\n\
         \x20 models.json|toml       providers and models (--info providers)\n\
         \x20 tools/<name>.js        custom tools (--info tools)\n\
         \x20 strategies/<name>.toml           phases (--info strategies)\n\
         \x20 strategies/tactics/<name>.toml   subtask phases\n\
         \x20 sessions/<id>/         one folder per run: messages.jsonl, tasks.jsonl,\n\
         \x20                        artifacts/task-<n>/<phase>/, and trace.jsonl, an ordered\n\
         \x20                        trace (ts_ms, scope, phase, event {type, ..}:\n\
         \x20                        phase, subtask_start/end, tool_call/result,\n\
         \x20                        transition) for debugging subtasks\n\
         \n\
         FORMATS\n\
         settings and models may each be JSON or TOML (use one file per\n\
         name). Keys are camelCase; snake_case is accepted too, so\n\
         defaultProvider and default_provider mean the same thing.\n\
         \n\
         ENVIRONMENT VARIABLES\n\
         Any string value beginning with $ is replaced by that environment\n\
         variable, anywhere in settings or models:\n\
         \n\
         \x20 \"apiKey\": \"$ANTHROPIC_API_KEY\"\n\
         \n\
         If the variable is unset, loading fails and the error names it.\n\
         Keep secrets in the environment, not in the file.\n\
         \n\
         CHECKING YOUR CONFIG\n\
         Run daneel -p \"hi\". A config problem is reported on stderr before\n\
         any model call, and the exit code is 1.\n",
    )
}
