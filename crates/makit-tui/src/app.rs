//! Main TUI application using rsille's Elm-like architecture.
//!
//! State + Msg + update + view pattern, using inline rendering mode.
//! The canvas preview cycles between context-sensitive views based on
//! the currently-selected tree node.

use std::time::Duration;

use canvas::Canvas;
use tui::prelude::*;

use crate::theme::THEME;
use crate::tree_data::build_tree_items;
use makit_geometry::drawing::{draw_rect, fill_rect, draw_arrow};

// ---------------------------------------------------------------------------
// Messages
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum Msg {
    /// Tree explorer: node was highlighted
    TreeFocused(String),
    /// Tree explorer: node was opened (Enter)
    TreeOpened(String),
    /// Toggle help overlay
    ToggleHelp,
    /// Animation frame tick
    Tick,
}

// ---------------------------------------------------------------------------
// Application State
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct AppState {
    /// Currently highlighted node in the tree
    active_node: String,
    /// Last opened (Enter) node
    opened_node: String,
    /// Show help panel
    show_help: bool,
    /// Logo rotation angle (radians)
    logo_angle: f64,
    /// Status message
    status: String,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            active_node: String::new(),
            opened_node: String::new(),
            show_help: false,
            logo_angle: 0.0,
            status: "Ready — use ↑↓ to navigate, → to expand".to_string(),
        }
    }
}

// ---------------------------------------------------------------------------
// Update
// ---------------------------------------------------------------------------

fn update(state: &mut AppState, msg: Msg) {
    match msg {
        Msg::TreeFocused(id) => {
            state.active_node = id;
        }
        Msg::TreeOpened(id) => {
            state.status = format!("▸ Opened: {}", id);
            state.opened_node = id;
        }
        Msg::ToggleHelp => {
            state.show_help = !state.show_help;
        }
        Msg::Tick => {
            state.logo_angle += 0.04;
        }
    }
}

// ---------------------------------------------------------------------------
// View
// ---------------------------------------------------------------------------

fn view(state: &AppState) -> impl Widget<Msg> {
    col::<Msg>()
        .gap(0)
        .child(header_bar())
        .child(main_content(state))
        .child(status_bar(state))
}

/// ─── Header bar ────────────────────────────────────
fn header_bar() -> impl Widget<Msg> {
    row::<Msg>()
        .padding(Padding::new(0, 1, 0, 1))
        .style(Style::default().bg(Color::Rgb(16, 18, 26)))
        .child(label::<Msg>("⬡").bold().fg(THEME.accent))
        .child(label::<Msg>(" makit ").bold().fg(THEME.primary))
        .child(spacer::<Msg>().fill())
        .child(label::<Msg>("Esc·quit").fg(THEME.dim))
        .child(label::<Msg>("  ?·help").fg(THEME.dim))
        .child(label::<Msg>("  Tab·focus").fg(THEME.dim))
}

/// ─── Main content ──────────────────────────────────
fn main_content(state: &AppState) -> impl Widget<Msg> {
    let tree_items = build_tree_items();

    let explorer = tree::<Msg>()
        .key("explorer")
        .height(20)
        .border(BorderStyle::Rounded)
        .items(tree_items)
        .on_change(Msg::TreeFocused)
        .on_submit(Msg::TreeOpened);

    let left_pane = col::<Msg>()
        .border(BorderStyle::Rounded)
        .padding(Padding::new(0, 1, 0, 1))
        .child(
            row::<Msg>()
                .child(label::<Msg>("⊟").fg(THEME.dim))
                .child(label::<Msg>(" Explorer").bold().fg(THEME.accent))
        )
        .child(divider::<Msg>().variant(DividerVariant::Dotted))
        .child(explorer);

    let right_pane = if state.show_help {
        help_panel()
    } else {
        detail_panel(state)
    };

    row::<Msg>()
        .gap(1)
        .child(left_pane)
        .child(right_pane)
}

