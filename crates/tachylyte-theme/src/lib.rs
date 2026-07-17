//! Native design tokens for the Obsidian 1.12.7 light theme.
//!
//! This crate intentionally contains no CSS parser or runtime theme loader.  The
//! values below are the compiled `.theme-light` palette and are exposed as GPUI
//! [`Hsla`] values for use by native views.

use gpui::Hsla;
use serde::{Deserialize, Serialize};

/// Flat semantic accessors for native view integrations.
pub mod accessors;
/// Versioned persistence helpers for appearance settings.
pub mod appearance;
/// Allocation-free packed-color conversion helpers.
pub mod color;

pub use appearance::{decode_appearance, encode_appearance, CURRENT_VERSION};
pub use color::{hex_rgb, hex_rgba, rgba8, HslaColorExt};

/// The requested appearance mode. `System` is resolved by the host application.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub enum ThemeKind {
    #[default]
    Light,
    Dark,
    System,
}

impl ThemeKind {
    /// Resolve a system preference (`true` means dark) into a concrete kind.
    pub const fn resolve(self, system_is_dark: bool) -> Self {
        match self {
            Self::System => {
                if system_is_dark {
                    Self::Dark
                } else {
                    Self::Light
                }
            }
            concrete => concrete,
        }
    }

    /// Alias useful to platform adapters that only expose a system preference.
    pub const fn resolve_system(self, system_is_dark: bool) -> Self {
        self.resolve(system_is_dark)
    }

    /// Resolve the kind and return its compiled token set.
    pub const fn tokens(self, system_is_dark: bool) -> &'static Palette {
        match self.resolve(system_is_dark) {
            Self::Dark => &DARK,
            _ => &LIGHT,
        }
    }
}

/// Persisted, serde-friendly appearance preferences.  Rendering tokens are not
/// persisted, so a future palette can be adopted without migrating settings.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct AppearanceSettings {
    pub theme: ThemeKind,
    pub font_size: f32,
    pub interface_scale: f32,
    pub reduced_motion: bool,
}

impl Default for AppearanceSettings {
    fn default() -> Self {
        Self {
            theme: ThemeKind::Light,
            font_size: 14.0,
            interface_scale: 1.0,
            reduced_motion: false,
        }
    }
}

impl AppearanceSettings {
    pub fn resolved_kind(&self, system_is_dark: bool) -> ThemeKind {
        self.theme.resolve(system_is_dark)
    }
    pub fn tokens(&self, system_is_dark: bool) -> &'static Palette {
        self.theme.tokens(system_is_dark)
    }
}

