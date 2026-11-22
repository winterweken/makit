//! The event loop and command executor.
//!
//! One thread owns the state: it draws, polls crossterm for input, drains
//! worker results from a channel, and emits animation ticks. `RunTask` and
//! `LoadIfc` commands run on named worker threads that report back as
//! [`Msg`]s, so slow handlers never block the UI.

use std::io::Write;
use std::panic::{self, AssertUnwindSafe};
use std::path::Path;
use std::sync::mpsc::{Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::anyhow;
use crossterm::event;
use makit_core::Registry;
use makit_tools::building_model;
use ratatui::backend::Backend;
use ratatui::Terminal;

use crate::app::AppState;
use crate::keymap;
use crate::msg::{Cmd, IfcOutcome, Msg};
use crate::node::NodeId;
use crate::task;
use crate::terminal::WORKER_THREAD_PREFIX;
use crate::update::update;
use crate::view::view;

/// Animation frame interval.
pub const TICK_RATE: Duration = Duration::from_millis(80);
/// Longest wait for input before checking worker results.
const POLL_INTERVAL: Duration = Duration::from_millis(50);

/// Run until the user quits.
pub fn run_loop<B: Backend>(
    terminal: &mut Terminal<B>,
    state: &mut AppState,
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
) -> anyhow::Result<()> {
    let mut last_tick = Instant::now();
    let mut dirty = true;
    loop {
        if dirty {
            terminal.draw(|frame| view(frame, state))?;
            dirty = false;
        }

        let timeout = if state.needs_tick() {
            TICK_RATE
                .saturating_sub(last_tick.elapsed())
                .min(POLL_INTERVAL)
        } else {
            POLL_INTERVAL
        };
        if event::poll(timeout)? {
            let ev = event::read()?;
            if let Some(msg) = keymap::map_event(state, &ev) {
                if dispatch(state, msg, &tx) {
                    return Ok(());
                }
            }
            dirty = true;
        }

        while let Ok(msg) = rx.try_recv() {
            if dispatch(state, msg, &tx) {
                return Ok(());
            }
            dirty = true;
        }

        if state.needs_tick() && last_tick.elapsed() >= TICK_RATE {
            last_tick = Instant::now();
            dispatch(state, Msg::Tick, &tx);
            dirty = true;
        }
    }
}

/// Apply `msg` and execute the resulting commands.
/// Returns `true` when the app should exit.
pub fn dispatch(state: &mut AppState, msg: Msg, tx: &Sender<Msg>) -> bool {
    let mut quit = false;
    for cmd in update(state, msg) {
        match cmd {
            Cmd::Quit => quit = true,
            Cmd::RunTask { id, node } => spawn_task(id, node, tx.clone()),
            Cmd::LoadIfc { path } => spawn_ifc_load(path, tx.clone()),
        }
    }
    quit
}

fn spawn_task(id: u64, node: NodeId, tx: Sender<Msg>) {
    let resolved = match Registry::global().read() {
        Ok(reg) => task::resolve_task(&reg, &node),
        Err(_) => Err(anyhow!("registry lock poisoned")),
    };
    let task = match resolved {
        Ok(task) => task,
        Err(err) => {
            let _ = tx.send(Msg::TaskFinished {
                id,
                result: Err(err.to_string()),
            });
            return;
        }
    };

    let tx_spawn_err = tx.clone();
    let spawned = thread::Builder::new()
        .name(format!("{WORKER_THREAD_PREFIX}task-{id}"))
        .spawn(move || {
            let name = task.name.clone();
            let outcome = panic::catch_unwind(AssertUnwindSafe(|| (task.handler)(&task.ctx)));
            let result = match outcome {
                Ok(Ok(())) => Ok(format!("Success: {name}")),
                Ok(Err(err)) => Err(format!("Error: {name}: {err:#}")),
                Err(_) => Err(format!("Panicked: {name}")),
            };
            let _ = std::io::stdout().flush();
            let _ = tx.send(Msg::TaskFinished { id, result });
        });
    if let Err(err) = spawned {
        let _ = tx_spawn_err.send(Msg::TaskFinished {
            id,
            result: Err(format!("could not start worker: {err}")),
        });
    }
}

fn spawn_ifc_load(path: String, tx: Sender<Msg>) {
    let tx_spawn_err = tx.clone();
    let path_for_err = path.clone();
    let spawned = thread::Builder::new()
        .name(format!("{WORKER_THREAD_PREFIX}ifc"))
        .spawn(move || {
            let result = load_ifc(Path::new(&path));
            let _ = tx.send(Msg::IfcLoaded { path, result });
        });
    if let Err(err) = spawned {
        let _ = tx_spawn_err.send(Msg::IfcLoaded {
            path: path_for_err,
            result: Err(format!("could not start worker: {err}")),
        });
    }
}

/// Find the JSON extraction for an IFC (or JSON) selection and analyse it.
pub fn load_ifc(path: &Path) -> Result<IfcOutcome, String> {
    let has_ext = |ext: &str| {
        path.extension()
            .is_some_and(|e| e.eq_ignore_ascii_case(ext))
    };
    let json_path = if has_ext("ifc") {
        building_model::find_json_for_ifc(path)
    } else if has_ext("json") {
        Some(path.to_path_buf())
    } else {
        None
    };
    match json_path {
        Some(json) => building_model::load_model(&json)
            .map(|model| IfcOutcome::Analysis(building_model::analyze_wwr(&model)))
            .map_err(|e| e.to_string()),
        None => Ok(IfcOutcome::NeedsExtraction),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::InitialData;
    use crate::tree::TreeNode;
    use makit_core::TaskHandler;
    use ratatui::layout::Rect;
    use std::sync::mpsc;
    use std::sync::Arc;

    fn example(path: &str) -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/IFC")
            .join(path)
    }

    #[test]
    fn example_ifc_resolves_to_its_json_extraction() {
        match load_ifc(&example("Building-Architecture.ifc")) {
            Ok(IfcOutcome::Analysis(a)) => assert!(a.wall_count > 0),
            other => panic!("expected analysis, got {other:?}"),
        }
    }

    #[test]
    fn ifc_without_extraction_needs_extraction() {
        let dir = std::env::temp_dir().join(format!("makit-tui-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let ifc = dir.join("lonely.ifc");
        std::fs::write(&ifc, "ISO-10303-21;").unwrap();
        let outcome = load_ifc(&ifc);
        std::fs::remove_dir_all(&dir).ok();
        assert!(matches!(outcome, Ok(IfcOutcome::NeedsExtraction)));
    }

    #[test]
    fn broken_json_is_an_error() {
        let dir = std::env::temp_dir().join(format!("makit-tui-json-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let json = dir.join("broken.json");
        std::fs::write(&json, "{ not json").unwrap();
        let outcome = load_ifc(&json);
        std::fs::remove_dir_all(&dir).ok();
        assert!(outcome.is_err());
    }

    #[test]
    fn executing_runs_the_handler_off_thread_and_reports_back() {
        // Register a uniquely-named action in the global registry.
        let name = format!("tui-test-action-{}", std::process::id());
        {
            let reg = Registry::global();
            let mut reg = reg.write().unwrap();
            let handler: TaskHandler = Arc::new(|ctx| {
                assert_eq!(ctx.get_option("mode", ""), "fast");
                Ok(())
            });
            reg.register_action(&name, "test", "test", handler)
                .add_option("mode", "Mode", "string", false, Some("fast"));
        }
        let mut state = AppState::new(InitialData {
            tree: vec![TreeNode::new(format!("action:{name}"), "▸ test")],
            viewport: Rect::new(0, 0, 100, 30),
            ..InitialData::default()
        });
        let (tx, rx) = mpsc::channel();
        assert!(!dispatch(&mut state, Msg::Execute, &tx));
        assert_eq!(state.tasks.len(), 1);

        let msg = rx
            .recv_timeout(Duration::from_secs(5))
            .expect("task result");
        dispatch(&mut state, msg, &tx);
        assert!(state.tasks.is_empty());
        assert_eq!(state.status, format!("✓ Success: {name}"));
    }

    #[test]
    fn unknown_task_reports_an_error_without_spawning() {
        let (tx, rx) = mpsc::channel();
        spawn_task(7, NodeId::Action("definitely-not-registered".into()), tx);
        match rx.recv_timeout(Duration::from_secs(1)).unwrap() {
            Msg::TaskFinished {
                id: 7,
                result: Err(e),
            } => assert!(e.contains("unknown action")),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn quit_command_stops_dispatch() {
        let mut state = AppState::new(InitialData::default());
        let (tx, _rx) = mpsc::channel();
        assert!(dispatch(&mut state, Msg::Quit, &tx));
    }
}
