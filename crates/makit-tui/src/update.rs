//! `update(state, msg) -> commands` — the only place state changes.
//!
//! Pure: no I/O, no registry access, no threads. Side effects are returned
//! as [`Cmd`]s for the runtime to perform; their results come back as
//! [`Msg`]s.

use std::path::Path;

use ratatui::layout::Rect;

use crate::app::{AppState, Editor, Focus, RunningTask, ValueSource};
use crate::layout;
use crate::msg::{Cmd, EditKey, IfcOutcome, Msg, Pane};
use crate::node::NodeId;
use crate::options::OptionSpec;
use makit_tools::building_model;

/// Logo rotation per tick (radians).
const LOGO_STEP: f64 = 0.04;
/// Output lines scrolled per mouse-wheel notch.
const OUTPUT_WHEEL_LINES: isize = 3;

/// Apply `msg` to `state`, returning any side effects to perform.
///
/// The option editor and the help overlay are modal: while one is open it
/// takes the keys, and only messages from the outside world (worker
/// results, resize, ticks) reach the rest of the app.
pub fn update(state: &mut AppState, msg: Msg) -> Vec<Cmd> {
    let cmds = if state.editor.is_some() {
        match msg {
            Msg::Edit(key) => edit(state, key),
            Msg::Quit => quit(state),
            msg if is_background(&msg) => apply(state, msg),
            _ => Vec::new(),
        }
    } else if state.show_help {
        match msg {
            Msg::Up => scroll_help(state, -1),
            Msg::Down => scroll_help(state, 1),
            Msg::PageUp => scroll_help(state, -help_page(state)),
            Msg::PageDown => scroll_help(state, help_page(state)),
            Msg::Home => scroll_help(state, isize::MIN),
            Msg::End => scroll_help(state, isize::MAX),
            Msg::ToggleHelp | Msg::Escape => close_help(state),
            Msg::Quit => quit(state),
            msg if is_background(&msg) => apply(state, msg),
            _ => Vec::new(),
        }
    } else {
        apply(state, msg)
    };
    normalize_focus(state);
    sync_scroll(state);
    cmds
}

/// Messages from the outside world rather than from a key or click.
fn is_background(msg: &Msg) -> bool {
    matches!(
        msg,
        Msg::Resize { .. }
            | Msg::Tick
            | Msg::Output(_)
            | Msg::TaskFinished { .. }
            | Msg::IfcLoaded { .. }
            | Msg::MurbLoaded(_)
    )
}

fn apply(state: &mut AppState, msg: Msg) -> Vec<Cmd> {
    match msg {
        Msg::Up => navigate(state, -1),
        Msg::Down => navigate(state, 1),
        Msg::PageUp => {
            let page = page_size(state);
            navigate(state, -page)
        }
        Msg::PageDown => {
            let page = page_size(state);
            navigate(state, page)
        }
        Msg::Home => jump(state, true),
        Msg::End => jump(state, false),
        Msg::Expand => {
            if state.focus == Focus::Tree {
                state.tree.expand();
            }
            Vec::new()
        }
        Msg::Collapse => {
            if state.focus == Focus::Tree {
                state.tree.collapse_or_parent();
            }
            Vec::new()
        }
        Msg::Activate => activate(state),
        Msg::Execute => execute(state),
        Msg::StopTask => stop(state),
        // No editor is open (checked in `update`).
        Msg::Edit(_) => Vec::new(),
        Msg::FocusNext => cycle_focus(state, 1),
        Msg::FocusPrev => cycle_focus(state, -1),
        Msg::ToggleOutput => {
            state.show_output = !state.show_output;
            state.output_auto_opened = true;
            Vec::new()
        }
        Msg::ClearOutput => {
            if state.focus == Focus::Output {
                state.log.clear();
                state.log_scroll = 0;
            }
            Vec::new()
        }
        Msg::ToggleHelp => {
            state.show_help = true;
            state.help_scroll = 0;
            Vec::new()
        }
        Msg::Escape | Msg::Quit => quit(state),
        Msg::Scroll { pane, delta } => scroll(state, pane, delta),
        Msg::Click { pane, row } => click(state, pane, row),
        Msg::Resize { width, height } => {
            state.viewport = Rect::new(0, 0, width, height);
            scroll_help(state, 0);
            Vec::new()
        }
        Msg::Tick => {
            state.logo_angle = (state.logo_angle + LOGO_STEP) % std::f64::consts::TAU;
            state.frame = state.frame.wrapping_add(1);
            Vec::new()
        }
        Msg::Output(line) => {
            state.push_log(line);
            if !state.show_output && !state.output_auto_opened {
                state.show_output = true;
                state.output_auto_opened = true;
            }
            Vec::new()
        }
        Msg::TaskFinished { id, result } => task_finished(state, id, result),
        Msg::IfcLoaded { path, result } => {
            ifc_loaded(state, path, result);
            Vec::new()
        }
        Msg::MurbLoaded(results) => {
            // An unreadable file leaves the results already shown in place.
            if let Some(results) = results {
                state.murb_results = Some(results);
            }
            Vec::new()
        }
    }
}

fn close_help(state: &mut AppState) -> Vec<Cmd> {
    state.show_help = false;
    state.help_scroll = 0;
    Vec::new()
}

/// Rows a page key moves the help by.
fn help_page(state: &AppState) -> isize {
    let rows = layout::bordered_inner(layout::help_rect(state.viewport)).height;
    rows.saturating_sub(1).max(1) as isize
}

fn scroll_help(state: &mut AppState, delta: isize) -> Vec<Cmd> {
    let max = usize::from(layout::help_max_scroll(state.viewport));
    let target = usize::from(state.help_scroll)
        .saturating_add_signed(delta)
        .min(max);
    state.help_scroll = target as u16;
    Vec::new()
}

fn quit(state: &mut AppState) -> Vec<Cmd> {
    state.should_quit = true;
    vec![Cmd::Quit]
}