/// A complete compiled palette, grouped by the surfaces where tokens are used.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    pub base00: Hsla,
    pub base05: Hsla,
    pub base10: Hsla,
    pub base20: Hsla,
    pub base25: Hsla,
    pub base30: Hsla,
    pub base35: Hsla,
    pub base40: Hsla,
    pub base50: Hsla,
    pub base60: Hsla,
    pub base70: Hsla,
    pub base100: Hsla,
    pub accent: Hsla,
    pub accent_hover: Hsla,
    pub accent_active: Hsla,
    pub red: Hsla,
    pub orange: Hsla,
    pub yellow: Hsla,
    pub green: Hsla,
    pub cyan: Hsla,
    pub blue: Hsla,
    pub purple: Hsla,
    pub background: BackgroundTokens,
    pub borders: BorderTokens,
    pub text: TextTokens,
    pub interactive: InteractiveTokens,
    pub titlebar: SurfaceTokens,
    pub sidebar: SurfaceTokens,
    pub editor: EditorTokens,
    pub status: SurfaceTokens,
    pub modal: SurfaceTokens,
    pub settings: SurfaceTokens,
    pub launcher: SurfaceTokens,
    pub spacing: SpacingTokens,
    pub radius: RadiusTokens,
    pub font_size: FontSizeTokens,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BackgroundTokens {
    pub app: Hsla,
    pub primary: Hsla,
    pub secondary: Hsla,
    pub elevated: Hsla,
    pub code: Hsla,
    pub selection: Hsla,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BorderTokens {
    pub subtle: Hsla,
    pub default: Hsla,
    pub strong: Hsla,
    pub focus: Hsla,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextTokens {
    pub normal: Hsla,
    pub muted: Hsla,
    pub faint: Hsla,
    pub on_accent: Hsla,
    pub link: Hsla,
    pub title: Hsla,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InteractiveTokens {
    pub hover: Hsla,
    pub active: Hsla,
    pub selected: Hsla,
    pub disabled: Hsla,
    pub focus_ring: Hsla,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SurfaceTokens {
    pub background: Hsla,
    pub foreground: Hsla,
    pub border: Hsla,
    pub hover: Hsla,
    pub active: Hsla,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EditorTokens {
    pub background: Hsla,
    pub foreground: Hsla,
    pub heading: Hsla,
    pub cursor: Hsla,
    pub line_highlight: Hsla,
    pub code: Hsla,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpacingTokens {
    pub xs: f32,
    pub sm: f32,
    pub md: f32,
    pub lg: f32,
    pub xl: f32,
    pub xxl: f32,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RadiusTokens {
    pub none: f32,
    pub sm: f32,
    pub md: f32,
    pub lg: f32,
    pub pill: f32,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FontSizeTokens {
    pub xs: f32,
    pub sm: f32,
    pub md: f32,
    pub lg: f32,
    pub xl: f32,
    pub xxl: f32,
    pub title: f32,
}

const fn c(h: f32, s: f32, l: f32) -> Hsla {
    Hsla {
        h: h / 360.0,
        s: s / 100.0,
        l: l / 100.0,
        a: 1.0,
    }
}
const fn rgba(h: f32, s: f32, l: f32, a: f32) -> Hsla {
    Hsla {
        h: h / 360.0,
        s: s / 100.0,
        l: l / 100.0,
        a,
    }
}

const LIGHT: Palette = Palette {
    base00: hex_rgb(0xffffff),
    base05: hex_rgb(0xfcfcfc),
    base10: hex_rgb(0xfafafa),
    base20: hex_rgb(0xf6f6f6),
    base25: hex_rgb(0xe3e3e3),
    base30: hex_rgb(0xe0e0e0),
    base35: hex_rgb(0xd4d4d4),
    base40: hex_rgb(0xbdbdbd),
    base50: hex_rgb(0xababab),
    base60: hex_rgb(0x707070),
    base70: hex_rgb(0x5c5c5c),
    base100: hex_rgb(0x222222),
    accent: c(258.0, 88.0, 66.0),
    accent_hover: c(255.0, 89.76, 75.9),
    accent_active: c(257.0, 88.88, 70.95),
    red: hex_rgb(0xe93147),
    orange: hex_rgb(0xec7500),
    yellow: hex_rgb(0xe0ac00),
    green: hex_rgb(0x08b94e),
    cyan: hex_rgb(0x00bfbc),
    blue: hex_rgb(0x086ddd),
    purple: hex_rgb(0x7852ee),
    background: BackgroundTokens {
        app: hex_rgb(0xffffff),
        primary: hex_rgb(0xffffff),
        secondary: hex_rgb(0xf6f6f6),
        elevated: hex_rgb(0xfcfcfc),
        code: hex_rgb(0xfafafa),
        selection: rgba(258.0, 88.0, 66.0, 0.2),
    },
    borders: BorderTokens {
        subtle: hex_rgb(0xe0e0e0),
        default: hex_rgb(0xe0e0e0),
        strong: hex_rgb(0xd4d4d4),
        focus: hex_rgb(0xbdbdbd),
    },
    text: TextTokens {
        normal: hex_rgb(0x222222),
        muted: hex_rgb(0x5c5c5c),
        faint: hex_rgb(0xababab),
        on_accent: c(0.0, 0.0, 100.0),
        link: c(215.0, 94.0, 45.0),
        title: hex_rgb(0x222222),
    },
    interactive: InteractiveTokens {
        hover: hex_rgb(0xfafafa),
        active: rgba(258.0, 88.0, 66.0, 0.1),
        selected: rgba(258.0, 88.0, 66.0, 0.15),
        disabled: hex_rgb(0xbdbdbd),
        focus_ring: hex_rgb(0xbdbdbd),
    },
    titlebar: SurfaceTokens {
        background: c(0.0, 0.0, 98.0),
        foreground: c(0.0, 0.0, 18.0),
        border: c(0.0, 0.0, 88.0),
        hover: c(0.0, 0.0, 94.0),
        active: c(0.0, 0.0, 90.0),
    },
    sidebar: SurfaceTokens {
        background: c(0.0, 0.0, 96.0),
        foreground: c(0.0, 0.0, 30.0),
        border: c(0.0, 0.0, 88.0),
        hover: c(0.0, 0.0, 92.0),
        active: rgba(258.0, 88.0, 66.0, 0.14),
    },
    editor: EditorTokens {
        background: c(0.0, 0.0, 100.0),
        foreground: c(0.0, 0.0, 18.0),
        heading: c(258.0, 88.0, 48.0),
        cursor: c(258.0, 88.0, 66.0),
        line_highlight: c(0.0, 0.0, 98.0),
        code: c(0.0, 0.0, 94.0),
    },
    status: SurfaceTokens {
        background: c(0.0, 0.0, 96.0),
        foreground: c(0.0, 0.0, 44.0),
        border: c(0.0, 0.0, 88.0),
        hover: c(0.0, 0.0, 92.0),
        active: c(0.0, 0.0, 88.0),
    },
    modal: SurfaceTokens {
        background: c(0.0, 0.0, 100.0),
        foreground: c(0.0, 0.0, 18.0),
        border: c(0.0, 0.0, 86.0),
        hover: c(0.0, 0.0, 96.0),
        active: c(0.0, 0.0, 92.0),
    },
    settings: SurfaceTokens {
        background: c(0.0, 0.0, 100.0),
        foreground: c(0.0, 0.0, 18.0),
        border: c(0.0, 0.0, 88.0),
        hover: c(0.0, 0.0, 96.0),
        active: c(0.0, 0.0, 92.0),
    },
    launcher: SurfaceTokens {
        background: c(0.0, 0.0, 100.0),
        foreground: c(0.0, 0.0, 18.0),
        border: c(0.0, 0.0, 86.0),
        hover: c(0.0, 0.0, 96.0),
        active: c(0.0, 0.0, 92.0),
    },
    spacing: SpacingTokens {
        xs: 4.0,
        sm: 8.0,
        md: 12.0,
        lg: 16.0,
        xl: 24.0,
        xxl: 32.0,
    },
    radius: RadiusTokens {
        none: 0.0,
        sm: 3.0,
        md: 5.0,
        lg: 8.0,
        pill: 999.0,
    },
    font_size: FontSizeTokens {
        xs: 11.0,
        sm: 12.0,
        md: 14.0,
        lg: 16.0,
        xl: 20.0,
        xxl: 24.0,
        title: 18.0,
    },
};

/// The Obsidian light palette (the public canonical token set).
pub const OBSIDIAN_LIGHT: Palette = LIGHT;

/// Return the canonical light token set.
pub const fn light() -> &'static Palette {
    &OBSIDIAN_LIGHT
}

/// Return the basic dark fallback token set.
pub const fn dark() -> &'static Palette {
    &DARK
}

/// A restrained dark fallback for `ThemeKind::Dark`; it keeps the same API while
/// the light theme remains the parity target.
pub const DARK: Palette = Palette {
    base00: c(0.0, 0.0, 12.0),
    base05: c(0.0, 0.0, 14.0),
    base10: c(0.0, 0.0, 16.0),
    base20: c(0.0, 0.0, 20.0),
    base25: c(0.0, 0.0, 22.0),
    base30: c(0.0, 0.0, 26.0),
    base35: c(0.0, 0.0, 30.0),
    base40: c(0.0, 0.0, 38.0),
    base50: c(0.0, 0.0, 52.0),
    base60: c(0.0, 0.0, 64.0),
    base70: c(0.0, 0.0, 72.0),
    base100: c(0.0, 0.0, 92.0),
    accent: c(258.0, 88.0, 66.0),
    accent_hover: c(258.0, 88.0, 72.0),
    accent_active: c(258.0, 88.0, 78.0),
    red: c(350.0, 80.0, 65.0),
    orange: c(28.0, 100.0, 60.0),
    yellow: c(45.0, 100.0, 60.0),
    green: c(145.0, 92.0, 52.0),
    cyan: c(178.0, 100.0, 52.0),
    blue: c(215.0, 94.0, 62.0),
    purple: c(256.0, 82.0, 72.0),
    background: BackgroundTokens {
        app: c(0.0, 0.0, 12.0),
        primary: c(0.0, 0.0, 14.0),
        secondary: c(0.0, 0.0, 18.0),
        elevated: c(0.0, 0.0, 20.0),
        code: c(0.0, 0.0, 18.0),
        selection: rgba(258.0, 88.0, 66.0, 0.25),
    },
    borders: BorderTokens {
        subtle: c(0.0, 0.0, 20.0),
        default: c(0.0, 0.0, 28.0),
        strong: c(0.0, 0.0, 38.0),
        focus: c(258.0, 88.0, 66.0),
    },
    text: TextTokens {
        normal: c(0.0, 0.0, 88.0),
        muted: c(0.0, 0.0, 64.0),
        faint: c(0.0, 0.0, 52.0),
        on_accent: c(0.0, 0.0, 100.0),
        link: c(215.0, 94.0, 62.0),
        title: c(0.0, 0.0, 92.0),
    },
    interactive: InteractiveTokens {
        hover: c(0.0, 0.0, 20.0),
        active: c(0.0, 0.0, 26.0),
        selected: rgba(258.0, 88.0, 66.0, 0.25),
        disabled: c(0.0, 0.0, 38.0),
        focus_ring: c(258.0, 88.0, 66.0),
    },
    titlebar: SurfaceTokens {
        background: c(0.0, 0.0, 14.0),
        foreground: c(0.0, 0.0, 88.0),
        border: c(0.0, 0.0, 24.0),
        hover: c(0.0, 0.0, 20.0),
        active: c(0.0, 0.0, 26.0),
    },
    sidebar: SurfaceTokens {
        background: c(0.0, 0.0, 18.0),
        foreground: c(0.0, 0.0, 80.0),
        border: c(0.0, 0.0, 24.0),
        hover: c(0.0, 0.0, 24.0),
        active: rgba(258.0, 88.0, 66.0, 0.25),
    },
    editor: EditorTokens {
        background: c(0.0, 0.0, 12.0),
        foreground: c(0.0, 0.0, 88.0),
        heading: c(258.0, 88.0, 76.0),
        cursor: c(258.0, 88.0, 66.0),
        line_highlight: c(0.0, 0.0, 16.0),
        code: c(0.0, 0.0, 18.0),
    },
    status: SurfaceTokens {
        background: c(0.0, 0.0, 18.0),
        foreground: c(0.0, 0.0, 64.0),
        border: c(0.0, 0.0, 24.0),
        hover: c(0.0, 0.0, 24.0),
        active: c(0.0, 0.0, 28.0),
    },
    modal: SurfaceTokens {
        background: c(0.0, 0.0, 20.0),
        foreground: c(0.0, 0.0, 88.0),
        border: c(0.0, 0.0, 30.0),
        hover: c(0.0, 0.0, 24.0),
        active: c(0.0, 0.0, 28.0),
    },
    settings: SurfaceTokens {
        background: c(0.0, 0.0, 14.0),
        foreground: c(0.0, 0.0, 88.0),
        border: c(0.0, 0.0, 24.0),
        hover: c(0.0, 0.0, 20.0),
        active: c(0.0, 0.0, 26.0),
    },
    launcher: SurfaceTokens {
        background: c(0.0, 0.0, 20.0),
        foreground: c(0.0, 0.0, 88.0),
        border: c(0.0, 0.0, 30.0),
        hover: c(0.0, 0.0, 24.0),
        active: c(0.0, 0.0, 28.0),
    },
    spacing: LIGHT.spacing,
    radius: LIGHT.radius,
    font_size: LIGHT.font_size,
};

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn light_palette_has_expected_anchor_values() {
        assert_eq!(OBSIDIAN_LIGHT.base00.l, 1.0);
        assert!((OBSIDIAN_LIGHT.accent.h - 258.0 / 360.0).abs() < f32::EPSILON);
        assert_eq!(ThemeKind::default(), ThemeKind::Light);
    }
}
