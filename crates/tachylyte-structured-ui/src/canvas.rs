use gpui::{div, prelude::*, px, rgb, Context, Render, SharedString, Window};
use std::collections::{BTreeMap, BTreeSet};
use tachylyte_structured::{CanvasDocument, Edge, Node, Point, Rect, Size};

use crate::canvas_geometry::{
    card_label, card_preview, fit_transform, grid_lines, node_type, orthogonal_route, parse_color,
    NodeType,
};
use crate::canvas_history::CanvasHistory;
use crate::canvas_input::{
    apply_selection, expand_group_selection, normalized_selection, nodes_in_selection, DragMoveState,
    ResizeState,
};

/// A point in the deterministic screen coordinate system.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ScreenPoint {
    pub x: f64,
    pub y: f64,
}

/// Pan and zoom transform used by both painting and hit testing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CanvasTransform {
    pub pan: ScreenPoint,
    pub zoom: f64,
}

impl Default for CanvasTransform {
    fn default() -> Self {
        Self { pan: ScreenPoint::default(), zoom: 1.0 }
    }
}

impl CanvasTransform {
    pub fn world_to_screen(self, p: Point) -> ScreenPoint {
        ScreenPoint { x: p.x * self.zoom + self.pan.x, y: p.y * self.zoom + self.pan.y }
    }

    pub fn screen_to_world(self, p: ScreenPoint) -> Point {
        Point { x: (p.x - self.pan.x) / self.zoom, y: (p.y - self.pan.y) / self.zoom }
    }

    pub fn translated(self, delta: ScreenPoint) -> Self {
        Self {
            pan: ScreenPoint { x: self.pan.x + delta.x, y: self.pan.y + delta.y },
            ..self
        }
    }

    pub fn zoom_at(self, focus: ScreenPoint, factor: f64) -> Self {
        if !factor.is_finite() || factor <= 0.0 {
            return self;
        }
        let zoom = (self.zoom * factor).clamp(0.1, 8.0);
        let world = self.screen_to_world(focus);
        Self {
            zoom,
            pan: ScreenPoint { x: focus.x - world.x * zoom, y: focus.y - world.y * zoom },
        }
    }

    fn rect(self, r: Rect) -> (f64, f64, f64, f64) {
        let p = self.world_to_screen(Point { x: r.x, y: r.y });
        (p.x, p.y, r.width * self.zoom, r.height * self.zoom)
    }
}

/// Context and toolbar actions. The view emits these as intents; persistence remains host-owned.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanvasContextAction {
    CreateText,
    CreateFile,
    CreateLink,
    CreateGroup,
    Delete,
    Duplicate,
    FitViewport,
    Undo,
    Redo,
}

/// Commands emitted by Canvas controls. Hosts apply them to a domain document.
#[derive(Clone, Debug, PartialEq)]
pub enum CanvasCommand {
    Mode(CanvasMode),
    Select(Option<String>),
    SelectMany(Vec<String>),
    Pan(ScreenPoint),
    Zoom { focus: ScreenPoint, factor: f64 },
    Move { id: String, to: Point },
    Resize { id: String, size: Size },
    Connect(Edge),
    Create(Node),
    Delete { id: String },
    Duplicate { source: String, id: String },
    Undo,
    Redo,
    FitViewport { viewport: Rect },
    Context { id: Option<String>, action: CanvasContextAction },
}

/// Pointer interaction mode exposed by the Canvas toolbar.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CanvasMode {
    #[default]
    Select,
    Pan,
    Connect,
}

/// Testable state and hit model backing [`CanvasView`]. The document is never serialized or
/// silently rewritten by this projection; commands are the boundary to the domain codec.
#[derive(Debug)]
pub struct CanvasModel {
    pub document: CanvasDocument,
    pub transform: CanvasTransform,
    pub selected: BTreeSet<String>,
    pub disabled: bool,
    pub mode: CanvasMode,
    commands: Vec<CanvasCommand>,
    history: CanvasHistory,
    selection_start: Option<ScreenPoint>,
    selection_rect: Option<Rect>,
    drag: Option<DragMoveState>,
    drag_origins: BTreeMap<String, Point>,
    resize: Option<ResizeState>,
    resize_id: Option<String>,
}

