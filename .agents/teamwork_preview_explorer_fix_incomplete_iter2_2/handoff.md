# Empty Task Context Investigation

## Observation
1. In `.agents/orchestrator/ITERATION_1_FAILURES.md`, the "Empty Task Context" failure states that `let ctx = makit_core::TaskContext::new();` is passed to the handler completely empty. It mentions that "Options selected in the tree UI (`opt:something`) are not parsed or injected, rendering parametrized tasks inoperable."
2. In `crates/makit-tui/src/app.rs`, line 108 initializes `let ctx = makit_core::TaskContext::new();` and immediately passes it to `(action.handler)(&ctx)` or `(source.handler)(&ctx)` without mutating it.
3. `TaskContext::new()` results in a context where `options` is an empty `HashMap`, `task` is empty, and `category` is empty.
4. Tree nodes in `crates/makit-tui/src/tree_data.rs` have IDs formatted like `action:NAME` or `action:NAME:opt:OPTNAME` (or `source:NAME` and `source:NAME:opt:OPTNAME`).
5. `Msg::Execute(id)` is triggered with the ID of the currently active tree node. It currently only parses `parts[1]` (the action or source name) to look up the task in the registry, completely ignoring any options defined in the registry for that action/source, and ignoring the selected option part (`parts[3]`).

## Logic Chain
1. Tasks and sources registered in `makit_core::Registry` have predefined options with `default` values (e.g., in `murb.rs`, `epw` has no default, `gfa` has no default, but `cop-htg` has `"0.85"`).
2. Because `ctx` is passed completely empty, even options with default values are missing, causing handlers (like `murb-simulate`) to fail validations immediately.
3. To correctly build the `TaskContext`, we must populate `ctx.options` with the default values for all options defined for that specific action or source.
4. Additionally, since the user can execute a specific option node (e.g., `id` is `action:murb-simulate:opt:epw`), we must parse the `opt:OPTNAME` parts from the string. If the ID contains `:opt:<name>`, we should inject that `<name>` into the context to indicate it was explicitly selected (even if it's just an empty string placeholder since the TUI has no text input widget).
5. Setting `ctx.task` and `ctx.category` is also necessary since they remain empty strings otherwise.

## Caveats
1. The TUI lacks text input widgets, so we cannot gather string values (like file paths or numbers) directly from the user in the TUI when they execute an option. We can only inject an empty string or dummy placeholder for the explicitly selected option. Handlers that strictly validate input formats (e.g., requiring a valid file path or float) will still fail gracefully (returning `Err`) rather than panicking due to an empty context.
2. I only provide the patch file for the context mapping; the synchronous freezing of the TUI (item #1 in failures) is a separate structural issue to be resolved by moving execution to a background task.

## Conclusion
The TUI needs to be modified to construct a mutable `TaskContext`, inject default values from the registry definition of the matched action/source, and parse the `:opt:<name>` segment from the tree node ID if present. I have provided a patch file in my directory.

## Verification Method
1. Apply `fix_task_context.patch`.
2. Run `cargo run -p makit -- tui`.
3. Navigate to an action node (e.g., `action:murb-simulate`) and execute it. The error displayed in the status bar should be the task's specific validation error (`Error: --epw is required...`) instead of panicking or acting on an entirely empty context. If the action has defaults, those defaults should be accessible within the handler.
4. Run `cargo test` to ensure it doesn't break existing tests.
