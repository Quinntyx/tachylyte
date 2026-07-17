//! Renderable, data-only navigation panes for the Tachylyte workspace.
//!
//! This crate deliberately has no vault, filesystem, or network access. Callers feed it
//! snapshots from `tachylyte-core`/`tachylyte-knowledge`, reduce actions, and subscribe to
//! the small event vocabulary below.

use gpui::{div, prelude::*, Context, Entity, Render, Window};
use std::collections::{BTreeMap, BTreeSet};
use unicode_normalization::{char::is_combining_mark, UnicodeNormalization};

/// Unicode-friendly (case-insensitive) substring matching used by every pane.
pub fn matches_filter(value: &str, filter: &str) -> bool {
    let fold = |text: &str| {
        text.nfkd()
            .filter(|c| !is_combining_mark(*c))
            .collect::<String>()
            .to_lowercase()
    };
    let needle = fold(filter.trim());
    needle.is_empty() || fold(value).contains(&needle)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PaneEvent {
    Selected(String),
    Activated(String),
    Toggled(String, bool),
    Command(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PaneAction {
    Filter(String),
    Up,
    Down,
    Home,
    End,
    Activate,
    /// Toggle a row's collapsed state; the emitted boolean is `true` when now expanded.
    Toggle(String),
}

/// Shared reducer state. Selection is always bounded after every action.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SelectionModel {
    pub filter: String,
    pub selected: usize,
    pub collapsed: BTreeSet<String>,
    pub events: Vec<PaneEvent>,
}
impl SelectionModel {
    pub fn activate_at(&mut self, index: usize, id: String) {
        self.selected = index;
        self.events.push(PaneEvent::Selected(id.clone()));
        self.events.push(PaneEvent::Activated(id));
    }
    pub fn reduce(
        &mut self,
        action: PaneAction,
        count: usize,
        selected_id: impl Fn(usize) -> String,
    ) {
        match action {
            PaneAction::Filter(filter) => {
                self.filter = filter;
                self.selected = 0;
            }
            PaneAction::Up => self.selected = self.selected.saturating_sub(1),
            PaneAction::Down => self.selected = (self.selected + 1).min(count.saturating_sub(1)),
            PaneAction::Home => self.selected = 0,
            PaneAction::End => self.selected = count.saturating_sub(1),
            PaneAction::Activate => {
                if count > 0 {
                    self.activate_at(self.selected, selected_id(self.selected));
                    return;
                }
            }
            PaneAction::Toggle(id) => {
                let open = self.collapsed.insert(id.clone());
                self.events.push(PaneEvent::Toggled(id, !open));
            }
        }
        if count == 0 {
            self.selected = 0;
        } else {
            self.selected = self.selected.min(count - 1);
        }
        if let Some(id) = (count > 0).then(|| selected_id(self.selected)) {
            self.events.push(PaneEvent::Selected(id));
        }
    }
    pub fn take_events(&mut self) -> Vec<PaneEvent> {
        std::mem::take(&mut self.events)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileNode {
    pub id: String,
    pub label: String,
    pub folder: bool,
    pub children: Vec<FileNode>,
}
#[derive(Clone, Debug, Default)]
pub struct FileExplorerModel {
    pub nodes: Vec<FileNode>,
    pub state: SelectionModel,
    pub feature_enabled: bool,
}
impl FileExplorerModel {
    pub fn visible(&self) -> Vec<&FileNode> {
        if !self.feature_enabled {
            return Vec::new();
        }
        flatten_nodes(&self.nodes, &self.state.collapsed, &self.state.filter)
    }
    pub fn reduce(&mut self, action: PaneAction) {
        if !self.feature_enabled {
            return;
        }
        let ids: Vec<_> = self.visible().iter().map(|n| n.id.clone()).collect();
        self.state.reduce(action, ids.len(), |i| ids[i].clone());
    }
}

fn flatten_nodes<'a>(
    nodes: &'a [FileNode],
    collapsed: &BTreeSet<String>,
    filter: &str,
) -> Vec<&'a FileNode> {
    fn visit<'a>(
        node: &'a FileNode,
        collapsed: &BTreeSet<String>,
        filter: &str,
    ) -> (bool, Vec<&'a FileNode>) {
        let direct = matches_filter(&node.label, filter);
        let reveal = !filter.trim().is_empty();
        let children = if node.folder && (!collapsed.contains(&node.id) || reveal) {
            node.children
                .iter()
                .map(|child| visit(child, collapsed, filter))
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let descendant = children.iter().any(|(matched, _)| *matched);
        let matched = direct || descendant;
        let mut out = if matched { vec![node] } else { Vec::new() };
        for (_, child_rows) in children {
            out.extend(child_rows);
        }
        (matched, out)
    }
    nodes
        .iter()
        .flat_map(|node| visit(node, collapsed, filter).1)
        .collect()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchItem {
    pub path: String,
    pub snippet: String,
    pub score: u32,
}
#[derive(Clone, Debug, Default)]
pub struct SearchResultsModel {
    pub items: Vec<SearchItem>,
    pub state: SelectionModel,
    pub feature_enabled: bool,
}
impl SearchResultsModel {
    pub fn visible(&self) -> Vec<&SearchItem> {
        if !self.feature_enabled {
            return Vec::new();
        }
        self.items
            .iter()
            .filter(|x| matches_filter(&format!("{} {}", x.path, x.snippet), &self.state.filter))
            .collect()
    }
    pub fn reduce(&mut self, action: PaneAction) {
        if !self.feature_enabled {
            return;
        }
        let ids: Vec<_> = self.visible().iter().map(|x| x.path.clone()).collect();
        self.state.reduce(action, ids.len(), |i| ids[i].clone());
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LabelItem {
    pub id: String,
    pub label: String,
}
#[derive(Clone, Debug, Default)]
pub struct ListPaneModel {
    pub items: Vec<LabelItem>,
    pub state: SelectionModel,
    pub feature_enabled: bool,
}
impl ListPaneModel {
    pub fn visible(&self) -> Vec<&LabelItem> {
        if !self.feature_enabled {
            return Vec::new();
        }
        self.items
            .iter()
            .filter(|x| matches_filter(&x.label, &self.state.filter))
            .collect()
    }
    pub fn reduce(&mut self, action: PaneAction) {
        if !self.feature_enabled {
            return;
        }
        let ids: Vec<_> = self.visible().iter().map(|x| x.id.clone()).collect();
        self.state.reduce(action, ids.len(), |i| ids[i].clone());
    }
}

/// Models for quick switcher, command palette, backlinks, outgoing links, outline, tags,
/// properties, bookmarks, graph list, and graph legend use the same neutral list semantics.
pub type QuickSwitcherModel = ListPaneModel;
pub type CommandPaletteModel = ListPaneModel;
pub type BacklinksModel = ListPaneModel;
pub type OutgoingLinksModel = ListPaneModel;
pub type OutlineModel = ListPaneModel;
pub type TagsModel = ListPaneModel;
pub type PropertiesModel = ListPaneModel;
pub type BookmarksModel = ListPaneModel;
pub type GraphListModel = ListPaneModel;
pub type GraphLegendModel = ListPaneModel;

#[derive(Clone, Debug)]
pub struct FeatureVisibility(pub BTreeMap<String, bool>);
impl Default for FeatureVisibility {
    fn default() -> Self {
        Self(
            tachylyte_core::CORE_FEATURES
                .iter()
                .map(|feature| ((*feature).to_string(), true))
                .collect(),
        )
    }
}
impl FeatureVisibility {
    pub fn from_registry(registry: &tachylyte_core::FeatureRegistry) -> Self {
        Self(
            tachylyte_core::CORE_FEATURES
                .iter()
                .map(|feature| ((*feature).to_string(), registry.is_enabled(feature)))
                .collect(),
        )
    }
    pub fn enabled(&self, feature: &str) -> bool {
        tachylyte_core::FeatureRegistry::is_known(feature)
            && self.0.get(feature).copied().unwrap_or(false)
    }
    pub fn set(&mut self, feature: &str, enabled: bool) -> bool {
        if !tachylyte_core::FeatureRegistry::is_known(feature) {
            return false;
        }
        self.0.insert(feature.to_string(), enabled);
        true
    }
}

impl From<tachylyte_knowledge::SearchResult> for SearchItem {
    fn from(result: tachylyte_knowledge::SearchResult) -> Self {
        Self {
            path: result.path,
            snippet: result.snippet,
            score: result.score,
        }
    }
}

/// State bridge used by every concrete pane entity.
pub trait PaneActivation {
    fn pane(&mut self) -> &mut NavigationPane;
    fn activate_id(&mut self, id: &str);
}

fn keyboard_action(key: &str, filter: &str) -> Option<PaneAction> {
    match key {
        "up" => Some(PaneAction::Up),
        "down" => Some(PaneAction::Down),
        "home" => Some(PaneAction::Home),
        "end" => Some(PaneAction::End),
        "enter" => Some(PaneAction::Activate),
        "backspace" => Some(PaneAction::Filter(
            filter
                .chars()
                .take(filter.chars().count().saturating_sub(1))
                .collect(),
        )),
        value if value.chars().count() == 1 => Some(PaneAction::Filter(format!("{filter}{value}"))),
        _ => None,
    }
}

fn reduce_key<E: PaneActivation + 'static>(target: &Entity<E>, key: &str, app: &mut gpui::App) {
    target.update(app, |view, cx| {
        if view.pane().reduce_key(key) {
            cx.notify();
        }
    });
}

/// A compact reusable GPUI view. The text label is intentionally present in the element tree,
/// making the pane usable by assistive tooling even though GPUI 0.2.2 has no ARIA builder.
#[derive(Clone, Debug)]
pub struct NavigationPane {
    pub title: String,
    pub items: Vec<LabelItem>,
    pub feature: String,
    pub features: FeatureVisibility,
    pub state: SelectionModel,
}
impl NavigationPane {
    pub fn new(
        title: impl Into<String>,
        feature: impl Into<String>,
        items: Vec<LabelItem>,
    ) -> Self {
        Self {
            title: title.into(),
            feature: feature.into(),
            items,
            features: FeatureVisibility::default(),
            state: SelectionModel::default(),
        }
    }
    pub fn visible(&self) -> Vec<&LabelItem> {
        if !self.features.enabled(&self.feature) {
            return Vec::new();
        }
        self.items
            .iter()
            .filter(|x| matches_filter(&x.label, &self.state.filter))
            .collect()
    }
    fn activate_id(&mut self, id: &str) {
        if !self.features.enabled(&self.feature) {
            return;
        }
        if let Some(index) = self.visible().iter().position(|item| item.id == id) {
            self.state.activate_at(index, id.to_string());
        }
    }
    pub fn reduce(&mut self, action: PaneAction) {
        if !self.features.enabled(&self.feature) {
            return;
        }
        let ids: Vec<_> = self.visible().iter().map(|item| item.id.clone()).collect();
        self.state.reduce(action, ids.len(), |i| ids[i].clone());
    }
    /// Reduce the same key values used by the GPUI key-down handler.
    pub fn reduce_key(&mut self, key: &str) -> bool {
        if !self.features.enabled(&self.feature) {
            return false;
        }
        let Some(action) = keyboard_action(key, &self.state.filter) else {
            return false;
        };
        self.reduce(action);
        true
    }
    pub fn set_feature_enabled(&mut self, enabled: bool) -> bool {
        let changed = self.features.set(&self.feature, enabled);
        if !self.features.enabled(&self.feature) {
            self.state.filter.clear();
            self.state.selected = 0;
            self.state.events.clear();
        }
        changed
    }
}
impl PaneActivation for NavigationPane {
    fn pane(&mut self) -> &mut NavigationPane {
        self
    }
    fn activate_id(&mut self, id: &str) {
        self.activate_id(id);
    }
}
impl Render for NavigationPane {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        render_navigation_content(self, cx.entity())
    }
}
fn render_navigation_content<E: PaneActivation + 'static>(
    pane: &mut NavigationPane,
    target: Entity<E>,
) -> impl IntoElement {
    if !pane.features.enabled(&pane.feature) {
        return div()
            .id("navigation-pane-disabled")
            .child(format!("{} unavailable", pane.title));
    }
    let title = pane.title.clone();
    let rows = pane.visible().into_iter().enumerate().map(|(i, item)| {
        let id = item.id.clone();
        let mut row = div()
            .id(("navigation-row", i))
            .focusable()
            .tab_index(i as isize)
            .p_1()
            .child(if i == pane.state.selected {
                format!("selected: {}", item.label)
            } else {
                item.label.clone()
            });
        let target = target.clone();
        row = row.on_click(move |_, _, cx| {
            target.update(cx, |view, cx| {
                view.activate_id(&id);
                cx.notify();
            });
        });
        row
    });
    let key_target = target.clone();
    div()
        .id("navigation-pane")
        .key_context("navigation-pane")
        .focusable()
        .tab_index(0)
        .on_key_down(move |event, _, app| reduce_key(&key_target, &event.keystroke.key, app))
        .flex()
        .flex_col()
        .child(title)
        .children(rows)
}

macro_rules! pane_view {
    ($name:ident) => {
        #[derive(Clone, Debug)]
        pub struct $name {
            pub pane: NavigationPane,
        }
        impl PaneActivation for $name {
            fn pane(&mut self) -> &mut NavigationPane {
                &mut self.pane
            }
            fn activate_id(&mut self, id: &str) {
                self.pane.activate_id(id);
            }
        }
        impl Render for $name {
            fn render(&mut self, _w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                render_navigation_content(&mut self.pane, cx.entity())
            }
        }
    };
}
pane_view!(FileExplorer);
pane_view!(SearchResults);
pane_view!(QuickSwitcher);
pane_view!(CommandPalette);
pane_view!(Backlinks);
pane_view!(OutgoingLinks);
pane_view!(Outline);
pane_view!(Tags);
pane_view!(Properties);
pane_view!(Bookmarks);
pane_view!(GraphList);
pane_view!(GraphLegend);

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selection_is_bounded_and_emits_events() {
        let mut s = SelectionModel::default();
        s.reduce(PaneAction::End, 2, |i| i.to_string());
        assert_eq!(s.selected, 1);
        s.reduce(PaneAction::Down, 2, |i| i.to_string());
        assert_eq!(s.selected, 1);
        assert!(s.take_events().contains(&PaneEvent::Selected("1".into())));
    }
    #[test]
    fn unicode_filtering() {
        assert!(matches_filter("Résumé Привет", "прив"));
        assert!(!matches_filter("Résumé", "xyz"));
    }
    #[test]
    fn disabled_features_are_hidden() {
        let mut p = NavigationPane::new("Graph", "graph", vec![]);
        p.features.0.insert("graph".into(), false);
        assert!(!p.features.enabled("graph"));
    }
    #[test]
    fn collapse_emits_toggle() {
        let mut s = SelectionModel::default();
        s.reduce(PaneAction::Toggle("docs".into()), 0, |_| String::new());
        assert_eq!(
            s.take_events(),
            vec![PaneEvent::Toggled("docs".into(), false)]
        );
    }
    #[test]
    fn click_activation_selects_before_activating() {
        let mut view = QuickSwitcher {
            pane: NavigationPane::new(
                "Quick switcher",
                "search",
                vec![LabelItem {
                    id: "a".into(),
                    label: "Alpha".into(),
                }],
            ),
        };
        view.activate_id("a");
        assert_eq!(view.pane.state.selected, 0);
        assert_eq!(
            view.pane.state.take_events(),
            vec![
                PaneEvent::Selected("a".into()),
                PaneEvent::Activated("a".into())
            ]
        );
    }
    #[test]
    fn collapsed_parent_reveals_matching_descendant_without_mutating_collapse() {
        let mut model = FileExplorerModel {
            nodes: vec![FileNode {
                id: "docs".into(),
                label: "Docs".into(),
                folder: true,
                children: vec![FileNode {
                    id: "résumé".into(),
                    label: "Résumé".into(),
                    folder: false,
                    children: vec![],
                }],
            }],
            feature_enabled: true,
            ..Default::default()
        };
        model.state.collapsed.insert("docs".into());
        assert_eq!(model.visible().len(), 1);
        model.state.filter = "resume".into();
        assert_eq!(
            model
                .visible()
                .iter()
                .map(|node| node.id.as_str())
                .collect::<Vec<_>>(),
            vec!["docs", "résumé"]
        );
        assert!(model.state.collapsed.contains("docs"));
        model.state.filter.clear();
        assert_eq!(
            model
                .visible()
                .iter()
                .map(|node| node.id.as_str())
                .collect::<Vec<_>>(),
            vec!["docs"]
        );
    }
    #[test]
    fn normalized_filter_matches_decomposed_and_composed_text() {
        assert!(matches_filter("café", "CAFE"));
        assert!(matches_filter("cafe\u{301}", "café"));
    }
    #[test]
    fn model_feature_flags_hide_rows() {
        let mut model = ListPaneModel {
            items: vec![LabelItem {
                id: "x".into(),
                label: "X".into(),
            }],
            ..Default::default()
        };
        assert!(model.visible().is_empty());
        model.feature_enabled = true;
        assert_eq!(model.visible().len(), 1);
    }
    #[test]
    fn keyboard_handler_reduces_navigation_actions() {
        assert_eq!(keyboard_action("down", ""), Some(PaneAction::Down));
        assert_eq!(keyboard_action("enter", "q"), Some(PaneAction::Activate));
        assert_eq!(
            keyboard_action("x", "q"),
            Some(PaneAction::Filter("qx".into()))
        );
        assert_eq!(
            keyboard_action("backspace", "é"),
            Some(PaneAction::Filter("".into()))
        );
        let mut pane = NavigationPane::new(
            "Search",
            "search",
            vec![LabelItem {
                id: "x".into(),
                label: "X".into(),
            }],
        );
        assert!(pane.reduce_key("down"));
        assert!(pane.reduce_key("x"));
        assert_eq!(pane.state.filter, "x");
    }
    #[test]
    fn disabled_pane_has_no_visible_rows_or_events() {
        let mut pane = NavigationPane::new(
            "Unknown",
            "not-a-feature",
            vec![LabelItem {
                id: "x".into(),
                label: "X".into(),
            }],
        );
        pane.state.filter = "x".into();
        pane.state.selected = 1;
        assert!(pane.visible().is_empty());
        pane.reduce(PaneAction::Filter("new".into()));
        pane.reduce(PaneAction::Down);
        pane.reduce(PaneAction::Activate);
        assert!(!pane.reduce_key("enter"));
        pane.activate_id("x");
        assert!(pane.state.events.is_empty());
        assert_eq!(pane.state.filter, "x");
    }
    #[test]
    fn disabling_feature_clears_interaction_state() {
        let mut pane = NavigationPane::new(
            "Search",
            "search",
            vec![LabelItem {
                id: "x".into(),
                label: "X".into(),
            }],
        );
        pane.state.filter = "x".into();
        pane.state.selected = 1;
        pane.state.events.push(PaneEvent::Selected("x".into()));
        assert!(pane.set_feature_enabled(false));
        assert!(pane.state.filter.is_empty());
        assert_eq!(pane.state.selected, 0);
        assert!(pane.state.events.is_empty());
        assert!(pane.set_feature_enabled(true));
        assert!(pane.visible().len() == 1);
    }
    #[test]
    fn unknown_feature_cannot_be_enabled() {
        let mut visibility = FeatureVisibility::default();
        assert!(!visibility.set("nope", true));
        assert!(!visibility.enabled("nope"));
    }
}
