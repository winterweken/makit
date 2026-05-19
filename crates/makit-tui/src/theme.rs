//! Theme and styling for the makit TUI.

/// Color palette for the makit TUI theme.
pub struct Theme {
    pub primary: crossterm::style::Color,
    pub secondary: crossterm::style::Color,
    pub accent: crossterm::style::Color,
    pub text: crossterm::style::Color,
    pub dim: crossterm::style::Color,
    pub border: crossterm::style::Color,
}

impl Default for Theme {
    fn default() -> Self {
        use crossterm::style::Color;
        Self {
            primary: Color::Rgb { r: 90, g: 200, b: 250 },
            secondary: Color::Rgb { r: 180, g: 130, b: 255 },
            accent: Color::Rgb { r: 255, g: 180, b: 80 },
            text: Color::Rgb { r: 230, g: 230, b: 230 },
            dim: Color::Rgb { r: 100, g: 100, b: 100 },
            border: Color::Rgb { r: 60, g: 60, b: 70 },
        }
    }
}
