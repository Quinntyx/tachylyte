//! Semantic accessors for [`crate::Palette`].
//!
//! The palette structs are useful when a caller wants to inspect the complete
//! token set.  These methods provide the flatter vocabulary generally used by
//! views, while retaining the palette as the single source of truth.

use gpui::Hsla;

use crate::Palette;

impl Palette {
    /// Application/window background.
    pub const fn app(&self) -> Hsla {
        self.background.app
    }
    /// Shell foreground used by the app window and navigation chrome.
    pub const fn app_foreground(&self) -> Hsla {
        self.text.normal
    }
    /// Default panel surface used by app sidebars, headers, and status chrome.
    pub const fn panel(&self) -> Hsla {
        self.sidebar.background
    }
    /// Title bar surface tokens.
    pub const fn titlebar(&self) -> &crate::SurfaceTokens {
        &self.titlebar
    }
    /// Sidebar/navigation surface tokens.
    pub const fn sidebar(&self) -> &crate::SurfaceTokens {
        &self.sidebar
    }
    /// Editor surface tokens.
    pub const fn editor(&self) -> &crate::EditorTokens {
        &self.editor
    }
    /// Status bar surface tokens.
    pub const fn status(&self) -> &crate::SurfaceTokens {
        &self.status
    }
    /// Modal surface tokens.
    pub const fn modal(&self) -> &crate::SurfaceTokens {
        &self.modal
    }
    /// Settings surface tokens.
    pub const fn settings(&self) -> &crate::SurfaceTokens {
        &self.settings
    }
    /// Launcher surface tokens.
    pub const fn launcher(&self) -> &crate::SurfaceTokens {
        &self.launcher
    }

    /// Main navigation background (an alias for the sidebar background).
    pub const fn navigation(&self) -> Hsla {
        self.sidebar.background
    }
    /// Navigation foreground for file rows and view labels.
    pub const fn navigation_text(&self) -> Hsla {
        self.sidebar.foreground
    }
    /// Navigation row hover surface.
    pub const fn navigation_hover(&self) -> Hsla {
        self.sidebar.hover
    }
    /// Navigation row active/selected surface.
    pub const fn navigation_active(&self) -> Hsla {
        self.sidebar.active
    }
    /// Structured-content background (the editor background).
    pub const fn structured(&self) -> Hsla {
        self.editor.background
    }
    /// Structured view foreground (Bases, properties, and metadata cells).
    pub const fn structured_text(&self) -> Hsla {
        self.editor.foreground
    }
    /// Canvas background (the elevated application surface).
    pub const fn canvas(&self) -> Hsla {
        self.background.elevated
    }
    /// Bases/database background (the secondary application surface).
    pub const fn bases(&self) -> Hsla {
        self.background.secondary
    }
    /// Canvas/Bases grid or code surface.
    pub const fn structured_border(&self) -> Hsla {
        self.borders.subtle
    }

    /// Primary application background.
    pub const fn background(&self) -> Hsla {
        self.background.primary
    }
    /// Elevated background surface.
    pub const fn elevated(&self) -> Hsla {
        self.background.elevated
    }
    /// Secondary background surface.
    pub const fn secondary(&self) -> Hsla {
        self.background.secondary
    }
    /// Code-block background.
    pub const fn code_background(&self) -> Hsla {
        self.background.code
    }
    /// Selection background.
    pub const fn selection(&self) -> Hsla {
        self.background.selection
    }

    /// Normal text.
    pub const fn text(&self) -> Hsla {
        self.text.normal
    }
    /// Generic text color alias for GPUI `.text_color(...)` call sites.
    pub const fn text_color(&self) -> Hsla {
        self.text.normal
    }
    /// Muted text.
    pub const fn muted_text(&self) -> Hsla {
        self.text.muted
    }
    /// Faint/placeholder text.
    pub const fn faint_text(&self) -> Hsla {
        self.text.faint
    }
    /// Text rendered on an accent surface.
    pub const fn on_accent(&self) -> Hsla {
        self.text.on_accent
    }
    /// Link text.
    pub const fn link(&self) -> Hsla {
        self.text.link
    }
    /// Heading/title text.
    pub const fn title_text(&self) -> Hsla {
        self.text.title
    }

    /// Subtle border.
    pub const fn border_subtle(&self) -> Hsla {
        self.borders.subtle
    }
    /// Default border.
    pub const fn border(&self) -> Hsla {
        self.borders.default
    }
    /// Strong border.
    pub const fn border_strong(&self) -> Hsla {
        self.borders.strong
    }
    /// Focus border.
    pub const fn focus_border(&self) -> Hsla {
        self.borders.focus
    }
    /// Hovered interactive surface.
    pub const fn hover(&self) -> Hsla {
        self.interactive.hover
    }
    /// Active interactive surface.
    pub const fn active(&self) -> Hsla {
        self.interactive.active
    }
    /// Selected interactive surface.
    pub const fn selected(&self) -> Hsla {
        self.interactive.selected
    }
    /// Disabled control color.
    pub const fn disabled(&self) -> Hsla {
        self.interactive.disabled
    }
    /// Keyboard focus ring.
    pub const fn focus_ring(&self) -> Hsla {
        self.interactive.focus_ring
    }

    /// Editor foreground.
    pub const fn editor_text(&self) -> Hsla {
        self.editor.foreground
    }
    /// Editor selection overlay.
    pub const fn editor_selection(&self) -> Hsla {
        self.background.selection
    }
    /// Editor heading color.
    pub const fn heading(&self) -> Hsla {
        self.editor.heading
    }
    /// Editor cursor color.
    pub const fn cursor(&self) -> Hsla {
        self.editor.cursor
    }
    /// Current-line highlight.
    pub const fn line_highlight(&self) -> Hsla {
        self.editor.line_highlight
    }

    /// Accent color.
    pub const fn accent(&self) -> Hsla {
        self.accent
    }
    /// Accent hover color.
    pub const fn accent_hover(&self) -> Hsla {
        self.accent_hover
    }
    /// Accent active color.
    pub const fn accent_active(&self) -> Hsla {
        self.accent_active
    }
    /// Red semantic color.
    pub const fn red(&self) -> Hsla {
        self.red
    }
    /// Orange semantic color.
    pub const fn orange(&self) -> Hsla {
        self.orange
    }
    /// Yellow semantic color.
    pub const fn yellow(&self) -> Hsla {
        self.yellow
    }
    /// Green semantic color.
    pub const fn green(&self) -> Hsla {
        self.green
    }
    /// Cyan semantic color.
    pub const fn cyan(&self) -> Hsla {
        self.cyan
    }
    /// Blue semantic color.
    pub const fn blue(&self) -> Hsla {
        self.blue
    }
    /// Purple semantic color.
    pub const fn purple(&self) -> Hsla {
        self.purple
    }

    /// The semantic surface used by command launchers and quick switchers.
    pub const fn launcher_background(&self) -> Hsla {
        self.launcher.background
    }
    /// The semantic surface used by dialogs and command palettes.
    pub const fn modal_background(&self) -> Hsla {
        self.modal.background
    }
    /// The semantic surface used by the status bar.
    pub const fn status_background(&self) -> Hsla {
        self.status.background
    }
}
