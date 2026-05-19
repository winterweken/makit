//! Theme and styling for the makit TUI.

use tui::prelude::Color;

/// Color palette for the makit TUI theme.
pub struct Theme {
    pub primary: Color,
    pub secondary: Color,
    pub accent: Color,
    pub text: Color,
    pub dim: Color,
    pub border: Color,
}

/// Default dark theme palette.
pub const THEME: Theme = Theme {
    primary: Color::Rgb(90, 200, 250),
    secondary: Color::Rgb(180, 130, 255),
    accent: Color::Rgb(255, 180, 80),
    text: Color::Rgb(230, 230, 230),
    dim: Color::Rgb(100, 100, 100),
    border: Color::Rgb(60, 60, 70),
};