impl CanvasModel {
    pub fn new(document: CanvasDocument) -> Self {
        Self {
            history: CanvasHistory::new(document.clone()),
            document,
            transform: CanvasTransform::default(),
            selected: BTreeSet::new(),
            disabled: false,
            mode: CanvasMode::default(),
            commands: Vec::new(),
            selection_start: None,
            selection_rect: None,
            drag: None,
            drag_origins: BTreeMap::new(),
            resize: None,
            resize_id: None,
        }
    }

    pub fn set_mode(&mut self, mode: CanvasMode) {
        if !self.disabled {
            self.mode = mode;
            self.commands.push(CanvasCommand::Mode(mode));
        }
    }

    pub fn pointer_down(&mut self, screen: ScreenPoint) -> Option<String> {
        match self.mode {
            CanvasMode::Select => self.select_at(screen),
            CanvasMode::Pan => None,
            CanvasMode::Connect => self.connect_at(screen),
        }
    }

    pub fn pointer_move(&mut self, delta: ScreenPoint) {
        if self.mode == CanvasMode::Pan {
            self.pan(delta);
        }
    }

    pub fn select_at_with_additive(&mut self, screen: ScreenPoint, additive: bool) -> Option<String> {
        if self.disabled {
            return None;
        }
        let id = self.document.hit_test(self.transform.screen_to_world(screen)).map(|n| n.id.clone());
        apply_selection(&mut self.selected, id.clone(), additive);
        self.selected = expand_group_selection(&self.document.nodes, &self.selected);
        self.commands.push(CanvasCommand::Select(id.clone()));
        id
    }

    pub fn select_at(&mut self, screen: ScreenPoint) -> Option<String> {
        self.select_at_with_additive(screen, false)
    }

    pub fn begin_selection(&mut self, screen: ScreenPoint) {
        if !self.disabled {
            self.selection_start = Some(screen);
            self.selection_rect = Some(Rect { x: screen.x, y: screen.y, width: 0.0, height: 0.0 });
        }
    }

    pub fn update_selection(&mut self, screen: ScreenPoint) {
        if let Some(start) = self.selection_start {
            self.selection_rect = Some(Rect { x: start.x, y: start.y, width: screen.x - start.x, height: screen.y - start.y });
        }
    }

    pub fn finish_selection(&mut self, screen: ScreenPoint, additive: bool) -> BTreeSet<String> {
        if self.disabled {
            return BTreeSet::new();
        }
        self.update_selection(screen);
        let selected = self.selection_rect.map(|r| {
            let world_a = self.transform.screen_to_world(ScreenPoint { x: r.x, y: r.y });
            let world_b = self.transform.screen_to_world(ScreenPoint { x: r.x + r.width, y: r.y + r.height });
            nodes_in_selection(&self.document.nodes, normalized_selection(world_a, world_b))
        }).unwrap_or_default();
        let selected = expand_group_selection(&self.document.nodes, &selected);
        apply_selection(&mut self.selected, selected.iter().cloned(), additive);
        self.commands.push(CanvasCommand::SelectMany(self.selected.iter().cloned().collect()));
        self.selection_start = None;
        self.selection_rect = None;
        selected
    }

    pub fn selection_rect(&self) -> Option<Rect> { self.selection_rect }

    pub fn begin_drag(&mut self, screen: ScreenPoint) {
        if self.disabled || self.selected.is_empty() { return; }
        self.drag = Some(DragMoveState::new(self.transform.screen_to_world(screen)));
        self.drag_origins = self.selected.iter().filter_map(|id| self.document.node(id).map(|n| (id.clone(), Point { x: n.x, y: n.y }))).collect();
    }

    pub fn drag_to(&mut self, screen: ScreenPoint) {
        let Some(drag) = &mut self.drag else { return; };
        drag.update(self.transform.screen_to_world(screen));
        let delta = drag.delta();
        for id in self.selected.clone() {
            if let Some(origin) = self.drag_origins.get(&id) {
                self.commands.push(CanvasCommand::Move { id, to: Point { x: origin.x + delta.x, y: origin.y + delta.y } });
            }
        }
    }

    pub fn finish_drag(&mut self) { self.drag = None; self.drag_origins.clear(); }

