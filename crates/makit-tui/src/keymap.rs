//! Translate terminal events into [`Msg`]s.

use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::layout::Position;

use crate::app::AppState;
use crate::layout::{self, AppLayout};
use crate::msg::{EditKey, Msg, Pane};

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
    if state.editor.is_some() {
        // The option form is modal and takes every printable key as text.
        return map_edit_key(key).map(Msg::Edit);
    }
    if state.show_help {
        // The help overlay is modal: it scrolls and closes, nothing else.
        return match key.code {
            KeyCode::Up | KeyCode::Char('k') => Some(Msg::Up),
            KeyCode::Down | KeyCode::Char('j') => Some(Msg::Down),
            KeyCode::PageUp => Some(Msg::PageUp),
            KeyCode::PageDown => Some(Msg::PageDown),
            KeyCode::Home | KeyCode::Char('g') => Some(Msg::Home),
            KeyCode::End | KeyCode::Char('G') => Some(Msg::End),
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
        KeyCode::Char('x') => Msg::StopTask,
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

/// Map a key press in the option form.
fn map_edit_key(key: &KeyEvent) -> Option<EditKey> {
    let chord = key
        .modifiers
        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT);
    let edit = match key.code {
        KeyCode::Enter => EditKey::Submit,
        KeyCode::Esc => EditKey::Cancel,
        KeyCode::Backspace => EditKey::Backspace,
        KeyCode::Delete => EditKey::Delete,
        KeyCode::Left => EditKey::Left,
        KeyCode::Right => EditKey::Right,
        KeyCode::Home => EditKey::Home,
        KeyCode::End => EditKey::End,
        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => EditKey::Clear,
        KeyCode::Char(c) if !chord => EditKey::Char(c),
        _ => return None,
    };
    Some(edit)
}

/// Map a mouse event using the same layout the view draws.
pub fn map_mouse(state: &AppState, mouse: &MouseEvent) -> Option<Msg> {
    if state.editor.is_some() {
        return None;
    }
    if state.show_help {
        return match mouse.kind {
            MouseEventKind::ScrollUp => Some(Msg::Up),
            MouseEventKind::ScrollDown => Some(Msg::Down),
            MouseEventKind::Down(MouseButton::Left) => Some(Msg::ToggleHelp),
            _ => None,
        };
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
    use crate::app::{Editor, InitialData};
    use crate::msg::EditKey;
    use crate::node::NodeId;
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
            ..InitialData::default()
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
        assert!(map_key(&s, &key(KeyCode::Char('e'))).is_none());
        assert!(map_key(&s, &key(KeyCode::Tab)).is_none());
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

    fn editing() -> AppState {
        let mut s = state();
        s.editor = Some(Editor::new(NodeId::Source("ifc".into()), "file", "", false));
        s
    }

    #[test]
    fn the_editor_captures_typing() {
        let s = editing();
        // Letters that are shortcuts elsewhere are just text here.
        for c in ['q', 'e', 'x', '?', 'j', ' ', 'É'] {
            assert!(
                matches!(map_key(&s, &key(KeyCode::Char(c))), Some(Msg::Edit(EditKey::Char(got))) if got == c),
                "{c:?}"
            );
        }
        let shifted = KeyEvent::new(KeyCode::Char('A'), KeyModifiers::SHIFT);
        assert!(matches!(
            map_key(&s, &shifted),
            Some(Msg::Edit(EditKey::Char('A')))
        ));
    }

    #[test]
    fn the_editor_has_its_own_editing_keys() {
        let s = editing();
        let edit = |code| match map_key(&s, &key(code)) {
            Some(Msg::Edit(k)) => k,
            other => panic!("unexpected {other:?}"),
        };
        assert_eq!(edit(KeyCode::Enter), EditKey::Submit);
        assert_eq!(edit(KeyCode::Esc), EditKey::Cancel);
        assert_eq!(edit(KeyCode::Backspace), EditKey::Backspace);
        assert_eq!(edit(KeyCode::Delete), EditKey::Delete);
        assert_eq!(edit(KeyCode::Left), EditKey::Left);
        assert_eq!(edit(KeyCode::Right), EditKey::Right);
        assert_eq!(edit(KeyCode::Home), EditKey::Home);
        assert_eq!(edit(KeyCode::End), EditKey::End);
        let ctrl = |c| KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL);
        assert!(matches!(
            map_key(&s, &ctrl('u')),
            Some(Msg::Edit(EditKey::Clear))
        ));
        // Other control chords are not inserted as text.
        assert!(map_key(&s, &ctrl('w')).is_none());
        assert!(map_key(&s, &key(KeyCode::Tab)).is_none());
        // Ctrl-C still quits.
        assert!(matches!(map_key(&s, &ctrl('c')), Some(Msg::Quit)));
    }

    #[test]
    fn the_editor_ignores_the_mouse() {
        let s = editing();
        let click = mouse(MouseEventKind::Down(MouseButton::Left), 5, 5);
        assert!(map_mouse(&s, &click).is_none());
        assert!(map_mouse(&s, &mouse(MouseEventKind::ScrollDown, 5, 5)).is_none());
    }

    #[test]
    fn x_stops_the_running_task() {
        let s = state();
        assert!(matches!(
            map_key(&s, &key(KeyCode::Char('x'))),
            Some(Msg::StopTask)
        ));
    }

    #[test]
    fn help_can_be_scrolled() {
        let mut s = state();
        s.show_help = true;
        assert!(matches!(map_key(&s, &key(KeyCode::Down)), Some(Msg::Down)));
        assert!(matches!(
            map_key(&s, &key(KeyCode::Char('k'))),
            Some(Msg::Up)
        ));
        assert!(matches!(
            map_key(&s, &key(KeyCode::PageDown)),
            Some(Msg::PageDown)
        ));
        assert!(matches!(map_key(&s, &key(KeyCode::Home)), Some(Msg::Home)));
        assert!(matches!(map_key(&s, &key(KeyCode::End)), Some(Msg::End)));
        assert!(matches!(
            map_mouse(&s, &mouse(MouseEventKind::ScrollDown, 5, 5)),
            Some(Msg::Down)
        ));
        assert!(matches!(
            map_mouse(&s, &mouse(MouseEventKind::ScrollUp, 5, 5)),
            Some(Msg::Up)
        ));
    }
}
