//! A mountable graph surface for [`tachylyte_knowledge`] data.
//!
//! [`GraphView`] is the GPUI-facing view. Construct it from a knowledge-graph
//! snapshot with [`GraphView::from_index`] (or use [`GraphView::new`] with a
//! prepared [`GraphViewModel`]), mount it in the host's layout, and call
//! [`GraphView::update`] when the index changes. The view keeps rendering state
//! separate from the knowledge index; interaction results are exposed as
//! [`GraphEvent`] values through [`GraphView::take_events`].
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