    pub fn begin_resize(&mut self, id: &str) {
        if self.disabled { return; }
        if let Some(node) = self.document.node(id) {
            self.resize = Some(ResizeState::new(Size { width: node.width, height: node.height }, Size { width: 24.0, height: 24.0 }));
            self.resize_id = Some(id.into());
        }
    }

    pub fn resize_to(&mut self, size: Size) {
        let Some(resize) = &mut self.resize else { return; };
        resize.update(size);
        if let Some(id) = &self.resize_id { self.commands.push(CanvasCommand::Resize { id: id.clone(), size: resize.size() }); }
    }

    pub fn finish_resize(&mut self) { self.resize = None; self.resize_id = None; }

    pub fn connect_at(&mut self, screen: ScreenPoint) -> Option<String> {
        if self.disabled { return None; }
        let target = self.document.hit_test(self.transform.screen_to_world(screen)).map(|node| node.id.clone());
        if let (Some(source), Some(target)) = (self.selected.iter().next().cloned(), target.clone()) {
            self.connect_nodes(&source, &target);
        }
        target
    }

    pub fn connect_nodes(&mut self, source: &str, target: &str) {
        if self.disabled || source == target { return; }
        let Some(from) = self.document.node(source) else { return; };
        let Some(to) = self.document.node(target) else { return; };
        let (from_side, to_side) = sides_for(from, to);
        self.connect_intent(Edge {
            id: format!("edge-{source}-{target}"), from_node: source.into(), to_node: target.into(),
            from_side: Some(from_side.into()), to_side: Some(to_side.into()), ..Default::default()
        });
    }

    pub fn move_intent(&mut self, id: &str, screen: ScreenPoint) {
        if !self.disabled { self.commands.push(CanvasCommand::Move { id: id.into(), to: self.transform.screen_to_world(screen) }); }
    }

    pub fn resize_intent(&mut self, id: &str, size: Size) {
        if !self.disabled { self.commands.push(CanvasCommand::Resize { id: id.into(), size }); }
    }

    pub fn connect_intent(&mut self, edge: Edge) {
        if !self.disabled { self.commands.push(CanvasCommand::Connect(edge)); }
    }

    pub fn pan(&mut self, delta: ScreenPoint) {
        if !self.disabled { self.transform = self.transform.translated(delta); self.commands.push(CanvasCommand::Pan(delta)); }
    }

    pub fn zoom(&mut self, focus: ScreenPoint, factor: f64) {
        if !self.disabled && factor.is_finite() && factor > 0.0 {
            self.transform = self.transform.zoom_at(focus, factor);
            self.commands.push(CanvasCommand::Zoom { focus, factor });
        }
    }

    pub fn fit_viewport(&mut self, viewport: Rect) {
        if self.disabled { return; }
        let fit = fit_transform(&self.document, viewport, 24.0);
        self.transform = CanvasTransform { pan: ScreenPoint { x: fit.pan.x, y: fit.pan.y }, zoom: fit.zoom };
        self.commands.push(CanvasCommand::FitViewport { viewport });
    }

    pub fn create_node(&mut self, node: Node) {
        if self.disabled { return; }
        if self.history.create_node(node.clone()).is_ok() { self.sync_history(); self.commands.push(CanvasCommand::Create(node)); }
    }

    pub fn delete_selected(&mut self) {
        let ids = self.selected.iter().cloned().collect::<Vec<_>>();
        for id in ids { self.delete_node(&id); }
    }

    pub fn delete_node(&mut self, id: &str) {
        if self.disabled { return; }
        if self.history.delete_node(id).is_ok() { self.sync_history(); self.selected.remove(id); self.commands.push(CanvasCommand::Delete { id: id.into() }); }
    }

    pub fn duplicate_node(&mut self, id: &str) -> Option<String> {
        if self.disabled { return None; }
        let copy = self.history.duplicate_node(id).ok()?;
        self.sync_history();
        self.commands.push(CanvasCommand::Duplicate { source: id.into(), id: copy.id.clone() });
        Some(copy.id)
    }

    pub fn undo(&mut self) {
        if !self.disabled && self.history.undo() { self.sync_history(); self.commands.push(CanvasCommand::Undo); }
    }

