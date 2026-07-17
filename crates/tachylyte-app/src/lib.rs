#![deny(missing_docs)]
//! A small, testable GPUI shell for a local Markdown vault.

use gpui::{div, prelude::*, px, rgb, App, Application, Context, Render, Window, WindowOptions};
use std::{env, path::Path};
use tachylyte_core::{FileKind, Vault, VaultEntry, VaultPath};
use tachylyte_knowledge::{Document as KnowledgeDocument, VaultIndex};
use tachylyte_markdown::{EditorDocument, ViewMode};

/// The visual palette used by the shell.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Theme {
    #[default]
    /// Dark charcoal palette.
    Dark,
    /// Light paper palette.
    Light,
}

/// A named, independently switchable view or action.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FeatureToggle {
    /// Stable feature name.
    pub name: &'static str,
    /// Current state.
    pub enabled: bool,
}

/// Features exposed by the initial shell.
pub const CORE_FEATURES: &[&str] = &[
    "File explorer",
    "Search",
    "Properties",
    "Outline",
    "Links",
    "Command palette",
    "Word count",
];

/// Persistable frame settings.
#[derive(Clone, Debug, PartialEq)]
pub struct SettingsState {
    /// Active theme.
    pub theme: Theme,
    /// Show explorer.
    pub show_left_sidebar: bool,
    /// Show outline/properties.
    pub show_right_sidebar: bool,
    /// Feature switches.
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
    /// Toggle one feature, returning false for an unknown name.
    pub fn toggle_feature(&mut self, name: &str) -> bool {
        self.features
            .iter_mut()
            .find(|f| f.name == name)
            .map(|f| {
                f.enabled = !f.enabled;
                true
            })
            .unwrap_or(false)
    }
    /// Whether a named feature is enabled.
    pub fn feature_enabled(&self, name: &str) -> bool {
        self.features
            .iter()
            .find(|f| f.name == name)
            .is_some_and(|f| f.enabled)
    }
    /// Toggle the explorer.
    pub fn toggle_left_sidebar(&mut self) {
        self.show_left_sidebar = !self.show_left_sidebar;
    }
    /// Toggle the details sidebar.
    pub fn toggle_right_sidebar(&mut self) {
        self.show_right_sidebar = !self.show_right_sidebar;
    }
    /// Toggle the theme.
    pub fn toggle_theme(&mut self) {
        self.theme = if self.theme == Theme::Dark {
            Theme::Light
        } else {
            Theme::Dark
        };
    }
}

/// A file opened by the runtime, including its parsed Markdown snapshot.
#[derive(Clone, Debug)]
pub struct OpenDocument {
    /// Vault-relative path.
    pub path: VaultPath,
    /// Mutable editor.
    pub editor: EditorDocument,
    /// Source/reading presentation mode.
    pub mode: ViewMode,
}