/// Move within the focused pane by `delta` rows.
fn navigate(state: &mut AppState, delta: isize) -> Vec<Cmd> {
    match state.focus {
        Focus::Tree => {
            state.tree.move_by(delta);
        }
        Focus::IfcPicker => {
            let last = state.ifc.files.len().saturating_sub(1);
            state.ifc.selected = state.ifc.selected.saturating_add_signed(delta).min(last);
        }
        // Up (negative delta) scrolls back through older lines.
        Focus::Output => scroll_log(state, -delta),
    }
    Vec::new()
}

fn jump(state: &mut AppState, to_top: bool) -> Vec<Cmd> {
    match state.focus {
        Focus::Tree => {
            if to_top {
                state.tree.first();
            } else {
                state.tree.last();
            }
        }
        Focus::IfcPicker => {
            state.ifc.selected = if to_top {
                0
            } else {
                state.ifc.files.len().saturating_sub(1)
            };
        }
        Focus::Output => {
            state.log_scroll = if to_top { max_log_scroll(state) } else { 0 };
        }
    }
    Vec::new()
}

fn page_size(state: &AppState) -> isize {
    let l = layout::compute(state.viewport, state);
    let rows = match state.focus {
        Focus::Tree => l.tree_inner.height,
        Focus::IfcPicker => l.ifc.and_then(|i| i.list_inner).map_or(1, |r| r.height),
        Focus::Output => l.output_inner.map_or(1, |r| r.height),
    };
    rows.saturating_sub(1).max(1) as isize
}

fn activate(state: &mut AppState) -> Vec<Cmd> {
    match state.focus {
        Focus::Tree => {
            let option = state.active_node().and_then(|node| {
                let opt = node.opt_name()?.to_string();
                Some((node.task_node()?, opt))
            });
            if let Some((task, opt)) = option {
                return edit_option(state, task, opt);
            }
            if let Some(node) = state.active_node() {
                state.status = format!("▸ Opened: {}", node.display_name());
                state.opened = Some(node);
            }
            state.tree.toggle();
            Vec::new()
        }
        Focus::IfcPicker => load_ifc(state, state.ifc.selected),
        Focus::Output => Vec::new(),
    }
}

/// `task --opt`, for status messages.
fn option_label(task: &NodeId, opt: &str) -> String {
    format!("{} --{opt}", task.display_name())
}

/// Enter on an option row: flip a flag, or open the form on its value.
fn edit_option(state: &mut AppState, task: NodeId, opt: String) -> Vec<Cmd> {
    let label = option_label(&task, &opt);
    let current = state.option_value(&task, &opt).map(|(value, _)| value);
    let is_flag = state
        .catalog
        .spec(&task, &opt)
        .is_some_and(OptionSpec::is_flag);
    if is_flag {
        let value = if current.as_deref() == Some("true") {
            "false"
        } else {
            "true"
        };
        state
            .values
            .entry(task)
            .or_default()
            .insert(opt, value.to_string());
        state.status = format!("✎ Set {label} = {value}");
        return Vec::new();
    }
    state.status = format!("✎ Editing {label} — ⏎ save · Esc cancel");
    state.editor = Some(Editor::new(
        task,
        opt,
        current.as_deref().unwrap_or(""),
        false,
    ));
    Vec::new()
}

/// A key press in the option form.
fn edit(state: &mut AppState, key: EditKey) -> Vec<Cmd> {
    let Some(editor) = state.editor.as_mut() else {
        return Vec::new();
    };
    match key {
        EditKey::Char(c) => editor.insert(c),
        EditKey::Backspace => editor.backspace(),
        EditKey::Delete => editor.delete(),
        EditKey::Left => editor.move_by(-1),
        EditKey::Right => editor.move_by(1),
        EditKey::Home => editor.home(),
        EditKey::End => editor.end(),
        EditKey::Clear => editor.clear(),
        EditKey::Submit => return submit(state),
        EditKey::Cancel => {
            state.status = if editor.run_after {
                format!("Cancelled — {} not run", editor.task.display_name())
            } else {
                format!(
                    "Cancelled — {} unchanged",
                    option_label(&editor.task, &editor.opt)
                )
            };
            state.editor = None;
        }
    }
    Vec::new()
}

/// Accept the form's value; run the task if that is what it was for.
fn submit(state: &mut AppState) -> Vec<Cmd> {
    let Some(editor) = state.editor.take() else {
        return Vec::new();
    };
    let Editor {
        task,
        opt,
        buffer,
        run_after,
        ..
    } = editor;
    let label = option_label(&task, &opt);
    let value = buffer.trim();

    if value.is_empty() {
        if let Some(values) = state.values.get_mut(&task) {
            values.remove(&opt);
        }
        state.status = match state.option_value(&task, &opt) {
            _ if run_after => format!("✗ {label} is required — not run"),
            Some((value, ValueSource::Default)) => {
                format!("✎ Cleared {label} — back to the default, {value}")
            }
            Some((value, _)) => format!("✎ Cleared {label} — using {value}"),
            None => format!("✎ Cleared {label}"),
        };
        return Vec::new();
    }

    // Accepting a default or picked value as it stands is not an override.
    let inherited = matches!(
        state.option_value(&task, &opt),
        Some((current, source)) if source != ValueSource::User && current == value
    );
    if inherited {
        state.status = format!("✎ {label} = {value} (unchanged)");
    } else {
        state
            .values
            .entry(task.clone())
            .or_default()
            .insert(opt, value.to_string());
        state.status = format!("✎ Set {label} = {value}");
    }
    if run_after {
        run_or_prompt(state, task)
    } else {
        Vec::new()
    }
}

fn execute(state: &mut AppState) -> Vec<Cmd> {
    let Some(node) = state.active_node() else {
        return Vec::new();
    };
    let Some(task) = node.task_node() else {
        state.status = format!(
            "✗ '{}' is not executable — select a source or action",
            node.display_name()
        );
        return Vec::new();
    };
    run_or_prompt(state, task)
}

