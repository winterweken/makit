//! Screen layout, computed purely from the viewport and state.
//!
//! Both `view` (rendering) and `keymap` (mouse hit-testing) call
//! [`compute`], so a click always lands on what is actually drawn there.

use ratatui::layout::{Constraint, Layout, Margin, Position, Rect};
use ratatui::widgets::{Block, Borders};

use crate::app::{AppState, WIDE_BREAKPOINT};
use crate::help;
use crate::msg::Pane;
use crate::preview;

/// Tallest the Output pane gets (including borders).
pub const OUTPUT_HEIGHT: u16 = 8;
/// Shortest useful Output pane: one line inside its border.
pub const MIN_OUTPUT_HEIGHT: u16 = 3;
/// Rows the panes above the Output pane must keep for it to be shown.
pub const MIN_BODY_HEIGHT: u16 = 4;
/// Tallest the Explorer gets (including borders) in the stacked layout.
pub const NARROW_TREE_HEIGHT: u16 = 12;
/// Shortest the Explorer gets in the stacked layout.
pub const MIN_TREE_HEIGHT: u16 = 3;
/// Fewest canvas rows a preview is worth drawing in; with less, the preview
/// is left out rather than smeared across a row or two.
pub const MIN_PREVIEW_HEIGHT: u16 = 3;
/// Fewest rows of a bordered list that still show an entry.
const MIN_LIST_HEIGHT: u16 = 3;
/// Maximum visible rows in the IFC picker list.
pub const IFC_LIST_MAX_ROWS: u16 = 6;
/// Explorer width share in the wide layout.
pub const TREE_PERCENT: u16 = 35;

/// Help overlay width (including borders).
pub const HELP_WIDTH: u16 = 66;
/// Option editor size (including borders).
pub const EDITOR_WIDTH: u16 = 66;
pub const EDITOR_HEIGHT: u16 = 7;

/// Rects for the IFC section of the detail pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IfcLayout {
    /// "IFC file" divider line.
    pub label: Rect,
    /// Bordered picker list (absent when no files were found, or when
    /// there is no room to show one).
    pub list: Option<Rect>,
    /// Picker rows inside the border.
    pub list_inner: Option<Rect>,
    /// Status / analysis lines under the picker.
    pub details: Rect,
}

/// Every rect the view draws into.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppLayout {
    pub header: Rect,
    pub tree: Rect,
    pub tree_inner: Rect,
    pub detail: Rect,
    /// Focus / Open / description lines.
    pub info: Rect,
    pub ifc: Option<IfcLayout>,
    /// "Preview" divider line.
    pub preview_label: Rect,
    /// Braille canvas area.
    pub preview: Rect,
    /// Caption lines under the canvas.
    pub caption: Rect,
    pub output: Option<Rect>,
    pub output_inner: Option<Rect>,
    pub status: Rect,
}

impl AppLayout {
    /// Which pane contains the cell `(x, y)`.
    pub fn pane_at(&self, x: u16, y: u16) -> Option<Pane> {
        let pos = Position::new(x, y);
        if self.tree.contains(pos) {
            return Some(Pane::Tree);
        }
        if let Some(list) = self.ifc.and_then(|i| i.list) {
            if list.contains(pos) {
                return Some(Pane::IfcPicker);
            }
        }
        if self.output.is_some_and(|o| o.contains(pos)) {
            return Some(Pane::Output);
        }
        if self.detail.contains(pos) {
            return Some(Pane::Detail);
        }
        None
    }
}

/// Whether `area` is wide enough for side-by-side panes.
pub fn is_wide(area: Rect) -> bool {
    area.width >= WIDE_BREAKPOINT
}

/// The inner area of a fully-bordered block.
pub fn bordered_inner(area: Rect) -> Rect {
    Block::default().borders(Borders::ALL).inner(area)
}

/// Height of the Output pane on a screen `viewport_height` rows tall: a
/// third of the rows between header and status bar, within its limits —
/// or 0 when that would squeeze the panes above it out.
pub fn output_height(viewport_height: u16) -> u16 {
    let rows = viewport_height.saturating_sub(2);
    let height = (rows / 3).clamp(MIN_OUTPUT_HEIGHT, OUTPUT_HEIGHT);
    if rows >= height + MIN_BODY_HEIGHT {
        height
    } else {
        0
    }
}

/// Lines of the focus / description block at the top of the detail pane.
fn info_height(state: &AppState) -> u16 {
    let is_option = state.active_node().is_some_and(|n| n.opt_name().is_some());
    2 + u16::from(is_option)
}

/// How the rows inside the detail pane are shared out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DetailRows {
    info: u16,
    ifc: u16,
    label: u16,
    preview: u16,
    caption: u16,
}

