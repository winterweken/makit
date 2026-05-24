# Iteration 2 Worker Plan

Please implement the following fixes based on the Explorers' findings from Iteration 1's failure:

1. **TUI Concurrency & Deadlock Fix (`makit-tui/src/app.rs`)**:
   - Add an `Option<std::sync::mpsc::Receiver<Result<String, String>>>` (or similar) to `AppState`.
   - In `Msg::Execute`, do NOT execute `action.handler` synchronously.
   - Look up the action/source in the registry. CLONE the handler (`action.handler.clone()`) and construct the `TaskContext`.
   - **Crucial**: DROP the `RwLockReadGuard` (`reg_guard`) before spawning the thread to avoid deadlocks.
   - Use `std::thread::spawn` to run the cloned handler in the background. Send the result back via the `mpsc::Sender`.
   - Update `state.status` to `"▸ Executing: ..."` immediately.
   - In the `Msg::Tick` match arm, check the `Receiver` via `try_recv()`. If it has a result, update `state.status` to `"Success"` or `"Error: ..."` and clear the receiver.

2. **Empty Task Context Fix (`makit-tui/src/app.rs`)**:
   - Before executing the task in `Msg::Execute`, populate the `TaskContext`.
   - Iterate over `action.options` (or `source.options`) and insert their `.default` values into `ctx.options`.
   - Parse the `id` string (e.g. `action:name:opt:key`) to insert any selected options into the context.

3. **Invalid Test Fix (`makit-core/src/config.rs`)**:
   - Rewrite `test_default_config` to be hermetic and properly assert the default values.
   - Use `Figment::new().merge(Yaml::string(""))` to extract a `Config` without relying on `~/.makit.yaml`.
   - Assert `config.general.editor == "code"`, `config.pyrevit.default_revit_version == "2024"`, etc.

Use `multi_replace_file_content` to carefully apply these changes without relying on `run_command` (which may time out). Write your completion report to `handoff.md`.