    pub fn redo(&mut self) {
        if !self.disabled && self.history.redo() { self.sync_history(); self.commands.push(CanvasCommand::Redo); }
    }

    pub fn can_undo(&self) -> bool { self.history.can_undo() }
    pub fn can_redo(&self) -> bool { self.history.can_redo() }

    pub fn context_action(&mut self, id: Option<&str>, action: CanvasContextAction) {
        if self.disabled { return; }
        let id_owned = id.map(str::to_owned);
        match action {
            CanvasContextAction::Delete => if let Some(id) = id { self.delete_node(id) },
            CanvasContextAction::Duplicate => if let Some(id) = id { self.duplicate_node(id); },
            CanvasContextAction::Undo => self.undo(),
            CanvasContextAction::Redo => self.redo(),
            _ => {}
        }
        self.commands.push(CanvasCommand::Context { id: id_owned, action });
    }

    fn sync_history(&mut self) { self.document = self.history.document().clone(); }

    pub fn edge_path(&self, edge: &Edge) -> Vec<(ScreenPoint, ScreenPoint)> {
        let (Some(from), Some(to)) = (self.document.node(&edge.from_node), self.document.node(&edge.to_node)) else { return Vec::new(); };
        orthogonal_route(from, to, edge).windows(2).map(|pair| (self.transform.world_to_screen(pair[0]), self.transform.world_to_screen(pair[1]))).collect()
    }

    pub fn take_commands(&mut self) -> Vec<CanvasCommand> { std::mem::take(&mut self.commands) }
}

fn sides_for(from: &Node, to: &Node) -> (&'static str, &'static str) {
    let from_center = Point { x: from.x + from.width / 2.0, y: from.y + from.height / 2.0 };
    let to_center = Point { x: to.x + to.width / 2.0, y: to.y + to.height / 2.0 };
    if (to_center.x - from_center.x).abs() >= (to_center.y - from_center.y).abs() {
        if to_center.x >= from_center.x { ("right", "left") } else { ("left", "right") }
    } else if to_center.y >= from_center.y { ("bottom", "top") } else { ("top", "bottom") }
}

fn color_u32(value: Option<&str>, fallback: u32) -> u32 {
    let [r, g, b, a] = parse_color(value);
    if value.is_some() { u32::from_be_bytes([r, g, b, a]) } else { fallback }
}

fn node_icon(kind: NodeType) -> &'static str {
    match kind { NodeType::Text => "▤", NodeType::File => "▣", NodeType::Link => "↗", NodeType::Group => "▦" }
}

/// A compact native GPUI Canvas viewport with toolbar and accessible labels.
pub struct CanvasView { pub model: CanvasModel }

impl CanvasView {
    pub fn new(model: CanvasModel) -> Self { Self { model } }
    pub fn from_document(document: CanvasDocument) -> Self { Self::new(CanvasModel::new(document)) }
    pub fn update_document(&mut self, document: CanvasDocument) {
        self.model = CanvasModel {
            transform: self.model.transform,
            selected: self.model.selected.clone(),
            disabled: self.model.disabled,
            mode: self.model.mode,
            ..CanvasModel::new(document)
        };
        self.model.selected.retain(|id| self.model.document.node(id).is_some());
    }
    pub fn set_disabled(&mut self, disabled: bool) { self.model.disabled = disabled; }
    pub fn pointer_down(&mut self, screen: ScreenPoint) -> Option<String> { self.model.pointer_down(screen) }
    pub fn pointer_move(&mut self, delta: ScreenPoint) { self.model.pointer_move(delta); }
    pub fn take_commands(&mut self) -> Vec<CanvasCommand> { self.model.take_commands() }
}