/// Run `task` with its current option values, or ask for the first
/// required option that has none.
fn run_or_prompt(state: &mut AppState, task: NodeId) -> Vec<Cmd> {
    let name = task.display_name().to_string();
    if let Some(opt) = state.missing_required(&task).into_iter().next() {
        state.status = format!("✎ {name} needs --{opt} — type a value · ⏎ run · Esc cancel");
        state.editor = Some(Editor::new(task, opt, "", true));
        return Vec::new();
    }
    let options = state.resolved_options(&task);
    let id = state.next_task_id;
    state.next_task_id += 1;
    let mut running = RunningTask::new(task.clone());
    running.results = task
        .results_option()
        .and_then(|opt| options.get(opt).cloned());
    state.tasks.insert(id, running);
    state.status = format!("▸ Executing: {name}");
    let given: String = options
        .iter()
        .map(|(key, value)| format!(" --{key} {value}"))
        .collect();
    state.push_log(format!("── ▸ Executing {name}{given}"));
    vec![Cmd::RunTask {
        id,
        node: task,
        options,
    }]
}

/// Ask the focused task — or the only one running — to stop.
fn stop(state: &mut AppState) -> Vec<Cmd> {
    if state.tasks.is_empty() {
        state.status = "Nothing is running".to_string();
        return Vec::new();
    }
    let focused = state.active_node().and_then(|node| node.task_node());
    let mut ids: Vec<u64> = state
        .tasks
        .iter()
        .filter(|(_, task)| Some(&task.node) == focused.as_ref())
        .map(|(id, _)| *id)
        .collect();
    if ids.is_empty() {
        if state.tasks.len() > 1 {
            state.status = format!(
                "{} tasks are running — focus one in the Explorer, then x",
                state.tasks.len()
            );
            return Vec::new();
        }
        ids.extend(state.tasks.keys());
    }
    let mut cmds = Vec::new();
    for id in ids {
        let Some(task) = state.tasks.get_mut(&id) else {
            continue;
        };
        if !task.stopping {
            task.stopping = true;
            cmds.push(Cmd::StopTask { id });
        }
        state.status = format!("■ Stopping {}…", task.name);
    }
    cmds
}

fn task_finished(state: &mut AppState, id: u64, result: Result<String, String>) -> Vec<Cmd> {
    let task = state.tasks.remove(&id);
    let stopped = task.as_ref().filter(|task| task.stopping);
    let mut cmds = Vec::new();
    state.status = match (&result, stopped) {
        (Ok(_), Some(task)) => format!("■ Stopped: {}", task.name),
        (Ok(msg), None) => format!("✓ {msg}"),
        (Err(err), _) => format!("✗ {err}"),
    };
    if result.is_ok() && stopped.is_none() {
        if let Some(path) = task.and_then(|task| task.results) {
            cmds.push(Cmd::LoadMurb { path });
        }
    }
    let line = format!("── {}", state.status);
    state.push_log(line);
    cmds
}

fn load_ifc(state: &mut AppState, index: usize) -> Vec<Cmd> {
    let Some(path) = state.ifc.files.get(index).cloned() else {
        return Vec::new();
    };
    state.status = format!("◌ Loading {}…", file_name(&path));
    state.ifc.loading = Some(path.clone());
    vec![Cmd::LoadIfc { path }]
}

fn ifc_loaded(state: &mut AppState, path: String, result: Result<IfcOutcome, String>) {
    // Ignore results for a selection that has since been superseded.
    if state.ifc.loading.as_deref() != Some(path.as_str()) {
        return;
    }
    state.ifc.loading = None;
    state.ifc.analysis = None;
    state.ifc.json = None;
    state.ifc.error = None;
    match result {
        Ok(IfcOutcome::Analysis { analysis, json }) => {
            state.status = format!(
                "◆ {} walls, WWR {:.1}% — {}",
                analysis.wall_count, analysis.overall_wwr, path
            );
            state.ifc.analysis = Some(analysis);
            state.ifc.json = Some(json);
        }
        Ok(IfcOutcome::NeedsExtraction) => {
            let json = building_model::extraction_path(Path::new(&path));
            state.status = format!("◆ {path} is not extracted yet (no {})", json.display());
        }
        Err(err) => {
            state.status = format!("✗ Error loading {path}: {err}");
            state.ifc.error = Some(err);
        }
    }
    state.ifc.loaded = Some(path);
}

fn cycle_focus(state: &mut AppState, step: isize) -> Vec<Cmd> {
    let order = state.focus_order();
    let current = order.iter().position(|f| *f == state.focus).unwrap_or(0);
    let next = (current as isize + step).rem_euclid(order.len() as isize) as usize;
    state.focus = order[next];
    Vec::new()
}

fn scroll(state: &mut AppState, pane: Pane, delta: isize) -> Vec<Cmd> {
    match pane {
        Pane::Tree => {
            state.tree.move_by(delta);
        }
        Pane::IfcPicker => {
            let last = state.ifc.files.len().saturating_sub(1);
            state.ifc.selected = state.ifc.selected.saturating_add_signed(delta).min(last);
        }
        Pane::Output => scroll_log(state, -delta * OUTPUT_WHEEL_LINES),
        Pane::Detail => {}
    }
    Vec::new()
}

fn click(state: &mut AppState, pane: Pane, row: Option<usize>) -> Vec<Cmd> {
    match pane {
        Pane::Tree => {
            state.focus = Focus::Tree;
            match row {
                Some(r) if r == state.tree.selected() => activate(state),
                Some(r) => {
                    state.tree.select(r);
                    Vec::new()
                }
                None => Vec::new(),
            }
        }
        Pane::IfcPicker => {
            state.focus = Focus::IfcPicker;
            match row {
                Some(r) => {
                    state.ifc.selected = r.min(state.ifc.files.len().saturating_sub(1));
                    load_ifc(state, state.ifc.selected)
                }
                None => Vec::new(),
            }
        }
        Pane::Output => {
            state.focus = Focus::Output;
            Vec::new()
        }
        Pane::Detail => Vec::new(),
    }
}

/// Keep each list's selection inside the rows it has room to draw.
///
/// This runs after every message, because many of them change that room
/// without touching a list: a resize, the Output pane opening, another
/// node taking focus. The view lets ratatui scroll a list to its selection
/// and the mouse hit-tests read these offsets, so a stale offset makes a
/// click land on a different row from the one drawn under it.
fn sync_scroll(state: &mut AppState) {
    let layout = layout::compute(state.viewport, state);
    state
        .tree
        .scroll_to_selected(usize::from(layout.tree_inner.height));

    let rows = layout
        .ifc
        .and_then(|ifc| ifc.list_inner)
        .map_or(0, |inner| usize::from(inner.height));
    if rows == 0 {
        // The picker is not drawn; its offset is settled when it returns.
        return;
    }
    let ifc = &mut state.ifc;
    if ifc.selected < ifc.offset {
        ifc.offset = ifc.selected;
    } else if ifc.selected >= ifc.offset + rows {
        ifc.offset = ifc.selected + 1 - rows;
    }
    ifc.offset = ifc.offset.min(ifc.files.len().saturating_sub(rows));
}

