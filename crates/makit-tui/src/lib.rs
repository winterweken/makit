//! ratatui-based TUI for makit.
//!
//! Elm architecture: [`app::AppState`] (model), [`msg::Msg`] (messages),
//! [`update::update`] (pure state transitions returning [`msg::Cmd`]s) and
//! [`view::view`] (rendering). [`runtime`] owns the event loop and executes
//! commands on worker threads.

pub mod app;
pub mod capture;
pub mod help;
pub mod keymap;
pub mod layout;
pub mod logo;
pub mod msg;
pub mod node;
pub mod options;
pub mod preview;
pub mod runtime;
pub mod signals;
pub mod task;
pub mod terminal;
pub mod theme;
pub mod tree;
pub mod tree_data;
pub mod update;
pub mod view;

use std::io::{self, IsTerminal};
use std::sync::mpsc;

use ratatui::layout::Rect;

/// Run the makit TUI until the user quits.
///
/// Tools must already be registered in the global registry.
pub fn run() -> anyhow::Result<()> {
    terminal::ensure_interactive(io::stdin().is_terminal(), io::stderr().is_terminal())?;

    let (tx, rx) = mpsc::channel();
    // Without handlers a signal still ends the process, just less tidily.
    let signals = signals::Signals::watch().unwrap_or_else(|_| signals::Signals::none());
    let mut terminal = terminal::init()?;
    let mailbox = capture::Mailbox::default();
    let capture = capture::StdoutCapture::start(tx.clone(), mailbox.clone());

    // Everything after init() must fall through to restore(), even on error.
    let result = (|| {
        let size = terminal.size()?;
        let mut state = app::AppState::new(app::load_initial_data(Rect::new(
            0,
            0,
            size.width,
            size.height,
        )));
        if let Err(err) = &capture {
            state.status = format!(
                "⚠ Output capture unavailable ({err}) — task output may disturb the screen"
            );
        }
        // Results follow the output stream only while there is one.
        let mut fx = runtime::Effects::new(tx, capture.is_ok().then_some(mailbox));
        runtime::run_loop(&mut terminal, &mut state, &mut fx, &rx, &signals)
    })();

    // `rx` outlives the capture so its reader thread never finds the
    // channel closed while stdout still points at the pipe.
    drop(capture);
    drop(rx);
    let restored = terminal::restore();
    // Report the loop's error first; only surface a restore failure otherwise.
    let exit = result?;
    restored?;
    if let runtime::Exit::Signal(signal) = exit {
        signals::die(signal);
    }
    Ok(())
}
