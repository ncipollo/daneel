//! The `usage` topic: running daneel and what it writes.

pub fn render() -> String {
    String::from(
        "RUNNING\n\
         daneel                       interactive TUI (needs a terminal)\n\
         daneel -p \"message\"          one-shot: run one message, print the\n\
         \x20                            reply, exit (exit code 1 on failure)\n\
         daneel -p --mode json \"msg\"  one-shot with newline-delimited JSON\n\
         \x20                            events on stdout instead of plain text\n\
         daneel --info [topic]        this documentation\n\
         \n\
         Both modes need a provider and model first (see --info providers).\n\
         \n\
         HOW A MESSAGE IS HANDLED\n\
         Every message becomes a task and runs through the strategy: an\n\
         ordered list of phases (by default Triage, Discover, Plan, Execute,\n\
         Validate, Deliver), each a separate model call with its own tools.\n\
         Subtasks run through the tactic. The final delivery is the reply.\n\
         A run that never delivers is an error, never an empty reply. See\n\
         --info strategies to change the phases.\n\
         \n\
         JSON EVENTS (--mode json)\n\
         One JSON object per line, each with \"v\" (schema version, 1) and\n\
         \"type\":\n\
         \x20 task         {task_type, description}  the message as a task\n\
         \x20 phase        {scope, name, number, total}  a phase is starting;\n\
         \x20              scope is \"strategy\" or \"subtask N\"\n\
         \x20 tool_call    {name, args}\n\
         \x20 tool_result  {name, output}\n\
         \x20 message      {text}  the final reply\n\
         \x20 done         the run finished\n\
         \x20 error        {message}  the run failed\n\
         Read until \"done\" or \"error\".\n\
         \n\
         FILES DAENEL WRITES\n\
         ~/.daneel/sessions/<id>/   what happened in each run; sessions\n\
         \x20                          older than 7 days are pruned\n\
         ~/.daneel/logs/daneel.log  debug trace; filter with RUST_LOG\n\
         ~/.daneel/strategies/      default strategy and tactic, written\n\
         \x20                          only when missing\n",
    )
}
