#![deny(missing_docs)]
//! Testable state models and the native GPUI shell for Tachylyte.

/// The visual palette used by the shell.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Theme {
    #[default]
    /// Dark charcoal workspace palette.
    Dark,
    /// Light paper workspace palette.
    Light,
}

/// The core settings exposed by the initial shell.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FeatureToggle {
    /// Stable display name and model key for the feature.
    pub name: &'static str,
    /// Whether this feature is currently enabled.
    pub enabled: bool,
}

/// Obsidian 1.12.7 core-feature switches, represented independently so each
/// setting can be changed without affecting its neighbours.
pub const CORE_FEATURES: &[&str] = &[
    "File explorer",
    "Search",
    "Backlinks",
    "Outgoing links",
    "Tags view",
    "Properties view",
    "Page preview",
    "Starred files",
    "Quick switcher",
    "Command palette",
    "Graph view",
    "Canvas",
    "Daily notes",
    "Templates",
    "Note composer",
    "Random note",
    "Outline",
    "Word count",
    "Audio recorder",
    "Slides",
    "Bookmarks",
    "Workspaces",
    "Publish",
    "Sync",
    "Web viewer",
    "Bases",
    "Import",
    "Help",
    "Zettelkasten links",
    "Markdown renderer",
];

/// Persistable settings and feature switches for the desktop frame.
#[derive(Clone, Debug, PartialEq)]
pub struct SettingsState {
    /// Active visual theme.
    pub theme: Theme,
    /// Whether the vault sidebar is visible.
    pub show_left_sidebar: bool,
    /// Whether the settings/outline sidebar is visible.
    pub show_right_sidebar: bool,
    /// Independently controlled core feature switches.
    pub features: Vec<FeatureToggle>,
}

impl Default for SettingsState {
    fn default() -> Self {
        Self {
            theme: Theme::Dark,
            show_left_sidebar: true,
            show_right_sidebar: true,
            features: CORE_FEATURES
                .iter()
                .map(|&name| FeatureToggle {
                    name,
                    enabled: true,
                })
                .collect(),
        }
    }
}

impl SettingsState {
    /// Toggle a named feature, returning `false` when the name is unknown.
    pub fn toggle_feature(&mut self, name: &str) -> bool {
        if let Some(feature) = self
            .features
            .iter_mut()
            .find(|feature| feature.name == name)
        {
            feature.enabled = !feature.enabled;
            true
        } else {
            false
        }
    }

    /// Toggle the left vault sidebar.
    pub fn toggle_left_sidebar(&mut self) {
        self.show_left_sidebar = !self.show_left_sidebar;
    }

    /// Toggle the right settings sidebar.
    pub fn toggle_right_sidebar(&mut self) {
        self.show_right_sidebar = !self.show_right_sidebar;
    }

    /// Switch between the dark and light palettes.
    pub fn toggle_theme(&mut self) {
        self.theme = match self.theme {
            Theme::Dark => Theme::Light,
            Theme::Light => Theme::Dark,
        };
    }
}

use gpui::{div, prelude::*, px, rgb, App, Application, Context, Render, Window, WindowOptions};

/// The native GPUI root view. The visual structure deliberately stays small,
/// while the settings model carries the complete feature surface.
#[derive(Default)]
pub struct Shell {
    /// Mutable state rendered by this view.
    pub settings: SettingsState,
}

