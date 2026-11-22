//! `update(state, msg) -> commands` — the only place state changes.
//!
//! Pure: no I/O, no registry access, no threads. Side effects are returned
//! as [`Cmd`]s for the runtime to perform; their results come back as
//! [`Msg`]s.

use std::path::Path;

use ratatui::layout::Rect;

use crate::app::{AppState, Focus, RunningTask};
use crate::layout;
use crate::msg::{Cmd, IfcOutcome, Msg, Pane};

/// Logo rotation per tick (radians).
const LOGO_STEP: f64 = 0.04;
/// Output lines scrolled per mouse-wheel notch.
const OUTPUT_WHEEL_LINES: isize = 3;

/// Apply `msg` to `state`, returning any side effects to perform.
pub fn update(state: &mut AppState, msg: Msg) -> Vec<Cmd> {
    let cmds = match msg {
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
                scroll_tree(state);
            }
            Vec::new()
        }
        Msg::Collapse => {
            if state.focus == Focus::Tree {
                state.tree.collapse_or_parent();
                scroll_tree(state);
            }
            Vec::new()
        }
        Msg::Activate => activate(state),
        Msg::Execute => execute(state),
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
            state.show_help = !state.show_help;
            Vec::new()
        }
        Msg::Escape => {
            if state.show_help {
                state.show_help = false;
                Vec::new()
            } else {
                quit(state)
            }
        }
        Msg::Quit => quit(state),
        Msg::Scroll { pane, delta } => scroll(state, pane, delta),
        Msg::Click { pane, row } => click(state, pane, row),
        Msg::Resize { width, height } => {
            state.viewport = Rect::new(0, 0, width, height);
            scroll_tree(state);
            scroll_ifc(state);
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
        Msg::TaskFinished { id, result } => {
            state.tasks.remove(&id);
            state.status = match result {
                Ok(msg) => format!("✓ {msg}"),
                Err(err) => format!("✗ {err}"),
            };
            let line = format!("── {}", state.status);
            state.push_log(line);
            Vec::new()
        }
        Msg::IfcLoaded { path, result } => {
            ifc_loaded(state, path, result);
            Vec::new()
        }
    };
    normalize_focus(state);
    cmds
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
            scroll_tree(state);
        }
        Focus::IfcPicker => {
            let last = state.ifc.files.len().saturating_sub(1);
            state.ifc.selected = state.ifc.selected.saturating_add_signed(delta).min(last);
            scroll_ifc(state);
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
            scroll_tree(state);
        }
        Focus::IfcPicker => {
            state.ifc.selected = if to_top {
                0
            } else {
                state.ifc.files.len().saturating_sub(1)
            };
            scroll_ifc(state);
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
            if let Some(node) = state.active_node() {
                state.status = format!("▸ Opened: {}", node.display_name());
                state.opened = Some(node);
            }
            state.tree.toggle();
            scroll_tree(state);
            Vec::new()
        }
        Focus::IfcPicker => load_ifc(state, state.ifc.selected),
        Focus::Output => Vec::new(),
    }
}

fn execute(state: &mut AppState) -> Vec<Cmd> {
    let Some(node) = state.active_node() else {
        return Vec::new();
    };
    let Some(name) = node.task_name().map(str::to_owned) else {
        state.status = format!(
            "✗ '{}' is not executable — select a source or action",
            node.display_name()
        );
        return Vec::new();
    };
    let id = state.next_task_id;
    state.next_task_id += 1;
    state.tasks.insert(id, RunningTask { name: name.clone() });
    state.status = format!("▸ Executing: {name}");
    state.push_log(format!("── ▸ Executing {name}"));
    vec![Cmd::RunTask { id, node }]
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
    match result {
        Ok(IfcOutcome::Analysis(analysis)) => {
            state.status = format!(
                "◆ {} walls, WWR {:.1}% — {}",
                analysis.wall_count, analysis.overall_wwr, path
            );
            state.ifc.analysis = Some(analysis);
        }
        Ok(IfcOutcome::NeedsExtraction) => {
            state.status = format!("◆ Selected: {path} (extract JSON first for analysis)");
            state.ifc.analysis = None;
        }
        Err(err) => {
            state.status = format!("✗ Error loading {path}: {err}");
            state.ifc.analysis = None;
        }
    }
    state.ifc.loaded = Some(path);
    // The details block changed height; keep the picker cursor visible.
    scroll_ifc(state);
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
            scroll_tree(state);
        }
        Pane::IfcPicker => {
            let last = state.ifc.files.len().saturating_sub(1);
            state.ifc.selected = state.ifc.selected.saturating_add_signed(delta).min(last);
            scroll_ifc(state);
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
                    scroll_tree(state);
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
                    scroll_ifc(state);
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

fn scroll_tree(state: &mut AppState) {
    let height = layout::compute(state.viewport, state).tree_inner.height;
    state.tree.scroll_to_selected(usize::from(height));
}

fn scroll_ifc(state: &mut AppState) {
    let Some(height) = layout::compute(state.viewport, state)
        .ifc
        .and_then(|i| i.list_inner)
        .map(|r| usize::from(r.height))
    else {
        return;
    };
    let ifc = &mut state.ifc;
    if height == 0 {
        return;
    }
    if ifc.selected < ifc.offset {
        ifc.offset = ifc.selected;
    } else if ifc.selected >= ifc.offset + height {
        ifc.offset = ifc.selected + 1 - height;
    }
    ifc.offset = ifc.offset.min(ifc.files.len().saturating_sub(height));
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
    use crate::app::InitialData;
    use crate::node::NodeId;
    use crate::tree::TreeNode;
    use makit_tools::building_model;

    /// Rows: 0 sources, 1 source:ifc, 2 source:revit, 3 actions, 4 cat:energy
    fn state() -> AppState {
        AppState::new(InitialData {
            tree: vec![
                TreeNode::new("sources", "Sources")
                    .child(
                        TreeNode::new("source:ifc", "◆ ifc")
                            .child(TreeNode::new("source:ifc:opt:path", "--path")),
                    )
                    .child(TreeNode::new("source:revit", "◆ revit")),
                TreeNode::new("actions", "Actions").child(
                    TreeNode::new("cat:energy", "⊞ energy")
                        .child(TreeNode::new("action:murb-simulate", "▸ murb")),
                ),
            ],
            ifc_files: vec!["examples/IFC/a.ifc".into(), "examples/IFC/b.ifc".into()],
            murb_results: None,
            viewport: Rect::new(0, 0, 120, 40),
        })
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
                node: NodeId::Source("revit".into())
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
                result: Ok(IfcOutcome::Analysis(analysis)),
            },
        );
        assert!(s.ifc.loading.is_none());
        assert_eq!(s.ifc.loaded.as_deref(), Some("examples/IFC/b.ifc"));
        assert!(s.ifc.analysis.is_some());
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

        s.ifc.loading = Some("y.ifc".into());
        update(
            &mut s,
            Msg::IfcLoaded {
                path: "y.ifc".into(),
                result: Ok(IfcOutcome::NeedsExtraction),
            },
        );
        assert!(s.status.contains("extract JSON first"));
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

    #[test]
    fn tick_advances_animation() {
        let mut s = state();
        update(&mut s, Msg::Tick);
        assert!(s.logo_angle > 0.0);
        assert_eq!(s.frame, 1);
    }
}
