//! Small application-facing helpers for the shared light theme.

use gpui::Hsla;

/// The colors used by the shell's light interface.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UiPalette {
    /// Window background.
    pub app: Hsla,
    /// Sidebar and chrome surface.
    pub panel: Hsla,
    /// Editor surface.
    pub editor: Hsla,
    /// Primary text.
    pub text: Hsla,
    /// Secondary and placeholder text.
    pub muted: Hsla,
    /// Default border.
    pub border: Hsla,
    /// Primary interactive color.
    pub accent: Hsla,
    /// Yellow semantic color.
    pub yellow: Hsla,
    /// Purple semantic color.
    pub purple: Hsla,
}

impl UiPalette {
    /// Build the shell palette from the canonical compiled light theme.
    pub const fn light() -> Self {
        let theme = tachylyte_theme::light();
        Self {
            app: theme.app(),
            panel: theme.panel(),
            editor: theme.structured(),
            text: theme.text(),
            muted: theme.muted_text(),
            border: theme.border(),
            accent: theme.accent(),
            yellow: theme.yellow(),
            purple: theme.purple(),
        }
    }
}
