//! The `tools` topic: built-in tools and custom JavaScript tools.

pub fn render() -> String {
    String::from(
        "BUILT-IN TOOLS\n\
         read_file, list_files, find_files, grep_files\n\
         \x20              every phase (read-only; find_files matches names with\n\
         \x20              * and ?, grep_files searches text, not a regex)\n\
         write_file, edit_file, invoke_shell\n\
         \x20              only in phases with can_write_working_directory = true\n\
         \x20              (see --info strategies); reviewed if safe tools is on.\n\
         \x20              write_file and edit_file refuse paths outside the\n\
         \x20              working directory (absolute, .. or symlink)\n\
         Phases also get advance_phase, skip_to_phase, retry_phase,\n\
         create_subtask and complete_task to move through the strategy.\n\
         \n\
         CUSTOM TOOLS\n\
         Put a JavaScript file at ~/.daneel/tools/<name>.js. The file name\n\
         (letters, digits, _ and -) is the tool name and may not match a\n\
         built-in. It defines:\n\
         \n\
         \x20 parameters()   required: JSON schema of the arguments\n\
         \x20 run(args)      required: returns the result (never undefined)\n\
         \x20 description()  optional: what the tool is for\n\
         \x20 output()       optional: schema; a result that does not match\n\
         \x20                is rejected\n\
         \n\
         Inside a script, shell(command) runs a command through the shell\n\
         tool and returns its output, and context.workingDir is the\n\
         project directory. There is no other file or network access.\n\
         Loops are capped, so a runaway script is stopped.\n\
         \n\
         Custom tools exist only in phases that may write, since they rely\n\
         on the shell tool. Their shell() calls get the same safe-tools\n\
         review. A script with an error is skipped and logged as a warning\n\
         in ~/.daneel/logs/daneel.log; the others still load.\n\
         \n\
         EXAMPLE (~/.daneel/tools/shout.js)\n\
         function description() { return \"Upper-cases text.\"; }\n\
         function parameters() {\n\
         \x20 return { type: \"object\",\n\
         \x20          properties: { text: { type: \"string\" } },\n\
         \x20          required: [\"text\"] };\n\
         }\n\
         function output() { return { type: \"string\" }; }\n\
         function run(args) { return args.text.toUpperCase(); }\n",
    )
}
