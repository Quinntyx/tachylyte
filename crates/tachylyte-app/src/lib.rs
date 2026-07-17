#![deny(missing_docs)]
//! A small, testable GPUI shell for a local Markdown vault.

use gpui::{
    div, prelude::*, px, rgb, App, Application, Context, FocusHandle, KeyDownEvent, Render, Size,
    Window, WindowBounds, WindowOptions,
};
use serde::{Deserialize, Serialize};
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};
use tachylyte_core::{FileKind, Vault, VaultEntry, VaultPath};
use tachylyte_knowledge::{Document as KnowledgeDocument, VaultIndex};
use tachylyte_markdown::{EditorDocument, ViewMode};

mod graph_view;
mod tab_notice;
mod tab_policy;
mod theme_helpers;
mod workspace_actions;

use graph_view::graph_scene;
use theme_helpers::UiPalette;

/// The visual palette used by the shell.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
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
            theme: Theme::Light,
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
            settings: load_settings(),
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
        if !self
            .entries
            .iter()
            .any(|entry| entry.kind == FileKind::Markdown && entry.path == *path)
        {
            self.status = format!("Not a scanned Markdown file: {path}");
            return false;
        }
        if self
            .document
            .as_ref()
            .is_some_and(|document| document.editor.is_dirty())
        {
            self.status = "Unsaved changes: save before switching files".into();
            return false;
        }
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
        let path = d.path.clone();
        let result = vault.write(&path, d.editor.source().as_bytes());
        match result {
            Ok(()) => {
                d.editor.mark_clean();
                let _ = d;
                self.rebuild_index();
                self.status = format!("Saved {path}");
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
    /// Return the detail/status sections currently projected by feature flags.
    pub fn detail_projection(&self) -> Vec<&'static str> {
        ["Outline", "Properties", "Links", "Word count"]
            .into_iter()
            .filter(|name| self.settings.feature_enabled(name))
            .collect()
    }
    /// Return matching Markdown paths using the knowledge query engine.
    pub fn search(&self) -> Vec<String> {
        tachylyte_knowledge::search(&self.index, &self.query)
            .map(|r| r.into_iter().map(|x| x.path).collect())
            .unwrap_or_default()
    }
    /// Move the source cursor one Unicode scalar left.
    pub fn move_left(&mut self) {
        if let Some(document) = &self.document {
            self.cursor = document.editor.source()[..self.cursor]
                .char_indices()
                .next_back()
                .map_or(0, |(offset, _)| offset);
        }
    }
    /// Move the source cursor one Unicode scalar right.
    pub fn move_right(&mut self) {
        if let Some(document) = &self.document {
            self.cursor = document.editor.source()[self.cursor..]
                .chars()
                .next()
                .map_or(self.cursor, |character| self.cursor + character.len_utf8());
        }
    }
    /// Move the source cursor to the nearest column on the previous line.
    pub fn move_up(&mut self) {
        let Some(document) = &self.document else {
            return;
        };
        let source = document.editor.source();
        let line_start = source[..self.cursor]
            .rfind('\n')
            .map_or(0, |offset| offset + 1);
        let column = self.cursor - line_start;
        let previous_end = line_start.saturating_sub(1);
        let previous_start = source[..previous_end]
            .rfind('\n')
            .map_or(0, |offset| offset + 1);
        self.cursor = (previous_start + column).min(previous_end);
        while self.cursor > previous_start && !source.is_char_boundary(self.cursor) {
            self.cursor -= 1;
        }
    }
    /// Move the source cursor to the nearest column on the next line.
    pub fn move_down(&mut self) {
        let Some(document) = &self.document else {
            return;
        };
        let source = document.editor.source();
        let line_start = source[..self.cursor]
            .rfind('\n')
            .map_or(0, |offset| offset + 1);
        let column = self.cursor - line_start;
        let Some(next_start) = source[self.cursor..]
            .find('\n')
            .map(|offset| self.cursor + offset + 1)
        else {
            return;
        };
        let next_end = source[next_start..]
            .find('\n')
            .map_or(source.len(), |offset| next_start + offset);
        self.cursor = (next_start + column).min(next_end);
        while self.cursor > next_start && !source.is_char_boundary(self.cursor) {
            self.cursor -= 1;
        }
    }
    /// Execute a small, stable command-palette command.
    pub fn execute_command(&mut self, command: &str) -> bool {
        match command.trim().to_ascii_lowercase().as_str() {
            "save" | "file.save" => self.save(),
            "source" | "mode.source" => {
                self.set_mode(ViewMode::Source);
                true
            }
            "reading" | "mode.reading" => {
                self.set_mode(ViewMode::Reading);
                true
            }
            "toggle explorer" | "sidebar.left" => {
                self.settings.toggle_left_sidebar();
                save_settings(&self.settings);
                true
            }
            "toggle details" | "sidebar.right" => {
                self.settings.toggle_right_sidebar();
                save_settings(&self.settings);
                true
            }
            "toggle settings" => {
                self.settings.toggle_feature("Properties");
                save_settings(&self.settings);
                true
            }
            "toggle theme" | "theme.toggle" => {
                self.settings.toggle_theme();
                save_settings(&self.settings);
                true
            }
            "manage vaults" | "vaults" => {
                self.status = "Use the vault manager to switch vaults".into();
                true
            }
            _ => {
                self.status = format!("Unknown command: {command}");
                false
            }
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct RecentVault {
    path: PathBuf,
    name: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct PersistedFeature {
    name: String,
    enabled: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct PersistedSettings {
    theme: Theme,
    show_left_sidebar: bool,
    show_right_sidebar: bool,
    features: Vec<PersistedFeature>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct UserConfig {
    #[serde(default)]
    recent: Vec<RecentVault>,
    settings: Option<PersistedSettings>,
}

impl From<&SettingsState> for PersistedSettings {
    fn from(settings: &SettingsState) -> Self {
        Self {
            theme: settings.theme,
            show_left_sidebar: settings.show_left_sidebar,
            show_right_sidebar: settings.show_right_sidebar,
            features: settings
                .features
                .iter()
                .map(|feature| PersistedFeature {
                    name: feature.name.to_owned(),
                    enabled: feature.enabled,
                })
                .collect(),
        }
    }
}

impl PersistedSettings {
    fn into_settings(self) -> SettingsState {
        let mut settings = SettingsState {
            theme: self.theme,
            show_left_sidebar: self.show_left_sidebar,
            show_right_sidebar: self.show_right_sidebar,
            ..SettingsState::default()
        };
        for feature in self.features {
            if let Some(current) = settings
                .features
                .iter_mut()
                .find(|f| f.name == feature.name)
            {
                current.enabled = feature.enabled;
            }
        }
        settings
    }
}

fn recent_file() -> Option<PathBuf> {
    dirs::config_dir().map(|p| p.join("tachylyte").join("recent-vaults.json"))
}

fn load_config() -> UserConfig {
    let settings = SettingsState::default();
    let Some(path) = recent_file() else {
        return UserConfig {
            recent: Vec::new(),
            settings: Some((&settings).into()),
        };
    };
    let Ok(contents) = fs::read_to_string(path) else {
        return UserConfig {
            recent: Vec::new(),
            settings: Some((&settings).into()),
        };
    };
    if let Ok(mut config) = serde_json::from_str::<UserConfig>(&contents) {
        config.settings.get_or_insert_with(|| (&settings).into());
        return config;
    }
    // Keep compatibility with the original format, which was just an array.
    let recent = serde_json::from_str::<Vec<RecentVault>>(&contents).unwrap_or_default();
    UserConfig {
        recent,
        settings: Some((&settings).into()),
    }
}

fn save_config(config: &UserConfig) {
    let Some(path) = recent_file() else { return };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(config) {
        let _ = fs::write(path, json);
    }
}

/// Load persisted frame settings, falling back to the documented defaults.
pub fn load_settings() -> SettingsState {
    load_config()
        .settings
        .expect("load_config always supplies settings")
        .into_settings()
}

/// Persist frame settings while retaining the recent-vault list.
pub fn save_settings(settings: &SettingsState) {
    let mut config = load_config();
    config.settings = Some(settings.into());
    save_config(&config);
}

fn load_recent() -> Vec<RecentVault> {
    load_config().recent
}

fn save_recent(recent: &[RecentVault]) {
    let mut config = load_config();
    config.recent = recent.to_vec();
    save_config(&config);
}

fn remember_vault(path: &Path, recent: &mut Vec<RecentVault>) {
    recent.retain(|v| v.path != path);
    recent.insert(
        0,
        RecentVault {
            path: path.to_path_buf(),
            name: path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("Vault")
                .to_owned(),
        },
    );
    recent.truncate(8);
    save_recent(recent);
}

/// Compact vault chooser shown when startup has no usable vault.
pub struct Launcher {
    recent: Vec<RecentVault>,
    name_input: String,
    path_input: String,
    editing_name: bool,
    message: String,
    focus_handle: FocusHandle,
}

impl Launcher {
    fn new(cx: &mut Context<Self>) -> Self {
        Self {
            recent: load_recent(),
            name_input: String::new(),
            path_input: String::new(),
            editing_name: false,
            message: String::new(),
            focus_handle: cx.focus_handle(),
        }
    }

    fn open_path(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        let mut controller = AppController::new();
        if controller.open_vault(&path) {
            remember_vault(&path, &mut self.recent);
            // `open_window` creates the workspace before returning, so only
            // remove the launcher after that operation succeeds.  This keeps
            // a failed open visible and avoids leaving two application
            // windows around during the launcher -> workspace transition.
            match open_workspace_window(cx, controller) {
                Ok(()) => {
                    window.remove_window();
                    window.prevent_default();
                    self.message = format!("Opened {}", path.display());
                }
                Err(error) => self.message = format!("Could not open workspace: {error}"),
            }
        } else {
            self.message = controller.status;
        }
        cx.notify();
    }

    fn create_vault(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = self.name_input.trim();
        let parent = self.path_input.trim();
        if name.is_empty() || parent.is_empty() {
            self.message = "Enter a vault name and parent folder.".into();
            cx.notify();
            return;
        }
        let path = Path::new(parent).join(name);
        match fs::create_dir_all(&path) {
            Ok(()) => self.open_path(path, window, cx),
            Err(e) => {
                self.message = format!("Could not create vault: {e}");
                cx.notify();
            }
        }
    }
}

impl Render for Launcher {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();
        let recent = self.recent.clone();
        let cards = recent.into_iter().enumerate().map(|(i, vault)| {
            let open_entity = entity.clone();
            let remove_entity = entity.clone();
            let path = vault.path.clone();
            let remove_path = path.clone();
            let reveal_path = path.clone();
            let stale = !path.is_dir();
            div()
                .flex()
                .items_center()
                .gap_2()
                .p_2()
                .border_1()
                .border_color(rgb(0xe0e0e0))
                .child(div().flex_1().child(format!(
                    "{}\n{}{}",
                    vault.name,
                    path.display(),
                    if stale { "  · unavailable" } else { "" }
                )))
                .child(
                    div()
                        .id(("recent-open", i))
                        .px_2()
                        .py_1()
                        .bg(rgb(0x7852ee))
                        .text_color(rgb(0xffffff))
                        .on_mouse_down(gpui::MouseButton::Left, move |_, w, c| {
                            open_entity.update(c, |l, c| l.open_path(path.clone(), w, c))
                        })
                        .child("Open"),
                )
                .child(
                    div()
                        .id(("recent-reveal", i))
                        .px_2()
                        .py_1()
                        .on_mouse_down(gpui::MouseButton::Left, move |_, _, _| {
                            let _ = Command::new("xdg-open").arg(reveal_path.clone()).spawn();
                        })
                        .child("Reveal"),
                )
                .child(
                    div()
                        .id(("recent-remove", i))
                        .px_2()
                        .py_1()
                        .on_mouse_down(gpui::MouseButton::Left, move |_, _, c| {
                            remove_entity.update(c, |l, c| {
                                l.recent.retain(|v| v.path != remove_path);
                                save_recent(&l.recent);
                                c.notify();
                            })
                        })
                        .child("Remove"),
                )
        });
        let create_entity = entity.clone();
        let pick_entity = entity.clone();
        let open_entity = entity.clone();
        let name_entity = entity.clone();
        let path_entity = entity.clone();
        let name_border = if self.editing_name {
            0x7852ee
        } else {
            0xe0e0e0
        };
        let path_border = if self.editing_name {
            0xe0e0e0
        } else {
            0x7852ee
        };
        div()
            .size_full()
            .bg(rgb(0xffffff))
            .text_color(rgb(0x222222))
            .flex()
            .flex_col()
            .items_center()
            .p_8()
            .child(div().text_xl().child("◈  TACHYLYTE"))
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(0x5c5c5c))
                    .child("Your notes, your space."),
            )
            .child(div().mt_6().w(px(620.)).child("Recent vaults"))
            .child(
                div()
                    .w(px(620.))
                    .flex()
                    .flex_col()
                    .gap_2()
                    .mt_2()
                    .children(cards),
            )
            .child(
                div()
                    .w(px(620.))
                    .mt_5()
                    .flex()
                    .gap_2()
                    .items_center()
                    .child("Create new vault")
                    .child(
                        div()
                            .flex_1()
                            .border_1()
                            .border_color(rgb(name_border))
                            .p_2()
                            .on_mouse_down(gpui::MouseButton::Left, move |_, _, c| {
                                name_entity.update(c, |l, c| {
                                    l.editing_name = true;
                                    c.notify();
                                })
                            })
                            .child(if self.name_input.is_empty() {
                                "Vault name".to_owned()
                            } else {
                                self.name_input.clone()
                            }),
                    )
                    .child(
                        div()
                            .id("create-vault")
                            .px_3()
                            .py_2()
                            .bg(rgb(0x7852ee))
                            .text_color(rgb(0xffffff))
                            .on_mouse_down(gpui::MouseButton::Left, move |_, w, c| {
                                create_entity.update(c, |l, c| l.create_vault(w, c))
                            })
                            .child("Create"),
                    ),
            )
            .child(
                div()
                    .w(px(620.))
                    .mt_2()
                    .flex()
                    .gap_2()
                    .items_center()
                    .child("Parent folder")
                    .child(
                        div()
                            .flex_1()
                            .border_1()
                            .border_color(rgb(path_border))
                            .p_2()
                            .on_mouse_down(gpui::MouseButton::Left, move |_, _, c| {
                                path_entity.update(c, |l, c| {
                                    l.editing_name = false;
                                    c.notify();
                                })
                            })
                            .child(if self.path_input.is_empty() {
                                "Type a path or browse".to_owned()
                            } else {
                                self.path_input.clone()
                            }),
                    )
                    .child(
                        div()
                            .id("pick-folder")
                            .px_3()
                            .py_2()
                            .on_mouse_down(gpui::MouseButton::Left, move |_, _, c| {
                                if let Some(path) = rfd::FileDialog::new()
                                    .set_title("Choose vault parent folder")
                                    .pick_folder()
                                {
                                    pick_entity.update(c, |l, c| {
                                        l.path_input = path.display().to_string();
                                        l.editing_name = false;
                                        c.notify();
                                    });
                                }
                            })
                            .child("Browse"),
                    ),
            )
            .child(
                div()
                    .w(px(620.))
                    .mt_2()
                    .flex()
                    .gap_2()
                    .items_center()
                    .child("Open folder as vault")
                    .child(
                        div()
                            .flex_1()
                            .border_1()
                            .border_color(rgb(if self.editing_name {
                                0xe0e0e0
                            } else {
                                0x7852ee
                            }))
                            .p_2()
                            .child(if self.path_input.is_empty() {
                                "Type a path or browse".to_owned()
                            } else {
                                self.path_input.clone()
                            }),
                    )
                    .child(
                        div()
                            .id("open-folder")
                            .px_3()
                            .py_2()
                            .bg(rgb(0x7852ee))
                            .text_color(rgb(0xffffff))
                            .on_mouse_down(gpui::MouseButton::Left, move |_, w, c| {
                                open_entity.update(c, |l, c| {
                                    l.open_path(PathBuf::from(l.path_input.trim()), w, c)
                                })
                            })
                            .child("Open"),
                    ),
            )
            .child(
                div()
                    .mt_4()
                    .text_color(rgb(0x5c5c5c))
                    .child(self.message.clone()),
            )
            .child(div().flex_1())
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(0x5c5c5c))
                    .child("Help  ·  English  ·  Manage your vaults locally"),
            )
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|launcher, event: &KeyDownEvent, _, cx| {
                let input = if launcher.editing_name {
                    &mut launcher.name_input
                } else {
                    &mut launcher.path_input
                };
                if event.keystroke.key.as_str() == "backspace" {
                    input.pop();
                } else if let Some(ch) = &event.keystroke.key_char {
                    if !event.keystroke.modifiers.control && !event.keystroke.modifiers.platform {
                        input.push_str(ch);
                    }
                }
                cx.notify();
            }))
    }
}