impl Render for CanvasView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();
        let mut viewport = div().id("canvas-viewport").relative().flex_1().bg(rgb(0xfafaf9ff)).overflow_hidden();
        let (grid_x, grid_y) = grid_lines(Rect { x: 0., y: 0., width: 4096., height: 4096. }, 64.);
        for world_x in grid_x {
            let offset = self.model.transform.world_to_screen(Point { x: world_x, y: 0. }).x as f32;
            viewport = viewport.child(div().absolute().left(px(offset)).top(px(0.)).w(px(1.)).h(px(4096.)).bg(rgb(0xe4e2ddff)));
        }
        for world_y in grid_y {
            let offset = self.model.transform.world_to_screen(Point { x: 0., y: world_y }).y as f32;
            viewport = viewport.child(div().absolute().left(px(0.)).top(px(offset)).w(px(4096.)).h(px(1.)).bg(rgb(0xe4e2ddff)));
        }
        for edge in &self.model.document.edges {
            let segments = self.model.edge_path(edge);
            for (start, end) in segments {
                let horizontal = (end.x - start.x).abs() >= (end.y - start.y).abs();
                let color = color_u32(edge.extra.get("color").and_then(|v| v.as_str()), 0xb8b5adff);
                viewport = viewport.child(if horizontal { div().absolute().left(px(start.x.min(end.x) as f32)).top(px(start.y as f32 - 1.)).w(px((end.x - start.x).abs().max(1.) as f32)).h(px(2.)).bg(rgb(color)) } else { div().absolute().left(px(start.x as f32 - 1.)).top(px(start.y.min(end.y) as f32)).w(px(2.)).h(px((end.y - start.y).abs().max(1.) as f32)).bg(rgb(color)) });
            }
            if let Some((_, end)) = self.model.edge_path(edge).last().copied() {
                viewport = viewport.child(div().absolute().left(px(end.x as f32 - 5.)).top(px(end.y as f32 - 7.)).text_color(rgb(0x68645dff)).child("▶"));
                if let Some(label) = &edge.label { viewport = viewport.child(div().absolute().left(px(end.x as f32 + 4.)).top(px(end.y as f32 - 10.)).text_color(rgb(0x68645dff)).child(label.clone())); }
            }
        }
        let mut nodes = self.model.document.nodes.iter().collect::<Vec<_>>();
        nodes.sort_by_key(|node| (node_type(node) != NodeType::Group, node.id.clone()));
        for node in nodes {
            let (x, y, w, h) = self.model.transform.rect(node.rect());
            let id = node.id.clone();
            let active = self.model.selected.contains(&id);
            let kind = node_type(node);
            let preview = card_preview(node).unwrap_or(&node.id).to_owned();
            let label = card_label(node);
            let bg = if active { 0xf1eee8ff } else { color_u32(node.color.as_deref(), if kind == NodeType::Group { 0xece8deff } else { 0xfffdf9ff }) };
            let e = entity.clone();
            let child = div().id(SharedString::from(format!("canvas-node-{id}"))).absolute().left(px(x as f32)).top(px(y as f32)).w(px(w.max(24.) as f32)).h(px(h.max(24.) as f32)).bg(rgb(bg)).border_1().border_color(rgb(if active { 0x8f887bff } else { 0xd6d3ccff })).p_2().text_color(rgb(0x222222ff)).child(div().child(format!("{}  {}", node_icon(kind), label))).child(if preview != label { div().text_color(rgb(0x68645dff)).child(preview) } else { div() }).on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| { e.update(cx, |view, cx| { view.model.select_at(ScreenPoint { x, y }); cx.notify(); }); });
            viewport = viewport.child(child);
        }
        if let Some(selection) = self.model.selection_rect {
            let r = Rect { x: selection.x.min(selection.x + selection.width), y: selection.y.min(selection.y + selection.height), width: selection.width.abs(), height: selection.height.abs() };
            viewport = viewport.child(div().absolute().left(px(r.x as f32)).top(px(r.y as f32)).w(px(r.width as f32)).h(px(r.height as f32)).border_1().border_color(rgb(0x6c8fbcff)).bg(rgb(0x6c8fbc22)));
        }
        let disabled = self.model.disabled;
        let select = entity.clone(); let pan = entity.clone(); let connect = entity.clone(); let fit = entity.clone(); let undo = entity.clone(); let redo = entity.clone(); let duplicate = entity.clone(); let delete = entity.clone();
        let status = if disabled { "Canvas unavailable" } else if self.model.document.nodes.is_empty() { "No nodes to display" } else { "" };
        div().flex().flex_col().size_full().text_color(rgb(0x222222ff)).child(div().h(px(36.)).flex().items_center().gap_2().px_2().bg(rgb(0xffffffff)).border_b_1().border_color(rgb(0xe0e0e0ff)).child(if disabled { "Canvas (disabled)" } else { "Canvas" }).child(toolbar_button("canvas-select", "Select", select, |v| v.model.set_mode(CanvasMode::Select))).child(toolbar_button("canvas-pan", "✋ Pan", pan, |v| v.model.set_mode(CanvasMode::Pan))).child(toolbar_button("canvas-connect", "⌁ Connect", connect, |v| v.model.set_mode(CanvasMode::Connect))).child(toolbar_button("canvas-fit", "Fit", fit, |v| v.model.fit_viewport(Rect { x: 0., y: 0., width: 800., height: 500. }))).child(toolbar_button("canvas-undo", "Undo", undo, |v| v.model.undo())).child(toolbar_button("canvas-redo", "Redo", redo, |v| v.model.redo())).child(toolbar_button("canvas-duplicate", "Duplicate", duplicate, |v| if let Some(id) = v.model.selected.iter().next().cloned() { v.model.duplicate_node(&id); })).child(toolbar_button("canvas-delete", "Delete", delete, |v| v.model.delete_selected()))).child(if status.is_empty() { viewport } else { viewport.child(div().absolute().top(px(24.)).left(px(24.)).p_3().bg(rgb(0xfffdf9ff)).border_1().border_color(rgb(0xd6d3ccff)).child(status)) })
    }
}

