use gpui::{div, prelude::*, px, rgb, Context, Render, SharedString, Window};
use std::collections::BTreeSet;
use tachylyte_structured::{CanvasDocument, Edge, Point, Rect, Size};

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
        Self {
            pan: ScreenPoint::default(),
            zoom: 1.0,
        }
    }
}
impl CanvasTransform {
    /// Convert a world point to screen coordinates.
    pub fn world_to_screen(self, p: Point) -> ScreenPoint {
        ScreenPoint {
            x: p.x * self.zoom + self.pan.x,
            y: p.y * self.zoom + self.pan.y,
        }
    }
    /// Convert a screen point to world coordinates.
    pub fn screen_to_world(self, p: ScreenPoint) -> Point {
        Point {
            x: (p.x - self.pan.x) / self.zoom,
            y: (p.y - self.pan.y) / self.zoom,
        }
    }
    /// Pan by a screen-space delta.
    pub fn translated(self, delta: ScreenPoint) -> Self {
        Self {
            pan: ScreenPoint {
                x: self.pan.x + delta.x,
                y: self.pan.y + delta.y,
            },
            ..self
        }
    }
    /// Zoom around a fixed screen point, keeping that point stable.
    pub fn zoom_at(self, focus: ScreenPoint, factor: f64) -> Self {
        if !factor.is_finite() || factor <= 0. {
            return self;
        }
        let zoom = (self.zoom * factor).clamp(0.1, 8.0);
        let world = self.screen_to_world(focus);
        Self {
            zoom,
            pan: ScreenPoint {
                x: focus.x - world.x * zoom,
                y: focus.y - world.y * zoom,
            },
        }
    }
    fn rect(self, r: Rect) -> (f64, f64, f64, f64) {
        let p = self.world_to_screen(Point { x: r.x, y: r.y });
        (p.x, p.y, r.width * self.zoom, r.height * self.zoom)
    }
}

/// Commands emitted by Canvas controls. Hosts apply these to a domain document.
#[derive(Clone, Debug, PartialEq)]
pub enum CanvasCommand {
    Mode(CanvasMode),
    Select(Option<String>),
    Pan(ScreenPoint),
    Zoom { focus: ScreenPoint, factor: f64 },
    Move { id: String, to: Point },
    Resize { id: String, size: Size },
    Connect(Edge),
}

/// Pointer interaction mode exposed by the Canvas toolbar.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CanvasMode {
    #[default]
    Select,
    Pan,
    Connect,
}

/// Testable state and hit model backing [`CanvasView`].
#[derive(Clone, Debug)]
pub struct CanvasModel {
    pub document: CanvasDocument,
    pub transform: CanvasTransform,
    pub selected: BTreeSet<String>,
    pub disabled: bool,
    pub mode: CanvasMode,
    commands: Vec<CanvasCommand>,
}
impl CanvasModel {
    /// Create a Canvas model with the identity transform.
    pub fn new(document: CanvasDocument) -> Self {
        Self {
            document,
            transform: CanvasTransform::default(),
            selected: BTreeSet::new(),
            disabled: false,
            mode: CanvasMode::default(),
            commands: Vec::new(),
        }
    }
    /// Change pointer mode and emit a toolbar command.
    pub fn set_mode(&mut self, mode: CanvasMode) {
        if !self.disabled {
            self.mode = mode;
            self.commands.push(CanvasCommand::Mode(mode));
        }
    }
    /// Dispatch a pointer press according to the active toolbar mode.
    pub fn pointer_down(&mut self, screen: ScreenPoint) -> Option<String> {
        match self.mode {
            CanvasMode::Select => self.select_at(screen),
            CanvasMode::Pan => None,
            CanvasMode::Connect => self.connect_at(screen),
        }
    }
    /// Dispatch a pointer delta; pan is only active in Pan mode.
    pub fn pointer_move(&mut self, delta: ScreenPoint) {
        if self.mode == CanvasMode::Pan {
            self.pan(delta);
        }
    }
    /// Connect the selected node to the node under the pointer.
    pub fn connect_at(&mut self, screen: ScreenPoint) -> Option<String> {
        if self.disabled {
            return None;
        }
        let target = self
            .document
            .hit_test(self.transform.screen_to_world(screen))
            .map(|node| node.id.clone());
        if let (Some(source), Some(target)) = (self.selected.iter().next().cloned(), target.clone())
        {
            if source != target {
                self.connect_intent(Edge {
                    id: format!("edge-{source}-{target}"),
                    from_node: source,
                    to_node: target,
                    ..Default::default()
                });
            }
        }
        target
    }
    /// Select the topmost node at a screen point and emit a selection command.
    pub fn select_at(&mut self, screen: ScreenPoint) -> Option<String> {
        if self.disabled {
            return None;
        };
        let id = self
            .document
            .hit_test(self.transform.screen_to_world(screen))
            .map(|n| n.id.clone());
        self.selected.clear();
        if let Some(id) = &id {
            self.selected.insert(id.clone());
        }
        self.commands.push(CanvasCommand::Select(id.clone()));
        id
    }
    /// Emit a move intent without mutating the domain document.
    pub fn move_intent(&mut self, id: &str, screen: ScreenPoint) {
        if !self.disabled {
            self.commands.push(CanvasCommand::Move {
                id: id.into(),
                to: self.transform.screen_to_world(screen),
            });
        }
    }
    /// Emit a resize intent.
    pub fn resize_intent(&mut self, id: &str, size: Size) {
        if !self.disabled {
            self.commands.push(CanvasCommand::Resize {
                id: id.into(),
                size,
            });
        }
    }
    /// Emit a connection intent.
    pub fn connect_intent(&mut self, edge: Edge) {
        if !self.disabled {
            self.commands.push(CanvasCommand::Connect(edge));
        }
    }
    /// Pan and emit an intent.
    pub fn pan(&mut self, delta: ScreenPoint) {
        if !self.disabled {
            self.transform = self.transform.translated(delta);
            self.commands.push(CanvasCommand::Pan(delta));
        }
    }
    /// Zoom and emit an intent.
    pub fn zoom(&mut self, focus: ScreenPoint, factor: f64) {
        if !self.disabled && factor.is_finite() && factor > 0. {
            self.transform = self.transform.zoom_at(focus, factor);
            self.commands.push(CanvasCommand::Zoom { focus, factor });
        }
    }
    /// Return a deterministic two-segment orthogonal path for an edge.
    pub fn edge_path(&self, edge: &Edge) -> Vec<(ScreenPoint, ScreenPoint)> {
        let Some(from) = self.document.node(&edge.from_node) else {
            return Vec::new();
        };
        let Some(to) = self.document.node(&edge.to_node) else {
            return Vec::new();
        };
        let start = self.transform.world_to_screen(Point {
            x: from.x + from.width / 2.,
            y: from.y + from.height / 2.,
        });
        let end = self.transform.world_to_screen(Point {
            x: to.x + to.width / 2.,
            y: to.y + to.height / 2.,
        });
        let bend = ScreenPoint {
            x: end.x,
            y: start.y,
        };
        vec![(start, bend), (bend, end)]
    }
    /// Drain commands emitted since the previous call.
    pub fn take_commands(&mut self) -> Vec<CanvasCommand> {
        std::mem::take(&mut self.commands)
    }
}

