//! Render-neutral state and reducer for a desktop workspace.
//!
//! The crate deliberately has no UI dependencies.  A GPUI adapter can render a
//! [`Workspace`] and execute the deterministic [`Effect`]s returned by
//! [`Workspace::dispatch`].

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

mod api;
mod history;
mod view_types;

pub use api::*;
pub use history::{ClosedTabStack, NavigationHistory};
pub use view_types::{ViewDescriptor, ViewKind};

pub type Id = String;

fn id() -> Id {
    Uuid::new_v4().to_string()
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct ExtraFields(pub BTreeMap<String, Value>);

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Orientation {
    Horizontal,
    Vertical,
}

impl From<SplitOrientation> for Orientation {
    fn from(value: SplitOrientation) -> Self {
        match value {
            SplitOrientation::Horizontal => Self::Horizontal,
            SplitOrientation::Vertical => Self::Vertical,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum LayoutNode {
    Split {
        orientation: Orientation,
        ratio: f32,
        first: Box<LayoutNode>,
        second: Box<LayoutNode>,
    },
    Tabs(TabGroup),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct TabGroup {
    pub id: Id,
    pub tabs: Vec<Tab>,
    pub active: usize,
    #[serde(default)]
    pub stacked: bool,
    #[serde(default)]
    pub extras: ExtraFields,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Tab {
    pub id: Id,
    pub view: View,
    /// Per-leaf navigation state.  The current view is mirrored in `view` so
    /// existing adapters can continue to render the compact tab model.
    #[serde(default)]
    pub history: NavigationHistory<View>,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub extras: ExtraFields,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct View {
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default)]
    pub state: Value,
    #[serde(default)]
    pub extras: ExtraFields,
}

impl View {
    pub fn new(kind: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            path: None,
            state: Value::Null,
            extras: ExtraFields::default(),
        }
    }

    pub fn with_path(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }

    pub fn with_state(mut self, state: Value) -> Self {
        self.state = state;
        self
    }

    pub fn descriptor(&self) -> ViewDescriptor {
        ViewDescriptor {
            kind: ViewKind::from(self.kind.clone()),
            path: self.path.clone(),
            state: self.state.clone(),
        }
    }

    pub fn from_descriptor(descriptor: ViewDescriptor) -> Self {
        Self {
            kind: descriptor.kind.to_kind_string(),
            path: descriptor.path,
            state: descriptor.state,
            extras: ExtraFields::default(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Window {
    pub id: Id,
    pub root: LayoutNode,
    pub bounds: Bounds,
    #[serde(default)]
    pub popout: bool,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub extras: ExtraFields,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Bounds {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct RibbonItem {
    pub id: Id,
    pub label: String,
    pub icon: String,
    pub command: Option<String>,
    #[serde(default)]
    pub extras: ExtraFields,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SidebarTab {
    pub id: Id,
    pub label: String,
    pub view: View,
    pub open: bool,
    #[serde(default)]
    pub extras: ExtraFields,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct StatusItem {
    pub id: Id,
    pub label: String,
    pub value: Option<String>,
    #[serde(default)]
    pub extras: ExtraFields,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct KeyChord {
    pub key: String,
    #[serde(default)]
    pub ctrl: bool,
    #[serde(default)]
    pub alt: bool,
    #[serde(default)]
    pub shift: bool,
    #[serde(default)]
    pub meta: bool,
}
impl KeyChord {
    pub fn new(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            ctrl: false,
            alt: false,
            shift: false,
            meta: false,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Command {
    pub id: String,
    pub label: String,
    pub hotkey: Option<KeyChord>,
    pub enabled: bool,
    pub core_feature: Option<String>,
    #[serde(default)]
    pub extras: ExtraFields,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct MenuItem {
    pub label: String,
    pub command: Option<String>,
    pub enabled: bool,
    #[serde(default)]
    pub children: Vec<MenuItem>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Menu {
    pub id: String,
    pub items: Vec<MenuItem>,
}
/// A hit-test result produced by a renderer during a drag operation.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum DockTarget {
    Group(Id),
    Split {
        group: Id,
        orientation: Orientation,
        before: bool,
    },
    Sidebar,
    NewWindow,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ContextMenu {
    pub id: String,
    pub anchor_label: String,
    pub items: Vec<MenuItem>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Notice {
    pub id: Id,
    pub message: String,
    pub level: NoticeLevel,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum NoticeLevel {
    Info,
    Success,
    Warning,
    Error,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Modal {
    pub id: Id,
    pub title: String,
    pub body: String,
    pub actions: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Theme {
    pub name: String,
    pub dark: bool,
    pub accent: String,
    #[serde(default)]
    pub vars: BTreeMap<String, String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Appearance {
    pub theme: Theme,
    pub font_size: f32,
    pub reduced_motion: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SettingsNavigation {
    pub selected: String,
    pub sections: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Workspace {
    pub schema_version: u32,
    pub windows: Vec<Window>,
    pub focused: Option<(Id, Id)>,
    pub mru: Vec<Id>,
    /// Most recently closed leaves, retained for an Obsidian-style reopen.
    #[serde(default)]
    pub closed: ClosedTabStack<Tab>,
    pub ribbons: Vec<RibbonItem>,
    pub sidebar: Vec<SidebarTab>,
    pub status: Vec<StatusItem>,
    pub commands: Vec<Command>,
    #[serde(default)]
    pub view_kinds: BTreeSet<String>,
    pub features: BTreeMap<String, bool>,
    pub appearance: Appearance,
    pub settings: SettingsNavigation,
    #[serde(default)]
    pub notices: Vec<Notice>,
    #[serde(default)]
    pub modals: Vec<Modal>,
    #[serde(default)]
    pub extras: ExtraFields,
}

impl Default for Workspace {
    fn default() -> Self {
        Self {
            schema_version: 1,
            windows: vec![Window {
                id: id(),
                root: LayoutNode::Tabs(TabGroup {
                    id: id(),
                    tabs: vec![],
                    active: 0,
                    stacked: false,
                    extras: ExtraFields::default(),
                }),
                bounds: Bounds {
                    x: 0.0,
                    y: 0.0,
                    width: 1200.0,
                    height: 800.0,
                },
                popout: false,
                title: None,
                extras: ExtraFields::default(),
            }],
            focused: None,
            mru: vec![],
            closed: ClosedTabStack::default(),
            ribbons: vec![],
            sidebar: vec![],
            status: vec![],
            commands: vec![],
            view_kinds: BTreeSet::new(),
            features: BTreeMap::new(),
            appearance: Appearance {
                theme: Theme {
                    name: "default".into(),
                    dark: false,
                    accent: "#7c3aed".into(),
                    vars: BTreeMap::new(),
                },
                font_size: 14.0,
                reduced_motion: false,
            },
            settings: SettingsNavigation {
                selected: "general".into(),
                sections: vec!["general".into()],
            },
            notices: vec![],
            modals: vec![],
            extras: ExtraFields::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Open {
        window: Option<Id>,
        view: View,
    },
    /// Open a view in the selected group, optionally forcing a new tab.
    OpenWith {
        window: Option<Id>,
        view: View,
        new_tab: bool,
    },
    /// Open a path-backed view, reusing an existing matching tab unless forced.
    OpenPath {
        window: Option<Id>,
        path: String,
        new_tab: bool,
    },
    Close {
        tab: Id,
    },
    CloseScope {
        tab: Id,
        scope: CloseScope,
    },
    Reorder {
        tab: Id,
        index: usize,
    },
    Split {
        tab: Id,
        orientation: Orientation,
    },
    Move {
        tab: Id,
        target_group: Id,
    },
    Duplicate {
        tab: Id,
    },
    Navigate {
        tab: Id,
        view: View,
    },
    History {
        tab: Id,
        direction: HistoryCommand,
    },
    ReopenClosed,
    Dock {
        tab: Id,
        target: DockTarget,
    },
    Focus {
        tab: Id,
    },
    Pin {
        tab: Id,
        pinned: bool,
    },
    Stack {
        group: Id,
        stacked: bool,
    },
    Popout {
        tab: Id,
    },
    SetFeature {
        feature: String,
        enabled: bool,
    },
    RegisterCommand(Command),
    InvokeCommand(String),
    Hotkey(KeyChord),
    Restore(String),
    Save(String),
    DismissNotice(Id),
    CloseModal(Id),
    Noop,
}
#[derive(Clone, Debug, PartialEq)]
pub enum Effect {
    ExecuteCommand(String),
    Persist(String),
    UnknownView(String),
    Focus(Id),
    Notice(String),
    Error(String),
}

impl Workspace {
    /// Open a view using the default reuse behavior.
    pub fn open(&mut self, view: View) -> Id {
        self.dispatch(Action::OpenWith {
            window: None,
            view,
            new_tab: false,
        });
        self.focused_tab_id().unwrap_or_default()
    }

    /// Open a view, with `new_tab` representing the modifier-key behavior.
    pub fn open_with_modifier(&mut self, view: View, new_tab: bool) -> Id {
        self.dispatch(Action::OpenWith {
            window: None,
            view,
            new_tab,
        });
        self.focused_tab_id().unwrap_or_default()
    }

    /// Open a markdown path, reusing the selected leaf unless forced new.
    pub fn open_reusable_path(&mut self, path: impl Into<String>) -> Id {
        let path = path.into();
        if let Some((window, tab)) = self.windows.iter().find_map(|w| {
            find_tab_with_path(&w.root, &path).map(|tab| (w.id.clone(), tab.id.clone()))
        }) {
            self.focus(&tab);
            self.focused = Some((window, tab.clone()));
            return tab;
        }
        self.open_with_modifier(
            View {
                kind: "markdown".into(),
                path: Some(path),
                state: Value::Null,
                extras: ExtraFields::default(),
            },
            false,
        )
    }

    pub fn close(&mut self, tab: &str) {
        self.dispatch(Action::Close { tab: tab.into() });
    }

    pub fn close_others(&mut self, tab: &str) {
        self.dispatch(Action::CloseScope {
            tab: tab.into(),
            scope: CloseScope::Others,
        });
    }

    pub fn close_right(&mut self, tab: &str) {
        self.dispatch(Action::CloseScope {
            tab: tab.into(),
            scope: CloseScope::ToRight,
        });
    }

    pub fn reorder(&mut self, tab: &str, index: usize) {
        self.dispatch(Action::Reorder {
            tab: tab.into(),
            index,
        });
    }

    pub fn split(&mut self, tab: &str, orientation: Orientation) {
        self.dispatch(Action::Split {
            tab: tab.into(),
            orientation,
        });
    }

    pub fn move_to_group(&mut self, tab: &str, target_group: &str) {
        self.dispatch(Action::Move {
            tab: tab.into(),
            target_group: target_group.into(),
        });
    }

    pub fn pin(&mut self, tab: &str, pinned: bool) {
        self.dispatch(Action::Pin {
            tab: tab.into(),
            pinned,
        });
    }

    pub fn duplicate(&mut self, tab: &str) -> Id {
        self.dispatch(Action::Duplicate { tab: tab.into() });
        self.focused_tab_id().unwrap_or_default()
    }

    pub fn back(&mut self) {
        if let Some(tab) = self.focused_tab_id() {
            self.dispatch(Action::History {
                tab,
                direction: HistoryCommand::Back,
            });
        }
    }

    pub fn navigate(&mut self, tab: &str, view: View) {
        self.dispatch(Action::Navigate {
            tab: tab.into(),
            view,
        });
    }

    pub fn forward(&mut self) {
        if let Some(tab) = self.focused_tab_id() {
            self.dispatch(Action::History {
                tab,
                direction: HistoryCommand::Forward,
            });
        }
    }

    pub fn reopen_last_closed(&mut self) -> Option<Id> {
        let before = self.closed.len();
        self.dispatch(Action::ReopenClosed);
        (self.closed.len() < before)
            .then(|| self.focused_tab_id())
            .flatten()
    }

    pub fn focused_tab_id(&self) -> Option<Id> {
        self.focused.as_ref().map(|(_, tab)| tab.clone())
    }

    pub fn active_group_id(&self) -> Option<Id> {
        let (window, tab) = self.focused.as_ref()?;
        self.windows
            .iter()
            .find(|item| &item.id == window)
            .and_then(|item| group_id_for_tab(&item.root, tab))
    }

    pub fn view_kinds(&self) -> &BTreeSet<String> {
        &self.view_kinds
    }

    fn open_view(
        &mut self,
        window_id: Option<&str>,
        view: View,
        new_tab: bool,
        effects: &mut Vec<Effect>,
    ) {
        if let Some(id) = window_id {
            if !self.windows.iter().any(|window| window.id == id) {
                effects.push(Effect::Error(format!("unknown window: {id}")));
                return;
            }
        }
        if !self.view_kinds.is_empty()
            && !self.view_kinds.contains(&view.kind)
            && !is_builtin_view_kind(&view.kind)
        {
            effects.push(Effect::UnknownView(view.kind));
            return;
        }
        if self.features.get(&view.kind).copied() == Some(false) {
            effects.push(Effect::Error(format!("feature disabled: {}", view.kind)));
            return;
        }
        if is_builtin_view_kind(&view.kind) {
            self.view_kinds.insert(view.kind.clone());
        }
        if !new_tab {
            if let Some((window, tab)) = self.windows.iter().find_map(|window| {
                find_tab_with_view(&window.root, &view)
                    .map(|tab| (window.id.clone(), tab.id.clone()))
            }) {
                self.focus(&tab);
                self.focused = Some((window, tab));
                return;
            }
        }
        let selected = self.focused.as_ref().map(|(_, tab)| tab.clone());
        let target_window = window_id
            .and_then(|id| self.windows.iter().position(|w| w.id == id))
            .or_else(|| {
                self.focused
                    .as_ref()
                    .and_then(|(id, _)| self.windows.iter().position(|w| &w.id == id))
            })
            .unwrap_or(0);
        let Some(window) = self.windows.get_mut(target_window) else {
            effects.push(Effect::Error("workspace has no window".into()));
            return;
        };
        let target = selected
            .as_deref()
            .and_then(|tab| group_id_for_tab(&window.root, tab))
            .or_else(|| first_group_id(&window.root));
        let Some(group_id) = target else {
            effects.push(Effect::Error("workspace has no tab group".into()));
            return;
        };
        let group = find_group_mut(&mut window.root, &group_id).expect("group just found");
        let mut history = NavigationHistory::default();
        history.visit(view.clone());
        let tab = Tab {
            id: id(),
            view,
            history,
            pinned: false,
            title: None,
            extras: ExtraFields::default(),
        };
        let tab_id = tab.id.clone();
        group.tabs.push(tab);
        group.active = group.tabs.len() - 1;
        self.focused = Some((window.id.clone(), tab_id.clone()));
        self.mru.retain(|x| x != &tab_id);
        self.mru.insert(0, tab_id);
    }

    fn close_scope(&mut self, tab: &str, scope: CloseScope) {
        let Some((window_id, group_id, index)) = self.tab_location(tab) else {
            return;
        };
        let ids: Vec<Id> = {
            let window = self.windows.iter().find(|w| w.id == window_id).unwrap();
            let group = find_group(&window.root, &group_id).unwrap();
            match scope {
                CloseScope::Single => vec![tab.into()],
                CloseScope::Others => group
                    .tabs
                    .iter()
                    .filter(|item| item.id != tab && !item.pinned)
                    .map(|item| item.id.clone())
                    .collect(),
                CloseScope::ToRight => group
                    .tabs
                    .iter()
                    .skip(index + 1)
                    .filter(|item| !item.pinned)
                    .map(|item| item.id.clone())
                    .collect(),
            }
        };
        for id in ids {
            self.remove_tab(&id);
        }
    }

    fn reorder_tab(&mut self, tab: &str, index: usize) {
        let Some((_, group_id, _)) = self.tab_location(tab) else {
            return;
        };
        let Some(group) = self.find_group_mut(&group_id) else {
            return;
        };
        let Some(from) = group.tabs.iter().position(|item| item.id == tab) else {
            return;
        };
        let item = group.tabs.remove(from);
        let pinned = item.pinned;
        let first_unpinned = group
            .tabs
            .iter()
            .position(|candidate| !candidate.pinned)
            .unwrap_or(group.tabs.len());
        let target = if pinned {
            index.min(first_unpinned)
        } else {
            index.max(first_unpinned).min(group.tabs.len())
        };
        group.tabs.insert(target, item);
        group.active = target;
    }

    fn set_pinned(&mut self, tab: &str, pinned: bool) {
        let Some((window_id, group_id, index)) = self.tab_location(tab) else {
            return;
        };
        let Some(window) = self.windows.iter_mut().find(|item| item.id == window_id) else {
            return;
        };
        let Some(group) = find_group_mut(&mut window.root, &group_id) else {
            return;
        };
        let mut item = group.tabs.remove(index);
        item.pinned = pinned;
        let target = if pinned {
            group
                .tabs
                .iter()
                .position(|candidate| !candidate.pinned)
                .unwrap_or(group.tabs.len())
        } else {
            group.tabs.len()
        };
        let item_id = item.id.clone();
        group.tabs.insert(target, item);
        group.active = target;
        self.focused = Some((window_id, item_id));
    }

    fn duplicate_tab(&mut self, tab: &str) {
        let Some((window_id, group_id, index)) = self.tab_location(tab) else {
            return;
        };
        let Some(window) = self.windows.iter_mut().find(|w| w.id == window_id) else {
            return;
        };
        let Some(group) = find_group_mut(&mut window.root, &group_id) else {
            return;
        };
        let Some(mut duplicate) = group.tabs.get(index).cloned() else {
            return;
        };
        duplicate.id = id();
        duplicate.pinned = false;
        let duplicate_id = duplicate.id.clone();
        group.tabs.insert(index + 1, duplicate);
        group.active = index + 1;
        self.focused = Some((window_id, duplicate_id.clone()));
        self.mru.retain(|x| x != &duplicate_id);
        self.mru.insert(0, duplicate_id);
    }

    fn reopen_last_closed_internal(&mut self) {
        let Some(tab) = self.closed.reopen() else {
            return;
        };
        let target_window = self
            .focused
            .as_ref()
            .and_then(|(window, _)| self.windows.iter().position(|item| &item.id == window))
            .unwrap_or(0);
        let Some(window) = self.windows.get_mut(target_window) else {
            return;
        };
        let Some(group_id) = self
            .focused
            .as_ref()
            .and_then(|(_, id)| group_id_for_tab(&window.root, id))
            .or_else(|| first_group_id(&window.root))
        else {
            return;
        };
        let group = find_group_mut(&mut window.root, &group_id).unwrap();
        let tab_id = tab.id.clone();
        let pinned = tab.pinned;
        group.tabs.push(tab);
        group.active = group.tabs.len() - 1;
        self.focused = Some((window.id.clone(), tab_id.clone()));
        self.mru.insert(0, tab_id);
        if pinned {
            self.set_pinned(self.focused_tab_id().as_deref().unwrap_or_default(), true);
        }
    }

    fn tab_location(&self, tab: &str) -> Option<(Id, Id, usize)> {
        self.windows.iter().find_map(|window| {
            find_group_location(&window.root, tab)
                .map(|(group, index)| (window.id.clone(), group, index))
        })
    }

    pub fn dispatch(&mut self, action: Action) -> Vec<Effect> {
        let mut effects = Vec::new();
        // Focus is observational: an invalid target must be a strict no-op.
        // In particular, do not run the post-action normalizer, since that
        // would make an unsuccessful focus request mutate persisted state.
        if let Action::Focus { tab } = &action {
            if self.find_tab(tab).is_none() {
                return effects;
            }
        }
        match action {
            Action::Open { window, view } => {
                if let Some(window_id) = window.as_ref() {
                    if !self.windows.iter().any(|item| &item.id == window_id) {
                        effects.push(Effect::Error(format!("unknown window: {window_id}")));
                        return effects;
                    }
                }
                if !self.view_kinds.is_empty()
                    && !self.view_kinds.contains(&view.kind)
                    && !is_builtin_view_kind(&view.kind)
                {
                    effects.push(Effect::UnknownView(view.kind.clone()));
                    return effects;
                }
                if self.features.get(&view.kind).copied() == Some(false) {
                    effects.push(Effect::Error(format!("feature disabled: {}", view.kind)));
                    return effects;
                }
                if is_builtin_view_kind(&view.kind) {
                    self.view_kinds.insert(view.kind.clone());
                }
                let index = match window {
                    Some(x) => match self.windows.iter().position(|w| w.id == x) {
                        Some(i) => i,
                        None => {
                            effects.push(Effect::Error(format!("unknown window: {x}")));
                            return effects;
                        }
                    },
                    None => 0,
                };
                if let Some(w) = self.windows.get_mut(index) {
                    if let LayoutNode::Tabs(g) = &mut w.root {
                        g.tabs.push(Tab {
                            id: id(),
                            history: {
                                let mut history = NavigationHistory::default();
                                history.visit(view.clone());
                                history
                            },
                            view,
                            pinned: false,
                            title: None,
                            extras: ExtraFields::default(),
                        });
                        g.active = g.tabs.len() - 1;
                    }
                }
            }
            Action::OpenWith {
                window,
                view,
                new_tab,
            } => {
                self.open_view(window.as_deref(), view, new_tab, &mut effects);
            }
            Action::OpenPath {
                window,
                path,
                new_tab,
            } => {
                let mut view = View::new("markdown");
                view.path = Some(path);
                self.open_view(window.as_deref(), view, new_tab, &mut effects);
            }
            Action::Close { tab } => {
                self.remove_tab(&tab);
            }
            Action::CloseScope { tab, scope } => {
                self.close_scope(&tab, scope);
            }
            Action::Reorder { tab, index } => {
                self.reorder_tab(&tab, index);
            }
            Action::Split { tab, orientation } => {
                if tab.is_empty() || !self.split_tab(&tab, orientation) {
                    effects.push(Effect::Error(
                        "split requires an existing tab and a non-empty sibling".into(),
                    ));
                }
            }
            Action::Move { tab, target_group } => {
                self.move_tab(&tab, &target_group);
            }
            Action::Duplicate { tab } => {
                self.duplicate_tab(&tab);
            }
            Action::Navigate { tab, view } => {
                if let Some(t) = self.find_tab_mut(&tab) {
                    t.history.visit(view.clone());
                    t.view = view;
                }
            }
            Action::History { tab, direction } => {
                if let Some(t) = self.find_tab_mut(&tab) {
                    let view = match direction {
                        HistoryCommand::Back => t.history.back(),
                        HistoryCommand::Forward => t.history.forward(),
                    };
                    if let Some(view) = view {
                        t.view = view;
                    }
                }
            }
            Action::ReopenClosed => {
                self.reopen_last_closed_internal();
            }
            Action::Dock { tab, target } => match target {
                DockTarget::Group(group) => {
                    self.move_tab(&tab, &group);
                }
                DockTarget::Split {
                    group,
                    orientation,
                    before,
                } => {
                    let target_count = self.group_tab_count(&group);
                    let same_target = self.group_has_tab(&group, &tab);
                    if target_count.is_none()
                        || target_count.unwrap_or(0) + usize::from(!same_target) < 2
                    {
                        effects.push(Effect::Error(
                            "split requires two tabs in the target group".into(),
                        ));
                    } else if (same_target || self.move_tab(&tab, &group))
                        && !self.split_group(&group, &tab, orientation, before)
                    {
                        effects.push(Effect::Error("split failed".into()));
                    }
                }
                DockTarget::Sidebar => {
                    if let Some(t) = self.take_tab(&tab) {
                        self.sidebar.push(SidebarTab {
                            id: t.id,
                            label: t.title.unwrap_or(t.view.kind.clone()),
                            view: t.view,
                            open: true,
                            extras: ExtraFields::default(),
                        });
                    }
                }
                DockTarget::NewWindow => {
                    if let Some(t) = self.take_tab(&tab) {
                        self.windows.push(Window {
                            id: id(),
                            root: LayoutNode::Tabs(TabGroup {
                                id: id(),
                                tabs: vec![t],
                                active: 0,
                                stacked: false,
                                extras: ExtraFields::default(),
                            }),
                            bounds: Bounds {
                                x: 80.0,
                                y: 80.0,
                                width: 800.0,
                                height: 600.0,
                            },
                            popout: true,
                            title: None,
                            extras: ExtraFields::default(),
                        });
                    }
                }
            },
            Action::Focus { tab } => {
                if self.find_tab(&tab).is_some() {
                    self.focus(&tab);
                    effects.push(Effect::Focus(tab));
                }
            }
            Action::Pin { tab, pinned } => {
                self.set_pinned(&tab, pinned);
            }
            Action::Stack { group, stacked } => {
                if let Some(g) = self.find_group_mut(&group) {
                    g.stacked = stacked;
                }
            }
            Action::Popout { tab } => {
                if let Some(t) = self.take_tab(&tab) {
                    self.windows.push(Window {
                        id: id(),
                        root: LayoutNode::Tabs(TabGroup {
                            id: id(),
                            tabs: vec![t],
                            active: 0,
                            stacked: false,
                            extras: ExtraFields::default(),
                        }),
                        bounds: Bounds {
                            x: 80.0,
                            y: 80.0,
                            width: 800.0,
                            height: 600.0,
                        },
                        popout: true,
                        title: None,
                        extras: ExtraFields::default(),
                    });
                }
            }
            Action::SetFeature { feature, enabled } => {
                self.features.insert(feature.clone(), enabled);
                if enabled {
                    self.commands
                        .iter_mut()
                        .filter(|c| c.core_feature.as_deref() == Some(&feature))
                        .for_each(|c| c.enabled = true);
                } else {
                    self.disable_feature(&feature);
                }
            }
            Action::RegisterCommand(c) => {
                self.commands.retain(|x| x.id != c.id);
                self.commands.push(c);
            }
            Action::InvokeCommand(c) => {
                if self.commands.iter().any(|x| {
                    x.id == c && x.enabled && self.feature_enabled(x.core_feature.as_deref())
                }) {
                    effects.push(Effect::ExecuteCommand(c));
                }
            }
            Action::Hotkey(key) => {
                let ids: Vec<_> = self
                    .commands
                    .iter()
                    .filter(|c| {
                        c.enabled
                            && c.hotkey.as_ref() == Some(&key)
                            && self.feature_enabled(c.core_feature.as_deref())
                    })
                    .map(|c| c.id.clone())
                    .collect();
                if ids.len() == 1 {
                    effects.push(Effect::ExecuteCommand(ids[0].clone()));
                } else if ids.len() > 1 {
                    effects.push(Effect::Notice("hotkey conflict".into()));
                }
            }
            Action::Restore(s) => match serde_json::from_str::<Workspace>(&s) {
                Ok(mut loaded) if loaded.schema_version <= 1 => {
                    migrate(&mut loaded);
                    if loaded.validate() {
                        *self = loaded;
                    } else {
                        effects.push(Effect::Error("invalid workspace layout".into()));
                    }
                }
                Ok(loaded) => effects.push(Effect::Error(format!(
                    "unsupported workspace schema {}",
                    loaded.schema_version
                ))),
                Err(error) => effects.push(Effect::Error(format!("restore failed: {error}"))),
            },
            Action::Save(path) => {
                if let Ok(s) = serde_json::to_string(self) {
                    effects.push(Effect::Persist(s));
                    effects.push(Effect::Notice(format!("workspace saved: {path}")));
                }
            }
            Action::DismissNotice(n) => self.notices.retain(|x| x.id != n),
            Action::CloseModal(n) => self.modals.retain(|x| x.id != n),
            Action::Noop => {}
        }
        self.normalize();
        effects
    }

    pub fn validate(&self) -> bool {
        let mut windows = BTreeSet::new();
        let mut groups = BTreeSet::new();
        let mut tabs = BTreeSet::new();
        self.windows.iter().all(|w| {
            !w.id.is_empty()
                && windows.insert(w.id.clone())
                && valid_node_ids(&w.root, &mut groups, &mut tabs)
        }) && self.focused.as_ref().is_none_or(|(window, tab)| {
            self.windows
                .iter()
                .any(|w| &w.id == window && find_tab(&w.root, tab).is_some())
        })
    }
    fn feature_enabled(&self, f: Option<&str>) -> bool {
        f.map(|x| self.features.get(x).copied().unwrap_or(true))
            .unwrap_or(true)
    }
    fn disable_feature(&mut self, f: &str) {
        self.commands
            .iter_mut()
            .filter(|c| c.core_feature.as_deref() == Some(f))
            .for_each(|c| c.enabled = false);
        self.windows
            .iter_mut()
            .for_each(|w| remove_matching(&mut w.root, |t| t.view.kind == f));
        self.sidebar.retain(|s| s.view.kind != f);
    }
    fn normalize(&mut self) {
        let mut windows = Vec::new();
        for mut w in self.windows.drain(..) {
            if let Some(root) = clean_node(w.root) {
                w.root = root;
                windows.push(w);
            }
        }
        self.windows = windows;
        let ids: BTreeSet<_> = self.windows.iter().flat_map(|w| tab_ids(&w.root)).collect();
        self.mru.retain(|x| ids.contains(x));
        if self.focused.as_ref().is_some_and(|(_, t)| !ids.contains(t)) {
            self.focused = None;
        }
    }
    pub fn focus(&mut self, tab: &str) {
        if self.find_tab(tab).is_some() {
            self.focused = self.find_tab(tab).map(|(w, _)| (w.id.clone(), tab.into()));
            for window in &mut self.windows {
                set_group_active(&mut window.root, tab);
            }
            self.mru.retain(|x| x != tab);
            self.mru.insert(0, tab.into());
        }
    }
    fn find_tab(&self, tab: &str) -> Option<(&Window, &Tab)> {
        self.windows
            .iter()
            .find_map(|w| find_tab(&w.root, tab).map(|t| (w, t)))
    }
    fn find_tab_mut(&mut self, tab: &str) -> Option<&mut Tab> {
        self.windows
            .iter_mut()
            .find_map(|w| find_tab_mut(&mut w.root, tab))
    }
    fn find_group_mut(&mut self, gid: &str) -> Option<&mut TabGroup> {
        self.windows
            .iter_mut()
            .find_map(|w| find_group_mut(&mut w.root, gid))
    }
    fn group_tab_count(&self, gid: &str) -> Option<usize> {
        self.windows
            .iter()
            .find_map(|w| find_group(&w.root, gid).map(|g| g.tabs.len()))
    }
    fn group_has_tab(&self, gid: &str, tab: &str) -> bool {
        self.windows
            .iter()
            .find_map(|w| find_group(&w.root, gid))
            .is_some_and(|g| g.tabs.iter().any(|t| t.id == tab))
    }
    fn take_tab(&mut self, tab: &str) -> Option<Tab> {
        self.windows
            .iter_mut()
            .find_map(|w| take_tab(&mut w.root, tab))
    }
    fn remove_tab(&mut self, tab: &str) {
        let was_focused = self.focused.as_ref().is_some_and(|(_, t)| t == tab);
        let removed = self.take_tab(tab);
        let did_remove = removed.is_some();
        if let Some(removed) = removed {
            self.closed.push_closed(removed);
        }
        if did_remove && was_focused {
            self.normalize();
            if let Some(next) = self.mru.first().cloned().or_else(|| {
                self.windows
                    .iter()
                    .find_map(|w| tab_ids(&w.root).into_iter().next())
            }) {
                self.focus(&next);
            } else {
                self.focused = None;
            }
        }
    }
    fn move_tab(&mut self, tab: &str, group: &str) -> bool {
        if self.find_group_mut(group).is_none() || self.find_tab(tab).is_none() {
            return false;
        }
        let t = match self.take_tab(tab) {
            Some(t) => t,
            None => return false,
        };
        if let Some(g) = self.find_group_mut(group) {
            let moved_id = t.id.clone();
            g.tabs.push(t);
            g.active = g.tabs.len() - 1;
            self.focus(&moved_id);
            true
        } else {
            false
        }
    }
    fn split_group(
        &mut self,
        group: &str,
        tab: &str,
        orientation: Orientation,
        before: bool,
    ) -> bool {
        for w in &mut self.windows {
            if split_group_node(&mut w.root, group, tab, orientation, before) {
                return true;
            }
        }
        false
    }
    fn split_tab(&mut self, tab: &str, o: Orientation) -> bool {
        for w in &mut self.windows {
            if split_node(&mut w.root, tab, o) {
                return true;
            }
        }
        false
    }
}

fn valid_node(n: &LayoutNode) -> bool {
    match n {
        LayoutNode::Tabs(g) => {
            (g.tabs.is_empty() && g.active == 0 || !g.tabs.is_empty() && g.active < g.tabs.len())
                && g.tabs.iter().all(|t| !t.id.is_empty())
        }
        LayoutNode::Split {
            ratio,
            first,
            second,
            ..
        } => {
            ratio.is_finite()
                && *ratio > 0.0
                && *ratio < 1.0
                && valid_node(first)
                && valid_node(second)
        }
    }
}
fn valid_node_ids(n: &LayoutNode, groups: &mut BTreeSet<Id>, tabs: &mut BTreeSet<Id>) -> bool {
    match n {
        LayoutNode::Tabs(g) => {
            !g.id.is_empty()
                && groups.insert(g.id.clone())
                && valid_node(n)
                && g.tabs
                    .iter()
                    .all(|t| !t.id.is_empty() && tabs.insert(t.id.clone()))
        }
        LayoutNode::Split { first, second, .. } => {
            valid_node_ids(first, groups, tabs) && valid_node_ids(second, groups, tabs)
        }
    }
}
fn clean_node(n: LayoutNode) -> Option<LayoutNode> {
    match n {
        LayoutNode::Tabs(mut g) => {
            if g.tabs.is_empty() {
                None
            } else {
                g.active = g.active.min(g.tabs.len() - 1);
                Some(LayoutNode::Tabs(g))
            }
        }
        LayoutNode::Split {
            orientation,
            ratio,
            first,
            second,
        } => match (clean_node(*first), clean_node(*second)) {
            (Some(a), Some(b)) => Some(LayoutNode::Split {
                orientation,
                ratio: ratio.clamp(0.1, 0.9),
                first: Box::new(a),
                second: Box::new(b),
            }),
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            _ => None,
        },
    }
}
fn find_tab<'a>(n: &'a LayoutNode, id: &str) -> Option<&'a Tab> {
    match n {
        LayoutNode::Tabs(g) => g.tabs.iter().find(|t| t.id == id),
        LayoutNode::Split { first, second, .. } => {
            find_tab(first, id).or_else(|| find_tab(second, id))
        }
    }
}
fn find_group<'a>(n: &'a LayoutNode, id: &str) -> Option<&'a TabGroup> {
    match n {
        LayoutNode::Tabs(g) => (g.id == id).then_some(g),
        LayoutNode::Split { first, second, .. } => {
            find_group(first, id).or_else(|| find_group(second, id))
        }
    }
}

fn first_group_id(n: &LayoutNode) -> Option<Id> {
    match n {
        LayoutNode::Tabs(group) => Some(group.id.clone()),
        LayoutNode::Split { first, second, .. } => {
            first_group_id(first).or_else(|| first_group_id(second))
        }
    }
}

fn group_id_for_tab(n: &LayoutNode, tab: &str) -> Option<Id> {
    match n {
        LayoutNode::Tabs(group) => group
            .tabs
            .iter()
            .any(|item| item.id == tab)
            .then(|| group.id.clone()),
        LayoutNode::Split { first, second, .. } => {
            group_id_for_tab(first, tab).or_else(|| group_id_for_tab(second, tab))
        }
    }
}

fn find_group_location(n: &LayoutNode, tab: &str) -> Option<(Id, usize)> {
    match n {
        LayoutNode::Tabs(group) => group
            .tabs
            .iter()
            .position(|item| item.id == tab)
            .map(|index| (group.id.clone(), index)),
        LayoutNode::Split { first, second, .. } => {
            find_group_location(first, tab).or_else(|| find_group_location(second, tab))
        }
    }
}

fn find_tab_with_path<'a>(n: &'a LayoutNode, path: &str) -> Option<&'a Tab> {
    match n {
        LayoutNode::Tabs(group) => group
            .tabs
            .iter()
            .find(|tab| tab.view.path.as_deref() == Some(path)),
        LayoutNode::Split { first, second, .. } => {
            find_tab_with_path(first, path).or_else(|| find_tab_with_path(second, path))
        }
    }
}

fn find_tab_with_view<'a>(n: &'a LayoutNode, view: &View) -> Option<&'a Tab> {
    match n {
        LayoutNode::Tabs(group) => group
            .tabs
            .iter()
            .find(|tab| tab.view.kind == view.kind && tab.view.path == view.path),
        LayoutNode::Split { first, second, .. } => {
            find_tab_with_view(first, view).or_else(|| find_tab_with_view(second, view))
        }
    }
}

fn set_group_active(n: &mut LayoutNode, tab: &str) -> bool {
    match n {
        LayoutNode::Tabs(group) => {
            if let Some(index) = group.tabs.iter().position(|item| item.id == tab) {
                group.active = index;
                true
            } else {
                false
            }
        }
        LayoutNode::Split { first, second, .. } => {
            set_group_active(first, tab) || set_group_active(second, tab)
        }
    }
}

fn is_builtin_view_kind(kind: &str) -> bool {
    matches!(kind, "markdown" | "graph" | "settings" | "media")
}
fn tab_ids(n: &LayoutNode) -> Vec<Id> {
    match n {
        LayoutNode::Tabs(g) => g.tabs.iter().map(|t| t.id.clone()).collect(),
        LayoutNode::Split { first, second, .. } => {
            let mut ids = tab_ids(first);
            ids.extend(tab_ids(second));
            ids
        }
    }
}
fn find_tab_mut<'a>(n: &'a mut LayoutNode, id: &str) -> Option<&'a mut Tab> {
    match n {
        LayoutNode::Tabs(g) => g.tabs.iter_mut().find(|t| t.id == id),
        LayoutNode::Split { first, second, .. } => {
            find_tab_mut(first, id).or_else(|| find_tab_mut(second, id))
        }
    }
}
fn find_group_mut<'a>(n: &'a mut LayoutNode, id: &str) -> Option<&'a mut TabGroup> {
    match n {
        LayoutNode::Tabs(g) => (g.id == id).then_some(g),
        LayoutNode::Split { first, second, .. } => {
            find_group_mut(first, id).or_else(|| find_group_mut(second, id))
        }
    }
}
fn take_tab(n: &mut LayoutNode, id: &str) -> Option<Tab> {
    match n {
        LayoutNode::Tabs(g) => g.tabs.iter().position(|t| t.id == id).map(|i| {
            g.active = g.active.min(i.saturating_sub(1));
            g.tabs.remove(i)
        }),
        LayoutNode::Split { first, second, .. } => {
            take_tab(first, id).or_else(|| take_tab(second, id))
        }
    }
}
fn split_node(n: &mut LayoutNode, tab: &str, o: Orientation) -> bool {
    match n {
        LayoutNode::Tabs(g) if g.tabs.iter().any(|t| t.id == tab) => {
            split_group_tabs(n, tab, o, false)
        }
        LayoutNode::Split { first, second, .. } => {
            split_node(first, tab, o) || split_node(second, tab, o)
        }
        _ => false,
    }
}
fn split_group_node(
    n: &mut LayoutNode,
    group: &str,
    tab: &str,
    o: Orientation,
    before: bool,
) -> bool {
    match n {
        LayoutNode::Tabs(g) if g.id == group => split_group_tabs(n, tab, o, before),
        LayoutNode::Split { first, second, .. } => {
            split_group_node(first, group, tab, o, before)
                || split_group_node(second, group, tab, o, before)
        }
        _ => false,
    }
}
fn split_group_tabs(n: &mut LayoutNode, tab: &str, o: Orientation, before: bool) -> bool {
    let LayoutNode::Tabs(g) = n else { return false };
    if g.tabs.len() < 2 {
        return false;
    }
    let Some(index) = g.tabs.iter().position(|t| t.id == tab) else {
        return false;
    };
    let moved = g.tabs.remove(index);
    g.active = g.active.min(g.tabs.len() - 1);
    let old = std::mem::replace(
        n,
        LayoutNode::Tabs(TabGroup {
            id: id(),
            tabs: vec![],
            active: 0,
            stacked: false,
            extras: ExtraFields::default(),
        }),
    );
    let sibling = LayoutNode::Tabs(TabGroup {
        id: id(),
        tabs: vec![moved],
        active: 0,
        stacked: false,
        extras: ExtraFields::default(),
    });
    *n = if before {
        LayoutNode::Split {
            orientation: o,
            ratio: 0.5,
            first: Box::new(sibling),
            second: Box::new(old),
        }
    } else {
        LayoutNode::Split {
            orientation: o,
            ratio: 0.5,
            first: Box::new(old),
            second: Box::new(sibling),
        }
    };
    true
}
fn remove_matching(n: &mut LayoutNode, predicate: impl Fn(&Tab) -> bool + Copy) {
    match n {
        LayoutNode::Tabs(g) => g.tabs.retain(|t| !predicate(t)),
        LayoutNode::Split { first, second, .. } => {
            remove_matching(first, predicate);
            remove_matching(second, predicate);
        }
    }
}

pub fn migrate(w: &mut Workspace) {
    if w.schema_version == 0 {
        w.schema_version = 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tab(w: &mut Workspace, kind: &str) -> Id {
        w.dispatch(Action::Open {
            window: None,
            view: View::new(kind),
        });
        match &w.windows[0].root {
            LayoutNode::Tabs(g) => g.tabs.last().unwrap().id.clone(),
            _ => unreachable!(),
        }
    }
    #[test]
    fn split_close_focus_preserves_invariants() {
        let mut w = Workspace::default();
        let t = tab(&mut w, "markdown");
        w.dispatch(Action::Focus { tab: t.clone() });
        w.dispatch(Action::Split {
            tab: t.clone(),
            orientation: Orientation::Horizontal,
        });
        assert!(w.validate());
        w.dispatch(Action::Close { tab: t });
        assert!(w.validate());
    }
    #[test]
    fn disable_feature_hides_views_and_commands() {
        let mut w = Workspace::default();
        let t = tab(&mut w, "graph");
        w.dispatch(Action::RegisterCommand(Command {
            id: "graph.open".into(),
            label: "Graph".into(),
            hotkey: None,
            enabled: true,
            core_feature: Some("graph".into()),
            extras: ExtraFields::default(),
        }));
        w.dispatch(Action::SetFeature {
            feature: "graph".into(),
            enabled: false,
        });
        assert!(w.find_tab(&t).is_none());
        assert!(!w.commands[0].enabled);
    }
    #[test]
    fn persistence_roundtrip_and_unknown_fields() {
        let mut w = Workspace::default();
        w.extras
            .0
            .insert("future".into(), Value::String("yes".into()));
        let s = serde_json::to_string(&w).unwrap();
        let x: Workspace = serde_json::from_str(&s).unwrap();
        assert_eq!(x.extras.0["future"], "yes");
    }
    #[test]
    fn conflicting_hotkeys_do_not_execute() {
        let mut w = Workspace::default();
        for id in ["a", "b"] {
            w.dispatch(Action::RegisterCommand(Command {
                id: id.into(),
                label: id.into(),
                hotkey: Some(KeyChord::new("K")),
                enabled: true,
                core_feature: None,
                extras: ExtraFields::default(),
            }));
        }
        assert_eq!(
            w.dispatch(Action::Hotkey(KeyChord::new("K"))),
            vec![Effect::Notice("hotkey conflict".into())]
        );
    }
    #[test]
    fn invalid_target_move_is_transactional_and_invalid_window_errors() {
        let mut w = Workspace::default();
        let t = tab(&mut w, "markdown");
        let before = serde_json::to_string(&w).unwrap();
        assert!(w
            .dispatch(Action::Move {
                tab: t.clone(),
                target_group: "missing".into()
            })
            .is_empty());
        assert_eq!(serde_json::to_string(&w).unwrap(), before);
        assert!(matches!(
            w.dispatch(Action::Open {
                window: Some("missing".into()),
                view: View::new("x")
            })
            .as_slice(),
            [Effect::Error(_)]
        ));
    }
    #[test]
    fn close_updates_focus_and_duplicate_ids_are_invalid() {
        let mut w = Workspace::default();
        let first = tab(&mut w, "a");
        let second = tab(&mut w, "b");
        w.dispatch(Action::Focus { tab: first.clone() });
        w.dispatch(Action::Close { tab: first });
        assert_eq!(w.focused.as_ref().map(|(_, id)| id), Some(&second));
        assert!(w.validate());
        if let LayoutNode::Tabs(g) = &mut w.windows[0].root {
            g.tabs.push(g.tabs[0].clone());
        }
        assert!(!w.validate());
    }
    #[test]
    fn restore_and_unknown_view_report_actionable_errors() {
        let mut w = Workspace::default();
        w.view_kinds.insert("known".into());
        let before = serde_json::to_string(&w).unwrap();
        assert!(matches!(
            w.dispatch(Action::Open {
                window: None,
                view: View::new("unknown")
            })
            .as_slice(),
            [Effect::UnknownView(_)]
        ));
        assert_eq!(serde_json::to_string(&w).unwrap(), before);
        assert!(matches!(
            w.dispatch(Action::Restore("not-json".into())).as_slice(),
            [Effect::Error(_)]
        ));
        let future = Workspace {
            schema_version: 999,
            ..Workspace::default()
        };
        let encoded = serde_json::to_string(&future).unwrap();
        assert!(matches!(
            w.dispatch(Action::Restore(encoded)).as_slice(),
            [Effect::Error(_)]
        ));
    }
    #[test]
    fn split_persists_two_nonempty_siblings_and_honors_before() {
        let mut w = Workspace::default();
        let a = tab(&mut w, "a");
        let b = tab(&mut w, "b");
        w.dispatch(Action::Split {
            tab: b.clone(),
            orientation: Orientation::Vertical,
        });
        assert!(w.validate());
        fn sides(n: &LayoutNode, wanted: &str) -> Option<bool> {
            match n {
                LayoutNode::Split { first, second, .. } => Some(find_tab(first, wanted).is_some())
                    .or_else(|| sides(first, wanted))
                    .or_else(|| sides(second, wanted)),
                LayoutNode::Tabs(_) => None,
            }
        }
        assert_eq!(sides(&w.windows[0].root, &b), Some(false));
        assert!(find_tab(&w.windows[0].root, &a).is_some());
        let group = match &w.windows[0].root {
            LayoutNode::Split { second, .. } => match &**second {
                LayoutNode::Tabs(g) => g.id.clone(),
                _ => unreachable!(),
            },
            _ => unreachable!(),
        };
        w.dispatch(Action::Dock {
            tab: a,
            target: DockTarget::Split {
                group,
                orientation: Orientation::Horizontal,
                before: true,
            },
        });
        assert!(w.validate());
    }
    #[test]
    fn invalid_focus_and_ids_are_rejected() {
        let mut w = Workspace::default();
        let _ = tab(&mut w, "valid");
        assert!(w
            .dispatch(Action::Focus {
                tab: "missing".into()
            })
            .is_empty());
        w.windows[0].id.clear();
        assert!(!w.validate());
        let mut w = Workspace {
            focused: Some(("wrong-window".into(), "wrong-tab".into())),
            ..Workspace::default()
        };
        assert!(!w.validate());
        if let LayoutNode::Tabs(g) = &mut w.windows[0].root {
            g.id.clear();
        }
        assert!(!w.validate());
    }
    #[test]
    fn invalid_focus_is_a_byte_for_byte_noop_without_normalization() {
        let mut w = Workspace::default();
        let first = tab(&mut w, "a");
        let second = tab(&mut w, "b");
        w.dispatch(Action::Split {
            tab: second,
            orientation: Orientation::Horizontal,
        });
        if let LayoutNode::Split { ratio, .. } = &mut w.windows[0].root {
            *ratio = 0.99;
        }
        let snapshot = serde_json::to_vec(&w).unwrap();
        assert!(w
            .dispatch(Action::Focus {
                tab: "does-not-exist".into()
            })
            .is_empty());
        assert_eq!(serde_json::to_vec(&w).unwrap(), snapshot);
        assert!(w.find_tab(&first).is_some());
    }
}
