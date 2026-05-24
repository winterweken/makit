# Handoff Report

## Observation
1. In `crates/makit-cli/src/commands/analyze.rs`, the code holds a read lock on the global registry while executing the handler:
```rust
    let reg = reg.read().unwrap();
    // ...
    if let Some(action) = reg.actions.get(analysis_type) {
        (action.handler)(&ctx)?;
    }
```
2. In `crates/makit-tui/src/app.rs`, the `Msg::Execute` logic mirrors this exact pattern, acquiring `reg_guard` via `reg.read()` and calling `(action.handler)(&ctx)` inside the block while the lock is held.
3. In `crates/makit-tui/src/app.rs`, the handler execution occurs synchronously inside the `update(state: &mut AppState, msg: Msg)` function, which is the main TUI event loop tick.
4. The worker explicitly acknowledged this limitation in their caveats: `"Msg::Execute(id) is implemented to run tasks linearly within the TUI event loop... (TUI might freeze briefly during sync work)"`.

## Logic Chain
1. **Deadlock Risk (Lock Holding)**: Holding an `RwLockReadGuard` on the global registry for the entire duration of a potentially long-running handler is a severe anti-pattern. If the handler (or any other thread) tries to acquire a write lock on the registry, it will instantly deadlock. Even for reads, long-held read locks can starve writers or trigger deadlocks on platforms that queue readers behind pending writers.
2. **Concurrency Bug (TUI Blocking)**: Because the `(action.handler)(&ctx)` call is synchronous and executed inside `update()`, the TUI event loop is completely blocked for the duration of the task. The UI will freeze, animations (like `Msg::Tick`) will halt, and the application will become unresponsive to user input. This is exactly the "TUI blockings" concurrency bug specified in the instructions to identify.

## Caveats
- Since `run_command` timed out during dynamic testing, this analysis was performed purely via static adversarial code review. However, the blocking behavior and lock-holding are undeniable structural flaws in the implementation.
- To resolve the lock holding, the `TaskHandler` (`Arc<dyn Fn...>`) should be cloned and the lock dropped *before* invocation.
- To resolve the TUI blocking, long-running tasks should be spawned on a separate thread (e.g., `std::thread::spawn`), potentially updating a shared `Arc<Mutex<String>>` in `AppState` so `view()` can reflect real-time status.

## Conclusion
**VERDICT: VETO**

The implementation introduces significant concurrency bugs. The global registry read lock is dangerously held during arbitrary handler execution, risking deadlocks. Furthermore, the synchronous execution within the TUI's `update` function blocks the event loop, freezing the application—a flaw explicitly targeted by the prompt.

## Verification Method
1. Inspect `crates/makit-cli/src/commands/analyze.rs` and observe `(action.handler)(&ctx)` executing while `reg.read().unwrap()` is in scope.
2. Inspect `crates/makit-tui/src/app.rs` lines ~106-141 and observe the same lock-holding pattern in `Msg::Execute`.
3. Notice that `update()` in `app.rs` does not offload work, inherently blocking the TUI thread.