/// A compact native GPUI Canvas viewport with toolbar and accessible labels.
pub struct CanvasView {
    pub model: CanvasModel,
}
impl CanvasView {
    /// Construct a Canvas view.
    pub fn new(model: CanvasModel) -> Self {
        Self { model }
    }
}
impl Render for CanvasView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let dark = 0x202124ff;
        let node_bg = 0x3b4252ff;
        let selected = 0x88c0d0ff;
        let entity = cx.entity();
        let mut viewport = div()
            .id("canvas-viewport")
            .relative()
            .flex_1()
            .bg(rgb(dark))
            .overflow_hidden();
        // A fixed-size grid keeps the viewport legible while remaining deterministic.
        for i in 0..32 {
            let offset = (i * 64) as f32;
            viewport = viewport.child(
                div()
                    .absolute()
                    .left(px(offset))
                    .top(px(0.))
                    .w(px(1.))
                    .h(px(4096.))
                    .bg(rgb(0x2a2d32ff)),
            );
            viewport = viewport.child(
                div()
                    .absolute()
                    .left(px(0.))
                    .top(px(offset))
                    .w(px(4096.))
                    .h(px(1.))
                    .bg(rgb(0x2a2d32ff)),
            );
        }
        // Edges are represented by deterministic midpoint connectors. A host can
        // replace this simple primitive with a custom paint layer without changing
        // the command/model contract.
        for edge in &self.model.document.edges {
            for (start, end) in self.model.edge_path(edge) {
                let horizontal = (end.x - start.x).abs() >= (end.y - start.y).abs();
                viewport = viewport.child(if horizontal {
                    div()
                        .absolute()
                        .left(px(start.x.min(end.x) as f32))
                        .top(px(start.y as f32))
                        .w(px((end.x - start.x).abs().max(1.) as f32))
                        .h(px(1.))
                        .bg(rgb(0x8fbcbbff))
                } else {
                    div()
                        .absolute()
                        .left(px(start.x as f32))
                        .top(px(start.y.min(end.y) as f32))
                        .w(px(1.))
                        .h(px((end.y - start.y).abs().max(1.) as f32))
                        .bg(rgb(0x8fbcbbff))
                });
            }
        }
        for node in &self.model.document.nodes {
            let (x, y, w, h) = self.model.transform.rect(node.rect());
            let id = node.id.clone();
            let active = self.model.selected.contains(&id);
            let e = entity.clone();
            let label = node
                .text
                .as_deref()
                .or(node.file.as_deref())
                .unwrap_or(&node.kind)
                .to_string();
            let child = div()
                .id(SharedString::from(format!("canvas-node-{id}")))
                .absolute()
                .left(px(x as f32))
                .top(px(y as f32))
                .w(px(w as f32))
                .h(px(h as f32))
                .bg(rgb(if active { selected } else { node_bg }))
                .border_1()
                .border_color(rgb(if active { 0xffffffff } else { 0x687080ff }))
                .p_2()
                .text_color(rgb(0xffffffff))
                .child(label)
                .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
                    e.update(cx, |view, cx| {
                        view.model.select_at(ScreenPoint {
                            x: x + w / 2.,
                            y: y + h / 2.,
                        });
                        cx.notify();
                    });
                });
            viewport = viewport.child(child);
        }
        let e = entity.clone();
        let select = entity.clone();
        let pan = entity.clone();
        let connect = entity.clone();
        div()
            .flex()
            .flex_col()
            .size_full()
            .text_color(rgb(0xffffffff))
            .child(
                div()
                    .h(px(36.))
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_2()
                    .bg(rgb(0x30343bff))
                    .child("Canvas")
                    .child(
                        div()
                            .id("canvas-select")
                            .p_1()
                            .child("Select")
                            .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
                                select.update(cx, |v, cx| {
                                    v.model.set_mode(CanvasMode::Select);
                                    cx.notify();
                                });
                            }),
                    )
                    .child(div().id("canvas-pan").p_1().child("Pan").on_mouse_down(
                        gpui::MouseButton::Left,
                        move |_, _, cx| {
                            pan.update(cx, |v, cx| {
                                v.model.set_mode(CanvasMode::Pan);
                                cx.notify();
                            });
                        },
                    ))
                    .child(
                        div()
                            .id("canvas-connect")
                            .p_1()
                            .child("Connect")
                            .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
                                connect.update(cx, |v, cx| {
                                    v.model.set_mode(CanvasMode::Connect);
                                    cx.notify();
                                });
                            }),
                    )
                    .child(div().id("canvas-zoom-in").p_1().child("+").on_mouse_down(
                        gpui::MouseButton::Left,
                        move |_, _, cx| {
                            e.update(cx, |v, cx| {
                                v.model.zoom(ScreenPoint { x: 400., y: 250. }, 1.1);
                                cx.notify();
                            });
                        },
                    ))
                    .child("Pan · Select · Connect"),
            )
            .child(viewport)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn node(id: &str) -> tachylyte_structured::Node {
        tachylyte_structured::Node {
            id: id.into(),
            kind: "text".into(),
            x: 10.,
            y: 20.,
            width: 30.,
            height: 40.,
            text: None,
            file: None,
            url: None,
            color: None,
            extra: Default::default(),
        }
    }
    #[test]
    fn transform_round_trip_and_focus_zoom() {
        let t = CanvasTransform {
            pan: ScreenPoint { x: 4., y: 5. },
            zoom: 2.,
        };
        let p = t.world_to_screen(Point { x: 3., y: 7. });
        assert_eq!(t.screen_to_world(p), Point { x: 3., y: 7. });
        assert_eq!(t.zoom_at(p, 2.).world_to_screen(Point { x: 3., y: 7. }), p);
    }
    #[test]
    fn selection_hit_and_disabled_commands() {
        let mut m = CanvasModel::new(CanvasDocument {
            nodes: vec![node("a")],
            ..Default::default()
        });
        assert_eq!(
            m.select_at(ScreenPoint { x: 20., y: 30. }).as_deref(),
            Some("a")
        );
        assert_eq!(
            m.take_commands(),
            vec![CanvasCommand::Select(Some("a".into()))]
        );
        m.disabled = true;
        m.move_intent("a", ScreenPoint { x: 1., y: 1. });
        assert!(m.take_commands().is_empty());
    }

    #[test]
    fn invalid_zoom_is_rejected_before_transform_or_command() {
        let mut model = CanvasModel::new(CanvasDocument::default());
        let original = model.transform;
        model.zoom(ScreenPoint::default(), 0.);
        model.zoom(ScreenPoint::default(), f64::NAN);
        assert_eq!(model.transform, original);
        assert!(model.take_commands().is_empty());
    }

    #[test]
    fn modes_dispatch_connect_and_edge_path_is_orthogonal() {
        let mut model = CanvasModel::new(CanvasDocument {
            nodes: vec![node("a"), {
                let mut n = node("b");
                n.x = 100.;
                n.y = 70.;
                n
            }],
            ..Default::default()
        });
        model.set_mode(CanvasMode::Connect);
        model.selected.insert("a".into());
        assert_eq!(
            model
                .pointer_down(ScreenPoint { x: 110., y: 90. })
                .as_deref(),
            Some("b")
        );
        assert!(matches!(
            model.take_commands().last(),
            Some(CanvasCommand::Connect(_))
        ));
        let path = model.edge_path(&Edge {
            id: "e".into(),
            from_node: "a".into(),
            to_node: "b".into(),
            ..Default::default()
        });
        assert_eq!(path.len(), 2);
        assert!(path.iter().all(|(a, b)| (a.x == b.x) ^ (a.y == b.y)));
    }
}
