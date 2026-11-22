//! The event loop and command executor.
//!
//! One thread owns the state: it draws, polls crossterm for input, drains
//! worker results from a channel, and emits animation ticks. `RunTask`,
//! `LoadIfc` and `LoadMurb` commands run on named worker threads that report
//! back as [`Msg`]s, so slow handlers never block the UI.

use std::collections::{BTreeMap, HashMap};
use std::io::Write;
use std::panic::{self, AssertUnwindSafe};
use std::path::Path;
use std::sync::mpsc::{Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::anyhow;
use crossterm::event;
use makit_core::models::CancelToken;
use makit_core::Registry;
use makit_tools::building_model;
use ratatui::backend::Backend;
use ratatui::Terminal;

use crate::app::{self, AppState};
use crate::capture::{self, Mailbox};
use crate::keymap;
use crate::msg::{Cmd, IfcOutcome, Msg};
use crate::node::NodeId;
use crate::signals::Signals;
use crate::task;
use crate::terminal::WORKER_THREAD_PREFIX;
use crate::update::update;
use crate::view::view;

/// Animation frame interval.
pub const TICK_RATE: Duration = Duration::from_millis(80);
/// Longest wait for input before checking worker results and signals.
const POLL_INTERVAL: Duration = Duration::from_millis(50);

/// Why the event loop ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exit {
    /// The user quit.
    Quit,
    /// A termination signal arrived; the caller restores the terminal and
    /// then dies by it.
    Signal(i32),
}

/// What the runtime needs to carry out commands.
pub struct Effects {
    tx: Sender<Msg>,
    /// Present while stdout is captured: workers leave their results here
    /// and the capture's reader thread delivers them in output order.
    /// Without it workers report straight to `tx`.
    mailbox: Option<Mailbox>,
    /// Stop switches of the running tasks.
    cancels: HashMap<u64, CancelToken>,
}

impl Effects {
    /// Workers report to `tx`, through `mailbox` when stdout is captured.
    pub fn new(tx: Sender<Msg>, mailbox: Option<Mailbox>) -> Self {
        Self {
            tx,
            mailbox,
            cancels: HashMap::new(),
        }
    }
}

/// Run until the user quits or a termination signal arrives.
pub fn run_loop<B: Backend>(
    terminal: &mut Terminal<B>,
    state: &mut AppState,
    fx: &mut Effects,
    rx: &Receiver<Msg>,
    signals: &Signals,
) -> anyhow::Result<Exit> {
    let mut last_tick = Instant::now();
    let mut dirty = true;
    loop {
        if dirty {
            terminal.draw(|frame| view(frame, state))?;
            dirty = false;
        }
        if let Some(signal) = signals.pending() {
            return Ok(Exit::Signal(signal));
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
                if dispatch(state, msg, fx) {
                    return Ok(Exit::Quit);
                }
            }
            dirty = true;
        }

        while let Ok(msg) = rx.try_recv() {
            if dispatch(state, msg, fx) {
                return Ok(Exit::Quit);
            }
            dirty = true;
        }

        if state.needs_tick() && last_tick.elapsed() >= TICK_RATE {
            last_tick = Instant::now();
            dispatch(state, Msg::Tick, fx);
            dirty = true;
        }
    }
}

/// Apply `msg` and execute the resulting commands.
/// Returns `true` when the app should exit.
pub fn dispatch(state: &mut AppState, msg: Msg, fx: &mut Effects) -> bool {
    if let Msg::TaskFinished { id, .. } = &msg {
        fx.cancels.remove(id);
    }
    let mut quit = false;
    for cmd in update(state, msg) {
        quit |= execute(cmd, fx);
    }
    quit
}

/// Carry out one command. Returns `true` when the app should exit.
fn execute(cmd: Cmd, fx: &mut Effects) -> bool {
    match cmd {
        Cmd::Quit => {
            // Let tasks that can stop (servers) wind down on the way out.
            for cancel in fx.cancels.values() {
                cancel.cancel();
            }
            return true;
        }
        Cmd::RunTask { id, node, options } => spawn_task(id, node, options, fx),
        Cmd::StopTask { id } => {
            if let Some(cancel) = fx.cancels.get(&id) {
                cancel.cancel();
            }
        }
        Cmd::LoadIfc { path } => spawn_ifc_load(path, fx.tx.clone()),
        Cmd::LoadMurb { path } => spawn_murb_load(path, fx.tx.clone()),
    }
    false
}

