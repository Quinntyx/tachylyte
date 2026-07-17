//! GPUI projections and interaction models for Canvas and Bases documents.
//!
//! This crate deliberately does not serialize documents.  Views turn user input into
//! [`CanvasCommand`] and [`BaseCommand`] values which the host can apply to the domain
//! crate (and persist through its codecs).

mod bases;
mod canvas;

pub use bases::{BaseCommand, BaseLayout, BaseModel, BaseProjection, BaseRow, BasesView};
pub use canvas::{CanvasCommand, CanvasModel, CanvasTransform, CanvasView, ScreenPoint};
