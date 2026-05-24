# Handoff Report

## 1. Observation
- **Concurrency/TUI Blocking (`makit-tui/src/app.rs:103`)**: `Msg::Execute` synchronously invokes `(action.handler)(&ctx)` on the TUI's main thread. This completely freezes the application, blocking `rsille` from processing terminal inputs and its periodic `on_tick` loop.
- **Ghost UX State (`makit-tui/src/app.rs:104`)**: The status is set to `"▸ Executing: ..."` right before the synchronous block, and set to `"Success"` or `"Error"` right after. Because the application never yields to the render loop during the block, the user never sees the "Executing" state.
- **Empty Task Context (`makit-tui/src/app.rs:108`)**: A raw `TaskContext::new()` is passed into the handler. Its `options` hashmap is entirely empty. Even if the user selected the action, the default options defined in `makit_core::models::TaskOption` are never parsed or injected.
- **Invalid Tests (`makit-core/src/config.rs:80`)**: `test_default_config` calls `Config::load(None)` and discards the result. Because there is no `assert!` or `.unwrap()`, the test silently passes even if configuration loading fails completely.

## 2. Logic Chain
1. **Concurrency and Ghost UX**: The TUI is driven by an Elm architecture using `rsille::tui::App::run_inline`. The app already has an `on_tick(Duration::from_millis(80), || Msg::Tick)` event that fires 12 times a second. We can leverage this to implement background processing without fundamentally rewriting the event loop.
2. If we add an `Option<std::sync::mpsc::Receiver<Result<String, String>>>` to `AppState` and spawn a background thread (`std::thread::spawn`) inside `Msg::Execute`, the main thread can return immediately. This allows the renderer to display the "Executing" text and continue processing ticks and animations.
3. On every `Msg::Tick`, we check the receiver using `try_recv()`. Once the background thread completes and sends the result, we update the status text to "Success" or "Error" and refresh any cached state (e.g., `murb_results`).
4. **Empty Context**: In `Msg::Execute`, before spawning the thread, we must look up the target `Action` or `Source` in the global registry and populate `ctx.options` with the `default` values defined in their respective `TaskOption`s. Without this, parametrized handlers receive an empty context and crash or do nothing.
5. **Robust Tests**: To fix `test_default_config`, we must temporarily mock the `HOME` environment variable to isolate the test from the user's actual `~/.makit.yaml`, then `.unwrap()` the result and assert that the parsed structure matches expected defaults (e.g., `config.general.editor == "code"`).

## 3. Caveats
- Passing the handler to the background thread requires `action.handler.clone()`. This is safe because `TaskHandler` is defined as `Arc<dyn Fn(&TaskContext) -> anyhow::Result<()> + Send + Sync>`.
- The `mpsc` strategy is slightly verbose for Elm architectures (which usually prefer a `Cmd` framework for side effects), but since we are relying on `run_inline` without a robust `Cmd` mechanism from `rsille`, polling on `Msg::Tick` is the most native, thread-safe, and dependency-light solution.
- For task context options, we only inject the *default* values. The current tree UI does not support text input to alter these defaults.

## 4. Conclusion
The blocking TUI and missing parameters must be resolved by:
1. Augmenting `AppState` with an `std::sync::mpsc::Receiver`.
2. Using `std::thread::spawn` in `Msg::Execute` to offload the `action.handler` execution and report back via an `mpsc::Sender`.
3. Updating `Msg::Tick` to poll the receiver and react to completion.
4. Parsing `action.options` inside `Msg::Execute` and populating `ctx.options` with `default` configurations before execution.
5. Re-writing `test_default_config` to explicitly `assert_eq!` the default fields (e.g. `2024`, `code`, `info`) after setting a fake `HOME` var to ensure test isolation.

## 5. Verification Method
1. **Concurrency and UX**: Run `cargo run -p makit -- tui` and press `Enter` on `action:murb-simulate`. The TUI logo should continue spinning smoothly, and the status bar should say `"▸ Executing: action:murb-simulate"`. Once the simulation finishes, it should update to `"Success"`.
2. **Context Passing**: Attempting to execute `action:murb-simulate` or `action:wall-orientations` should no longer fail due to missing options or arguments.
3. **Tests**: Run `cargo test -p makit-core`. The `test_default_config` should pass, proving the config defaults are properly loaded and verified.
