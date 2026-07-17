//! Render-neutral state and geometry for the knowledge graph view.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use tachylyte_knowledge::{graph, GraphEdge, GraphFilter, GraphNode, VaultIndex};

/// A point in graph-world or screen coordinates.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

/// Pan and zoom state shared by rendering and interaction.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewTransform {
    pub pan: Point,
    pub zoom: f32,
}

impl Default for ViewTransform {
    fn default() -> Self {
        Self {
            pan: Point::default(),
            zoom: 1.0,
        }
    }
}

impl ViewTransform {
    pub fn screen(&self, point: Point) -> Point {
        Point {
            x: point.x * self.zoom + self.pan.x,
            y: point.y * self.zoom + self.pan.y,
        }
    }

    pub fn world(&self, point: Point) -> Point {
        Point {
            x: (point.x - self.pan.x) / self.zoom,
            y: (point.y - self.pan.y) / self.zoom,
        }
    }

    pub fn zoom_by(&mut self, factor: f32) {
        if factor.is_finite() && factor > 0.0 {
            self.zoom = (self.zoom * factor).clamp(0.1, 8.0);
        }
    }
}

/// Scope shown by the graph toolbar.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GraphMode {
    #[default]
    Global,
    Local,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeStyle {
    Active,
    Inactive,
    Unresolved,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PositionedNode {
    pub node: GraphNode,
    pub position: Point,
    pub style: NodeStyle,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EdgeSegment {
    pub edge: GraphEdge,
    pub from: Point,
    pub to: Point,
    pub active: bool,
}

/// Events emitted by selecting a node or activating the open control.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GraphEvent {
    Select(String),
    Open(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct GraphSettings {
    pub show_unresolved: bool,
    pub sparkle_enabled: bool,
    pub node_scale: f32,
}

impl Default for GraphSettings {
    fn default() -> Self {
        Self {
            show_unresolved: true,
            sparkle_enabled: true,
            node_scale: 1.0,
        }
    }
}

/// Data and interaction state used by [`crate::GraphView`].
#[derive(Clone, Debug)]
pub struct GraphViewModel {
    index: VaultIndex,
    pub mode: GraphMode,
    pub filter: GraphFilter,
    pub settings: GraphSettings,
    pub transform: ViewTransform,
    pub selected: Option<String>,
    pub nodes: Vec<PositionedNode>,
    pub edges: Vec<EdgeSegment>,
    events: VecDeque<GraphEvent>,
}

impl GraphViewModel {
    /// Build a deterministic graph snapshot from a vault index.
    pub fn new(index: &VaultIndex) -> Self {
        let filter = GraphFilter {
            include_unresolved: true,
            ..GraphFilter::default()
        };
        let mut model = Self {
            index: index.clone(),
            mode: GraphMode::Global,
            filter,
            settings: GraphSettings::default(),
            transform: ViewTransform::default(),
            selected: None,
            nodes: Vec::new(),
            edges: Vec::new(),
            events: VecDeque::new(),
        };
        model.rebuild(index);
        model
    }

    /// Replace the source snapshot while retaining viewport and selection state.
    pub fn rebuild(&mut self, index: &VaultIndex) {
        self.index = index.clone();
        let (mut raw_nodes, mut raw_edges) = graph(index, &self.filter);
        if self.mode == GraphMode::Local {
            if let Some(selected) = &self.selected {
                let mut local_ids = BTreeSet::from([selected.clone()]);
                for edge in &raw_edges {
                    if edge.from == *selected || edge.to == *selected {
                        local_ids.insert(edge.from.clone());
                        local_ids.insert(edge.to.clone());
                    }
                }
                raw_nodes.retain(|node| local_ids.contains(&node.id));
                raw_edges
                    .retain(|edge| local_ids.contains(&edge.from) && local_ids.contains(&edge.to));
            }
        }
        self.nodes = layout(raw_nodes, self.settings.show_unresolved);
        if let Some(selected) = &self.selected {
            for item in &mut self.nodes {
                if !item.node.unresolved && item.node.id != *selected {
                    item.style = NodeStyle::Inactive;
                }
            }
        }
        let positions = self
            .nodes
            .iter()
            .map(|node| (node.node.id.as_str(), node.position))
            .collect::<BTreeMap<_, _>>();
        self.edges = raw_edges
            .into_iter()
            .filter_map(|edge| {
                Some(EdgeSegment {
                    from: *positions.get(edge.from.as_str())?,
                    to: *positions.get(edge.to.as_str())?,
                    active: self
                        .selected
                        .as_ref()
                        .is_none_or(|id| id == &edge.from || id == &edge.to),
                    edge,
                })
            })
            .collect();
    }

    pub fn set_search(&mut self, query: impl Into<String>) {
        let query = query.into();
        self.filter.query = (!query.trim().is_empty()).then_some(query);
        let index = self.index.clone();
        self.rebuild(&index);
    }

    pub fn set_mode(&mut self, mode: GraphMode) {
        self.mode = mode;
        let index = self.index.clone();
        self.rebuild(&index);
    }

    pub fn set_show_unresolved(&mut self, show: bool) {
        self.settings.show_unresolved = show;
        self.filter.include_unresolved = show;
        let index = self.index.clone();
        self.rebuild(&index);
    }

    pub fn set_sparkle_enabled(&mut self, enabled: bool) {
        self.settings.sparkle_enabled = enabled;
    }

    pub fn toggle_group(&mut self, group: String) {
        if !self.filter.groups.insert(group.clone()) {
            self.filter.groups.remove(&group);
        }
        let index = self.index.clone();
        self.rebuild(&index);
    }

    pub fn select(&mut self, id: impl Into<String>) {
        let id = id.into();
        self.selected = Some(id.clone());
        let index = self.index.clone();
        self.rebuild(&index);
        self.events.push_back(GraphEvent::Select(id));
    }

    pub fn open(&mut self, id: impl Into<String>) {
        self.events.push_back(GraphEvent::Open(id.into()));
    }

    pub fn next_event(&mut self) -> Option<GraphEvent> {
        self.events.pop_front()
    }

    /// Move the viewport in screen-space pixels.
    pub fn pan_by(&mut self, delta: Point) {
        self.transform.pan.x += delta.x;
        self.transform.pan.y += delta.y;
    }
}

fn layout(nodes: Vec<GraphNode>, show_unresolved: bool) -> Vec<PositionedNode> {
    let nodes = nodes
        .into_iter()
        .filter(|node| show_unresolved || !node.unresolved)
        .collect::<Vec<_>>();
    let count = nodes.len().max(1) as f32;
    nodes
        .into_iter()
        .enumerate()
        .map(|(index, node)| {
            let angle = index as f32 * std::f32::consts::TAU / count;
            let radius = 120.0 + (index % 5) as f32 * 18.0;
            let style = if node.unresolved {
                NodeStyle::Unresolved
            } else {
                NodeStyle::Active
            };
            PositionedNode {
                node,
                position: Point {
                    x: angle.cos() * radius,
                    y: angle.sin() * radius,
                },
                style,
            }
        })
        .collect()
}
