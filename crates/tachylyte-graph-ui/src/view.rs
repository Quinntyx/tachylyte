//! Native GPUI presentation for the graph model.

use gpui::{div, prelude::*, px, Context, Entity, Render, SharedString, Window};
use tachylyte_theme::light;

use crate::model::{GraphMode, GraphViewModel, NodeStyle, Point};

/// A mountable native GPUI graph surface.
#[derive(Clone, Debug)]
pub struct GraphView {
    pub model: GraphViewModel,
}

impl GraphView {
    pub fn new(model: GraphViewModel) -> Self {
        Self { model }
    }

    /// Construct directly from a knowledge-index snapshot.
    pub fn from_index(index: &tachylyte_knowledge::VaultIndex) -> Self {
        Self::new(GraphViewModel::new(index))
    }

    /// Replace graph data while retaining viewport, filters, and selection.
    pub fn update(&mut self, index: &tachylyte_knowledge::VaultIndex) {
        self.model.rebuild(index);
    }

    /// Drain selection and open events emitted since the last call.
    pub fn take_events(&mut self) -> Vec<crate::GraphEvent> {
        let mut events = Vec::new();
        while let Some(event) = self.model.next_event() {
            events.push(event);
        }
        events
    }
}

fn button<E: 'static>(
    label: impl Into<SharedString>,
    id: &'static str,
    target: Entity<E>,
    f: impl Fn(&mut E) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .px_2()
        .py_1()
        .rounded_sm()
        .hover(|style| style.bg(light().hover()))
        .child(label.into())
        .on_click(move |_, _, cx| {
            target.update(cx, |view, cx| {
                f(view);
                cx.notify();
            });
        })
}

impl Render for GraphView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();
        let palette = light();
        let mut canvas = div()
            .id("graph-canvas")
            .relative()
            .flex_1()
            .overflow_hidden()
            .bg(palette.canvas());

        // Draw edge segments before nodes, so nodes remain legible.
        for segment in self.model.edges.clone() {
            let start = self.model.transform.screen(segment.from);
            let end = self.model.transform.screen(segment.to);
            let color = if segment.active {
                palette.purple()
            } else {
                palette.border_subtle()
            };
            let horizontal = (end.x - start.x).abs() >= (end.y - start.y).abs();
            let (x, y, width, height) = if horizontal {
                (
                    start.x.min(end.x),
                    start.y,
                    (end.x - start.x).abs().max(1.0),
                    2.0,
                )
            } else {
                (
                    start.x,
                    start.y.min(end.y),
                    2.0,
                    (end.y - start.y).abs().max(1.0),
                )
            };
            canvas = canvas.child(
                div()
                    .absolute()
                    .left(px(x))
                    .top(px(y))
                    .w(px(width))
                    .h(px(height))
                    .bg(color),
            );
        }

        for positioned in self.model.nodes.clone() {
            let point = self.model.transform.screen(positioned.position);
            let id = positioned.node.id.clone();
            let selected = self.model.selected.as_deref() == Some(id.as_str());
            let (fill, border, text) = match positioned.style {
                NodeStyle::Unresolved => {
                    (palette.secondary(), palette.border(), palette.faint_text())
                }
                NodeStyle::Inactive => (
                    palette.background(),
                    palette.border_subtle(),
                    palette.muted_text(),
                ),
                NodeStyle::Active if selected => {
                    (palette.selection(), palette.purple(), palette.text())
                }
                NodeStyle::Active => (palette.background(), palette.border(), palette.text()),
            };
            let target = entity.clone();
            let click_id = id.clone();
            let label = positioned.node.id.clone();
            canvas = canvas.child(
                div()
                    .id(SharedString::from(format!("graph-node-{id}")))
                    .absolute()
                    .left(px(point.x - 42.0))
                    .top(px(point.y - 18.0))
                    .w(px(84.0))
                    .h(px(36.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .px_1()
                    .bg(fill)
                    .border_1()
                    .border_color(border)
                    .rounded_md()
                    .text_color(text)
                    .child(label)
                    .on_click(move |_, _, cx| {
                        target.update(cx, |view, cx| {
                            view.model.select(click_id.clone());
                            cx.notify();
                        });
                    }),
            );
        }

        let selected = self.model.selected.clone();
        let toolbar = div()
            .h(px(42.0))
            .flex()
            .items_center()
            .gap_1()
            .px_2()
            .bg(palette.panel())
            .border_b_1()
            .border_color(palette.border_subtle())
            .text_color(palette.text())
            .child("Graph")
            .child(button(
                "Global",
                "graph-global",
                entity.clone(),
                |view: &mut GraphView| view.model.set_mode(GraphMode::Global),
            ))
            .child(button(
                "Local",
                "graph-local",
                entity.clone(),
                |view: &mut GraphView| view.model.set_mode(GraphMode::Local),
            ))
            .child(button(
                "⌕ Search",
                "graph-search",
                entity.clone(),
                |view: &mut GraphView| view.model.set_search(""),
            ))
            .child(if self.model.filter.query.is_some() {
                "Filtered"
            } else {
                "All notes"
            })
            .child(button(
                "←",
                "graph-pan-left",
                entity.clone(),
                |view: &mut GraphView| view.model.pan_by(Point { x: 24.0, y: 0.0 }),
            ))
            .child(button(
                "→",
                "graph-pan-right",
                entity.clone(),
                |view: &mut GraphView| view.model.pan_by(Point { x: -24.0, y: 0.0 }),
            ))
            .child(button(
                "−",
                "graph-zoom-out",
                entity.clone(),
                |view: &mut GraphView| view.model.transform.zoom_by(0.9),
            ))
            .child(button(
                "+",
                "graph-zoom-in",
                entity.clone(),
                |view: &mut GraphView| view.model.transform.zoom_by(1.1),
            ))
            .child(button(
                "Open",
                "graph-open",
                entity.clone(),
                move |view: &mut GraphView| {
                    if let Some(id) = selected.clone() {
                        view.model.open(id);
                    }
                },
            ))
            .child(button(
                "⚙",
                "graph-settings",
                entity.clone(),
                |view: &mut GraphView| {
                    view.model
                        .set_show_unresolved(!view.model.settings.show_unresolved);
                },
            ))
            .child(button(
                "✦",
                "graph-sparkle",
                entity,
                |view: &mut GraphView| {
                    view.model
                        .set_sparkle_enabled(!view.model.settings.sparkle_enabled);
                },
            ));

        div()
            .id("graph-view")
            .size_full()
            .flex()
            .flex_col()
            .text_color(palette.text())
            .child(toolbar)
            .child(canvas)
    }
}