fn max_log_scroll(state: &AppState) -> usize {
    let rows = layout::compute(state.viewport, state)
        .output_inner
        .map_or(0, |r| usize::from(r.height));
    state.log.len().saturating_sub(rows)
}

fn scroll_log(state: &mut AppState, back_by: isize) {
    let max = max_log_scroll(state);
    state.log_scroll = state.log_scroll.saturating_add_signed(back_by).min(max);
}

fn normalize_focus(state: &mut AppState) {
    if !state.focus_order().contains(&state.focus) {
        state.focus = Focus::Tree;
    }
}

fn file_name(path: &str) -> &str {
    Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{InitialData, ValueSource};
    use crate::msg::EditKey;
    use crate::node::NodeId;
    use crate::options::{Catalog, OptionSpec};
    use crate::tree::TreeNode;
    use makit_tools::building_model;
    use std::collections::BTreeMap;

    /// Rows: 0 sources, 1 source:ifc, 2 source:revit, 3 actions, 4 cat:energy
    fn state() -> AppState {
        let spec = |name: &str, kind: &str, required: bool, default: Option<&str>| OptionSpec {
            name: name.into(),
            description: format!("{name} description"),
            kind: kind.into(),
            required,
            default: default.map(str::to_string),
        };
        let mut catalog = Catalog::default();
        catalog.insert(
            NodeId::Source("ifc".into()),
            vec![spec("file", "string", true, None)],
        );
        catalog.insert(NodeId::Source("revit".into()), Vec::new());
        catalog.insert(
            NodeId::Action("murb-simulate".into()),
            vec![
                spec("epw", "string", true, None),
                spec("cop", "float", false, Some("0.85")),
                spec("verbose", "bool", false, None),
                spec("output", "string", false, Some("murb_results.json")),
            ],
        );
        AppState::new(InitialData {
            tree: vec![
                TreeNode::new("sources", "Sources")
                    .child(
                        TreeNode::new("source:ifc", "◆ ifc")
                            .child(TreeNode::new("source:ifc:opt:file", "--file")),
                    )
                    .child(TreeNode::new("source:revit", "◆ revit")),
                TreeNode::new("actions", "Actions").child(
                    TreeNode::new("cat:energy", "⊞ energy").child(
                        TreeNode::new("action:murb-simulate", "▸ murb")
                            .child(TreeNode::new("action:murb-simulate:opt:epw", "--epw"))
                            .child(TreeNode::new("action:murb-simulate:opt:cop", "--cop"))
                            .child(TreeNode::new(
                                "action:murb-simulate:opt:verbose",
                                "--verbose",
                            )),
                    ),
                ),
            ],
            ifc_files: vec!["examples/IFC/a.ifc".into(), "examples/IFC/b.ifc".into()],
            murb_results: None,
            catalog,
            viewport: Rect::new(0, 0, 120, 40),
        })
    }

    fn murb() -> NodeId {
        NodeId::Action("murb-simulate".into())
    }

    /// Expand the energy category and the murb action, then select `id`.
    fn select(s: &mut AppState, id: &str) {
        for parent in ["cat:energy", "action:murb-simulate"] {
            let row = s.tree.rows().iter().position(|r| r.id == parent).unwrap();
            s.tree.select(row);
            s.tree.expand();
        }
        let row = s
            .tree
            .rows()
            .iter()
            .position(|r| r.id == id)
            .unwrap_or_else(|| panic!("no row {id}"));
        s.tree.select(row);
    }

    fn type_text(s: &mut AppState, text: &str) {
        for c in text.chars() {
            update(s, Msg::Edit(EditKey::Char(c)));
        }
    }

    fn opts(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    fn sample_analysis() -> building_model::WwrAnalysis {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../examples/IFC/building-model.json"
        );
        let model = building_model::load_model(Path::new(path)).expect("example model");
        building_model::analyze_wwr(&model)
    }

    #[test]
    fn arrow_keys_move_the_tree_cursor() {
        let mut s = state();
        update(&mut s, Msg::Down);
        update(&mut s, Msg::Down);
        assert_eq!(s.tree.selected_id(), Some("source:revit"));
        update(&mut s, Msg::Up);
        assert_eq!(s.tree.selected_id(), Some("source:ifc"));
        update(&mut s, Msg::End);
        assert_eq!(s.tree.selected_id(), Some("cat:energy"));
        update(&mut s, Msg::Home);
        assert_eq!(s.tree.selected(), 0);
    }

    #[test]
    fn enter_opens_and_toggles() {
        let mut s = state();
        s.tree.select(1);
        update(&mut s, Msg::Activate);
        assert_eq!(s.opened, NodeId::parse("source:ifc"));
        assert_eq!(s.status, "▸ Opened: ifc");
        assert!(s.tree.selected_row().unwrap().expanded);
        update(&mut s, Msg::Activate);
        assert!(!s.tree.selected_row().unwrap().expanded);
    }

    #[test]
    fn execute_emits_run_task_with_fresh_ids() {
        let mut s = state();
        s.tree.select(2); // source:revit
        let cmds = update(&mut s, Msg::Execute);
        assert_eq!(
            cmds,
            vec![Cmd::RunTask {
                id: 1,
                node: NodeId::Source("revit".into()),
                options: BTreeMap::new(),
            }]
        );
        assert_eq!(s.status, "▸ Executing: revit");
        let cmds = update(&mut s, Msg::Execute);
        assert!(matches!(cmds[..], [Cmd::RunTask { id: 2, .. }]));
        assert_eq!(s.tasks.len(), 2);
    }

    #[test]
    fn concurrent_tasks_both_report_back() {
        let mut s = state();
        s.tree.select(2);
        update(&mut s, Msg::Execute);
        update(&mut s, Msg::Execute);
        update(
            &mut s,
            Msg::TaskFinished {
                id: 2,
                result: Err("Error: revit: boom".into()),
            },
        );
        assert_eq!(s.status, "✗ Error: revit: boom");
        assert_eq!(s.tasks.keys().copied().collect::<Vec<_>>(), vec![1]);
        update(
            &mut s,
            Msg::TaskFinished {
                id: 1,
                result: Ok("Success: revit".into()),
            },
        );
        assert_eq!(s.status, "✓ Success: revit");
        assert!(s.tasks.is_empty());
        assert!(s.log.iter().any(|l| l.contains("✗ Error: revit: boom")));
    }

    #[test]
    fn execute_on_a_group_explains_instead_of_running() {
        let mut s = state();
        s.tree.select(0);
        assert!(update(&mut s, Msg::Execute).is_empty());
        assert!(s.status.contains("not executable"));
        assert!(s.tasks.is_empty());
    }

    #[test]
    fn tab_cycles_only_through_available_panes() {
        let mut s = state();
        update(&mut s, Msg::FocusNext);
        assert_eq!(s.focus, Focus::Tree); // nothing else to focus yet
        s.tree.select(1); // ifc → picker becomes focusable
        update(&mut s, Msg::FocusNext);
        assert_eq!(s.focus, Focus::IfcPicker);
        update(&mut s, Msg::ToggleOutput);
        update(&mut s, Msg::FocusNext);
        assert_eq!(s.focus, Focus::Output);
        update(&mut s, Msg::FocusNext);
        assert_eq!(s.focus, Focus::Tree);
        update(&mut s, Msg::FocusPrev);
        assert_eq!(s.focus, Focus::Output);
        // Hiding the focused pane returns focus to the tree.
        update(&mut s, Msg::ToggleOutput);
        assert_eq!(s.focus, Focus::Tree);
    }

    #[test]
    fn ifc_picker_loads_on_enter_and_ignores_stale_results() {
        let mut s = state();
        s.tree.select(1);
        s.focus = Focus::IfcPicker;
        update(&mut s, Msg::Down);
        let cmds = update(&mut s, Msg::Activate);
        assert_eq!(
            cmds,
            vec![Cmd::LoadIfc {
                path: "examples/IFC/b.ifc".into()
            }]
        );
        assert_eq!(s.ifc.loading.as_deref(), Some("examples/IFC/b.ifc"));
        assert_eq!(s.status, "◌ Loading b.ifc…");

        // A result for a different path is stale.
        update(
            &mut s,
            Msg::IfcLoaded {
                path: "examples/IFC/a.ifc".into(),
                result: Ok(IfcOutcome::NeedsExtraction),
            },
        );
        assert!(s.ifc.loading.is_some());

        let analysis = sample_analysis();
        let walls = analysis.wall_count;
        update(
            &mut s,
            Msg::IfcLoaded {
                path: "examples/IFC/b.ifc".into(),
                result: Ok(IfcOutcome::Analysis {
                    analysis,
                    json: "examples/IFC/building-model.json".into(),
                }),
            },
        );
        assert!(s.ifc.loading.is_none());
        assert_eq!(s.ifc.loaded.as_deref(), Some("examples/IFC/b.ifc"));
        assert!(s.ifc.analysis.is_some());
        assert_eq!(
            s.ifc.json.as_deref(),
            Some("examples/IFC/building-model.json")
        );
        assert!(s.ifc.error.is_none());
        assert!(s.status.starts_with(&format!("◆ {walls} walls, WWR")));
    }

    #[test]
    fn ifc_errors_and_missing_extraction_clear_analysis() {
        let mut s = state();
        s.ifc.analysis = Some(sample_analysis());
        s.ifc.loading = Some("x.ifc".into());
        update(
            &mut s,
            Msg::IfcLoaded {
                path: "x.ifc".into(),
                result: Err("bad json".into()),
            },
        );
        assert!(s.ifc.analysis.is_none());
        assert_eq!(s.status, "✗ Error loading x.ifc: bad json");
        // The detail pane must be able to show what went wrong.
        assert_eq!(s.ifc.error.as_deref(), Some("bad json"));
        assert!(s.ifc.json.is_none());

        s.ifc.loading = Some("m/y.ifc".into());
        update(
            &mut s,
            Msg::IfcLoaded {
                path: "m/y.ifc".into(),
                result: Ok(IfcOutcome::NeedsExtraction),
            },
        );
        assert_eq!(s.status, "◆ m/y.ifc is not extracted yet (no m/y.json)");
        assert!(s.ifc.error.is_none());
    }

    #[test]
    fn first_output_opens_the_pane_once() {
        let mut s = state();
        update(&mut s, Msg::Output("hello".into()));
        assert!(s.show_output);
        update(&mut s, Msg::ToggleOutput); // user hides it
        update(&mut s, Msg::Output("again".into()));
        assert!(!s.show_output);
        assert_eq!(s.log.len(), 2);
    }

    #[test]
    fn output_scrolling_is_clamped() {
        let mut s = state();
        s.show_output = true;
        s.focus = Focus::Output;
        for i in 0..20 {
            s.push_log(format!("{i}"));
        }
        update(&mut s, Msg::Up);
        assert_eq!(s.log_scroll, 1);
        update(&mut s, Msg::Home);
        assert_eq!(s.log_scroll, 20 - 6); // 6 visible rows
        update(&mut s, Msg::Up);
        assert_eq!(s.log_scroll, 14);
        update(&mut s, Msg::End);
        assert_eq!(s.log_scroll, 0);
        update(&mut s, Msg::ClearOutput);
        assert!(s.log.is_empty());
    }

    #[test]
    fn escape_closes_help_before_quitting() {
        let mut s = state();
        update(&mut s, Msg::ToggleHelp);
        assert!(update(&mut s, Msg::Escape).is_empty());
        assert!(!s.show_help);
        assert_eq!(update(&mut s, Msg::Escape), vec![Cmd::Quit]);
        assert!(s.should_quit);
    }

    #[test]
    fn clicks_select_then_toggle() {
        let mut s = state();
        update(
            &mut s,
            Msg::Click {
                pane: Pane::Tree,
                row: Some(1),
            },
        );
        assert_eq!(s.tree.selected_id(), Some("source:ifc"));
        assert!(!s.tree.selected_row().unwrap().expanded);
        update(
            &mut s,
            Msg::Click {
                pane: Pane::Tree,
                row: Some(1),
            },
        );
        assert!(s.tree.selected_row().unwrap().expanded);

        let cmds = update(
            &mut s,
            Msg::Click {
                pane: Pane::IfcPicker,
                row: Some(0),
            },
        );
        assert_eq!(s.focus, Focus::IfcPicker);
        assert_eq!(
            cmds,
            vec![Cmd::LoadIfc {
                path: "examples/IFC/a.ifc".into()
            }]
        );
    }

    #[test]
    fn wheel_moves_tree_without_changing_focus() {
        let mut s = state();
        s.show_output = true;
        s.focus = Focus::Output;
        update(
            &mut s,
            Msg::Scroll {
                pane: Pane::Tree,
                delta: 2,
            },
        );
        assert_eq!(s.tree.selected(), 2);
        assert_eq!(s.focus, Focus::Output);
    }

    #[test]
    fn resize_updates_viewport_and_keeps_selection_visible() {
        let mut s = state();
        s.tree.last();
        update(
            &mut s,
            Msg::Resize {
                width: 60,
                height: 8,
            },
        );
        assert_eq!(s.viewport, Rect::new(0, 0, 60, 8));
        let visible = layout::compute(s.viewport, &s).tree_inner.height as usize;
        assert!(s.tree.selected() < s.tree.offset() + visible.max(1));
    }

    /// The tree rows the Explorer has room to draw.
    fn tree_window(s: &AppState) -> std::ops::Range<usize> {
        let height = usize::from(layout::compute(s.viewport, s).tree_inner.height);
        s.tree.offset()..s.tree.offset() + height
    }

    /// The files the IFC picker has room to draw.
    fn ifc_window(s: &AppState) -> std::ops::Range<usize> {
        let height = layout::compute(s.viewport, s)
            .ifc
            .and_then(|ifc| ifc.list_inner)
            .map_or(0, |inner| usize::from(inner.height));
        s.ifc.offset..s.ifc.offset + height
    }

    /// Nine tree rows on a 120×16 screen, the last one selected. They all
    /// fit until the Output pane takes rows from the Explorer.
    fn last_row_selected_on_a_short_screen() -> AppState {
        let mut s = state();
        select(&mut s, "action:murb-simulate:opt:verbose");
        update(
            &mut s,
            Msg::Resize {
                width: 120,
                height: 16,
            },
        );
        assert_eq!(s.tree.offset(), 0);
        assert!(tree_window(&s).contains(&s.tree.selected()));
        s
    }

    /// Six files in the picker on a 120×16 screen, the last one selected.
    /// They all fit until the Detail pane loses rows.
    fn last_file_selected_on_a_short_screen() -> AppState {
        let mut s = state();
        s.ifc.files = (0..6).map(|i| format!("examples/IFC/{i}.ifc")).collect();
        s.tree.select(1); // source:ifc shows the picker
        update(
            &mut s,
            Msg::Resize {
                width: 120,
                height: 16,
            },
        );
        update(&mut s, Msg::FocusNext);
        assert_eq!(s.focus, Focus::IfcPicker);
        update(&mut s, Msg::End);
        assert_eq!((s.ifc.selected, s.ifc.offset), (5, 0));
        assert!(ifc_window(&s).contains(&5));
        s
    }

    #[test]
    fn opening_output_keeps_the_tree_selection_in_its_window() {
        let mut s = last_row_selected_on_a_short_screen();
        update(&mut s, Msg::ToggleOutput);
        assert!(s.output_visible());
        let window = tree_window(&s);
        assert!(
            window.len() <= s.tree.selected(),
            "the Explorer should have lost rows, has {}",
            window.len()
        );
        assert!(
            window.contains(&s.tree.selected()),
            "row {} is outside {window:?}",
            s.tree.selected()
        );

        // Closing it gives the rows back, with none left scrolled away.
        update(&mut s, Msg::ToggleOutput);
        assert_eq!(s.tree.offset(), 0);
    }

    #[test]
    fn the_first_output_line_keeps_the_tree_selection_in_its_window() {
        let mut s = last_row_selected_on_a_short_screen();
        update(&mut s, Msg::Output("hello".into()));
        assert!(s.output_visible());
        let window = tree_window(&s);
        assert!(window.len() <= s.tree.selected());
        assert!(
            window.contains(&s.tree.selected()),
            "row {} is outside {window:?}",
            s.tree.selected()
        );
    }

    #[test]
    fn opening_output_keeps_the_picker_selection_in_its_window() {
        let mut s = last_file_selected_on_a_short_screen();
        update(&mut s, Msg::ToggleOutput);
        assert!(s.output_visible());
        let window = ifc_window(&s);
        assert!(
            !window.is_empty() && window.len() < 6,
            "the picker should have lost rows, has {}",
            window.len()
        );
        assert!(
            window.contains(&s.ifc.selected),
            "file {} is outside {window:?}",
            s.ifc.selected
        );
    }

    #[test]
    fn a_picker_resized_while_hidden_comes_back_with_its_selection_in_view() {
        let mut s = last_file_selected_on_a_short_screen();
        update(&mut s, Msg::FocusPrev);
        assert_eq!(s.focus, Focus::Tree);
        update(&mut s, Msg::Down); // source:revit has no picker
        assert!(!s.ifc_section_visible());
        update(
            &mut s,
            Msg::Resize {
                width: 120,
                height: 12,
            },
        );
        update(&mut s, Msg::Up); // back to source:ifc
        let window = ifc_window(&s);
        assert!(
            !window.is_empty() && window.len() < 6,
            "the picker should have lost rows, has {}",
            window.len()
        );
        assert!(
            window.contains(&s.ifc.selected),
            "file {} is outside {window:?}",
            s.ifc.selected
        );
    }

    #[test]
    fn tick_advances_animation() {
        let mut s = state();
        update(&mut s, Msg::Tick);
        assert!(s.logo_angle > 0.0);
        assert_eq!(s.frame, 1);
    }

    // ─── Option values and the editor ───────────────────────────────────

    #[test]
    fn execute_prompts_for_a_missing_required_option() {
        let mut s = state();
        select(&mut s, "action:murb-simulate");
        assert!(update(&mut s, Msg::Execute).is_empty());
        let editor = s.editor.as_ref().expect("editor opens");
        assert_eq!(editor.task, murb());
        assert_eq!(editor.opt, "epw");
        assert!(editor.run_after);
        assert!(s.status.contains("needs --epw"), "{}", s.status);
        assert!(s.tasks.is_empty());
    }

    #[test]
    fn submitting_the_prompt_runs_the_task_with_the_value() {
        let mut s = state();
        select(&mut s, "action:murb-simulate");
        update(&mut s, Msg::Execute);
        type_text(&mut s, "/w/van.epw");
        let cmds = update(&mut s, Msg::Edit(EditKey::Submit));
        assert_eq!(
            cmds,
            vec![Cmd::RunTask {
                id: 1,
                node: murb(),
                options: opts(&[
                    ("cop", "0.85"),
                    ("epw", "/w/van.epw"),
                    ("output", "murb_results.json"),
                ]),
            }]
        );
        assert!(s.editor.is_none());
        assert_eq!(s.tasks.len(), 1);
        // The value is remembered for the next run.
        assert!(matches!(
            update(&mut s, Msg::Execute)[..],
            [Cmd::RunTask { id: 2, .. }]
        ));
    }

    #[test]
    fn empty_answer_to_a_required_prompt_does_not_run() {
        let mut s = state();
        select(&mut s, "action:murb-simulate");
        update(&mut s, Msg::Execute);
        type_text(&mut s, "   ");
        assert!(update(&mut s, Msg::Edit(EditKey::Submit)).is_empty());
        assert!(s.editor.is_none());
        assert!(s.tasks.is_empty());
        assert!(s.status.contains("required"), "{}", s.status);
    }

    #[test]
    fn enter_on_an_option_edits_and_stores_its_value() {
        let mut s = state();
        select(&mut s, "action:murb-simulate:opt:cop");
        assert!(update(&mut s, Msg::Activate).is_empty());
        let editor = s.editor.as_ref().expect("editor opens");
        assert_eq!((editor.opt.as_str(), editor.run_after), ("cop", false));
        // The current (default) value is offered for editing.
        assert_eq!(editor.buffer, "0.85");

        update(&mut s, Msg::Edit(EditKey::Clear));
        type_text(&mut s, "3.1");
        assert!(update(&mut s, Msg::Edit(EditKey::Submit)).is_empty());
        assert_eq!(
            s.option_value(&murb(), "cop"),
            Some(("3.1".to_string(), ValueSource::User))
        );
        assert_eq!(s.status, "✎ Set murb-simulate --cop = 3.1");
        // Enter on an option no longer "opens" it like a group.
        assert!(s.opened.is_none());
    }

    #[test]
    fn clearing_a_value_restores_the_default() {
        let mut s = state();
        s.values
            .entry(murb())
            .or_default()
            .insert("cop".into(), "3.1".into());
        select(&mut s, "action:murb-simulate:opt:cop");
        update(&mut s, Msg::Activate);
        update(&mut s, Msg::Edit(EditKey::Clear));
        update(&mut s, Msg::Edit(EditKey::Submit));
        assert_eq!(
            s.option_value(&murb(), "cop"),
            Some(("0.85".to_string(), ValueSource::Default))
        );
        assert!(s.status.contains("Cleared"), "{}", s.status);
    }

    #[test]
    fn cancelling_the_edit_keeps_the_old_value() {
        let mut s = state();
        select(&mut s, "action:murb-simulate:opt:epw");
        update(&mut s, Msg::Activate);
        type_text(&mut s, "/tmp/x.epw");
        assert!(update(&mut s, Msg::Edit(EditKey::Cancel)).is_empty());
        assert!(s.editor.is_none());
        assert_eq!(s.option_value(&murb(), "epw"), None);
    }

    #[test]
    fn enter_on_a_flag_toggles_it_without_an_editor() {
        let mut s = state();
        select(&mut s, "action:murb-simulate:opt:verbose");
        update(&mut s, Msg::Activate);
        assert!(s.editor.is_none());
        assert_eq!(
            s.option_value(&murb(), "verbose"),
            Some(("true".to_string(), ValueSource::User))
        );
        update(&mut s, Msg::Activate);
        assert_eq!(
            s.option_value(&murb(), "verbose"),
            Some(("false".to_string(), ValueSource::User))
        );
    }

    #[test]
    fn execute_on_an_option_runs_its_task_without_forcing_the_option() {
        let mut s = state();
        s.values
            .entry(murb())
            .or_default()
            .insert("epw".into(), "/w.epw".into());
        select(&mut s, "action:murb-simulate:opt:verbose");
        let cmds = update(&mut s, Msg::Execute);
        let [Cmd::RunTask { node, options, .. }] = &cmds[..] else {
            panic!("expected one RunTask, got {cmds:?}");
        };
        assert_eq!(node, &murb());
        assert!(!options.contains_key("verbose"));
        assert_eq!(options.get("epw").map(String::as_str), Some("/w.epw"));
    }

    #[test]
    fn the_picked_ifc_file_fills_the_ifc_option() {
        let mut s = state();
        s.tree.select(1); // source:ifc
        s.ifc.loaded = Some("examples/IFC/a.ifc".into());
        assert_eq!(
            s.option_value(&NodeId::Source("ifc".into()), "file"),
            Some(("examples/IFC/a.ifc".to_string(), ValueSource::Picker))
        );
        let cmds = update(&mut s, Msg::Execute);
        assert_eq!(
            cmds,
            vec![Cmd::RunTask {
                id: 1,
                node: NodeId::Source("ifc".into()),
                options: opts(&[("file", "examples/IFC/a.ifc")]),
            }]
        );
        assert!(s.editor.is_none());

        // A typed value beats the picker.
        s.values
            .entry(NodeId::Source("ifc".into()))
            .or_default()
            .insert("file".into(), "/other.ifc".into());
        assert_eq!(
            s.option_value(&NodeId::Source("ifc".into()), "file"),
            Some(("/other.ifc".to_string(), ValueSource::User))
        );
    }

    #[test]
    fn navigation_is_ignored_while_editing() {
        let mut s = state();
        select(&mut s, "action:murb-simulate:opt:epw");
        let before = s.tree.selected();
        update(&mut s, Msg::Activate);
        update(&mut s, Msg::Down);
        update(&mut s, Msg::Execute);
        assert_eq!(s.tree.selected(), before);
        assert!(s.editor.is_some());
        assert!(s.tasks.is_empty());
    }

    // ─── Stopping tasks ─────────────────────────────────────────────────

    #[test]
    fn stop_marks_the_focused_task_and_reports_it_stopped() {
        let mut s = state();
        s.tree.select(2); // source:revit
        update(&mut s, Msg::Execute);
        let cmds = update(&mut s, Msg::StopTask);
        assert_eq!(cmds, vec![Cmd::StopTask { id: 1 }]);
        assert!(s.tasks[&1].stopping);
        assert_eq!(s.status, "■ Stopping revit…");
        // Asking again does not send a second stop.
        assert!(update(&mut s, Msg::StopTask).is_empty());

        update(
            &mut s,
            Msg::TaskFinished {
                id: 1,
                result: Ok("Success: revit".into()),
            },
        );
        assert_eq!(s.status, "■ Stopped: revit");
        assert!(s.tasks.is_empty());
    }

    #[test]
    fn stop_reaches_the_only_running_task_from_anywhere() {
        let mut s = state();
        s.tree.select(2);
        update(&mut s, Msg::Execute);
        s.tree.select(0); // the Sources group
        assert_eq!(update(&mut s, Msg::StopTask), vec![Cmd::StopTask { id: 1 }]);
    }

    #[test]
    fn stop_with_several_tasks_elsewhere_asks_for_focus() {
        let mut s = state();
        s.tree.select(2);
        update(&mut s, Msg::Execute);
        s.ifc.loaded = Some("a.ifc".into());
        s.tree.select(1);
        update(&mut s, Msg::Execute);
        s.tree.select(0);
        assert!(update(&mut s, Msg::StopTask).is_empty());
        assert!(s.status.contains("focus"), "{}", s.status);
        // Focusing one of them stops just that one.
        s.tree.select(1);
        assert_eq!(update(&mut s, Msg::StopTask), vec![Cmd::StopTask { id: 2 }]);
    }

    #[test]
    fn stop_with_nothing_running_says_so() {
        let mut s = state();
        assert!(update(&mut s, Msg::StopTask).is_empty());
        assert_eq!(s.status, "Nothing is running");
    }

    // ─── MURB results ───────────────────────────────────────────────────

    #[test]
    fn a_successful_simulation_reloads_its_results() {
        let mut s = state();
        s.values
            .entry(murb())
            .or_default()
            .insert("epw".into(), "/w.epw".into());
        select(&mut s, "action:murb-simulate");
        update(&mut s, Msg::Execute);
        let cmds = update(
            &mut s,
            Msg::TaskFinished {
                id: 1,
                result: Ok("Success: murb-simulate".into()),
            },
        );
        assert_eq!(
            cmds,
            vec![Cmd::LoadMurb {
                path: "murb_results.json".into()
            }]
        );
    }

    #[test]
    fn a_failed_simulation_does_not_reload_results() {
        let mut s = state();
        s.values
            .entry(murb())
            .or_default()
            .insert("epw".into(), "/w.epw".into());
        select(&mut s, "action:murb-simulate");
        update(&mut s, Msg::Execute);
        let cmds = update(
            &mut s,
            Msg::TaskFinished {
                id: 1,
                result: Err("Error: murb-simulate: no python".into()),
            },
        );
        assert!(cmds.is_empty());
    }

    #[test]
    fn loaded_results_replace_the_old_ones_but_a_miss_keeps_them() {
        let mut s = state();
        let results = makit_tools::murb::MurbResults {
            name: "run".into(),
            monthly_heating_demand: vec![1.0; 12],
            monthly_cooling_demand: vec![0.0; 12],
            monthly_heating_consumption: vec![1.0; 12],
            monthly_cooling_consumption: vec![0.0; 12],
            monthly_total_consumption: vec![1.0; 12],
            monthly_ghg_emissions: vec![0.1; 12],
            tedi_kwh_m2: 12.0,
            teui_kwh_m2: 34.0,
            ghgi_kg_m2: 5.0,
            gfa: 100.0,
            weather_file: "x.epw".into(),
        };
        update(&mut s, Msg::MurbLoaded(Some(results)));
        assert_eq!(s.murb_results.as_ref().map(|r| r.tedi_kwh_m2), Some(12.0));
        update(&mut s, Msg::MurbLoaded(None));
        assert!(s.murb_results.is_some());
    }

    // ─── Help scrolling ─────────────────────────────────────────────────

    #[test]
    fn help_scrolls_within_its_content_on_a_short_terminal() {
        let mut s = state();
        update(
            &mut s,
            Msg::Resize {
                width: 40,
                height: 10,
            },
        );
        let selected = s.tree.selected();
        update(&mut s, Msg::ToggleHelp);
        let max = layout::help_max_scroll(s.viewport);
        assert!(max > 0);
        update(&mut s, Msg::Down);
        update(&mut s, Msg::Down);
        assert_eq!(s.help_scroll, 2);
        update(&mut s, Msg::End);
        assert_eq!(s.help_scroll, max);
        update(&mut s, Msg::Down);
        assert_eq!(s.help_scroll, max);
        update(&mut s, Msg::Home);
        assert_eq!(s.help_scroll, 0);
        // The tree behind the overlay did not move.
        assert_eq!(s.tree.selected(), selected);

        // Reopening starts from the top.
        update(&mut s, Msg::Down);
        update(&mut s, Msg::ToggleHelp);
        update(&mut s, Msg::ToggleHelp);
        assert_eq!(s.help_scroll, 0);
    }

    #[test]
    fn help_does_not_scroll_when_it_fits() {
        let mut s = state();
        update(&mut s, Msg::ToggleHelp);
        update(&mut s, Msg::Down);
        assert_eq!(s.help_scroll, 0);
    }
}