/// Render-independent application controller. All filesystem and editing behavior lives here.
#[derive(Debug)]
pub struct AppController {
    /// Current vault, if one was successfully opened.
    pub vault: Option<Vault>,
    /// Scanned supported files.
    pub entries: Vec<VaultEntry>,
    /// Current Markdown file.
    pub document: Option<OpenDocument>,
    /// Searchable knowledge index.
    pub index: VaultIndex,
    /// Frame settings.
    pub settings: SettingsState,
    /// Search/palette input.
    pub query: String,
    /// Whether the command palette overlay is visible.
    pub palette_open: bool,
    /// Last recoverable error/status.
    pub status: String,
    cursor: usize,
}
impl Default for AppController {
    fn default() -> Self {
        Self::new()
    }
}
impl AppController {
    /// Construct an empty controller without touching disk.
    pub fn new() -> Self {
        Self {
            vault: None,
            entries: Vec::new(),
            document: None,
            index: VaultIndex::new(),
            settings: SettingsState::default(),
            query: String::new(),
            palette_open: false,
            status: "No vault opened".into(),
            cursor: 0,
        }
    }
    /// Open the CLI path, `TACHYLYTE_VAULT`, or no vault when neither is present.
    pub fn open_from_environment() -> Self {
        let mut c = Self::new();
        let path = env::args_os()
            .nth(1)
            .or_else(|| env::var_os("TACHYLYTE_VAULT"));
        if let Some(path) = path {
            c.open_vault(Path::new(&path));
        }
        c
    }
    /// Open and scan a vault, preserving a useful error in the status bar.
    pub fn open_vault(&mut self, path: &Path) -> bool {
        match Vault::open(path).and_then(|v| {
            let entries = v.scan()?;
            Ok((v, entries))
        }) {
            Ok((vault, entries)) => {
                self.vault = Some(vault);
                self.entries = entries;
                self.document = None;
                self.rebuild_index();
                self.status = if self.entries.is_empty() {
                    "Vault is empty".into()
                } else {
                    format!("{} files", self.entries.len())
                };
                true
            }
            Err(e) => {
                self.status = format!("Unable to open vault: {e}");
                false
            }
        }
    }
    fn rebuild_index(&mut self) {
        self.index = VaultIndex::new();
        let Some(vault) = &self.vault else { return };
        for e in &self.entries {
            if e.kind != FileKind::Markdown {
                continue;
            }
            if let Ok(bytes) = vault.read(&e.path) {
                if let Ok(source) = String::from_utf8(bytes) {
                    let d = tachylyte_markdown::Document::parse(source.clone());
                    let properties = d
                        .properties()
                        .iter()
                        .map(|p| (p.key.clone(), p.value.clone()))
                        .collect();
                    self.index.upsert(KnowledgeDocument {
                        path: e.path.to_string(),
                        content: source,
                        tags: d.tags(),
                        properties,
                        tasks: Vec::new(),
                        modified: 0,
                    });
                }
            }
        }
    }
    /// Select a Markdown file and parse it through tachylyte-markdown.
    pub fn select(&mut self, path: &VaultPath) -> bool {
        let Some(vault) = &self.vault else {
            self.status = "Open a vault first".into();
            return false;
        };
        match vault.read(path).and_then(|b| {
            String::from_utf8(b).map_err(|e| tachylyte_core::CoreError::Unsupported(e.to_string()))
        }) {
            Ok(source) => {
                self.cursor = source.len();
                self.document = Some(OpenDocument {
                    path: path.clone(),
                    editor: EditorDocument::new(source),
                    mode: ViewMode::Source,
                });
                self.status = path.to_string();
                true
            }
            Err(e) => {
                self.status = format!("Unable to read {}: {e}", path);
                false
            }
        }
    }
    /// Set source cursor, clamped to a UTF-8 boundary.
    pub fn set_cursor(&mut self, byte: usize) {
        if let Some(d) = &self.document {
            self.cursor = byte.min(d.editor.source().len());
            while self.cursor > 0 && !d.editor.source().is_char_boundary(self.cursor) {
                self.cursor -= 1;
            }
        }
    }
    /// Insert Unicode text at the cursor.
    pub fn insert_text(&mut self, text: &str) -> bool {
        let at = self.cursor;
        let Some(d) = &mut self.document else {
            return false;
        };
        if d.editor
            .edit(tachylyte_markdown::Span::new(at, at), text)
            .is_ok()
        {
            self.cursor += text.len();
            true
        } else {
            false
        }
    }
    /// Insert a newline at the cursor.
    pub fn newline(&mut self) -> bool {
        self.insert_text("\n")
    }
    /// Delete the preceding Unicode scalar.
    pub fn backspace(&mut self) -> bool {
        let Some(d) = &mut self.document else {
            return false;
        };
        if self.cursor == 0 {
            return false;
        }
        let start = d.editor.source()[..self.cursor]
            .char_indices()
            .next_back()
            .map_or(0, |(i, _)| i);
        if d.editor
            .edit(tachylyte_markdown::Span::new(start, self.cursor), "")
            .is_ok()
        {
            self.cursor = start;
            true
        } else {
            false
        }
    }
    /// Atomically save the selected source through tachylyte-core.
    pub fn save(&mut self) -> bool {
        let Some(vault) = &self.vault else {
            self.status = "No vault opened".into();
            return false;
        };
        let Some(d) = &mut self.document else {
            return false;
        };
        match vault.write(&d.path, d.editor.source().as_bytes()) {
            Ok(()) => {
                d.editor.mark_clean();
                self.status = format!("Saved {}", d.path);
                true
            }
            Err(e) => {
                self.status = format!("Save failed: {e}");
                false
            }
        }
    }
    /// Change source, live preview, or reading mode.
    pub fn set_mode(&mut self, mode: ViewMode) {
        if let Some(d) = &mut self.document {
            d.mode = mode;
        }
    }
    /// Toggle command palette visibility.
    pub fn toggle_palette(&mut self) {
        self.palette_open = !self.palette_open;
    }
    /// Return selected document's parsed outline/properties/link summary.
    pub fn details(&self) -> Option<(tachylyte_markdown::Outline, usize, usize, usize)> {
        self.document.as_ref().map(|d| {
            let doc = d.editor.document();
            (
                doc.outline(),
                doc.properties().len(),
                doc.links().len() + doc.wikilinks().len(),
                doc.word_count(),
            )
        })
    }
    /// Return matching Markdown paths using the knowledge query engine.
    pub fn search(&self) -> Vec<String> {
        tachylyte_knowledge::search(&self.index, &self.query)
            .map(|r| r.into_iter().map(|x| x.path).collect())
            .unwrap_or_default()
    }
}