/// ─── Detail panel ──────────────────────────────────
fn detail_panel(state: &AppState) -> Flex<Msg> {
    let canvas_lines = render_canvas_lines(&state.active_node, state.logo_angle);

    let active_display = if state.active_node.is_empty() {
        "—".to_string()
    } else {
        // Show just the meaningful part of the node id
        state.active_node
            .rsplit(':')
            .next()
            .unwrap_or(&state.active_node)
            .to_string()
    };

    let opened_display = if state.opened_node.is_empty() {
        "—".to_string()
    } else {
        state.opened_node
            .rsplit(':')
            .next()
            .unwrap_or(&state.opened_node)
            .to_string()
    };

    let mut panel = col::<Msg>()
        .border(BorderStyle::Rounded)
        .padding(Padding::new(0, 1, 0, 1))
        .gap(0)
        .child(
            row::<Msg>()
                .child(label::<Msg>("◈").fg(THEME.dim))
                .child(label::<Msg>(" Detail").bold().fg(THEME.secondary))
        )
        .child(divider::<Msg>().variant(DividerVariant::Dotted))
        .child(
            row::<Msg>()
                .gap(1)
                .child(label::<Msg>("Focus:").fg(THEME.dim))
                .child(label::<Msg>(active_display).bold().fg(THEME.text))
        )
        .child(
            row::<Msg>()
                .gap(1)
                .child(label::<Msg>("Open:").fg(THEME.dim))
                .child(label::<Msg>(opened_display).fg(THEME.dim))
        )
        .child(divider::<Msg>().text("Preview"));

    // Render each line of the braille canvas as a separate label
    for line in &canvas_lines {
        panel = panel.child(label::<Msg>(line.clone()).fg(THEME.accent));
    }

    panel
        .child(divider::<Msg>().text("Actions"))
        .child(
            row::<Msg>()
                .gap(2)
                .child(
                    button::<Msg>("Execute")
                        .variant(ButtonVariant::Primary)
                        .on_click(|| Msg::TreeOpened("execute".to_owned())),
                )
                .child(
                    button::<Msg>("Help (?)")
                        .variant(ButtonVariant::Secondary)
                        .on_click(|| Msg::ToggleHelp),
                ),
        )
}

/// ─── Help panel ────────────────────────────────────
fn help_panel() -> Flex<Msg> {
    col::<Msg>()
        .border(BorderStyle::Rounded)
        .padding(Padding::uniform(1))
        .gap(0)
        .child(
            row::<Msg>()
                .child(label::<Msg>("◈").fg(THEME.dim))
                .child(label::<Msg>(" Help").bold().fg(THEME.accent))
        )
        .child(divider::<Msg>().variant(DividerVariant::Dotted))
        .child(label::<Msg>(""))
        .child(
            row::<Msg>().gap(1)
                .child(label::<Msg>("Tab").bold().fg(THEME.primary))
                .child(label::<Msg>("Switch focus between panes"))
        )
        .child(
            row::<Msg>().gap(1)
                .child(label::<Msg>("↑ ↓").bold().fg(THEME.primary))
                .child(label::<Msg>("Navigate tree items"))
        )
        .child(
            row::<Msg>().gap(1)
                .child(label::<Msg>("→  ").bold().fg(THEME.primary))
                .child(label::<Msg>("Expand tree node"))
        )
        .child(
            row::<Msg>().gap(1)
                .child(label::<Msg>("←  ").bold().fg(THEME.primary))
                .child(label::<Msg>("Collapse tree node"))
        )
        .child(
            row::<Msg>().gap(1)
                .child(label::<Msg>("⏎  ").bold().fg(THEME.primary))
                .child(label::<Msg>("Open / execute item"))
        )
        .child(
            row::<Msg>().gap(1)
                .child(label::<Msg>("?  ").bold().fg(THEME.primary))
                .child(label::<Msg>("Toggle this panel"))
        )
        .child(
            row::<Msg>().gap(1)
                .child(label::<Msg>("Esc").bold().fg(THEME.primary))
                .child(label::<Msg>("Quit"))
        )
        .child(label::<Msg>(""))
        .child(divider::<Msg>().text("Legend"))
        .child(
            row::<Msg>().gap(1)
                .child(label::<Msg>("◆").fg(THEME.accent))
                .child(label::<Msg>("Sources — geometry input drivers"))
        )
        .child(
            row::<Msg>().gap(1)
                .child(label::<Msg>("▸").fg(THEME.accent))
                .child(label::<Msg>("Actions — operations on geometry"))
        )
        .child(
            row::<Msg>().gap(1)
                .child(label::<Msg>("⊞").fg(THEME.accent))
                .child(label::<Msg>("Groups — analysis · extraction · reporting"))
        )
        .child(label::<Msg>(""))
        .child(
            button::<Msg>("Close Help")
                .variant(ButtonVariant::Secondary)
                .on_click(|| Msg::ToggleHelp),
        )
}

/// ─── Status bar ────────────────────────────────────
fn status_bar(state: &AppState) -> impl Widget<Msg> {
    row::<Msg>()
        .padding(Padding::new(0, 1, 0, 1))
        .style(Style::default().bg(Color::Rgb(16, 18, 26)))
        .child(label::<Msg>(state.status.clone()).fg(THEME.dim))
        .child(spacer::<Msg>().fill())
        .child(label::<Msg>("makit v0.1.0").fg(Color::Rgb(60, 62, 70)))
        .child(label::<Msg>(" · ").fg(Color::Rgb(40, 42, 50)))
        .child(label::<Msg>("rsille").fg(Color::Rgb(60, 62, 70)))
}

