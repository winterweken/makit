## Iteration 1 Failures

The Worker's implementation failed the Gate verification due to the following reasons:

1. **Concurrency/TUI Blocking (`makit-tui/src/app.rs`)**: `Msg::Execute` runs `action.handler` synchronously on the TUI main thread. For operations like the Revit bridge or Python subprocesses, this completely freezes the TUI (dropping inputs and ticks). A background task/async execution model is required.
2. **Empty Task Context (`makit-tui/src/app.rs`)**: `let ctx = makit_core::TaskContext::new();` is passed to the handler completely empty. Options selected in the tree UI (`opt:something`) are not parsed or injected, rendering parametrized tasks inoperable.
3. **Ghost UX State (`makit-tui/src/app.rs`)**: The "Executing" status text is set and immediately overwritten by the result in the same execution frame because it's synchronous, so the user never sees it.
4. **Invalid Tests (`makit-core/src/config.rs`)**: `test_default_config` is a dummy/facade test that discards its `Result` without assertions, meaning it silently passes even if loading fails. This must be a robust test with proper assertions.

Please analyze these failures and propose a comprehensive strategy to fix them correctly.