/// Native GPUI root view.
pub struct Shell {
    /// Application controller.
    pub controller: AppController,
}
impl Default for Shell {
    fn default() -> Self {
        Self {
            controller: AppController::open_from_environment(),
        }
    }
}
impl Render for Shell {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let light = self.controller.settings.theme == Theme::Light;
        let bg = if light { 0xf7f7f7ff } else { 0x1e1e1eff };
        let panel = if light { 0xffffffff } else { 0x252526ff };
        let fg = if light { 0x242424ff } else { 0xd4d4d4ff };
        let entries = self
            .controller
            .entries
            .iter()
            .filter(|e| e.kind == FileKind::Markdown)
            .map(|e| e.path.to_string())
            .collect::<Vec<_>>();
        let selected = self
            .controller
            .document
            .as_ref()
            .map(|d| d.path.to_string())
            .unwrap_or_default();
        let status = self.controller.status.clone();
        let source = self.controller.document.as_ref().map_or_else(
            || "Select a Markdown file to begin.".into(),
            |d| d.editor.source().to_owned(),
        );
        let entity = _cx.entity();
        let rows = entries.into_iter().map(|name| {
            let e = entity.clone();
            let path = name.clone();
            div()
                .id("file-row")
                .p_1()
                .w_full()
                .hover(|s| s.bg(rgb(0x3b3b3bff)))
                .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
                    if let Ok(p) = VaultPath::new(&path) {
                        e.update(cx, |shell, cx| {
                            shell.controller.select(&p);
                            cx.notify();
                        });
                    }
                })
                .child(name)
        });
        let left = if self.controller.settings.show_left_sidebar
            && self.controller.settings.feature_enabled("File explorer")
        {
            div()
                .w(px(220.))
                .bg(rgb(panel))
                .p_3()
                .child("FILES")
                .children(rows)
        } else {
            div().w(px(0.))
        };
        let right = if self.controller.settings.show_right_sidebar {
            let details = self.controller.details();
            let text = details.map_or_else(
                || "No document\n\nOutline\nProperties\nLinks".into(),
                |(o, p, l, w)| {
                    format!(
                        "OUTLINE\n{}\n\nProperties: {p}\nLinks: {l}\nWords: {w}",
                        o.headings
                            .iter()
                            .map(|h| h.text.as_str())
                            .collect::<Vec<_>>()
                            .join("\n")
                    )
                },
            );
            div().w(px(230.)).bg(rgb(panel)).p_3().child(text)
        } else {
            div().w(px(0.))
        };
        let palette = if self.controller.palette_open
            && self.controller.settings.feature_enabled("Command palette")
        {
            div()
                .absolute()
                .top(px(70.))
                .left(px(260.))
                .w(px(420.))
                .bg(rgb(panel))
                .p_4()
                .border_1()
                .border_color(rgb(0x808080ff))
                .child(format!(
                    "COMMAND PALETTE\n{}\nCtrl/Cmd+S Save · Source · Reading",
                    self.controller.query
                ))
        } else {
            div()
        };
        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(bg))
            .text_color(rgb(fg))
            .child(
                div()
                    .h(px(42.))
                    .flex()
                    .items_center()
                    .px_4()
                    .bg(rgb(panel))
                    .child(format!(
                        "TACHYLYTE  {}{}",
                        selected,
                        if self
                            .controller
                            .document
                            .as_ref()
                            .is_some_and(|d| d.editor.is_dirty())
                        {
                            " •"
                        } else {
                            ""
                        }
                    )),
            )
            .child(
                div()
                    .flex()
                    .flex_1()
                    .child(left)
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .p_5()
                            .child(
                                div()
                                    .flex()
                                    .gap_2()
                                    .child("SOURCE")
                                    .child("  LIVE PREVIEW  READING"),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .p_3()
                                    .border_1()
                                    .border_color(rgb(0x3b3b3bff))
                                    .child(source),
                            )
                            .child(div().h(px(28.)).child(status)),
                    )
                    .child(right),
            )
            .child(
                div()
                    .h(px(26.))
                    .px_3()
                    .bg(rgb(panel))
                    .child("Ready  ·  Ctrl/Cmd+P palette  ·  Ctrl/Cmd+S save"),
            )
            .child(palette)
    }
}

