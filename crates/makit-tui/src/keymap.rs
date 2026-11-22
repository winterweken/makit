//! Translate terminal events into [`Msg`]s.

use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::layout::Position;

use crate::app::AppState;
use crate::layout::{self, AppLayout};
use crate::msg::{Msg, Pane};

/// Map any terminal event to a message, if it means something.
pub fn map_event(state: &AppState, event: &Event) -> Option<Msg> {
    match event {
        Event::Key(key) => map_key(state, key),
        Event::Mouse(mouse) => map_mouse(state, mouse),
        Event::Resize(width, height) => Some(Msg::Resize {
            width: *width,
            height: *height,
        }),
        _ => None,
    }
}

/// Map a key press.
pub fn map_key(state: &AppState, key: &KeyEvent) -> Option<Msg> {
    // Windows reports releases too; act on presses and auto-repeat only.
    if !matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
        return None;
    }
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        return Some(Msg::Quit);
    }
    if state.show_help {
        // The help overlay is modal.
        return match key.code {
            KeyCode::Esc | KeyCode::Char('?') => Some(Msg::ToggleHelp),
            KeyCode::Char('q') => Some(Msg::Quit),
            _ => None,
        };
    }
    let msg = match key.code {
        KeyCode::Up | KeyCode::Char('k') => Msg::Up,
        KeyCode::Down | KeyCode::Char('j') => Msg::Down,
        KeyCode::PageUp => Msg::PageUp,
        KeyCode::PageDown => Msg::PageDown,
        KeyCode::Home | KeyCode::Char('g') => Msg::Home,
        KeyCode::End | KeyCode::Char('G') => Msg::End,
        KeyCode::Right | KeyCode::Char('l') => Msg::Expand,
        KeyCode::Left | KeyCode::Char('h') => Msg::Collapse,
        KeyCode::Enter => Msg::Activate,
        KeyCode::Char('e') => Msg::Execute,
        KeyCode::Tab => Msg::FocusNext,
        KeyCode::BackTab => Msg::FocusPrev,
        KeyCode::Char('o') => Msg::ToggleOutput,
        KeyCode::Char('c') => Msg::ClearOutput,
        KeyCode::Char('?') => Msg::ToggleHelp,
        KeyCode::Esc => Msg::Escape,
        KeyCode::Char('q') => Msg::Quit,
        _ => return None,
    };
    Some(msg)
}

/// Map a mouse event using the same layout the view draws.
pub fn map_mouse(state: &AppState, mouse: &MouseEvent) -> Option<Msg> {
    if state.show_help {
        return matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left))
            .then_some(Msg::ToggleHelp);
    }
    let layout = layout::compute(state.viewport, state);
    let pane = layout.pane_at(mouse.column, mouse.row)?;
    match mouse.kind {
        MouseEventKind::ScrollUp => Some(Msg::Scroll { pane, delta: -1 }),
        MouseEventKind::ScrollDown => Some(Msg::Scroll { pane, delta: 1 }),
        MouseEventKind::Down(MouseButton::Left) => Some(Msg::Click {
            pane,
            row: row_at(state, &layout, pane, mouse.column, mouse.row),
        }),
        _ => None,
    }
}