/// The user's home directory, for `~` in typed paths.
fn home_dir() -> Option<String> {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .ok()
}

fn spawn_task(id: u64, node: NodeId, options: BTreeMap<String, String>, fx: &mut Effects) {
    let home = home_dir();
    let options: BTreeMap<String, String> = options
        .into_iter()
        .map(|(key, value)| {
            let value = task::expand_tilde(&value, home.as_deref());
            (key, value)
        })
        .collect();
    let resolved = match Registry::global().read() {
        Ok(reg) => task::resolve_task(&reg, &node, &options),
        Err(_) => Err(anyhow!("registry lock poisoned")),
    };
    let mut task = match resolved {
        Ok(task) => task,
        Err(err) => {
            let _ = fx.tx.send(Msg::TaskFinished {
                id,
                result: Err(err.to_string()),
            });
            return;
        }
    };
    let cancel = CancelToken::new();
    task.ctx.cancel = cancel.clone();

    let tx = fx.tx.clone();
    let mailbox = fx.mailbox.clone();
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
            report_finished(id, result, &tx, mailbox.as_ref());
        });
    match spawned {
        Ok(_) => {
            fx.cancels.insert(id, cancel);
        }
        Err(err) => {
            let _ = fx.tx.send(Msg::TaskFinished {
                id,
                result: Err(format!("could not start worker: {err}")),
            });
        }
    }
}

