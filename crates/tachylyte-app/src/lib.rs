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
use tachylyte_launcher_model::LauncherModel;
use tachylyte_markdown::{EditorDocument, ViewMode};
use tachylyte_navigation_ui::{ExplorerIntent, ExplorerModel, FileExplorerView};
use tachylyte_workspace::{Orientation, Workspace};

mod editor_surface;
mod graph_view;
mod settings_surface;
mod tab_notice;
mod tab_policy;
mod theme_helpers;
mod workflow_surface;
mod workspace_actions;

use editor_surface::EditorSurface;
use graph_view::GraphMount;
use settings_surface::SettingsSurface;
use theme_helpers::UiPalette;
use workflow_surface::{CommandPaletteSurface, QuickSwitcherSurface};

/// The visual palette used by the shell.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
pub enum Theme {
    #[default]
    /// Dark charcoal palette.
    Dark,
    /// Light paper palette.
    Light,
}

/// The routed surface for the currently selected vault entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LeafKind {
    /// A Markdown document rendered by `MarkdownEditor`.
    Markdown,
    /// An Obsidian Canvas document routed to the structured leaf state.
    Canvas,
    /// A Bases document routed to the structured leaf state.
    Bases,
    /// An image, audio, video, or PDF routed to the media leaf state.
    Media(FileKind),
    /// No file is currently selected.
    Empty,
}

impl LeafKind {
    fn from_entry(entry: &VaultEntry) -> Self {
        match entry.kind {
            FileKind::Markdown => Self::Markdown,
            FileKind::Canvas => Self::Canvas,
            FileKind::Image | FileKind::Audio | FileKind::Video | FileKind::Pdf => {
                Self::Media(entry.kind)
            }
        }
    }
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
    /// Currently selected path, including non-Markdown leaves.
    pub selected_path: Option<VaultPath>,
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
            selected_path: None,
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
                self.selected_path = None;
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
    /// Select a scanned file and route it to its typed workspace leaf.
    pub fn select(&mut self, path: &VaultPath) -> bool {
        let Some(entry) = self.entries.iter().find(|entry| entry.path == *path) else {
            self.status = format!("Not a scanned vault file: {path}");
            return false;
        };
        let leaf = LeafKind::from_entry(entry);
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
        let result = vault.read(path);
        match result {
            Ok(bytes) => {
                self.selected_path = Some(path.clone());
                self.cursor = 0;
                match String::from_utf8(bytes) {
                    Ok(source) => {
                        self.cursor = source.len();
                        self.document = Some(OpenDocument {
                            path: path.clone(),
                            editor: EditorDocument::new(source),
                            mode: ViewMode::Source,
                        });
                    }
                    Err(_) => self.document = None,
                }
                self.status = format!("{} · {:?}", path, leaf);
                true
            }
            Err(e) => {
                self.status = format!("Unable to read {}: {e}", path);
                false
            }
        }
    }

    /// Return the typed leaf route for the current selection.
    pub fn leaf_kind(&self) -> LeafKind {
        let Some(path) = &self.selected_path else {
            return LeafKind::Empty;
        };
        if path.as_path().extension().and_then(|e| e.to_str()) == Some("base") {
            return LeafKind::Bases;
        }
        self.entries
            .iter()
            .find(|entry| entry.path == *path)
            .map_or(LeafKind::Empty, LeafKind::from_entry)
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

#[allow(dead_code)] // Legacy JSON migration support; LauncherModel owns new writes.
fn save_recent(recent: &[RecentVault]) {
    let mut config = load_config();
    config.recent = recent.to_vec();
    save_config(&config);
}

#[allow(dead_code)] // Legacy JSON migration support; LauncherModel owns new writes.
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
    model: Option<LauncherModel>,
    name_input: String,
    path_input: String,
    editing_name: bool,
    message: String,
    focus_handle: FocusHandle,
}

impl Launcher {
    fn new(cx: &mut Context<Self>) -> Self {
        let mut model = LauncherModel::open_default().ok();
        // Migrate the pre-registry recent list once.  The model owns all
        // subsequent recent/open/create persistence.
        if let Some(model) = &mut model {
            if model.recent().is_empty() {
                for recent in load_recent() {
                    let _ = model.import(&recent.path);
                }
            }
        }
        Self {
            model,
            name_input: String::new(),
            path_input: String::new(),
            editing_name: false,
            message: String::new(),
            focus_handle: cx.focus_handle(),
        }
    }

