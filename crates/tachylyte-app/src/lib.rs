//! Testable state models for the Tachylyte desktop shell.

/// The visual palette used by the shell.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Theme {
    #[default]
    Dark,
    Light,
}

/// The core settings exposed by the initial shell.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FeatureToggle {
    pub name: &'static str,
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

#[derive(Clone, Debug, PartialEq)]
pub struct SettingsState {
    pub theme: Theme,
    pub show_left_sidebar: bool,
    pub show_right_sidebar: bool,
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
}

use gpui::{div, prelude::*, px, rgb, App, Application, Context, Render, Window, WindowOptions};

/// The native GPUI root view. The visual structure deliberately stays small,
/// while the settings model carries the complete feature surface.
#[derive(Default)]
pub struct Shell {
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
        let features = self.settings.features.iter().map(|feature| {
            let mark = if feature.enabled { "☑" } else { "☐" };
            format!("{mark} {}", feature.name)
        });
        let right = if self.settings.show_right_sidebar {
            div()
                .w(px(210.))
                .bg(rgb(palette.1))
                .p_4()
                .child("SETTINGS\n\nCore features")
                .children(features)
        } else {
            div().w(px(0.))
        };

        div().flex().flex_col().size_full().bg(rgb(palette.0)).text_color(rgb(palette.2))
            .child(div().h(px(42.)).flex().items_center().px_4().bg(rgb(palette.1))
                .child("TACHYLYTE   •   Welcome.md     +     ⚙ Settings"))
            .child(div().flex().flex_1()
                .child(div().w(px(48.)).bg(rgb(palette.3)).flex().flex_col().items_center().p_2()
                    .child("✦\n\n⌕\n\n✎\n\n⚙"))
                .child(left)
                .child(div().flex_1().flex().flex_col().p_6()
                    .child(div().h(px(34.)).border_b_1().border_color(rgb(palette.3)).child("Welcome.md"))
                    .child(div().flex_1().p_6().child("# Welcome to Tachylyte\n\nYour native knowledge workspace is ready."))
                    .child(div().h(px(26.)).text_color(rgb(0x8a8a8aff)).child("Ready   •   0 words   •   Markdown")))
                .child(right))
            .child(div().h(px(28.)).px_3().bg(rgb(palette.3)).child(format!("Settings · {enabled_count} core features enabled")))
    }
}

/// Start the native GPUI application and open the initial desktop frame.
pub fn launch() {
    Application::new().run(|cx: &mut App| {
        cx.open_window(WindowOptions::default(), |_window, cx| {
            cx.new(|_| Shell::default())
        })
        .expect("failed to open Tachylyte window");
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_core_feature_is_independently_toggleable() {
        let mut settings = SettingsState::default();
        assert_eq!(settings.features.len(), CORE_FEATURES.len());
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
}
