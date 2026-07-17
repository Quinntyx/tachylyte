//! A mountable graph surface for [`tachylyte_knowledge`] data.
//!
//! [`GraphView`] is the GPUI-facing view. Construct it from a knowledge-graph
//! snapshot with [`GraphView::from_index`] (or use [`GraphView::new`] with a
//! prepared [`GraphViewModel`]), mount it in the host's layout, and call
//! [`GraphView::update`] when the index changes. The view keeps rendering state
//! separate from the knowledge index; interaction results are exposed as
//! [`GraphEvent`] values through [`GraphView::take_events`].
//!
//! ## Public lifecycle
//!
//! [`GraphViewModel::new`] creates render-neutral state from a
//! [`tachylyte_knowledge::VaultIndex`]. Use [`GraphView::new`] to wrap that
//! model, or [`GraphView::from_index`] as the one-step constructor. When a new
//! index snapshot is available, [`GraphView::update`] replaces the graph data
//! while preserving the model's viewport, filters, and selection. Code that
//! works with the model directly can perform the same refresh with
//! [`GraphViewModel::rebuild`].
//!
//! User interaction is communicated through [`GraphEvent::Select`] and
//! [`GraphEvent::Open`]. Call [`GraphView::take_events`] after updates or
//! rendering to drain events; each call returns only events not returned by a
//! previous call.
//!
//! [`ViewTransform`] converts between graph-world and screen coordinates. Its
//! zoom and pan fields are private, so invalid viewport state cannot be
//! introduced through a struct literal. Use [`ViewTransform::new`],
//! [`ViewTransform::set_zoom`], or [`ViewTransform::zoom_by`]; all of these
//! keep zoom finite and non-zero, while [`ViewTransform::screen`] and
//! [`ViewTransform::world`] sanitize non-finite input/output coordinates.
//!
//! The model supports [`GraphMode::Global`] and [`GraphMode::Local`] views.
//! Settings controls include unresolved-node visibility and the optional
//! sparkle treatment; callers can also adjust search and group filters on the
//! public model. Selection and open actions are reported as
//! [`GraphEvent::Select`] and [`GraphEvent::Open`] respectively.

mod model;
mod view;

pub use model::{
    EdgeSegment, GraphEvent, GraphMode, GraphSettings, GraphViewModel, NodeStyle, Point,
    PositionedNode, ViewTransform,
};
pub use view::*;
