//! Screen layout, computed purely from the viewport and state.
//!
//! Both `view` (rendering) and `keymap` (mouse hit-testing) call
//! [`compute`], so a click always lands on what is actually drawn there.

use ratatui::layout::{Constraint, Layout, Margin, Position, Rect};
use ratatui::widgets::{Block, Borders};

use crate::app::{AppState, WIDE_BREAKPOINT};
use crate::msg::Pane;
use crate::preview;

/// Height of the Output pane (including borders) when shown.
pub const OUTPUT_HEIGHT: u16 = 8;
/// Explorer height (including borders) in the stacked narrow layout.
pub const NARROW_TREE_HEIGHT: u16 = 12;
/// Maximum visible rows in the IFC picker list.
pub const IFC_LIST_MAX_ROWS: u16 = 6;
/// Explorer width share in the wide layout.
pub const TREE_PERCENT: u16 = 35;

/// Rects for the IFC section of the detail pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IfcLayout {
    /// "IFC file" divider line.
    pub label: Rect,
    /// Bordered picker list (absent when no files were found).
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

/// Lay out the whole screen.
pub fn compute(area: Rect, state: &AppState) -> AppLayout {
    let output_h = if state.show_output { OUTPUT_HEIGHT } else { 0 };
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
        Layout::vertical([Constraint::Length(NARROW_TREE_HEIGHT), Constraint::Min(0)]).areas(body)
    };

    let detail_inner = bordered_inner(detail).inner(Margin::new(1, 0));
    let kind = state.preview_kind();
    let caption_h = preview::caption(kind, state.murb_results.as_ref()).len() as u16;
    let ifc_h = ifc_section_height(state);

    let [info, ifc_area, preview_label, preview, caption] = Layout::vertical([
        Constraint::Length(2),
        Constraint::Length(ifc_h),
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(caption_h),
    ])
    .areas(detail_inner);

    let ifc = state
        .ifc_section_visible()
        .then(|| ifc_layout(ifc_area, state));
    let output = state.show_output.then_some(output_area);

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
    let list_h = if state.ifc.files.is_empty() {
        1
    } else {
        ifc_list_rows(state) + 2
    };
    let [label, list_area, details] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(list_h),
        Constraint::Min(0),
    ])
    .areas(area);
    let (list, list_inner) = if state.ifc.files.is_empty() {
        (None, None)
    } else {
        (Some(list_area), Some(bordered_inner(list_area)))
    };
    IfcLayout {
        label,
        list,
        list_inner,
        details: if state.ifc.files.is_empty() {
            // "No files" message occupies the list slot.
            list_area
        } else {
            details
        },
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
}