impl Render for Shell {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let palette = match self.settings.theme {
            Theme::Dark => (0x1e1e1eff, 0x252526ff, 0xd4d4d4ff, 0x3b3b3bff),
            Theme::Light => (0xf7f7f7ff, 0xffffffff, 0x242424ff, 0xd8d8d8ff),
        };
        let left = if self.settings.show_left_sidebar {
            div()
                .w(px(190.))
                .bg(rgb(palette.1))
                .p_4()
                .child("VAULT\n\n▸ Welcome\n▸ Notes\n▸ Attachments")
        } else {
            div().w(px(0.))
        };
        let enabled_count = self
            .settings
            .features
            .iter()
            .filter(|feature| feature.enabled)
            .count();
        let entity = _cx.entity();
        let feature_rows = self.settings.features.iter().map(|feature| {
            let name = feature.name;
            let mark = if feature.enabled { "☑" } else { "☐" };
            let row_entity = entity.clone();
            div()
                .id(name)
                .w_full()
                .p_1()
                .hover(|style| style.bg(rgb(palette.3)))
                .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
                    row_entity.update(cx, |shell, cx| {
                        shell.settings.toggle_feature(name);
                        cx.notify();
                    });
                })
                .child(format!("{mark} {name}"))
        });
        let right = if self.settings.show_right_sidebar {
            div()
                .w(px(210.))
                .bg(rgb(palette.1))
                .p_4()
                .child("SETTINGS\n\nCore features")
                .children(feature_rows)
        } else {
            div().w(px(0.))
        };

        let left_toggle = entity.clone();
        let right_toggle = entity.clone();
        let theme_toggle = entity.clone();
        div().flex().flex_col().size_full().bg(rgb(palette.0)).text_color(rgb(palette.2))
            .child(div().h(px(42.)).flex().items_center().px_4().bg(rgb(palette.1))
                .child("TACHYLYTE   •   Welcome.md     +     ⚙ Settings")
                .child(div().flex_1())
                .child(div().id("theme-toggle").p_2().hover(|style| style.bg(rgb(palette.3)))
                    .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
                        theme_toggle.update(cx, |shell, cx| { shell.settings.toggle_theme(); cx.notify(); });
                    })
                    .child(if self.settings.theme == Theme::Dark { "☀ Light" } else { "☾ Dark" })))
            .child(div().flex().flex_1()
                .child(div().w(px(48.)).bg(rgb(palette.3)).flex().flex_col().items_center().p_2()
                    .child(div().id("left-collapse").p_2().hover(|style| style.bg(rgb(palette.1)))
                        .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
                            left_toggle.update(cx, |shell, cx| { shell.settings.toggle_left_sidebar(); cx.notify(); });
                        })
                        .child(if self.settings.show_left_sidebar { "◀" } else { "▶" }))
                    .child("\n✦\n\n⌕\n\n✎")
                    .child(div().id("right-collapse").p_2().hover(|style| style.bg(rgb(palette.1)))
                        .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
                            right_toggle.update(cx, |shell, cx| { shell.settings.toggle_right_sidebar(); cx.notify(); });
                        })
                        .child(if self.settings.show_right_sidebar { "▶" } else { "◀" })))
                .child(left)
                .child(div().flex_1().flex().flex_col().p_6()
                    .child(div().h(px(34.)).border_b_1().border_color(rgb(palette.3)).child("Welcome.md"))
                    .child(div().flex_1().p_6().child("# Welcome to Tachylyte\n\nYour native knowledge workspace is ready."))
                    .child(div().h(px(26.)).text_color(rgb(0x8a8a8aff)).child("Ready   •   0 words   •   Markdown")))
                .child(right))
            .child(div().h(px(28.)).px_3().bg(rgb(palette.3)).child(format!("Settings · {enabled_count} core features enabled")))
    }
}

/// Open the initial desktop frame in a running GPUI application.
pub fn open_shell_window(cx: &mut App) -> gpui::Result<()> {
    cx.open_window(WindowOptions::default(), |_window, cx| {
        cx.new(|_| Shell::default())
    })
    .map(|_| ())
}

/// Start the native GPUI application and log startup errors without panicking.
pub fn launch() {
    Application::new().run(|cx: &mut App| {
        if let Err(error) = open_shell_window(cx) {
            eprintln!("failed to open Tachylyte window: {error}");
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_core_feature_is_independently_toggleable() {
        let mut settings = SettingsState::default();
        assert_eq!(settings.features.len(), CORE_FEATURES.len());
        let mut unique = settings
            .features
            .iter()
            .map(|feature| feature.name)
            .collect::<Vec<_>>();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), CORE_FEATURES.len());
        assert!(settings.features.iter().all(|feature| feature.enabled));
        assert!(settings.toggle_feature("Search"));
        assert!(
            !settings
                .features
                .iter()
                .find(|f| f.name == "Search")
                .unwrap()
                .enabled
        );
        assert!(
            settings
                .features
                .iter()
                .find(|f| f.name == "Backlinks")
                .unwrap()
                .enabled
        );
        assert!(!settings.toggle_feature("not a feature"));
    }

    #[test]
    fn shell_settings_transitions_are_reversible() {
        let mut settings = SettingsState::default();
        assert_eq!(settings.theme, Theme::Dark);
        settings.toggle_theme();
        assert_eq!(settings.theme, Theme::Light);
        settings.toggle_theme();
        assert_eq!(settings.theme, Theme::Dark);
        settings.toggle_left_sidebar();
        settings.toggle_right_sidebar();
        assert!(!settings.show_left_sidebar);
        assert!(!settings.show_right_sidebar);
        settings.toggle_left_sidebar();
        settings.toggle_right_sidebar();
        assert!(settings.show_left_sidebar);
        assert!(settings.show_right_sidebar);
    }
}
