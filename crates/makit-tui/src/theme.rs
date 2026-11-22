//! Theme and styling for the makit TUI.

use ratatui::style::Color;

/// Color palette for the makit TUI theme.
pub struct Theme {
    pub primary: Color,
    pub secondary: Color,
    pub accent: Color,
    pub text: Color,
    pub dim: Color,
    pub faint: Color,
    pub border: Color,
    pub bar_bg: Color,
    pub selection_bg: Color,
    pub success: Color,
    pub error: Color,
}

/// Default dark theme palette (same RGB values as the original rsille TUI).
pub const THEME: Theme = Theme {
    primary: Color::Rgb(90, 200, 250),
    secondary: Color::Rgb(180, 130, 255),
    accent: Color::Rgb(255, 180, 80),
    text: Color::Rgb(230, 230, 230),
    dim: Color::Rgb(100, 100, 100),
    faint: Color::Rgb(60, 62, 70),
    border: Color::Rgb(60, 60, 70),
    bar_bg: Color::Rgb(16, 18, 26),
    selection_bg: Color::Rgb(38, 44, 62),
    success: Color::Rgb(120, 210, 140),
    error: Color::Rgb(240, 110, 110),
};