/// Native GPUI root view.
pub struct Shell {
    /// Application controller.
    pub controller: AppController,
    /// Focus target for keyboard editing.
    pub focus_handle: FocusHandle,
}
impl Shell {
    fn with_controller(controller: AppController, cx: &mut Context<Self>) -> Self {
        Self {
            controller,
            focus_handle: cx.focus_handle(),
        }
    }

    /// Open the vault chooser in its own compact window.
    fn open_vault_manager(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::centered(
                Size {
                    width: px(800.),
                    height: px(600.),
                },
                cx,
            )),
            ..WindowOptions::default()
        };
        let _ = cx.open_window(options, |_window, cx| cx.new(Launcher::new));
    }

    /// Execute a workspace command, including commands that open another window.
    #[allow(dead_code)]
    fn execute_command(&mut self, command: &str, window: &mut Window, cx: &mut Context<Self>) {
        if command.trim().eq_ignore_ascii_case("manage vaults")
            || command.trim().eq_ignore_ascii_case("vaults")
        {
            self.open_vault_manager(window, cx);
        } else {
            self.controller.execute_command(command);
        }
    }
}
#[cfg(any())]
impl Render for Shell {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let light = self.controller.settings.theme == Theme::Light;
        let bg = if light { 0xfafafa } else { 0x1e1e1e };
        let panel = if light { 0xffffff } else { 0x252526 };
        let fg = if light { 0x222222 } else { 0xd4d4d4 };
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
        let entity = cx.entity();
        let manage_entity = entity.clone();
        let palette_entity = entity.clone();
        let source_entity = entity.clone();
        let reading_entity = entity.clone();
        let rows = entries.into_iter().map(|name| {
            let e = entity.clone();
            let path = name.clone();
            div()
                .id("file-row")
                .p_1()
                .w_full()
                .hover(|s| s.bg(rgb(0xe0e0e0)))
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
                || "No document".into(),
                |(outline, properties, links, words)| {
                    let mut sections = Vec::new();
                    if self.controller.settings.feature_enabled("Outline") {
                        sections.push(format!(
                            "OUTLINE\n{}",
                            outline
                                .headings
                                .iter()
                                .map(|heading| heading.text.as_str())
                                .collect::<Vec<_>>()
                                .join("\n")
                        ));
                    }
                    if self.controller.settings.feature_enabled("Properties") {
                        sections.push(format!("Properties: {properties}"));
                    }
                    if self.controller.settings.feature_enabled("Links") {
                        sections.push(format!("Links: {links}"));
                    }
                    if self.controller.settings.feature_enabled("Word count") {
                        sections.push(format!("Words: {words}"));
                    }
                    sections.join("\n\n")
                },
            );
            div().w(px(230.)).bg(rgb(panel)).p_3().child(text)
        } else {
            div().w(px(0.))
        };
        let search_results = if self.controller.settings.feature_enabled("Search")
            && !self.controller.query.is_empty()
        {
            self.controller.search().join("\n")
        } else {
            String::new()
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
                .border_color(rgb(0xe0e0e0))
                .child(format!(
                    "COMMAND PALETTE\n{}\n{}\nCtrl/Cmd+S Save · Enter execute",
                    self.controller.query, search_results
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
                    ))
                    .child(
                        div()
                            .id("palette-button")
                            .p_2()
                            .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
                                palette_entity.update(cx, |shell, cx| {
                                    shell.controller.toggle_palette();
                                    cx.notify();
                                });
                            })
                            .child("⌘P Palette"),
                    ),
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
                                    .child(
                                        div()
                                            .id("source-mode")
                                            .on_mouse_down(
                                                gpui::MouseButton::Left,
                                                move |_, _, cx| {
                                                    source_entity.update(cx, |shell, cx| {
                                                        shell.controller.set_mode(ViewMode::Source);
                                                        cx.notify();
                                                    });
                                                },
                                            )
                                            .child("SOURCE"),
                                    )
                                    .child("  LIVE PREVIEW  ")
                                    .child(
                                        div()
                                            .id("reading-mode")
                                            .on_mouse_down(
                                                gpui::MouseButton::Left,
                                                move |_, _, cx| {
                                                    reading_entity.update(cx, |shell, cx| {
                                                        shell
                                                            .controller
                                                            .set_mode(ViewMode::Reading);
                                                        cx.notify();
                                                    });
                                                },
                                            )
                                            .child("READING"),
                                    ),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .p_3()
                                    .border_1()
                                    .border_color(rgb(0xe0e0e0))
                                    .track_focus(&self.focus_handle)
                                    .on_key_down(cx.listener(
                                        |shell, event: &KeyDownEvent, window, cx| {
                                            let key = event.keystroke.key.as_str();
                                            let modified = event.keystroke.modifiers.control
                                                || event.keystroke.modifiers.platform;
                                            if shell.controller.palette_open {
                                                match key {
                                                    "enter" => {
                                                        let command =
                                                            shell.controller.query.clone();
                                                        shell.execute_command(&command, window, cx);
                                                        shell.controller.palette_open = false;
                                                    }
                                                    "backspace" => {
                                                        shell.controller.query.pop();
                                                    }
                                                    "escape" => {
                                                        shell.controller.palette_open = false
                                                    }
                                                    _ if event.keystroke.key_char.is_some()
                                                        && !modified =>
                                                    {
                                                        shell.controller.query.push_str(
                                                            event
                                                                .keystroke
                                                                .key_char
                                                                .as_deref()
                                                                .unwrap_or_default(),
                                                        )
                                                    }
                                                    _ => {}
                                                }
                                            } else if modified && key.eq_ignore_ascii_case("s") {
                                                shell.controller.save();
                                            } else if modified && key.eq_ignore_ascii_case("p") {
                                                shell.controller.toggle_palette();
                                            } else {
                                                match key {
                                                    "backspace" => {
                                                        shell.controller.backspace();
                                                    }
                                                    "enter" => {
                                                        shell.controller.newline();
                                                    }
                                                    "arrowleft" | "left" => {
                                                        shell.controller.move_left()
                                                    }
                                                    "arrowright" | "right" => {
                                                        shell.controller.move_right()
                                                    }
                                                    "arrowup" | "up" => shell.controller.move_up(),
                                                    "arrowdown" | "down" => {
                                                        shell.controller.move_down()
                                                    }
                                                    "escape" => {
                                                        shell.controller.set_mode(ViewMode::Reading)
                                                    }
                                                    _ if event.keystroke.key_char.is_some()
                                                        && !modified =>
                                                    {
                                                        shell.controller.insert_text(
                                                            event
                                                                .keystroke
                                                                .key_char
                                                                .as_deref()
                                                                .unwrap_or_default(),
                                                        );
                                                    }
                                                    _ => {}
                                                }
                                            }
                                            cx.notify();
                                            if modified && key.eq_ignore_ascii_case("s") {
                                                window.prevent_default();
                                            }
                                        },
                                    ))
                                    .on_mouse_down(
                                        gpui::MouseButton::Left,
                                        cx.listener(|shell, _, window, _| {
                                            window.focus(&shell.focus_handle);
                                        }),
                                    )
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
                    .child(
                        div()
                            .id("manage-vaults")
                            .text_color(rgb(0x7852ee))
                            .on_mouse_down(gpui::MouseButton::Left, move |_, window, cx| {
                                manage_entity.update(cx, |shell, cx| {
                                    shell.execute_command("manage vaults", window, cx);
                                    cx.notify();
                                })
                            })
                            .child("Manage vaults"),
                    )
                    .child("  ·  Ready  ·  Ctrl/Cmd+P palette  ·  Ctrl/Cmd+S save"),
            )
            .child(palette)
    }
}