/// Open the desktop window.
pub fn open_shell_window(cx: &mut App) -> gpui::Result<()> {
    cx.open_window(WindowOptions::default(), |_window, cx| {
        cx.new(|_| Shell::default())
    })
    .map(|_| ())
}
/// Start the native GPUI application.
pub fn launch() {
    Application::new().run(|cx: &mut App| {
        if let Err(e) = open_shell_window(cx) {
            eprintln!("failed to open Tachylyte window: {e}");
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;
    #[test]
    fn vault_select_unicode_edit_save_and_modes() {
        let d = tempdir().unwrap();
        fs::write(d.path().join("note.md"), "# Héllo\n世界").unwrap();
        let mut c = AppController::new();
        assert!(c.open_vault(d.path()));
        let p = VaultPath::new("note.md").unwrap();
        assert!(c.select(&p));
        c.set_cursor(c.document.as_ref().unwrap().editor.source().len());
        assert!(c.backspace());
        assert_eq!(c.document.as_ref().unwrap().editor.source(), "# Héllo\n世");
        assert!(c.newline());
        assert!(c.insert_text("✓"));
        assert!(c.document.as_ref().unwrap().editor.is_dirty());
        c.set_mode(ViewMode::Reading);
        assert!(c.save());
        assert_eq!(
            fs::read_to_string(d.path().join("note.md")).unwrap(),
            "# Héllo\n世\n✓"
        );
    }
    #[test]
    fn empty_and_missing_vault_are_graceful() {
        let d = tempdir().unwrap();
        let mut c = AppController::new();
        assert!(c.open_vault(d.path()));
        assert!(c.entries.is_empty());
        assert!(c.status.contains("empty"));
        assert!(!c.open_vault(&d.path().join("missing")));
        assert!(c.status.contains("Unable"));
    }
    #[test]
    fn toggles_and_search_are_render_independent() {
        let mut c = AppController::new();
        assert!(c.settings.toggle_feature("Search"));
        assert!(!c.settings.feature_enabled("Search"));
        assert!(!c.settings.toggle_feature("unknown"));
        c.toggle_palette();
        assert!(c.palette_open);
    }
}
