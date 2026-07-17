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
///
/// The fields are private so an invalid zoom cannot be introduced through a
/// struct literal. Use [`Self::new`], [`Self::set_zoom`], or [`Self::zoom_by`]
/// to update the viewport.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewTransform {
    pan: Point,
    zoom: f32,
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
    const MIN_ZOOM: f32 = 0.1;
    const MAX_ZOOM: f32 = 8.0;

    /// Construct a transform, clamping zoom to the supported finite range.
    pub fn new(pan: Point, zoom: f32) -> Self {
        Self {
            pan: finite_point(pan),
            zoom: valid_zoom(zoom),
        }
    }

    /// Return the current screen-space pan.
    pub fn pan(&self) -> Point {
        self.pan
    }

    /// Return the current positive, finite zoom factor.
    pub fn zoom(&self) -> f32 {
        self.zoom
    }

    /// Replace the pan, saturating non-finite components to safe values.
    pub fn set_pan(&mut self, pan: Point) {
        self.pan = finite_point(pan);
    }

    /// Replace the zoom, clamping it to the supported range.
    pub fn set_zoom(&mut self, zoom: f32) {
        self.zoom = valid_zoom(zoom);
    }

    pub fn screen(&self, point: Point) -> Point {
        Point {
            x: finite(point.x * self.zoom + self.pan.x),
            y: finite(point.y * self.zoom + self.pan.y),
        }
    }

    pub fn world(&self, point: Point) -> Point {
        Point {
            x: finite((point.x - self.pan.x) / self.zoom),
            y: finite((point.y - self.pan.y) / self.zoom),
        }
    }

    pub fn zoom_by(&mut self, factor: f32) {
        if factor.is_finite() && factor > 0.0 {
            self.zoom = valid_zoom(self.zoom * factor);
        }
    }
}

fn valid_zoom(zoom: f32) -> f32 {
    if zoom.is_nan() {
        1.0
    } else if zoom.is_sign_negative() || zoom == 0.0 {
        ViewTransform::MIN_ZOOM
    } else {
        zoom.clamp(ViewTransform::MIN_ZOOM, ViewTransform::MAX_ZOOM)
    }
}

fn finite(value: f32) -> f32 {
    if value.is_nan() {
        0.0
    } else if value.is_infinite() {
        value.signum() * f32::MAX
    } else {
        value
    }
}

fn finite_point(point: Point) -> Point {
    Point {
        x: finite(point.x),
        y: finite(point.y),
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
        self.transform.set_pan(Point {
            x: self.transform.pan().x + delta.x,
            y: self.transform.pan().y + delta.y,
        });
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