    fn open_path(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        let path = if let Some(model) = &mut self.model {
            match model.import(&path) {
                Ok(entry) => {
                    let path = entry.path_buf();
                    let _ = model.select(entry.id.clone());
                    path
                }
                Err(error) => {
                    self.message = format!("Could not register vault: {error}");
                    cx.notify();
                    return;
                }
            }
        } else {
            path
        };
        let mut controller = AppController::new();
        if controller.open_vault(&path) {
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
        let Some(model) = &mut self.model else {
            self.message = "Vault registry unavailable".into();
            cx.notify();
            return;
        };
        match model
            .create_plan(name, PathBuf::from(parent))
            .and_then(|plan| model.execute_create(&plan))
        {
            Ok(entry) => self.open_path(entry.path_buf(), window, cx),
            Err(error) => {
                self.message = format!("Could not create vault: {error}");
                cx.notify();
            }
        }
    }
}

impl Render for Launcher {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();
        let recent = self
            .model
            .as_ref()
            .map(|model| {
                model
                    .recent()
                    .into_iter()
                    .map(|entry| (entry.id.clone(), entry.name.clone(), entry.path_buf()))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let cards = recent.into_iter().enumerate().map(|(i, (id, name, path))| {
            let open_entity = entity.clone();
            let remove_entity = entity.clone();
            let remove_id = id.clone();
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
                    name,
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
                                if let Some(model) = &mut l.model {
                                    let _ = model.remove(&remove_id);
                                }
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
    /// Render-neutral workspace layout and tab/history state.
    pub workspace: Workspace,
    /// Shared launcher registry/settings facade.
    launcher_model: Option<LauncherModel>,
    graph: Option<GraphMount>,
    editor: Option<EditorSurface>,
    settings_surface: Option<SettingsSurface>,
    command_palette: Option<CommandPaletteSurface>,
    quick_switcher: Option<QuickSwitcherSurface>,
    explorer: Option<gpui::Entity<FileExplorerView>>,
    settings_open: bool,
    quick_switcher_open: bool,
}
impl Shell {
    fn with_controller(mut controller: AppController, cx: &mut Context<Self>) -> Self {
        let launcher_model = LauncherModel::open_default().ok();
        if let Some(model) = &launcher_model {
            if model.settings().appearance.theme.as_deref() == Some("dark") {
                controller.settings.theme = Theme::Dark;
            }
        }
        let mut workspace = Workspace::default();
        if let Some(path) = controller.selected_path.as_ref() {
            workspace.open_reusable_path(path.to_string());
        }
        Self {
            controller,
            focus_handle: cx.focus_handle(),
            workspace,
            launcher_model,
            graph: None,
            editor: None,
            settings_surface: None,
            command_palette: None,
            quick_switcher: None,
            explorer: None,
            settings_open: false,
            quick_switcher_open: false,
        }
    }

    fn ensure_surfaces(&mut self, cx: &mut Context<Self>) {
        let paths = self
            .controller
            .entries
            .iter()
            .map(|entry| entry.path.to_string())
            .collect::<Vec<_>>();
        if let Some(explorer) = &self.explorer {
            explorer.update(cx, |view, cx| {
                view.model_mut().update_paths(paths.clone());
                cx.notify();
            });
        } else {
            self.explorer =
                Some(cx.new(|_| FileExplorerView::new(ExplorerModel::from_paths(paths))));
        }
        if self.graph.is_none() {
            self.graph = Some(GraphMount::mount(&self.controller.index, cx));
        } else if let Some(graph) = &self.graph {
            graph.sync(&self.controller.index, cx);
        }
        let source = self
            .controller
            .document
            .as_ref()
            .map_or_else(String::new, |d| d.editor.source().to_owned());
        let mode = self
            .controller
            .document
            .as_ref()
            .map_or(ViewMode::Source, |d| d.mode);
        if let Some(editor) = &self.editor {
            editor.sync(source, mode, cx);
        } else {
            self.editor = Some(EditorSurface::mount(source, mode, cx));
        }
        if self.settings_surface.is_none() {
            self.settings_surface = Some(SettingsSurface::new(cx));
        }
        if self.command_palette.is_none() {
            self.command_palette = Some(CommandPaletteSurface::from_commands(
                [
                    ("save", "Save current file"),
                    ("mode.source", "Open source mode"),
                    ("mode.reading", "Open reading mode"),
                    ("sidebar.left", "Toggle explorer sidebar"),
                    ("sidebar.right", "Toggle graph sidebar"),
                    ("theme.toggle", "Toggle theme"),
                    ("quick-switcher", "Open quick switcher"),
                    ("manage vaults", "Manage vaults"),
                ],
                cx,
            ));
        }
        if self.quick_switcher.is_none() {
            self.quick_switcher = Some(QuickSwitcherSurface::from_paths(
                self.controller
                    .entries
                    .iter()
                    .map(|entry| (entry.path.to_string(), entry.path.to_string())),
                cx,
            ));
        }
    }

    fn apply_explorer_intents(&mut self, cx: &mut Context<Self>) {
        let Some(explorer) = self.explorer.clone() else {
            return;
        };
        let intents = explorer.update(cx, |view, _| view.take_intents());
        for intent in intents {
            match intent {
                ExplorerIntent::Select { path } | ExplorerIntent::Open { path } => {
                    if let Ok(path) = VaultPath::new(path) {
                        self.open_path_in_workspace(&path);
                    }
                }
                ExplorerIntent::Activate => {
                    if let Some(path) = explorer
                        .read(cx)
                        .model()
                        .active_path()
                        .and_then(|p| VaultPath::new(p).ok())
                    {
                        self.open_path_in_workspace(&path);
                    }
                }
                ExplorerIntent::NewNote { .. } => {
                    if self.controller.create_note() {
                        self.sync_workspace_to_controller();
                    }
                }
                ExplorerIntent::NewFolder { .. } => {
                    self.controller.create_folder();
                }
                ExplorerIntent::Rename { path, new_name } => {
                    self.controller.rename_entry(&path, &new_name);
                }
                ExplorerIntent::Delete { path } => {
                    self.controller.delete_entry(&path);
                }
                ExplorerIntent::Move { path, destination }
                | ExplorerIntent::DragMove {
                    source: path,
                    destination,
                } => {
                    self.controller.move_entry(&path, &destination);
                }
                ExplorerIntent::Duplicate { path, destination } => {
                    self.controller.duplicate_entry(&path, &destination);
                }
                ExplorerIntent::Reveal { path } => {
                    if let (Some(vault), Ok(path)) = (&self.controller.vault, VaultPath::new(path))
                    {
                        let full = vault.root().join(path.as_path());
                        let _ = Command::new("xdg-open").arg(full).spawn();
                    }
                }
                ExplorerIntent::Toggle { path, expanded } => {
                    if let Some(explorer) = &self.explorer {
                        explorer.update(cx, |view, _| {
                            if expanded {
                                view.model_mut().expanded.insert(path.clone());
                            } else {
                                view.model_mut().expanded.remove(&path);
                            }
                        });
                    }
                }
                ExplorerIntent::SetFilter { value } => {
                    if let Some(explorer) = &self.explorer {
                        explorer.update(cx, |view, _| view.model_mut().set_filter(value.clone()));
                    }
                }
                ExplorerIntent::SetSort { mode } => {
                    if let Some(explorer) = &self.explorer {
                        let mode = match mode {
                            tachylyte_navigation_ui::ExplorerSortMode::Name => {
                                tachylyte_navigation_ui::SortMode::Name
                            }
                            tachylyte_navigation_ui::ExplorerSortMode::Modified => {
                                tachylyte_navigation_ui::SortMode::Modified
                            }
                            tachylyte_navigation_ui::ExplorerSortMode::Created => {
                                tachylyte_navigation_ui::SortMode::Created
                            }
                            tachylyte_navigation_ui::ExplorerSortMode::Kind => {
                                tachylyte_navigation_ui::SortMode::Name
                            }
                        };
                        explorer.update(cx, |view, _| view.model_mut().set_sort_mode(mode));
                    }
                }
                other => self.controller.status = format!("Explorer intent deferred: {other}"),
            }
        }
    }

    fn open_path_in_workspace(&mut self, path: &VaultPath) -> bool {
        if !self.controller.open_file(path) {
            return false;
        }
        self.workspace.open_reusable_path(path.to_string());
        true
    }

    fn sync_workspace_to_controller(&mut self) {
        if let Some(path) = self.controller.selected_path.clone() {
            self.workspace.open_reusable_path(path.to_string());
        }
    }

    fn apply_surface_events(&mut self, cx: &mut Context<Self>) {
        if let Some(graph) = &self.graph {
            for event in graph.drain_events(cx) {
                match event {
                    tachylyte_graph_ui::GraphEvent::Select(path)
                    | tachylyte_graph_ui::GraphEvent::Open(path) => {
                        if let Ok(path) = VaultPath::new(path) {
                            let _ = self.open_path_in_workspace(&path);
                        }
                    }
                }
            }
        }
        if let Some(editor) = &self.editor {
            for event in editor.drain_events(cx) {
                match event {
                    tachylyte_editor_ui::EditorEvent::Changed { .. } => {
                        let source = editor.entity().read(cx).source().to_owned();
                        if let Some(document) = &mut self.controller.document {
                            let old_len = document.editor.source().len();
                            let _ = document
                                .editor
                                .edit(tachylyte_markdown::Span::new(0, old_len), &source);
                        }
                    }
                    tachylyte_editor_ui::EditorEvent::ModeChanged(mode) => {
                        self.controller.set_mode(mode);
                    }
                    tachylyte_editor_ui::EditorEvent::SaveRequested => {
                        self.controller.save();
                    }
                    _ => {}
                }
            }
        }
        if let Some(settings) = &self.settings_surface {
            for event in settings.drain_events(cx) {
                match event {
                    tachylyte_settings_ui::SettingsEvent::ThemeChanged(theme) => {
                        self.controller.settings.theme = match theme {
                            tachylyte_settings_ui::Theme::Dark => Theme::Dark,
                            tachylyte_settings_ui::Theme::Light
                            | tachylyte_settings_ui::Theme::System => Theme::Light,
                        };
                        save_settings(&self.controller.settings);
                        if let Some(model) = &mut self.launcher_model {
                            model.settings_mut().appearance.theme = Some(
                                match theme {
                                    tachylyte_settings_ui::Theme::Dark => "dark",
                                    tachylyte_settings_ui::Theme::Light
                                    | tachylyte_settings_ui::Theme::System => "light",
                                }
                                .into(),
                            );
                            let _ = model.save_settings();
                        }
                    }
                    tachylyte_settings_ui::SettingsEvent::PluginChanged { id, enabled } => {
                        match id.as_str() {
                            "file-explorer" => self.controller.settings.show_left_sidebar = enabled,
                            "graph" => self.controller.settings.show_right_sidebar = enabled,
                            "global-search" => {
                                set_feature(&mut self.controller.settings, "Search", enabled)
                            }
                            "outline" => {
                                set_feature(&mut self.controller.settings, "Outline", enabled)
                            }
                            "properties" => {
                                set_feature(&mut self.controller.settings, "Properties", enabled)
                            }
                            "word-count" => {
                                set_feature(&mut self.controller.settings, "Word count", enabled)
                            }
                            _ => {}
                        }
                        save_settings(&self.controller.settings);
                        if let Some(model) = &mut self.launcher_model {
                            model.settings_mut().features.show_missing = enabled;
                            let _ = model.save_settings();
                        }
                    }
                    tachylyte_settings_ui::SettingsEvent::CloseRequested => {
                        self.settings_open = false;
                    }
                    _ => {}
                }
            }
        }
        if let Some(palette) = &self.command_palette {
            for intent in palette.drain_intents(cx) {
                if let tachylyte_workflow_ui::WorkflowIntent::RunCommand { command } = intent {
                    if command == "quick-switcher" {
                        self.quick_switcher_open = true;
                        self.controller.palette_open = false;
                    } else {
                        self.controller.execute_command(&command);
                    }
                }
            }
        }
        if let Some(switcher) = &self.quick_switcher {
            for intent in switcher.drain_intents(cx) {
                if let tachylyte_workflow_ui::WorkflowIntent::OpenPath { path } = intent {
                    if let Ok(path) = VaultPath::new(path) {
                        let _ = self.open_path_in_workspace(&path);
                        self.quick_switcher_open = false;
                    }
                }
            }
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

fn set_feature(settings: &mut SettingsState, name: &str, enabled: bool) {
    if let Some(feature) = settings
        .features
        .iter_mut()
        .find(|feature| feature.name == name)
    {
        feature.enabled = enabled;
    }
}

fn workspace_tab_count(workspace: &Workspace) -> usize {
    fn count(node: &tachylyte_workspace::LayoutNode) -> usize {
        match node {
            tachylyte_workspace::LayoutNode::Tabs(group) => group.tabs.len(),
            tachylyte_workspace::LayoutNode::Split { first, second, .. } => {
                count(first) + count(second)
            }
        }
    }
    workspace
        .windows
        .iter()
        .map(|window| count(&window.root))
        .sum()
}

fn workspace_focused_path(workspace: &Workspace) -> Option<String> {
    fn find(node: &tachylyte_workspace::LayoutNode, id: &str) -> Option<String> {
        match node {
            tachylyte_workspace::LayoutNode::Tabs(group) => group
                .tabs
                .iter()
                .find(|tab| tab.id == id)
                .and_then(|tab| tab.view.path.clone()),
            tachylyte_workspace::LayoutNode::Split { first, second, .. } => {
                find(first, id).or_else(|| find(second, id))
            }
        }
    }
    let id = workspace.focused_tab_id()?;
    workspace
        .windows
        .iter()
        .find_map(|window| find(&window.root, &id))
}

fn workspace_history_state(workspace: &Workspace) -> (bool, bool) {
    fn find(node: &tachylyte_workspace::LayoutNode, id: &str) -> Option<(bool, bool)> {
        match node {
            tachylyte_workspace::LayoutNode::Tabs(group) => group
                .tabs
                .iter()
                .find(|tab| tab.id == id)
                .map(|tab| (tab.history.can_back(), tab.history.can_forward())),
            tachylyte_workspace::LayoutNode::Split { first, second, .. } => {
                find(first, id).or_else(|| find(second, id))
            }
        }
    }
    let Some(id) = workspace.focused_tab_id() else {
        return (false, false);
    };
    workspace
        .windows
        .iter()
        .find_map(|window| find(&window.root, &id))
        .unwrap_or((false, false))
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
                                            } else if modified && key.eq_ignore_ascii_case("o") {
                                                shell.quick_switcher_open =
                                                    !shell.quick_switcher_open;
                                                shell.controller.palette_open =
                                                    shell.quick_switcher_open;
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
        self.apply_surface_events(cx);
        self.ensure_surfaces(cx);
        self.apply_explorer_intents(cx);
        if let Some(palette) = &self.command_palette {
            palette.sync_query(self.controller.query.clone(), cx);
        }
        if let Some(switcher) = &self.quick_switcher {
            switcher.sync_query(self.controller.query.clone(), cx);
        }
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
            .selected_path
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_default();
        let leaf_kind = self.controller.leaf_kind();
        let title = self
            .controller
            .selected_path
            .as_ref()
            .map(|path| {
                path.as_path()
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
        let graph_surface = self.graph.as_ref().map(GraphMount::entity);
        let editor_surface = self.editor.as_ref().map(EditorSurface::entity);
        let settings_surface = self.settings_surface.as_ref();

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
                        if shell.controller.create_note() {
                            shell.sync_workspace_to_controller();
                        }
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
                        shell.settings_open = true;
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

        let explorer_entity = self.explorer.as_ref().map(|e| e.clone());
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
                .child(explorer_entity.expect("explorer is mounted before render"))
                .child(div().flex_1())
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
        let tab_controls = self
            .controller
            .tab_controls(workspace_tab_count(&self.workspace));
        let split_entity = entity.clone();
        let split_tab = self.workspace.focused_tab_id();
        let (can_back, can_forward) = workspace_history_state(&self.workspace);
        let back_entity = entity.clone();
        let forward_entity = entity.clone();
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
            .child(if tab_controls.can_split {
                div()
                    .id("split-note-tab")
                    .px_2()
                    .text_color(accent)
                    .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
                        if let Some(tab) = split_tab.clone() {
                            split_entity.update(cx, |shell, cx| {
                                shell.workspace.split(&tab, Orientation::Horizontal);
                                cx.notify();
                            });
                        }
                    })
                    .child("Split")
                    .into_any_element()
            } else {
                div().into_any_element()
            })
            .child(
                div()
                    .id("close-note-tab")
                    .px_3()
                    .text_color(muted)
                    .when(tab_controls.can_close, |el| {
                        el.on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
                            close_title.update(cx, |shell, cx| {
                                if shell.controller.try_close_document() {
                                    if let Some(tab) = shell.workspace.focused_tab_id() {
                                        shell.workspace.close(&tab);
                                    }
                                    if let Some(path) = workspace_focused_path(&shell.workspace)
                                        .and_then(|path| VaultPath::new(path).ok())
                                    {
                                        let _ = shell.controller.open_file(&path);
                                    }
                                    cx.notify();
                                }
                            });
                        })
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
                            if shell.controller.create_note() {
                                shell.sync_workspace_to_controller();
                            }
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
        let editor_body = match leaf_kind {
            LeafKind::Markdown => editor_surface
                .map(|editor| editor.into_any_element())
                .unwrap_or_else(|| {
                    div()
                        .child("Markdown editor unavailable")
                        .into_any_element()
                }),
            LeafKind::Canvas => div()
                .p_5()
                .text_color(text)
                .child("CANVAS LEAF")
                .child("\nThis .canvas document is routed to the merged structured surface.")
                .into_any_element(),
            LeafKind::Bases => div()
                .p_5()
                .text_color(text)
                .child("BASES LEAF")
                .child("\nThis .base document is routed to the merged structured surface.")
                .into_any_element(),
            LeafKind::Media(kind) => div()
                .p_5()
                .text_color(text)
                .child(format!("MEDIA LEAF · {kind:?}"))
                .child(format!("\n{selected}"))
                .into_any_element(),
            LeafKind::Empty => div()
                .p_5()
                .text_color(text)
                .child("Select a file from the explorer to open its routed leaf.")
                .into_any_element(),
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
                    .child(
                        div()
                            .id("history-back")
                            .when(can_back, |el| {
                                el.on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
                                    back_entity.update(cx, |shell, cx| {
                                        shell.workspace.back();
                                        if let Some(path) = workspace_focused_path(&shell.workspace)
                                            .and_then(|path| VaultPath::new(path).ok())
                                        {
                                            let _ = shell.controller.open_file(&path);
                                        }
                                        cx.notify();
                                    });
                                })
                            })
                            .child("‹"),
                    )
                    .child(
                        div()
                            .id("history-forward")
                            .when(can_forward, |el| {
                                el.on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
                                    forward_entity.update(cx, |shell, cx| {
                                        shell.workspace.forward();
                                        if let Some(path) = workspace_focused_path(&shell.workspace)
                                            .and_then(|path| VaultPath::new(path).ok())
                                        {
                                            let _ = shell.controller.open_file(&path);
                                        }
                                        cx.notify();
                                    });
                                })
                            })
                            .child("›"),
                    )
                    .child("   ⌂  /  Notes")
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
                .child(graph_surface.expect("graph surface is mounted before render"))
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
                                    shell.settings_open = true;
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
                .child(if self.quick_switcher_open {
                    self.quick_switcher
                        .as_ref()
                        .map(QuickSwitcherSurface::entity)
                        .expect("quick switcher is mounted before render")
                        .into_any_element()
                } else {
                    self.command_palette
                        .as_ref()
                        .map(CommandPaletteSurface::entity)
                        .expect("command palette is mounted before render")
                        .into_any_element()
                })
                .child("\nEnter runs the selected typed workflow intent.")
        } else {
            div()
        };
        let settings_overlay = if self.settings_open {
            settings_surface
                .map(SettingsSurface::modal)
                .expect("settings surface is mounted before render")
                .into_any_element()
        } else {
            div().into_any_element()
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
            .child(settings_overlay)
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
    fn typed_non_markdown_files_route_to_explicit_leaves() {
        let d = tempdir().unwrap();
        fs::write(d.path().join("board.canvas"), "{\"nodes\":[]}").unwrap();
        fs::write(d.path().join("photo.png"), [0xff_u8, 0, 1, 2]).unwrap();
        let mut c = AppController::new();
        assert!(c.open_vault(d.path()));

        assert!(c.select(&VaultPath::new("board.canvas").unwrap()));
        assert_eq!(c.leaf_kind(), LeafKind::Canvas);
        assert!(c.document.is_some());

        assert!(c.select(&VaultPath::new("photo.png").unwrap()));
        assert!(matches!(c.leaf_kind(), LeafKind::Media(FileKind::Image)));
        assert!(c.document.is_none());
    }

    #[test]
    fn workspace_bridge_reuses_paths_and_tracks_tab_controls() {
        let mut workspace = Workspace::default();
        let first = workspace.open_reusable_path("a.md");
        let reused = workspace.open_reusable_path("a.md");
        assert_eq!(first, reused);
        let second = workspace.open_reusable_path("b.md");
        assert_ne!(first, second);
        assert_eq!(workspace_tab_count(&workspace), 2);
        let controls =
            tab_policy::TabControls::from_workspace(true, false, workspace_tab_count(&workspace));
        assert!(controls.can_split);
    }
    #[test]
    fn palette_commands_and_feature_projection_are_deterministic() {
        let mut c = AppController::new();
        c.settings = SettingsState::default();
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