/// List row under the cursor for list panes.
fn row_at(state: &AppState, layout: &AppLayout, pane: Pane, x: u16, y: u16) -> Option<usize> {
    let pos = Position::new(x, y);
    match pane {
        Pane::Tree => {
            let inner = layout.tree_inner;
            inner
                .contains(pos)
                .then(|| state.tree.row_at(usize::from(y - inner.y)))
                .flatten()
        }
        Pane::IfcPicker => {
            let inner = layout.ifc.and_then(|i| i.list_inner)?;
            if !inner.contains(pos) {
                return None;
            }
            let index = state.ifc.offset + usize::from(y - inner.y);
            (index < state.ifc.files.len()).then_some(index)
        }
        Pane::Output | Pane::Detail => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::InitialData;
    use crate::tree::TreeNode;
    use crossterm::event::KeyEventState;
    use ratatui::layout::Rect;

    fn state() -> AppState {
        AppState::new(InitialData {
            tree: vec![TreeNode::new("sources", "Sources")
                .child(TreeNode::new("source:ifc", "◆ ifc"))
                .child(TreeNode::new("source:revit", "◆ revit"))],
            ifc_files: vec!["a.ifc".into(), "b.ifc".into()],
            murb_results: None,
            viewport: Rect::new(0, 0, 120, 40),
        })
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn mouse(kind: MouseEventKind, column: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind,
            column,
            row,
            modifiers: KeyModifiers::NONE,
        }
    }

    #[test]
    fn navigation_keys() {
        let s = state();
        assert!(matches!(
            map_key(&s, &key(KeyCode::Char('j'))),
            Some(Msg::Down)
        ));
        assert!(matches!(map_key(&s, &key(KeyCode::Up)), Some(Msg::Up)));
        assert!(matches!(
            map_key(&s, &key(KeyCode::Right)),
            Some(Msg::Expand)
        ));
        assert!(matches!(
            map_key(&s, &key(KeyCode::Char('h'))),
            Some(Msg::Collapse)
        ));
        assert!(matches!(
            map_key(&s, &key(KeyCode::Enter)),
            Some(Msg::Activate)
        ));
        assert!(matches!(
            map_key(&s, &key(KeyCode::Char('e'))),
            Some(Msg::Execute)
        ));
        assert!(matches!(
            map_key(&s, &key(KeyCode::BackTab)),
            Some(Msg::FocusPrev)
        ));
        assert!(matches!(map_key(&s, &key(KeyCode::Esc)), Some(Msg::Escape)));
        assert!(map_key(&s, &key(KeyCode::Char('z'))).is_none());
    }

    #[test]
    fn ctrl_c_always_quits() {
        let mut s = state();
        s.show_help = true;
        let ev = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert!(matches!(map_key(&s, &ev), Some(Msg::Quit)));
    }

    #[test]
    fn help_overlay_is_modal() {
        let mut s = state();
        s.show_help = true;
        assert!(map_key(&s, &key(KeyCode::Down)).is_none());
        assert!(matches!(
            map_key(&s, &key(KeyCode::Esc)),
            Some(Msg::ToggleHelp)
        ));
        let click = mouse(MouseEventKind::Down(MouseButton::Left), 5, 5);
        assert!(matches!(map_mouse(&s, &click), Some(Msg::ToggleHelp)));
    }

    #[test]
    fn key_releases_are_ignored() {
        let s = state();
        let release = KeyEvent {
            code: KeyCode::Down,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Release,
            state: KeyEventState::NONE,
        };
        assert!(map_key(&s, &release).is_none());
    }

    #[test]
    fn resize_event_maps_to_resize_msg() {
        let s = state();
        assert!(matches!(
            map_event(&s, &Event::Resize(90, 30)),
            Some(Msg::Resize {
                width: 90,
                height: 30
            })
        ));
    }

    #[test]
    fn clicking_a_tree_row_reports_its_index() {
        let s = state();
        let l = layout::compute(s.viewport, &s);
        let inner = l.tree_inner;
        let ev = mouse(
            MouseEventKind::Down(MouseButton::Left),
            inner.x + 1,
            inner.y + 2,
        );
        match map_mouse(&s, &ev) {
            Some(Msg::Click {
                pane: Pane::Tree,
                row: Some(2),
            }) => {}
            other => panic!("unexpected {other:?}"),
        }
        // Below the last row: pane hit, no row.
        let ev = mouse(
            MouseEventKind::Down(MouseButton::Left),
            inner.x + 1,
            inner.y + 10,
        );
        assert!(matches!(
            map_mouse(&s, &ev),
            Some(Msg::Click {
                pane: Pane::Tree,
                row: None
            })
        ));
    }

    #[test]
    fn clicking_the_ifc_picker_reports_file_index() {
        let mut s = state();
        s.tree.select(1);
        let l = layout::compute(s.viewport, &s);
        let inner = l.ifc.unwrap().list_inner.unwrap();
        let ev = mouse(
            MouseEventKind::Down(MouseButton::Left),
            inner.x,
            inner.y + 1,
        );
        assert!(matches!(
            map_mouse(&s, &ev),
            Some(Msg::Click {
                pane: Pane::IfcPicker,
                row: Some(1)
            })
        ));
    }

    #[test]
    fn wheel_targets_the_pane_under_the_cursor() {
        let mut s = state();
        s.show_output = true;
        let l = layout::compute(s.viewport, &s);
        let out = l.output.unwrap();
        let ev = mouse(MouseEventKind::ScrollUp, out.x + 3, out.y + 2);
        assert!(matches!(
            map_mouse(&s, &ev),
            Some(Msg::Scroll {
                pane: Pane::Output,
                delta: -1
            })
        ));
        let ev = mouse(MouseEventKind::ScrollDown, 2, 3);
        assert!(matches!(
            map_mouse(&s, &ev),
            Some(Msg::Scroll {
                pane: Pane::Tree,
                delta: 1
            })
        ));
        // Header is not a pane.
        let ev = mouse(MouseEventKind::ScrollDown, 2, 0);
        assert!(map_mouse(&s, &ev).is_none());
    }
}
