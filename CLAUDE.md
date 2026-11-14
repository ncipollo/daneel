# Daneel

A slow, careful agent built for small, local models.

## Architecture

The code is broken down into layers (see `docs/arch.md`):

- `cli` — The view layer, built with clap. `main` calls the top-level router in this module; each command gets its own file and is called from the router. No actual logic lives in this layer.
- `feature` — Where all domain logic lives. The cli layer calls through to feature.

## After Each Change
Run the following commands after every code change and fix any issues before considering the change complete:

1. `cargo fmt` - Format all code
2. `cargo test` - Run all tests
3. `cargo clippy` - Run linter; fix all warnings and errors before completing the change
4. `(git diff --name-only HEAD; git ls-files --others --exclude-standard) | smell -q -` - Run smell against the changed/new files; fix all reported limit failures before completing the change
5. If the change touches the TUI or anything it depends on (chat, tools, events, config, startup), validate it in a real terminal as described below. Tests alone do not catch layout, key handling or spinner problems.

### Validating the TUI with tmux
The TUI needs a tty, which agent shells lack, so drive it through tmux. Always use a scratch `HOME` so your real `~/.daneel` is untouched.

1. Build and prepare a scratch home. To see the error path, point Ollama at a closed port; to see a working turn, point `baseUrl` at a real model.
   ```bash
   cargo build
   H=$(mktemp -d); mkdir -p $H/.daneel
   echo '{"defaultProvider":"ollama","defaultModel":"m"}' > $H/.daneel/settings.json
   echo '{"providers":{"ollama":{"baseUrl":"http://127.0.0.1:9"}}}' > $H/.daneel/models.json
   ```
2. Start it in a detached session with a fixed size:
   ```bash
   tmux new-session -d -s daneel -x 100 -y 30 "HOME=$H ./target/debug/daneel"
   ```
3. Drive it and read the screen. Wait a moment after each key, since the UI redraws asynchronously:
   ```bash
   tmux send-keys -t daneel 'hello' Enter
   sleep 1; tmux capture-pane -t daneel -p
   ```
   Useful keys: `Enter` submits, `M-Enter` or `C-j` adds a newline, `PageUp`/`PageDown` scroll, `C-c` quits.
4. Check what your change affects: the layout renders, input and newlines work, a slow turn shows the spinner with elapsed seconds, a failed turn shows a short error and returns to `ready`, and the session log under `$H/.daneel/sessions/` and `$H/.daneel/logs/daneel.log` hold what you expect. Try a narrow window too (`tmux resize-window -t daneel -x 30 -y 12`).
5. Clean up: `tmux kill-session -t daneel`. For `-p` mode no tmux is needed: run `HOME=$H ./target/debug/daneel -p "hi"` directly.

Report what you saw. If you could not run the TUI (no tmux, no model), say so rather than claiming it was validated.

### Fixing Clippy Complexity Warnings
When clippy reports `cognitive_complexity`, `too_many_lines`, or `too_many_arguments` warnings, fix them by refactoring — never suppress with `#[allow]`:
- Extract logical sub-steps into well-named helper functions.
- When a file accumulates many functions, reorganize into helper files and structs (following the module conventions below).

### Fixing Smell Warnings
When `smell` reports a limit failure, fix it by refactoring — never suppress by raising the limit in `smell.toml`:
- **Too complex** (`max_complexity`) - break the function up into sub-functions.
- **Too complex from a big switch/match** (`max_complexity`) - convert it into a map of lambdas instead.
- **Too many methods** (`max_methods`) - the type probably has too many responsibilities; enumerate its responsibilities and split each into a helper type/file.
- **Too many declarations** (`max_declarations`) - the file itself is too big, usually from mixing responsibilities and tests; split it into separate files along responsibility lines.
- **Too many lines** (`max_lines`) - same fix as too many declarations.
- **Too many/long comments** (`max_comments`, `max_comment_lines`) - delete comments that restate the code; keep only WHY comments, and keep those short.

## Dependencies
Always use exact versions for dependencies in `Cargo.toml` (e.g., `"4.5.60"` not `"4"`). Check `Cargo.lock` for the resolved version when pinning.

## Module Conventions
Never use `mod.rs`. Always use the modern Rust style: create a top-level file (e.g., `foo.rs`) as the module root, and a matching folder (`foo/`) for any submodules.

## Imports
Always use `use` imports rather than full crate paths at call sites. For example, prefer `use crate::feature::greeting;` + `greeting::message()` over `crate::feature::greeting::message()`.