fn toolbar_button<F>(id: &'static str, label: &'static str, entity: gpui::Entity<CanvasView>, action: F) -> impl IntoElement
where F: Fn(&mut CanvasView) + 'static {
    div().id(id).h(px(28.)).px_2().items_center().hover(|s| s.bg(rgb(0xeeeeeeff))).child(label).on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| { entity.update(cx, |view, cx| { action(view); cx.notify(); }); })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn node(id: &str) -> Node { Node { id: id.into(), kind: "text".into(), x: 10., y: 20., width: 30., height: 40., text: None, file: None, url: None, color: None, extra: Default::default() } }
    #[test] fn transform_round_trip_and_focus_zoom() { let t = CanvasTransform { pan: ScreenPoint { x: 4., y: 5. }, zoom: 2. }; let p = t.world_to_screen(Point { x: 3., y: 7. }); assert_eq!(t.screen_to_world(p), Point { x: 3., y: 7. }); assert_eq!(t.zoom_at(p, 2.).world_to_screen(Point { x: 3., y: 7. }), p); }
    #[test] fn selection_hit_and_disabled_commands() { let mut m = CanvasModel::new(CanvasDocument { nodes: vec![node("a")], ..Default::default() }); assert_eq!(m.select_at(ScreenPoint { x: 20., y: 30. }).as_deref(), Some("a")); assert!(m.take_commands().iter().any(|c| matches!(c, CanvasCommand::Select(Some(id)) if id == "a"))); m.disabled = true; m.move_intent("a", ScreenPoint { x: 1., y: 1. }); assert!(m.take_commands().is_empty()); }
    #[test] fn modes_dispatch_connect_and_edge_path_is_orthogonal() { let mut b = node("b"); b.x = 100.; b.y = 70.; let mut model = CanvasModel::new(CanvasDocument { nodes: vec![node("a"), b], ..Default::default() }); model.set_mode(CanvasMode::Connect); model.selected.insert("a".into()); assert_eq!(model.pointer_down(ScreenPoint { x: 110., y: 90. }).as_deref(), Some("b")); assert!(model.take_commands().iter().any(|c| matches!(c, CanvasCommand::Connect(_)))); let path = model.edge_path(&Edge { id: "e".into(), from_node: "a".into(), to_node: "b".into(), ..Default::default() }); assert_eq!(path.len(), 2); }
    #[test] fn rectangle_selection_and_crud_history() { let mut m = CanvasModel::new(CanvasDocument { nodes: vec![node("a")], ..Default::default() }); m.begin_selection(ScreenPoint { x: 0., y: 0. }); assert_eq!(m.finish_selection(ScreenPoint { x: 100., y: 100. }, false), BTreeSet::from(["a".into()])); let copy = m.duplicate_node("a").unwrap(); assert!(m.document.node(&copy).is_some()); m.undo(); assert!(m.document.node(&copy).is_none()); m.redo(); assert!(m.document.node(&copy).is_some()); }
}
