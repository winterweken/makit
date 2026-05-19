//! Main TUI application.
//!
//! Scaffolded for rsille's tui::app framework — will be fleshed out in Phase 4.

/// The makit TUI application.
pub struct App {
    pub running: bool,
}

impl App {
    pub fn new() -> Self {
        Self { running: true }
    }

    /// Run the TUI — placeholder until rsille TUI integration in Phase 4.
    pub fn run(&mut self) -> anyhow::Result<()> {
        println!("makit TUI — coming soon (rsille native)");
        Ok(())
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}
