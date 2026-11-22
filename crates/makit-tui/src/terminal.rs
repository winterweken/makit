//! Terminal setup and teardown.
//!
//! The TUI renders to **stderr** so stdout can be captured into the Output
//! pane (task handlers print with `println!`).

use std::io::{self, Stderr};
use std::panic;
use std::sync::Once;
use std::thread;

use crossterm::cursor;
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

/// The terminal type used by the TUI.
pub type Tui = Terminal<CrosstermBackend<Stderr>>;

/// Name prefix for background threads spawned by the TUI.
pub const WORKER_THREAD_PREFIX: &str = "makit-worker-";

/// Enter raw mode + alternate screen with mouse capture, and install a panic
/// hook that restores the terminal before the panic message is printed.
pub fn init() -> io::Result<Tui> {
    install_panic_hook();
    enable_raw_mode()?;
    if let Err(err) = execute!(io::stderr(), EnterAlternateScreen, EnableMouseCapture) {
        let _ = restore();
        return Err(err);
    }
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stderr()))?;
    terminal.clear()?;
    Ok(terminal)
}

/// Leave the alternate screen and raw mode. Safe to call more than once.
pub fn restore() -> io::Result<()> {
    let raw = disable_raw_mode();
    let screen = execute!(
        io::stderr(),
        DisableMouseCapture,
        LeaveAlternateScreen,
        cursor::Show
    );
    raw.and(screen)
}

fn install_panic_hook() {
    static INSTALL: Once = Once::new();
    INSTALL.call_once(|| {
        let previous = panic::take_hook();
        panic::set_hook(Box::new(move |info| {
            if is_worker_thread() {
                // Worker panics are caught and reported as task failures.
                // Print via stdout so the message lands in the Output pane
                // instead of over the UI.
                println!("⚠ {info}");
                return;
            }
            let _ = restore();
            previous(info);
        }));
    });
}

fn is_worker_thread() -> bool {
    thread::current()
        .name()
        .is_some_and(|name| name.starts_with(WORKER_THREAD_PREFIX))
}
