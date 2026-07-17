//! GPUI-independent state and events for the settings window.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Category {
    About,
    FilesAndLinks,
    Appearance,
    Editor,
    Hotkeys,
    CorePlugins,
}

impl Category {
    pub const ALL: [Self; 6] = [
        Self::About,
        Self::FilesAndLinks,
        Self::Appearance,
        Self::Editor,
        Self::Hotkeys,
        Self::CorePlugins,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::About => "About",
            Self::FilesAndLinks => "Files and links",
            Self::Appearance => "Appearance",
            Self::Editor => "Editor",
            Self::Hotkeys => "Hotkeys",
            Self::CorePlugins => "Core plugins",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Theme {
    #[default]
    Light,
    System,
    Dark,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Hotkey {
    pub id: &'static str,
    pub label: &'static str,
    pub shortcut: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EditorSettings {
    pub spellcheck: bool,
    pub readable_line_length: bool,
    pub strict_line_breaks: bool,
    pub fold_heading: bool,
    pub fold_indent: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FilesAndLinksSettings {
    pub confirm_file_deletion: bool,
    pub default_new_note_location: NewNoteLocation,
    pub new_link_format: NewLinkFormat,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum NewNoteLocation {
    #[default]
    Root,
    CurrentFolder,
    SpecifiedFolder,
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum NewLinkFormat {
    #[default]
    ShortestPath,
    RelativePath,
    AbsolutePath,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SettingsEvent {
    CategoryChanged(Category),
    ThemeChanged(Theme),
    AccentChanged(String),
    EditorChanged(EditorSettings),
    FilesAndLinksChanged(FilesAndLinksSettings),
    HotkeyChanged {
        id: String,
        shortcut: Option<String>,
    },
    PluginChanged {
        id: String,
        enabled: bool,
    },
    SearchChanged(String),
    CloseRequested,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CorePlugin {
    pub id: &'static str,
    pub label: &'static str,
    pub enabled: bool,
}

const HOTKEYS: &[(&str, &str, Option<&str>)] = &[
    ("open-settings", "Open settings", Some("Ctrl+,")),
    ("toggle-sidebar", "Toggle sidebar", Some("Ctrl+\\")),
    ("quick-switcher", "Quick switcher", Some("Ctrl+O")),
    ("command-palette", "Command palette", Some("Ctrl+P")),
    ("search", "Search in all files", Some("Ctrl+Shift+F")),
    ("close-pane", "Close current pane", Some("Ctrl+W")),
];

// The built-in plugins exposed by Obsidian's core-plugin settings.  Keeping
// this as data makes the list easy for a view to render without GPUI types.
const PLUGINS: &[(&str, &str)] = &[
    ("file-explorer", "File explorer"),
    ("global-search", "Global search"),
    ("switcher", "Quick switcher"),
    ("graph", "Graph view"),
    ("backlink", "Backlinks"),
    ("canvas", "Canvas"),
    ("outgoing-link", "Outgoing links"),
    ("tag-pane", "Tags view"),
    ("page-preview", "Page preview"),
    ("daily-notes", "Daily notes"),
    ("templates", "Templates"),
    ("note-composer", "Note composer"),
    ("command-palette", "Command palette"),
    ("slash-command", "Slash commands"),
    ("random-note", "Random note"),
    ("outline", "Outline"),
    ("word-count", "Word count"),
    ("audio-recorder", "Audio recorder"),
    ("workspaces", "Workspaces"),
    ("file-recovery", "File recovery"),
    ("unique-note-creator", "Unique note creator"),
    ("properties", "Properties view"),
    ("bookmarks", "Bookmarks"),
    ("footnotes", "Footnotes"),
    ("markdown-importer", "Markdown importer"),
    ("slides", "Slides"),
    ("publish", "Publish"),
    ("sync", "Sync"),
    ("webviewer", "Web viewer"),
    ("bases", "Bases"),
];

#[derive(Clone, Debug)]
pub struct Settings {
    pub category: Category,
    pub theme: Theme,
    pub accent: String,
    pub editor: EditorSettings,
    pub files_and_links: FilesAndLinksSettings,
    pub hotkeys: Vec<Hotkey>,
    pub plugins: Vec<CorePlugin>,
    search: String,
    events: Vec<SettingsEvent>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            category: Category::Appearance,
            theme: Theme::Light,
            accent: "#7c3aed".into(),
            editor: EditorSettings {
                spellcheck: true,
                readable_line_length: false,
                strict_line_breaks: false,
                fold_heading: true,
                fold_indent: true,
            },
            files_and_links: FilesAndLinksSettings {
                confirm_file_deletion: true,
                default_new_note_location: NewNoteLocation::Root,
                new_link_format: NewLinkFormat::ShortestPath,
            },
            hotkeys: HOTKEYS
                .iter()
                .map(|&(id, label, shortcut)| Hotkey {
                    id,
                    label,
                    shortcut: shortcut.map(str::to_owned),
                })
                .collect(),
            plugins: PLUGINS
                .iter()
                .map(|&(id, label)| CorePlugin {
                    id,
                    label,
                    enabled: true,
                })
                .collect(),
            search: String::new(),
            events: Vec::new(),
        }
    }
}

impl Settings {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn search(&self) -> &str {
        &self.search
    }
    pub fn set_category(&mut self, category: Category) {
        self.category = category;
        self.events.push(SettingsEvent::CategoryChanged(category));
    }
    pub fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
        self.events.push(SettingsEvent::ThemeChanged(theme));
    }
    pub fn set_accent<S: Into<String>>(&mut self, accent: S) {
        let accent = accent.into();
        self.accent = accent.clone();
        self.events.push(SettingsEvent::AccentChanged(accent));
    }
    pub fn set_search<S: Into<String>>(&mut self, search: S) {
        self.search = search
            .into()
            .chars()
            .filter(|ch| !ch.is_control())
            .collect();
        self.events
            .push(SettingsEvent::SearchChanged(self.search.clone()));
    }
    pub fn set_editor(&mut self, editor: EditorSettings) {
        self.editor = editor;
        self.events.push(SettingsEvent::EditorChanged(editor));
    }
    pub fn toggle_spellcheck(&mut self) {
        let mut editor = self.editor;
        editor.spellcheck = !editor.spellcheck;
        self.set_editor(editor);
    }
    pub fn toggle_readable_line_length(&mut self) {
        let mut editor = self.editor;
        editor.readable_line_length = !editor.readable_line_length;
        self.set_editor(editor);
    }
    pub fn toggle_strict_line_breaks(&mut self) {
        let mut editor = self.editor;
        editor.strict_line_breaks = !editor.strict_line_breaks;
        self.set_editor(editor);
    }
    pub fn toggle_fold_heading(&mut self) {
        let mut editor = self.editor;
        editor.fold_heading = !editor.fold_heading;
        self.set_editor(editor);
    }
    pub fn toggle_fold_indent(&mut self) {
        let mut editor = self.editor;
        editor.fold_indent = !editor.fold_indent;
        self.set_editor(editor);
    }
    pub fn set_files_and_links(&mut self, value: FilesAndLinksSettings) {
        self.files_and_links = value;
        self.events.push(SettingsEvent::FilesAndLinksChanged(value));
    }
    pub fn set_hotkey(&mut self, id: String, shortcut: Option<String>) {
        if let Some(h) = self.hotkeys.iter_mut().find(|h| h.id == id) {
            h.shortcut = shortcut.clone();
        }
        self.events
            .push(SettingsEvent::HotkeyChanged { id, shortcut });
    }
    pub fn set_plugin_enabled(&mut self, id: &str, enabled: bool) {
        if let Some(p) = self.plugins.iter_mut().find(|p| p.id == id) {
            p.enabled = enabled;
            self.events.push(SettingsEvent::PluginChanged {
                id: id.into(),
                enabled,
            });
        }
    }
    pub fn toggle_plugin(&mut self, id: &str) {
        if let Some(plugin) = self.plugins.iter().find(|p| p.id == id) {
            self.set_plugin_enabled(id, !plugin.enabled);
        }
    }
    pub fn toggle_plugin_by_name(&mut self, label: &str) {
        if let Some(plugin) = self.plugins.iter().find(|p| p.label == label) {
            let id = plugin.id;
            self.toggle_plugin(id);
        }
    }
    pub fn filtered_hotkeys(&self) -> Vec<&Hotkey> {
        let q = self.search.to_lowercase();
        self.hotkeys
            .iter()
            .filter(|h| q.is_empty() || h.label.to_lowercase().contains(&q) || h.id.contains(&q))
            .collect()
    }
    pub fn filtered_plugins(&self) -> Vec<&CorePlugin> {
        let q = self.search.to_lowercase();
        self.plugins
            .iter()
            .filter(|p| q.is_empty() || p.label.to_lowercase().contains(&q) || p.id.contains(&q))
            .collect()
    }
    pub fn request_close(&mut self) {
        self.events.push(SettingsEvent::CloseRequested);
    }
    pub fn close(&mut self) {
        self.request_close();
    }
    pub fn drain_events(&mut self) -> Vec<SettingsEvent> {
        std::mem::take(&mut self.events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn defaults_are_obsidian_like() {
        let s = Settings::default();
        assert_eq!(s.theme, Theme::Light);
        assert_eq!(s.plugins.len(), PLUGINS.len());
        assert!(s.plugins.iter().all(|p| p.enabled));
    }
    #[test]
    fn filtering_and_events_work() {
        let mut s = Settings::default();
        s.set_search("graph");
        assert_eq!(s.filtered_plugins()[0].id, "graph");
        s.set_theme(Theme::Dark);
        assert!(matches!(
            s.drain_events()[1],
            SettingsEvent::ThemeChanged(Theme::Dark)
        ));
    }

    #[test]
    fn search_ignores_control_characters() {
        let mut s = Settings::default();

        s.set_search("gr\n\0ap\u{7}h");

        assert_eq!(s.search(), "graph");
        assert_eq!(
            s.drain_events(),
            vec![SettingsEvent::SearchChanged("graph".into())]
        );
    }
}
