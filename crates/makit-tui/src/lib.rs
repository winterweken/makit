//! ratatui-based TUI for makit.
//!
//! Elm architecture: [`app::AppState`] (model), [`msg::Msg`] (messages),
//! [`update::update`] (pure state transitions returning [`msg::Cmd`]s) and
//! [`view::view`] (rendering). [`runtime`] owns the event loop and executes
//! commands on worker threads.

pub mod app;
pub mod capture;
pub mod keymap;
pub mod layout;
pub mod msg;
pub mod node;
pub mod preview;
pub mod runtime;
pub mod task;
pub mod terminal;
pub mod theme;
pub mod tree;
pub mod tree_data;
pub mod update;
pub mod view;

use std::sync::mpsc;

use ratatui::layout::Rect;

/// Run the makit TUI until the user quits.
///
/// Tools must already be registered in the global registry.
pub fn run() -> anyhow::Result<()> {
    let (tx, rx) = mpsc::channel();
    let mut terminal = terminal::init()?;
    let capture = capture::StdoutCapture::start(tx.clone());

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
        runtime::run_loop(&mut terminal, &mut state, tx, rx)
    })();

    drop(capture);
    let restored = terminal::restore();
    // Report the loop's error first; only surface a restore failure otherwise.
    result?;
    restored?;
    Ok(())
}
