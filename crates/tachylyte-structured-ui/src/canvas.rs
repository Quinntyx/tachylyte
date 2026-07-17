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
    Select(Option<String>),
    Pan(ScreenPoint),
    Zoom { focus: ScreenPoint, factor: f64 },
    Move { id: String, to: Point },
    Resize { id: String, size: Size },
    Connect(Edge),
}

/// Testable state and hit model backing [`CanvasView`].
#[derive(Clone, Debug)]
pub struct CanvasModel {
    pub document: CanvasDocument,
    pub transform: CanvasTransform,
    pub selected: BTreeSet<String>,
    pub disabled: bool,
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
            commands: Vec::new(),
        }
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
        if !self.disabled {
            self.transform = self.transform.zoom_at(focus, factor);
            self.commands.push(CanvasCommand::Zoom { focus, factor });
        }
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
            let Some(from) = self.model.document.node(&edge.from_node) else {
                continue;
            };
            let Some(to) = self.model.document.node(&edge.to_node) else {
                continue;
            };
            let a = self.model.transform.world_to_screen(Point {
                x: from.x + from.width / 2.,
                y: from.y + from.height / 2.,
            });
            let b = self.model.transform.world_to_screen(Point {
                x: to.x + to.width / 2.,
                y: to.y + to.height / 2.,
            });
            let width = (b.x - a.x).abs().max(1.);
            viewport = viewport.child(
                div()
                    .absolute()
                    .left(px(a.x.min(b.x) as f32))
                    .top(px(a.y.min(b.y) as f32))
                    .w(px(width as f32))
                    .h(px(1.))
                    .bg(rgb(0x8fbcbbff))
                    .child(edge.label.clone().unwrap_or_default()),
            );
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
}