impl Render for Shell {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tokens = if self.controller.settings.theme == Theme::Light {
            tachylyte_theme::light()
        } else {
            tachylyte_theme::dark()
        };
        let palette = UiPalette::light();
        let app = tokens.app();
        let panel = tokens.panel();
        let editor = tokens.structured();
        let text = tokens.text();
        let muted = tokens.muted_text();
        let border = tokens.border();
        let accent = tokens.accent();
        let yellow = tokens.yellow();
        let selected_surface = tokens.selected();
        let entity = cx.entity();

        let selected = self
            .controller
            .document
            .as_ref()
            .map(|document| document.path.to_string())
            .unwrap_or_default();
        let title = self
            .controller
            .document
            .as_ref()
            .map(|document| {
                document
                    .path
                    .as_path()
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("Untitled")
                    .to_owned()
            })
            .unwrap_or_else(|| "No note open".into());
        let source = self
            .controller
            .document
            .as_ref()
            .map(|document| document.editor.source().to_owned())
            .unwrap_or_else(|| "Select a note from the explorer to begin.".into());
        let mode = self
            .controller
            .document
            .as_ref()
            .map(|document| document.mode)
            .unwrap_or(ViewMode::Source);
        let note_paths = self
            .controller
            .entries
            .iter()
            .filter(|entry| entry.kind == FileKind::Markdown)
            .map(|entry| entry.path.to_string())
            .collect::<Vec<_>>();

