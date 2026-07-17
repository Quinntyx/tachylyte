//! Renderable, data-only navigation panes for the Tachylyte workspace.
//!
//! This crate deliberately has no vault, filesystem, or network access. Callers feed it
//! snapshots from `tachylyte-core`/`tachylyte-knowledge`, reduce actions, and subscribe to
//! the small event vocabulary below.

use gpui::{div, prelude::*, Context, Entity, Render, Window};
use std::collections::{BTreeMap, BTreeSet};

/// Unicode-friendly (case-insensitive) substring matching used by every pane.
pub fn matches_filter(value: &str, filter: &str) -> bool {
    let needle = filter.trim().to_lowercase();
    needle.is_empty() || value.to_lowercase().contains(&needle)
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
                    self.events
                        .push(PaneEvent::Activated(selected_id(self.selected)));
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
        flatten_nodes(&self.nodes, &self.state.collapsed, &self.state.filter)
    }
    pub fn reduce(&mut self, action: PaneAction) {
        let ids: Vec<_> = self.visible().iter().map(|n| n.id.clone()).collect();
        self.state.reduce(action, ids.len(), |i| ids[i].clone());
    }
}

fn flatten_nodes<'a>(
    nodes: &'a [FileNode],
    collapsed: &BTreeSet<String>,
    filter: &str,
) -> Vec<&'a FileNode> {
    let mut out = Vec::new();
    for node in nodes {
        if matches_filter(&node.label, filter) {
            out.push(node);
        }
        if node.folder && !collapsed.contains(&node.id) {
            out.extend(flatten_nodes(&node.children, collapsed, filter));
        }
    }
    out
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
        self.items
            .iter()
            .filter(|x| matches_filter(&format!("{} {}", x.path, x.snippet), &self.state.filter))
            .collect()
    }
    pub fn reduce(&mut self, action: PaneAction) {
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
        self.items
            .iter()
            .filter(|x| matches_filter(&x.label, &self.state.filter))
            .collect()
    }
    pub fn reduce(&mut self, action: PaneAction) {
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

#[derive(Clone, Debug, Default)]
pub struct FeatureVisibility(pub BTreeMap<String, bool>);
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
        self.0.get(feature).copied().unwrap_or(true)
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
        self.items
            .iter()
            .filter(|x| matches_filter(&x.label, &self.state.filter))
            .collect()
    }
}
impl Render for NavigationPane {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.render_content(Some(cx.entity()))
    }
}
impl NavigationPane {
    fn render_content(&mut self, click_target: Option<Entity<Self>>) -> impl IntoElement {
        if !self.features.enabled(&self.feature) {
            return div()
                .id("navigation-pane-disabled")
                .child(format!("{} unavailable", self.title));
        }
        let title = self.title.clone();
        let rows = self.visible().into_iter().enumerate().map(|(i, item)| {
            let id = item.id.clone();
            let mut row =
                div()
                    .id(("navigation-row", i))
                    .p_1()
                    .child(if i == self.state.selected {
                        format!("› {}", item.label)
                    } else {
                        item.label.clone()
                    });
            if let Some(target) = click_target.clone() {
                row = row.on_click(move |_, _, cx| {
                    target.update(cx, |pane, cx| {
                        pane.state.events.push(PaneEvent::Activated(id.clone()));
                        cx.notify();
                    });
                });
            }
            row
        });
        div()
            .id("navigation-pane")
            .key_context("navigation-pane")
            .flex()
            .flex_col()
            .child(title)
            .children(rows)
    }
}

macro_rules! pane_view {
    ($name:ident) => {
        #[derive(Clone, Debug)]
        pub struct $name {
            pub pane: NavigationPane,
        }
        impl Render for $name {
            fn render(&mut self, _w: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
                self.pane.render_content(None)
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
}
