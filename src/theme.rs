//! nightshadeNeon theme for cyberia.
//!
//! The xvoidsx design language: neon pink accents, neon green borders,
//! cyan links, on near-black surfaces. Centralized here so the whole UI
//! speaks one visual language.

/// Raw nightshadeNeon palette.
pub mod palette {
    use ratatui::style::Color;

    pub const NEON_PINK: Color = Color::Rgb(255, 16, 240);
    pub const NEON_GREEN: Color = Color::Rgb(57, 255, 20);
    pub const CYAN: Color = Color::Rgb(0, 255, 255);
    pub const NEON_RED: Color = Color::Rgb(255, 49, 49);
    pub const WHITE: Color = Color::Rgb(255, 255, 255);
    pub const BLACK: Color = Color::Rgb(0, 0, 0);

    // surfaces
    pub const BG_DEEP: Color = Color::Rgb(10, 10, 10);
    pub const BG_PANEL: Color = Color::Rgb(17, 17, 17);
    pub const BG_SELECTED: Color = Color::Rgb(26, 26, 26);
    pub const BG_MUTED: Color = Color::Rgb(51, 51, 51);
    pub const FG_DIM: Color = Color::Rgb(160, 160, 160);
}

/// Semantic roles. Widgets should use these, never raw palette colors.
pub mod role {
    use super::palette;
    use ratatui::style::{Color, Modifier, Style};

    pub fn accent() -> Style {
        Style::default().fg(palette::NEON_PINK)
    }
    pub fn accent_bold() -> Style {
        Style::default()
            .fg(palette::NEON_PINK)
            .add_modifier(Modifier::BOLD)
    }
    pub fn border() -> Style {
        Style::default().fg(palette::NEON_GREEN)
    }
    pub fn text() -> Style {
        Style::default().fg(palette::WHITE)
    }
    pub fn dim() -> Style {
        Style::default().fg(palette::FG_DIM)
    }
    pub fn success() -> Style {
        Style::default().fg(palette::NEON_GREEN)
    }
    pub fn error() -> Style {
        Style::default()
            .fg(palette::NEON_RED)
            .add_modifier(Modifier::BOLD)
    }
    pub fn warning() -> Style {
        Style::default().fg(palette::NEON_RED)
    }
    pub fn link() -> Style {
        Style::default()
            .fg(palette::CYAN)
            .add_modifier(Modifier::UNDERLINED)
    }
    pub fn typing() -> Style {
        Style::default().fg(palette::CYAN)
    }
    pub fn loading() -> Style {
        Style::default().fg(palette::NEON_GREEN)
    }
    pub fn selected_bg() -> Style {
        Style::default().bg(palette::BG_SELECTED)
    }
    pub fn highlight_symbol() -> Style {
        Style::default()
            .fg(palette::NEON_PINK)
            .add_modifier(Modifier::BOLD)
    }

    /// The C Y B E R I A header: bold neon pink on reset background.
    pub fn brand() -> Style {
        Style::default()
            .fg(palette::NEON_PINK)
            .add_modifier(Modifier::BOLD)
    }

    /// Convenience: expose the raw colors for one-off use.
    pub const NEON_PINK: Color = palette::NEON_PINK;
    pub const NEON_GREEN: Color = palette::NEON_GREEN;
    pub const CYAN: Color = palette::CYAN;
    pub const NEON_RED: Color = palette::NEON_RED;
}
