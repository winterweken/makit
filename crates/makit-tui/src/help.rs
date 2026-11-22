//! Help overlay content, shared by `view` (drawing) and `layout` (sizing
//! and scroll limits).

/// One line of the help overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HelpLine {
    Blank,
    /// A key (or key group) and what it does.
    Key(&'static str, &'static str),
    /// A section divider.
    Section(&'static str),
    /// A tree icon and what it marks.
    Legend(&'static str, &'static str),
}

/// The help text, top to bottom.
pub const LINES: &[HelpLine] = &[
    HelpLine::Blank,
    HelpLine::Key("↑ ↓  j k", "Move selection"),
    HelpLine::Key("PgUp PgDn", "Page up / down"),
    HelpLine::Key("Home End", "First / last item"),
    HelpLine::Key("→  l", "Expand node"),
    HelpLine::Key("←  h", "Collapse node / go to parent"),
    HelpLine::Key("⏎", "Open node · edit option · load IFC file"),
    HelpLine::Key("e", "Run source or action (asks for required options)"),
    HelpLine::Key("x", "Stop the running task, if it can be stopped"),
    HelpLine::Key("Tab", "Cycle focus: Explorer → IFC files → Output"),
    HelpLine::Key("o", "Toggle the Output pane"),
    HelpLine::Key("c", "Clear Output (when focused)"),
    HelpLine::Key("?", "Toggle this help"),
    HelpLine::Key("Esc  q", "Close help · quit"),
    HelpLine::Key("Mouse", "Wheel scrolls · click selects / opens"),
    HelpLine::Blank,
    HelpLine::Section("Editing an option"),
    HelpLine::Key("⏎", "Save · an empty value restores the default"),
    HelpLine::Key("Esc", "Cancel"),
    HelpLine::Key("Ctrl-U", "Clear the field"),
    HelpLine::Blank,
    HelpLine::Section("Legend"),
    HelpLine::Legend("◆", "Sources — geometry input drivers"),
    HelpLine::Legend("▸", "Actions — operations on geometry"),
    HelpLine::Legend("⊞", "Groups — analysis · extraction · reporting"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_binding_is_documented() {
        let keys: Vec<&str> = LINES
            .iter()
            .filter_map(|line| match line {
                HelpLine::Key(key, _) => Some(*key),
                _ => None,
            })
            .collect();
        for key in ["e", "x", "o", "c", "?", "Tab", "⏎", "Ctrl-U"] {
            assert!(keys.contains(&key), "{key} is missing from the help");
        }
    }
}
