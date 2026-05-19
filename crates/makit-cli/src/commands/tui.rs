//! `makit tui` — launch the interactive TUI.

use anyhow::Result;

pub fn run() -> Result<()> {
    let mut app = makit_tui::App::new();
    app.run()
}