// ---------------------------------------------------------------------------
// Canvas preview rendering
// ---------------------------------------------------------------------------

/// Render a braille canvas for the current context, return individual lines.
fn render_canvas_lines(active_node: &str, angle: f64) -> Vec<String> {
    let mut c = Canvas::new();

    if active_node.contains("murb") {
        render_energy_bars(&mut c);
    } else if active_node.contains("wall") {
        render_walls_preview(&mut c);
    } else if active_node.contains("floor") || active_node.contains("room") {
        render_floor_plan(&mut c);
    } else {
        render_logo(&mut c, angle);
    }

    let mut buf = Vec::new();
    c.print_on(&mut buf, false).unwrap_or_default();
    let output = String::from_utf8(buf).unwrap_or_default();
    output.lines().map(|l| l.to_string()).collect()
}

/// Rotating hexagonal logo with inner counter-rotation and spokes
fn render_logo(c: &mut Canvas, angle: f64) {
    let cx = 20.0;
    let cy = 16.0;
    let r = 12.0;
    let sides = 6;

    // Outer hexagon
    for i in 0..sides {
        let a1 = angle + (i as f64) * std::f64::consts::TAU / sides as f64;
        let a2 = angle + ((i + 1) as f64) * std::f64::consts::TAU / sides as f64;
        c.line(
            (cx + r * a1.cos(), cy + r * a1.sin()),
            (cx + r * a2.cos(), cy + r * a2.sin()),
        );
    }

    // Inner hexagon (counter-rotating)
    let r2 = r * 0.55;
    for i in 0..sides {
        let a1 = -angle + (i as f64) * std::f64::consts::TAU / sides as f64;
        let a2 = -angle + ((i + 1) as f64) * std::f64::consts::TAU / sides as f64;
        c.line(
            (cx + r2 * a1.cos(), cy + r2 * a1.sin()),
            (cx + r2 * a2.cos(), cy + r2 * a2.sin()),
        );
    }

    // Spokes from inner to outer vertices
    for i in 0..sides {
        let a = angle + (i as f64) * std::f64::consts::TAU / sides as f64;
        c.line(
            (cx + r2 * (-a).cos(), cy + r2 * (-a).sin()),
            (cx + r * a.cos(), cy + r * a.sin()),
        );
    }
}

/// Wall orientation preview — building envelope with interior walls
fn render_walls_preview(c: &mut Canvas) {
    // Envelope
    c.line((5.0, 28.0), (35.0, 28.0));
    c.line((35.0, 28.0), (35.0, 4.0));
    c.line((35.0, 4.0), (5.0, 4.0));
    c.line((5.0, 4.0), (5.0, 28.0));
    // Interior
    c.line((20.0, 4.0), (20.0, 20.0));
    c.line((5.0, 16.0), (20.0, 16.0));
    // Cardinal arrows
    draw_arrow(c, 20.0, 32.0, 20.0, 36.0);
    draw_arrow(c, 40.0, 16.0, 44.0, 16.0);
}

/// Energy bar chart (monthly heating demand)
fn render_energy_bars(c: &mut Canvas) {
    let months = [28, 24, 18, 10, 5, 2, 1, 2, 6, 14, 22, 26];
    for (i, height) in months.iter().enumerate() {
        let x = (i as i32) * 3 + 2;
        fill_rect(c, x, 0, 2, *height);
    }
    // Axes
    c.line((0.0, 0.0), (40.0, 0.0));
    c.line((0.0, 0.0), (0.0, 32.0));
}

/// Floor plan preview — rooms with door openings
fn render_floor_plan(c: &mut Canvas) {
    draw_rect(c, 2.0, 2.0, 36.0, 28.0);
    c.line((20.0, 2.0), (20.0, 22.0));
    c.line((2.0, 18.0), (36.0, 18.0));
    c.line((20.0, 24.0), (20.0, 30.0));
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Run the makit TUI.
pub fn run() -> anyhow::Result<()> {
    // Clear terminal before starting for a clean slate
    print!("\x1B[2J\x1B[H");

    let app = App::new(AppState::default())
        .on_key(KeyCode::Char('?'), || Msg::ToggleHelp)
        .on_tick(Duration::from_millis(80), || Msg::Tick);

    app.run_inline(update, view)
        .map_err(|e| anyhow::anyhow!("TUI error: {}", e))?;

    Ok(())
}