        let collapse_entity = entity.clone();
        let new_note_entity = entity.clone();
        let new_folder_entity = entity.clone();
        let graph_entity = entity.clone();
        let settings_entity = entity.clone();
        let manage_entity = entity.clone();
        let close_entity = entity.clone();
        let plus_entity = entity.clone();
        let help_entity = entity.clone();
        let settings_bottom_entity = entity.clone();

        let button = |id: &'static str, label: &'static str| {
            div()
                .id(id)
                .w_full()
                .h(px(34.))
                .flex()
                .items_center()
                .justify_center()
                .text_color(muted)
                .text_size(px(16.))
                .hover(|style| style.bg(tokens.hover()))
                .child(label)
        };

        let ribbon = div()
            .w(px(40.))
            .h_full()
            .flex()
            .flex_col()
            .items_center()
            .border_1()
            .border_color(border)
            .bg(panel)
            .child(
                div()
                    .id("collapse-sidebar")
                    .w_full()
                    .h(px(40.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(text)
                    .hover(|style| style.bg(tokens.hover()))
                    .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
                        collapse_entity.update(cx, |shell, cx| {
                            shell.controller.settings.toggle_left_sidebar();
                            save_settings(&shell.controller.settings);
                            cx.notify();
                        });
                    })
                    .child("‹"),
            )
            .child(button("ribbon-search", "⌕").on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(|shell, _, _, cx| {
                    shell.controller.toggle_palette();
                    cx.notify();
                }),
            ))
            .child(button("ribbon-new-note", "+").on_mouse_down(
                gpui::MouseButton::Left,
                move |_, _, cx| {
                    new_note_entity.update(cx, |shell, cx| {
                        shell.controller.create_note();
                        cx.notify();
                    });
                },
            ))
            .child(button("ribbon-new-folder", "▱").on_mouse_down(
                gpui::MouseButton::Left,
                move |_, _, cx| {
                    new_folder_entity.update(cx, |shell, cx| {
                        shell.controller.create_folder();
                        cx.notify();
                    });
                },
            ))
            .child(button("ribbon-graph", "⌘").on_mouse_down(
                gpui::MouseButton::Left,
                move |_, _, cx| {
                    graph_entity.update(cx, |shell, cx| {
                        shell.controller.settings.toggle_right_sidebar();
                        save_settings(&shell.controller.settings);
                        cx.notify();
                    });
                },
            ))
            .child(button("ribbon-settings", "⚙").on_mouse_down(
                gpui::MouseButton::Left,
                move |_, _, cx| {
                    settings_entity.update(cx, |shell, cx| {
                        shell.controller.status = "Appearance settings".into();
                        shell.controller.toggle_palette();
                        cx.notify();
                    });
                },
            ))
            .child(div().flex_1())
            .child(button("ribbon-help", "?").on_mouse_down(
                gpui::MouseButton::Left,
                move |_, _, cx| {
                    help_entity.update(cx, |shell, cx| {
                        shell.controller.status = "Help: Ctrl/Cmd+P for commands".into();
                        cx.notify();
                    });
                },
            ));

        let rows = self
            .controller
            .entries
            .iter()
            .filter(|entry| entry.kind == FileKind::Markdown)
            .enumerate()
            .map(|(index, entry)| {
                let path = entry.path.clone();
                let label = entry.path.to_string();
                let is_selected = label == selected;
                let row_entity = entity.clone();
                div()
                    .id(("explorer-file", index))
                    .w_full()
                    .h(px(28.))
                    .px_2()
                    .flex()
                    .items_center()
                    .text_size(px(13.))
                    .text_color(if is_selected { text } else { muted })
                    .bg(if is_selected { selected_surface } else { panel })
                    .hover(|style| style.bg(tokens.hover()))
                    .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
                        row_entity.update(cx, |shell, cx| {
                            shell.controller.select(&path);
                            cx.notify();
                        });
                    })
                    .child(format!("▱  {label}"))
            });

        let explorer = if self.controller.settings.show_left_sidebar
            && self.controller.settings.feature_enabled("File explorer")
        {
            div()
                .w(px(295.))
                .h_full()
                .flex()
                .flex_col()
                .border_1()
                .border_color(border)
                .bg(panel)
                .child(
                    div()
                        .h(px(44.))
                        .px_3()
                        .flex()
                        .items_center()
                        .justify_between()
                        .text_size(px(11.))
                        .text_color(muted)
                        .child("VAULT")
                        .child(div().flex().gap_2().child("⌕").child("•••")),
                )
                .child(
                    div()
                        .px_3()
                        .pb_2()
                        .text_size(px(11.))
                        .text_color(muted)
                        .child("FILES"),
                )
                .children(rows)
                .child(div().flex_1())
                .child(
                    div()
                        .h(px(42.))
                        .px_3()
                        .flex()
                        .items_center()
                        .justify_between()
                        .border_1()
                        .border_color(border)
                        .text_size(px(12.))
                        .text_color(muted)
                        .child("⌄  Vault files")
                        .child("↗"),
                )
        } else {
            div().w(px(0.))
        };

        let mode_button = |label: &'static str, button_mode: ViewMode| {
            let mode_entity = entity.clone();
            let active = mode == button_mode;
            div()
                .id(label)
                .px_2()
                .py_1()
                .text_size(px(11.))
                .text_color(if active { accent } else { muted })
                .bg(if active { selected_surface } else { editor })
                .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
                    mode_entity.update(cx, |shell, cx| {
                        shell.controller.set_mode(button_mode);
                        cx.notify();
                    });
                })
                .child(label)
        };

        let close_title = close_entity.clone();
        let tab_strip = div()
            .h(px(38.))
            .flex()
            .items_center()
            .border_1()
            .border_color(border)
            .bg(panel)
            .child(
                div()
                    .h_full()
                    .min_w(px(180.))
                    .px_3()
                    .flex()
                    .items_center()
                    .border_1()
                    .border_color(border)
                    .bg(editor)
                    .text_color(text)
                    .text_size(px(13.))
                    .child(format!("▱  {title}")),
            )
            .child(div().flex_1())
            .child(
                div()
                    .id("close-note-tab")
                    .px_3()
                    .text_color(muted)
                    .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
                        close_title.update(cx, |shell, cx| {
                            shell.controller.try_close_document();
                            cx.notify();
                        });
                    })
                    .child("×"),
            )
            .child(
                div()
                    .id("new-note-tab")
                    .px_3()
                    .text_color(accent)
                    .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
                        plus_entity.update(cx, |shell, cx| {
                            shell.controller.create_note();
                            cx.notify();
                        });
                    })
                    .child("+"),
            );

        let heading = if selected.is_empty() {
            "Welcome to your workspace".to_owned()
        } else {
            title.clone()
        };
        let editor_body = match mode {
            ViewMode::Source => div()
                .text_color(text)
                .text_size(px(15.))
                .child(source.clone()),
            ViewMode::LivePreview => div()
                .text_color(text)
                .text_size(px(15.))
                .child(format!("Live preview\n\n{source}")),
            ViewMode::Reading => div()
                .text_color(text)
                .text_size(px(16.))
                .child(format!("Reading\n\n{source}")),
        };
        let editor_column = div()
            .flex_1()
            .h_full()
            .flex()
            .flex_col()
            .bg(editor)
            .child(tab_strip)
            .child(
                div()
                    .h(px(46.))
                    .px_5()
                    .flex()
                    .items_center()
                    .justify_between()
                    .border_1()
                    .border_color(border)
                    .text_color(muted)
                    .text_size(px(12.))
                    .child("‹  ›   ⌂  /  Notes")
                    .child("⋮"),
            )
            .child(
                div()
                    .px_5()
                    .pt_4()
                    .text_color(text)
                    .text_size(px(24.))
                    .child(heading),
            )
            .child(
                div()
                    .px_5()
                    .pt_3()
                    .flex()
                    .gap_1()
                    .child(mode_button("Source", ViewMode::Source))
                    .child(mode_button("Live Preview", ViewMode::LivePreview))
                    .child(mode_button("Reading", ViewMode::Reading)),
            )
            .child(
                div()
                    .id("editor-leaf")
                    .flex_1()
                    .p_5()
                    .track_focus(&self.focus_handle)
                    .on_key_down(cx.listener(|shell, event: &KeyDownEvent, window, cx| {
                        let key = event.keystroke.key.as_str();
                        let modified =
                            event.keystroke.modifiers.control || event.keystroke.modifiers.platform;
                        if modified && key.eq_ignore_ascii_case("s") {
                            shell.controller.save();
                            window.prevent_default();
                        } else if key == "backspace" {
                            shell.controller.backspace();
                        } else if key == "enter" {
                            shell.controller.newline();
                        } else if let Some(character) = &event.keystroke.key_char {
                            if !modified && !event.keystroke.modifiers.alt {
                                shell.controller.insert_text(character);
                            }
                        }
                        cx.notify();
                    }))
                    .child(editor_body),
            );

        let right = if self.controller.settings.show_right_sidebar {
            div()
                .w(px(340.))
                .h_full()
                .flex()
                .flex_col()
                .border_1()
                .border_color(border)
                .bg(panel)
                .child(
                    div()
                        .h(px(38.))
                        .px_3()
                        .flex()
                        .items_center()
                        .justify_between()
                        .border_1()
                        .border_color(border)
                        .text_size(px(12.))
                        .text_color(text)
                        .child("Graph")
                        .child("×"),
                )
                .child(
                    div()
                        .h(px(34.))
                        .px_3()
                        .flex()
                        .items_center()
                        .text_size(px(11.))
                        .text_color(accent)
                        .child("GRAPH VIEW"),
                )
                .child(graph_scene(&note_paths, &selected))
                .child(div().flex_1())
                .child(
                    div()
                        .p_3()
                        .border_1()
                        .border_color(border)
                        .text_size(px(12.))
                        .text_color(muted)
                        .child("Backlinks")
                        .child(format!(
                            "\n{} incoming links",
                            self.controller.details().map_or(0, |d| d.2)
                        )),
                )
        } else {
            div().w(px(0.))
        };

        let vault_name = self
            .controller
            .vault
            .as_ref()
            .and_then(|vault| vault.root().file_name())
            .and_then(|name| name.to_str())
            .unwrap_or("No vault")
            .to_owned();
        let word_count = self.controller.details().map_or(0, |details| details.3);
        let status = self.controller.status.clone();
        let bottom = div()
            .h(px(28.))
            .px_3()
            .flex()
            .items_center()
            .justify_between()
            .border_1()
            .border_color(border)
            .bg(panel)
            .text_size(px(11.))
            .text_color(muted)
            .child(
                div()
                    .flex()
                    .gap_3()
                    .child(vault_name)
                    .child("Help")
                    .child(
                        div()
                            .id("bottom-settings")
                            .text_color(accent)
                            .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
                                settings_bottom_entity.update(cx, |shell, cx| {
                                    shell.controller.toggle_palette();
                                    cx.notify();
                                });
                            })
                            .child("Settings"),
                    )
                    .child(
                        div()
                            .id("bottom-manage-vaults")
                            .text_color(accent)
                            .on_mouse_down(gpui::MouseButton::Left, move |_, window, cx| {
                                manage_entity.update(cx, |shell, cx| {
                                    shell.open_vault_manager(window, cx);
                                });
                            })
                            .child("Manage vaults"),
                    ),
            )
            .child(format!(
                "{status}   ·   {word_count} words   ·   {} characters",
                source.chars().count()
            ));

        let palette_overlay = if self.controller.palette_open {
            div()
                .absolute()
                .top(px(52.))
                .left(px(340.))
                .w(px(420.))
                .p_4()
                .border_1()
                .border_color(border)
                .bg(tokens.modal_background())
                .text_color(text)
                .child("COMMAND PALETTE")
                .child("\nType a command, or use the ribbon controls.")
                .child(format!("\n{}", self.controller.query))
        } else {
            div()
        };

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(if self.controller.settings.theme == Theme::Light {
                palette.app
            } else {
                app
            })
            .text_color(text)
            .child(div().h(px(2.)).bg(yellow))
            .child(
                div()
                    .h(px(42.))
                    .flex()
                    .items_center()
                    .px_3()
                    .border_1()
                    .border_color(border)
                    .bg(panel)
                    .text_color(text)
                    .child("◈  TACHYLYTE")
                    .child(div().flex_1())
                    .child("⌘P")
                    .child("  ·  ")
                    .child("□"),
            )
            .child(
                div()
                    .flex_1()
                    .flex()
                    .child(ribbon)
                    .child(explorer)
                    .child(editor_column)
                    .child(right),
            )
            .child(bottom)
            .child(palette_overlay)
    }
}