/// Share `rows` between the detail pane's sections, top to bottom. What the
/// user acts on comes first: the focus lines, then the IFC section. The
/// preview takes what is left — whole (heading plus a readable canvas), or
/// not at all — and its caption yields before the canvas does.
fn detail_rows(rows: u16, info: u16, ifc: u16, caption: u16) -> DetailRows {
    let info = info.min(rows);
    let ifc = ifc.min(rows - info);
    let left = rows - info - ifc;
    let Some(spare) = left.checked_sub(1 + MIN_PREVIEW_HEIGHT) else {
        return DetailRows {
            info,
            ifc,
            label: 0,
            preview: 0,
            caption: 0,
        };
    };
    let caption = caption.min(spare);
    DetailRows {
        info,
        ifc,
        label: 1,
        preview: left - 1 - caption,
        caption,
    }
}

/// Lay out the whole screen.
pub fn compute(area: Rect, state: &AppState) -> AppLayout {
    let output_h = if state.show_output {
        output_height(area.height)
    } else {
        0
    };
    let [header, body, output_area, status] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(output_h),
        Constraint::Length(1),
    ])
    .areas(area);

    let [tree, detail] = if is_wide(area) {
        Layout::horizontal([Constraint::Percentage(TREE_PERCENT), Constraint::Min(0)]).areas(body)
    } else {
        // Share a short screen evenly rather than starve the detail pane.
        let tree_h = NARROW_TREE_HEIGHT.min(body.height / 2).max(MIN_TREE_HEIGHT);
        Layout::vertical([Constraint::Length(tree_h), Constraint::Min(0)]).areas(body)
    };

    let detail_inner = bordered_inner(detail).inner(Margin::new(1, 0));
    let kind = state.preview_kind();
    let caption_h = preview::caption(kind, state.preview_data()).len() as u16;
    let ifc_h = ifc_section_height(state);

    let rows = detail_rows(detail_inner.height, info_height(state), ifc_h, caption_h);
    let [info, ifc_area, preview_label, preview, caption] = Layout::vertical([
        Constraint::Length(rows.info),
        Constraint::Length(rows.ifc),
        Constraint::Length(rows.label),
        Constraint::Length(rows.preview),
        Constraint::Length(rows.caption),
    ])
    .areas(detail_inner);

    let ifc = state
        .ifc_section_visible()
        .then(|| ifc_layout(ifc_area, state));
    let output = (output_h > 0).then_some(output_area);

    AppLayout {
        header,
        tree,
        tree_inner: bordered_inner(tree),
        detail,
        info,
        ifc,
        preview_label,
        preview,
        caption,
        output,
        output_inner: output.map(bordered_inner),
        status,
    }
}

/// Centre a `width`×`height` popup inside `area` (clamped to fit).
pub fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let w = width.min(area.width);
    let h = height.min(area.height);
    Rect::new(
        area.x + (area.width - w) / 2,
        area.y + (area.height - h) / 2,
        w,
        h,
    )
}

/// The help overlay: as tall as its text, or as tall as the screen allows.
pub fn help_rect(viewport: Rect) -> Rect {
    centered(viewport, HELP_WIDTH, help::LINES.len() as u16 + 2)
}

/// How far the help can scroll before its last line reaches the bottom.
pub fn help_max_scroll(viewport: Rect) -> u16 {
    let visible = bordered_inner(help_rect(viewport)).height;
    (help::LINES.len() as u16).saturating_sub(visible)
}

/// The option editor form.
pub fn editor_rect(viewport: Rect) -> Rect {
    centered(viewport, EDITOR_WIDTH, EDITOR_HEIGHT)
}

/// Rows of the IFC picker list (excluding borders).
fn ifc_list_rows(state: &AppState) -> u16 {
    (state.ifc.files.len() as u16).min(IFC_LIST_MAX_ROWS)
}

/// Lines under the picker: analysis table, loading/selected notes.
pub fn ifc_details_height(state: &AppState) -> u16 {
    if let Some(a) = &state.ifc.analysis {
        let project = u16::from(!a.project_name.is_empty());
        // project + table header + one row per orientation + summary line
        project + 1 + a.orientations.len() as u16 + 1
    } else if state.ifc.loading.is_some() {
        1
    } else if state.ifc.loaded.is_some() {
        // File name + either the error or the extraction hint.
        2
    } else {
        0
    }
}

fn ifc_section_height(state: &AppState) -> u16 {
    if !state.ifc_section_visible() {
        return 0;
    }
    let list_h = if state.ifc.files.is_empty() {
        1
    } else {
        ifc_list_rows(state) + 2
    };
    1 + list_h + ifc_details_height(state)
}