/// Report a task's result from its worker thread.
fn report_finished(
    id: u64,
    result: Result<String, String>,
    tx: &Sender<Msg>,
    mailbox: Option<&Mailbox>,
) {
    match mailbox {
        // stdout is captured: the result goes down the same pipe as the
        // task's output, so it arrives after everything the task printed.
        Some(mailbox) => {
            mailbox.post(id, result);
            capture::announce_done(id);
        }
        None => {
            let _ = std::io::stdout().flush();
            let _ = tx.send(Msg::TaskFinished { id, result });
        }
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

fn spawn_murb_load(path: String, tx: Sender<Msg>) {
    let tx_spawn_err = tx.clone();
    let spawned = thread::Builder::new()
        .name(format!("{WORKER_THREAD_PREFIX}murb"))
        .spawn(move || {
            let path = task::expand_tilde(&path, home_dir().as_deref());
            let results = app::load_murb_results(Path::new(&path));
            let _ = tx.send(Msg::MurbLoaded(results));
        });
    if spawned.is_err() {
        let _ = tx_spawn_err.send(Msg::MurbLoaded(None));
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
            .map(|model| IfcOutcome::Analysis {
                analysis: building_model::analyze_wwr(&model),
                json: json.to_string_lossy().into_owned(),
            })
            .map_err(|e| e.to_string()),
        None => Ok(IfcOutcome::NeedsExtraction),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::InitialData;
    use crate::capture::Mailbox;
    use crate::options::Catalog;
    use crate::tree::TreeNode;
    use makit_core::TaskHandler;
    use ratatui::backend::TestBackend;
    use ratatui::layout::Rect;
    use std::sync::mpsc;
    use std::sync::Arc;

    /// Register `handler` as a uniquely named action and build a state with
    /// that action under the cursor.
    fn state_with_action(
        tag: &str,
        handler: TaskHandler,
        options: &[(&str, bool, Option<&str>)],
    ) -> (String, AppState) {
        let name = format!("tui-test-{tag}-{}", std::process::id());
        {
            let reg = Registry::global();
            let mut reg = reg.write().unwrap();
            let task = reg.register_action(&name, "test", "test", handler);
            for (opt, required, default) in options {
                task.add_option(opt, "Option", "string", *required, *default);
            }
        }
        let state = AppState::new(InitialData {
            tree: vec![TreeNode::new(format!("action:{name}"), "▸ test")],
            catalog: Catalog::from_global(),
            viewport: Rect::new(0, 0, 100, 30),
            ..InitialData::default()
        });
        (name, state)
    }

    fn effects() -> (Effects, mpsc::Receiver<Msg>) {
        let (tx, rx) = mpsc::channel();
        (Effects::new(tx, None), rx)
    }

    fn next(rx: &mpsc::Receiver<Msg>) -> Msg {
        rx.recv_timeout(Duration::from_secs(5))
            .expect("worker result")
    }

    fn example(path: &str) -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/IFC")
            .join(path)
    }

    #[test]
    fn example_ifc_resolves_to_its_json_extraction() {
        match load_ifc(&example("Building-Architecture.ifc")) {
            Ok(IfcOutcome::Analysis { analysis, json }) => {
                assert!(analysis.wall_count > 0);
                assert!(json.ends_with(".json"), "{json}");
            }
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
        let handler: TaskHandler = Arc::new(|ctx| {
            assert_eq!(ctx.get_option("mode", ""), "fast");
            Ok(())
        });
        let (name, mut state) =
            state_with_action("action", handler, &[("mode", false, Some("fast"))]);
        let (mut fx, rx) = effects();
        assert!(!dispatch(&mut state, Msg::Execute, &mut fx));
        assert_eq!(state.tasks.len(), 1);

        let msg = next(&rx);
        dispatch(&mut state, msg, &mut fx);
        assert!(state.tasks.is_empty());
        assert_eq!(state.status, format!("✓ Success: {name}"));
    }

    #[test]
    fn typed_values_reach_the_handler_with_the_home_directory_expanded() {
        let home = std::env::var("HOME").expect("HOME is set");
        let want = format!("{}/models/a.ifc", home.trim_end_matches('/'));
        let handler: TaskHandler = Arc::new(move |ctx| {
            let got = ctx.get_option("path", "");
            anyhow::ensure!(got == want, "handler saw {got:?}, wanted {want:?}");
            Ok(())
        });
        let (name, mut state) = state_with_action("values", handler, &[("path", true, None)]);
        let (mut fx, rx) = effects();

        // Nothing typed yet: the form opens instead of running.
        dispatch(&mut state, Msg::Execute, &mut fx);
        assert!(state.editor.is_some());
        for c in "~/models/a.ifc".chars() {
            dispatch(&mut state, Msg::Edit(crate::msg::EditKey::Char(c)), &mut fx);
        }
        dispatch(&mut state, Msg::Edit(crate::msg::EditKey::Submit), &mut fx);

        let msg = next(&rx);
        dispatch(&mut state, msg, &mut fx);
        assert_eq!(state.status, format!("✓ Success: {name}"));
    }

    #[test]
    fn stop_reaches_the_running_handler() {
        let handler: TaskHandler = Arc::new(|ctx| {
            let deadline = Instant::now() + Duration::from_secs(5);
            while !ctx.cancel.is_cancelled() {
                anyhow::ensure!(Instant::now() < deadline, "never asked to stop");
                thread::sleep(Duration::from_millis(5));
            }
            Ok(())
        });
        let (name, mut state) = state_with_action("stop", handler, &[]);
        let (mut fx, rx) = effects();
        dispatch(&mut state, Msg::Execute, &mut fx);
        dispatch(&mut state, Msg::StopTask, &mut fx);
        assert_eq!(state.status, format!("■ Stopping {name}…"));

        let msg = next(&rx);
        dispatch(&mut state, msg, &mut fx);
        assert_eq!(state.status, format!("■ Stopped: {name}"));
        // The stop switch is forgotten with the task.
        assert!(fx.cancels.is_empty());
    }

    #[test]
    fn quitting_asks_running_tasks_to_stop() {
        let (seen_tx, seen_rx) = mpsc::channel();
        let seen_tx = std::sync::Mutex::new(seen_tx);
        let handler: TaskHandler = Arc::new(move |ctx| {
            let deadline = Instant::now() + Duration::from_secs(5);
            while !ctx.cancel.is_cancelled() && Instant::now() < deadline {
                thread::sleep(Duration::from_millis(5));
            }
            let _ = seen_tx.lock().unwrap().send(ctx.cancel.is_cancelled());
            Ok(())
        });
        let (_, mut state) = state_with_action("quit", handler, &[]);
        let (mut fx, _rx) = effects();
        dispatch(&mut state, Msg::Execute, &mut fx);
        assert!(dispatch(&mut state, Msg::Quit, &mut fx));
        assert!(seen_rx.recv_timeout(Duration::from_secs(5)).unwrap());
    }

    #[test]
    fn with_capture_the_result_waits_for_the_output_stream() {
        let handler: TaskHandler = Arc::new(|_| Ok(()));
        let (name, mut state) = state_with_action("mailbox", handler, &[]);
        let (tx, rx) = mpsc::channel();
        let mailbox = Mailbox::default();
        let mut fx = Effects::new(tx, Some(mailbox.clone()));
        dispatch(&mut state, Msg::Execute, &mut fx);

        let deadline = Instant::now() + Duration::from_secs(5);
        let result = loop {
            if let Some(result) = mailbox.take(1) {
                break result;
            }
            assert!(Instant::now() < deadline, "result never posted");
            thread::sleep(Duration::from_millis(5));
        };
        assert_eq!(result, Ok(format!("Success: {name}")));
        // Not sent directly: the stdout reader delivers it, in order.
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn unknown_task_reports_an_error_without_spawning() {
        let (mut fx, rx) = effects();
        spawn_task(
            7,
            NodeId::Action("definitely-not-registered".into()),
            BTreeMap::new(),
            &mut fx,
        );
        match next(&rx) {
            Msg::TaskFinished {
                id: 7,
                result: Err(e),
            } => assert!(e.contains("unknown action")),
            other => panic!("unexpected {other:?}"),
        }
        assert!(fx.cancels.is_empty());
    }

    #[test]
    fn simulation_results_are_reloaded_from_disk() {
        let results = makit_tools::murb::MurbResults {
            name: "reload".into(),
            monthly_heating_demand: vec![1.0; 12],
            monthly_cooling_demand: vec![0.0; 12],
            monthly_heating_consumption: vec![1.0; 12],
            monthly_cooling_consumption: vec![0.0; 12],
            monthly_total_consumption: vec![1.0; 12],
            monthly_ghg_emissions: vec![0.1; 12],
            tedi_kwh_m2: 42.0,
            teui_kwh_m2: 84.0,
            ghgi_kg_m2: 5.0,
            gfa: 100.0,
            weather_file: "x.epw".into(),
        };
        let dir = std::env::temp_dir().join(format!("makit-tui-murb-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("results.json");
        std::fs::write(&path, serde_json::to_string(&results).unwrap()).unwrap();

        let (mut fx, rx) = effects();
        let load = |file: &str| Cmd::LoadMurb {
            path: dir.join(file).to_string_lossy().into_owned(),
        };
        assert!(!execute(load("results.json"), &mut fx));
        let loaded = next(&rx);
        assert!(!execute(load("missing.json"), &mut fx));
        let missing = next(&rx);
        std::fs::remove_dir_all(&dir).ok();

        assert!(matches!(loaded, Msg::MurbLoaded(Some(r)) if r.tedi_kwh_m2 == 42.0));
        assert!(matches!(missing, Msg::MurbLoaded(None)));
    }

    #[test]
    fn quit_command_stops_dispatch() {
        let mut state = AppState::new(InitialData::default());
        let (mut fx, _rx) = effects();
        assert!(dispatch(&mut state, Msg::Quit, &mut fx));
    }

    #[test]
    fn a_pending_signal_ends_the_loop_before_any_input_is_read() {
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        let mut state = AppState::new(InitialData {
            viewport: Rect::new(0, 0, 80, 24),
            ..InitialData::default()
        });
        let (mut fx, rx) = effects();
        let signals = Signals::already(15);
        let exit = run_loop(&mut terminal, &mut state, &mut fx, &rx, &signals).unwrap();
        assert_eq!(exit, Exit::Signal(15));
    }
}