/// Open the normal workspace window with a controller that has a valid vault.
pub fn open_workspace_window(cx: &mut App, mut controller: AppController) -> gpui::Result<()> {
    if controller.vault.is_some()
        && !controller
            .entries
            .iter()
            .any(|entry| entry.kind == FileKind::Markdown)
    {
        controller.ensure_welcome();
    }
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::centered(
            Size {
                width: px(1280.),
                height: px(800.),
            },
            cx,
        )),
        ..WindowOptions::default()
    };
    cx.open_window(options, move |_window, cx| {
        cx.new(|cx| Shell::with_controller(controller, cx))
    })
    .map(|_| ())
}

/// Open the desktop workspace window.
pub fn open_shell_window(cx: &mut App) -> gpui::Result<()> {
    let controller = AppController::open_from_environment();
    open_workspace_window(cx, controller)
}

/// Open the launcher window used by the vault manager.
pub fn open_launcher_window(cx: &mut App) -> gpui::Result<()> {
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::centered(
            Size {
                width: px(800.),
                height: px(600.),
            },
            cx,
        )),
        ..WindowOptions::default()
    };
    cx.open_window(options, |_window, cx| cx.new(Launcher::new))
        .map(|_| ())
}
/// Start the native GPUI application.
pub fn launch() {
    Application::new().run(|cx: &mut App| {
        let controller = AppController::open_from_environment();
        let result = if controller.vault.is_some() {
            open_workspace_window(cx, controller)
        } else {
            open_launcher_window(cx)
        };
        if let Err(e) = result {
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
    fn empty_vault_seeds_and_opens_welcome() {
        let d = tempdir().unwrap();
        let mut c = AppController::new();
        assert!(c.open_vault(d.path()));
        assert!(c.ensure_welcome());
        assert_eq!(
            c.document
                .as_ref()
                .map(|document| document.path.to_string()),
            Some("Welcome.md".into())
        );
        assert!(fs::read_to_string(d.path().join("Welcome.md"))
            .unwrap()
            .contains("Welcome to Tachylyte"));
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
    #[test]
    fn dirty_switch_is_explicitly_blocked() {
        let d = tempdir().unwrap();
        fs::write(d.path().join("a.md"), "a").unwrap();
        fs::write(d.path().join("b.md"), "b").unwrap();
        let mut c = AppController::new();
        assert!(c.open_vault(d.path()));
        assert!(c.select(&VaultPath::new("a.md").unwrap()));
        assert!(c.insert_text("changed"));
        assert!(!c.select(&VaultPath::new("b.md").unwrap()));
        assert!(c.status.contains("Unsaved"));
        assert!(!c.try_close_document());
        assert!(c.document.is_some());
        assert!(c.status.contains("save"));
        assert!(c.save());
        assert!(c.try_close_document());
        assert!(c.document.is_none());
    }
    #[test]
    fn save_reindexes_changed_content_and_rejects_unscanned_files() {
        let d = tempdir().unwrap();
        fs::write(d.path().join("note.md"), "old").unwrap();
        fs::write(d.path().join("data.txt"), "old").unwrap();
        let mut c = AppController::new();
        assert!(c.open_vault(d.path()));
        assert!(!c.select(&VaultPath::new("data.txt").unwrap()));
        assert!(c.select(&VaultPath::new("note.md").unwrap()));
        c.set_cursor(0);
        assert!(c.insert_text("new "));
        c.query = "new".into();
        assert!(c.search().is_empty());
        assert!(c.save());
        assert_eq!(c.search(), vec!["note.md"]);
    }
    #[test]
    fn palette_commands_and_feature_projection_are_deterministic() {
        let mut c = AppController::new();
        assert!(c.execute_command("sidebar.left"));
        assert!(!c.settings.show_left_sidebar);
        assert!(c.execute_command("mode.reading"));
        assert!(c.settings.toggle_feature("Links"));
        assert!(!c.detail_projection().contains(&"Links"));
        assert!(c.detail_projection().contains(&"Outline"));
        assert!(!c.execute_command("not-a-command"));
        assert!(c.status.contains("Unknown"));
    }

    #[test]
    fn recent_vault_and_workspace_preferences_survive_their_updates() {
        let config = tempdir().unwrap();
        let previous = env::var_os("XDG_CONFIG_HOME");
        env::set_var("XDG_CONFIG_HOME", config.path());

        let vault = config.path().join("notes");
        fs::create_dir(&vault).unwrap();
        let mut recent = Vec::new();
        remember_vault(&vault, &mut recent);

        let mut c = AppController::new();
        c.settings = SettingsState::default();
        assert!(c.open_vault(&vault));
        c.settings.toggle_theme();
        c.settings.toggle_left_sidebar();
        c.settings.toggle_right_sidebar();
        assert!(c.settings.toggle_feature("Search"));
        save_settings(&c.settings);

        assert_eq!(load_recent().first().map(|v| &v.path), Some(&vault));
        let restored = load_settings();
        assert_eq!(restored.theme, Theme::Dark);
        assert!(!restored.show_left_sidebar);
        assert!(!restored.show_right_sidebar);
        assert!(!restored.feature_enabled("Search"));

        match previous {
            Some(value) => env::set_var("XDG_CONFIG_HOME", value),
            None => env::remove_var("XDG_CONFIG_HOME"),
        }
    }
}