fn ifc_layout(area: Rect, state: &AppState) -> IfcLayout {
    let room = area.height.saturating_sub(1);
    let list_h = if state.ifc.files.is_empty() {
        // The "no files" message takes the list's place.
        0
    } else {
        // A short section shows fewer entries (the list scrolls); with no
        // room for even one, the list is left out.
        Some((ifc_list_rows(state) + 2).min(room))
            .filter(|&h| h >= MIN_LIST_HEIGHT)
            .unwrap_or(0)
    };
    let [label, list_area, details] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(list_h),
        Constraint::Min(0),
    ])
    .areas(area);
    let list = (list_h > 0).then_some(list_area);
    IfcLayout {
        label,
        list,
        list_inner: list.map(bordered_inner),
        details,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::InitialData;
    use crate::tree::TreeNode;

    fn state(w: u16, h: u16) -> AppState {
        AppState::new(InitialData {
            tree: vec![TreeNode::new("sources", "Sources")
                .child(TreeNode::new("source:ifc", "◆ ifc"))
                .child(TreeNode::new("source:revit", "◆ revit"))],
            ifc_files: vec!["a.ifc".into(), "b.ifc".into()],
            murb_results: None,
            viewport: Rect::new(0, 0, w, h),
            ..InitialData::default()
        })
    }

    #[test]
    fn wide_layout_is_side_by_side() {
        let s = state(120, 40);
        let l = compute(s.viewport, &s);
        assert_eq!(l.header.height, 1);
        assert_eq!(l.status.y, 39);
        assert_eq!(l.tree.y, l.detail.y);
        assert!(l.tree.x < l.detail.x);
        assert_eq!(l.tree.width, 42); // 35% of 120
        assert!(l.output.is_none());
        assert!(l.ifc.is_none());
    }

    #[test]
    fn narrow_layout_stacks() {
        let s = state(70, 40);
        let l = compute(s.viewport, &s);
        assert_eq!(l.tree.x, l.detail.x);
        assert_eq!(l.tree.height, NARROW_TREE_HEIGHT);
        assert!(l.detail.y > l.tree.y);
    }

    #[test]
    fn output_pane_reserves_rows_above_status() {
        let mut s = state(120, 40);
        s.show_output = true;
        let l = compute(s.viewport, &s);
        let out = l.output.unwrap();
        assert_eq!(out.height, OUTPUT_HEIGHT);
        assert_eq!(out.y + out.height, l.status.y);
        assert_eq!(l.output_inner.unwrap().height, OUTPUT_HEIGHT - 2);
    }

    #[test]
    fn ifc_section_appears_for_ifc_nodes() {
        let mut s = state(120, 40);
        s.tree.select(1); // source:ifc
        let l = compute(s.viewport, &s);
        let ifc = l.ifc.unwrap();
        assert_eq!(ifc.list.unwrap().height, 2 + 2);
        assert_eq!(ifc.list_inner.unwrap().height, 2);
        assert!(l.preview.y > ifc.list.unwrap().y);
    }

    #[test]
    fn hit_testing_finds_panes() {
        let mut s = state(120, 40);
        s.tree.select(1);
        s.show_output = true;
        let l = compute(s.viewport, &s);
        assert_eq!(l.pane_at(1, 5), Some(Pane::Tree));
        let list = l.ifc.unwrap().list.unwrap();
        assert_eq!(l.pane_at(list.x + 2, list.y + 1), Some(Pane::IfcPicker));
        let out = l.output.unwrap();
        assert_eq!(l.pane_at(out.x + 2, out.y + 1), Some(Pane::Output));
        assert_eq!(l.pane_at(l.preview.x, l.preview.y), Some(Pane::Detail));
        assert_eq!(l.pane_at(5, 0), None); // header
    }

    #[test]
    fn centered_popup_is_clamped() {
        let area = Rect::new(0, 0, 40, 10);
        assert_eq!(centered(area, 20, 4), Rect::new(10, 3, 20, 4));
        assert_eq!(centered(area, 100, 100), area);
    }

    #[test]
    fn tiny_terminals_do_not_panic() {
        for (w, h) in [(1, 1), (10, 3), (0, 0)] {
            let mut s = state(w, h);
            s.show_output = true;
            s.tree.select(1);
            let _ = compute(s.viewport, &s);
        }
    }

    // ─── Short terminals ────────────────────────────────────────────────

    #[test]
    fn output_pane_shrinks_on_short_terminals() {
        let mut s = state(120, 24);
        s.show_output = true;
        let l = compute(s.viewport, &s);
        // A third of the 22 rows between header and status bar.
        assert_eq!(l.output.unwrap().height, 7);
        assert_eq!(l.tree.height, 15);

        let mut s = state(120, 12);
        s.show_output = true;
        let l = compute(s.viewport, &s);
        assert_eq!(l.output.unwrap().height, 3);
        assert_eq!(l.tree.height, 7);
    }

    #[test]
    fn output_pane_gives_way_when_there_is_no_room() {
        let mut s = state(120, 8);
        s.show_output = true;
        let l = compute(s.viewport, &s);
        assert!(l.output.is_none());
        assert_eq!(l.tree.height, 6);
        // A pane that is not drawn cannot take focus either.
        assert!(!s.focus_order().contains(&crate::app::Focus::Output));
    }

    #[test]
    fn stacked_explorer_leaves_room_for_the_detail_pane() {
        let s = state(70, 16);
        let l = compute(s.viewport, &s);
        assert_eq!(l.tree.height, 7);
        assert_eq!(l.detail.height, 7);

        let s = state(70, 6);
        let l = compute(s.viewport, &s);
        assert_eq!(l.tree.height, 3);
        assert_eq!(l.detail.height, 1);
    }

    #[test]
    fn detail_rows_keep_the_preview_whole_or_leave_it_out() {
        let rows = |info, ifc, label, preview, caption| DetailRows {
            info,
            ifc,
            label,
            preview,
            caption,
        };
        // Plenty of room: each section gets what it asks for, the canvas the rest.
        assert_eq!(detail_rows(20, 2, 0, 1), rows(2, 0, 1, 16, 1));
        // The caption gives way before the canvas does.
        assert_eq!(detail_rows(6, 2, 0, 2), rows(2, 0, 1, 3, 0));
        // Two rows left over are not a preview.
        assert_eq!(detail_rows(13, 2, 9, 0), rows(2, 9, 0, 0, 0));
        // The IFC section is cut before the focus lines are.
        assert_eq!(detail_rows(5, 2, 9, 1), rows(2, 3, 0, 0, 0));
        assert_eq!(detail_rows(1, 2, 9, 1), rows(1, 0, 0, 0, 0));
    }

    #[test]
    fn a_short_detail_pane_drops_the_preview_and_shrinks_the_picker() {
        let mut s = state(120, 10);
        s.tree.select(1); // source:ifc
        let l = compute(s.viewport, &s);
        // 6 rows inside the border: 2 focus lines, the label, a 1-row picker.
        let ifc = l.ifc.unwrap();
        assert_eq!(ifc.list.unwrap().height, 3);
        assert_eq!(ifc.list_inner.unwrap().height, 1);
        assert_eq!(l.preview_label.height, 0);
        assert_eq!(l.preview.height, 0);
    }

    #[test]
    fn a_picker_with_no_room_for_a_row_is_not_drawn_or_focusable() {
        let mut s = state(120, 8);
        s.tree.select(1);
        let l = compute(s.viewport, &s);
        // 4 rows inside: 2 focus lines and the label leave 1 — not a list.
        assert!(l.ifc.unwrap().list.is_none());
        assert!(!s.focus_order().contains(&crate::app::Focus::IfcPicker));
    }

    // ─── Overlays ───────────────────────────────────────────────────────

    #[test]
    fn help_is_as_tall_as_its_text_when_the_screen_allows() {
        let viewport = Rect::new(0, 0, 120, 40);
        let rect = help_rect(viewport);
        assert_eq!(rect.height, help::LINES.len() as u16 + 2);
        assert_eq!(rect.width, HELP_WIDTH);
        assert_eq!(help_max_scroll(viewport), 0);
    }

    #[test]
    fn help_scrolls_by_what_does_not_fit() {
        let viewport = Rect::new(0, 0, 80, 12);
        assert_eq!(help_rect(viewport), Rect::new(7, 0, HELP_WIDTH, 12));
        // 10 rows inside the border.
        assert_eq!(help_max_scroll(viewport), help::LINES.len() as u16 - 10);
        assert_eq!(
            help_max_scroll(Rect::new(0, 0, 3, 2)),
            help::LINES.len() as u16
        );
    }

    #[test]
    fn option_nodes_get_a_third_info_line() {
        let mut s = AppState::new(InitialData {
            tree: vec![TreeNode::new("source:ifc", "◆ ifc")
                .child(TreeNode::new("source:ifc:opt:file", "--file"))],
            viewport: Rect::new(0, 0, 120, 40),
            ..InitialData::default()
        });
        assert_eq!(compute(s.viewport, &s).info.height, 2);
        s.tree.expand();
        s.tree.select(1);
        assert_eq!(compute(s.viewport, &s).info.height, 3);
    }

    #[test]
    fn a_failed_ifc_load_reserves_room_for_the_error() {
        let mut s = state(120, 40);
        s.ifc.loaded = Some("a.ifc".into());
        s.ifc.error = Some("bad json".into());
        assert_eq!(ifc_details_height(&s), 2);
    }
}
