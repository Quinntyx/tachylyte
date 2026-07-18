//! GPUI projections and interaction models for Canvas and Bases documents.
//!
//! This crate deliberately does not serialize documents.  Views turn user input into
//! [`CanvasCommand`] and [`BaseCommand`] values which the host can apply to the domain
//! crate (and persist through its codecs).
//! Feature-app integration remains external; applications can gate these views
//! behind their own `--structured-ui` input flag.

mod bases;
mod bases_formula;
mod bases_intents;
mod bases_layout;
mod canvas;

pub use bases::{
    format_value, BaseCommand, BaseLayout, BaseModel, BaseProjection, BaseRow, BaseSummary,
    BasesEvent, BasesIntent, BasesView, ColumnVisibility, EditCellIntent, FormulaError,
    ToolbarAction,
};
pub use bases_formula::{display_property, evaluate_formula, FormulaDisplay, FormulaSummary};
pub use bases_intents::{BaseEvent, BaseIntent};
pub use bases_layout::{
    layout_label, BasesLayoutState, CardCover, ColumnWidth, LayoutKind, PropertyColumn,
};
pub use canvas::{
    CanvasCommand, CanvasMode, CanvasModel, CanvasTransform, CanvasView, ScreenPoint,
};

/// A single event stream for the two structured views in a workspace.
#[derive(Clone, Debug, PartialEq)]
pub enum StructuredCommand {
    Canvas(CanvasCommand),
    Bases(BaseCommand),
}

/// Compact composite surface for integrations that mount Canvas and Bases
/// side-by-side. The child views remain public so hosts can place them in any
/// GPUI layout without reconstructing their models or command plumbing.
pub struct StructuredViews {
    pub canvas: CanvasView,
    pub bases: BasesView,
}

impl StructuredViews {
    /// Construct both structured views from host-owned document snapshots.
    pub fn new(
        canvas: tachylyte_structured::CanvasDocument,
        base: tachylyte_structured::BaseDocument,
        records: Vec<tachylyte_structured::Record>,
    ) -> Self {
        Self {
            canvas: CanvasView::from_document(canvas),
            bases: BasesView::from_document(base, records),
        }
    }

    /// Replace snapshots without rebuilding the mounted child views.
    pub fn update(
        &mut self,
        canvas: tachylyte_structured::CanvasDocument,
        base: tachylyte_structured::BaseDocument,
        records: Vec<tachylyte_structured::Record>,
    ) {
        self.canvas.update_document(canvas);
        self.bases.update_document(base);
        self.bases.update_records(records);
    }

    /// Set both views read-only, useful while a host is loading or unavailable.
    pub fn set_disabled(&mut self, disabled: bool) {
        self.canvas.set_disabled(disabled);
        self.bases.set_disabled(disabled);
    }

    /// Drain child commands, preserving their source view. Canvas commands are
    /// returned before Bases commands because the child views have independent
    /// GPUI event queues.
    pub fn take_commands(&mut self) -> Vec<StructuredCommand> {
        let mut commands = self
            .canvas
            .take_commands()
            .into_iter()
            .map(StructuredCommand::Canvas)
            .collect::<Vec<_>>();
        commands.extend(
            self.bases
                .take_commands()
                .into_iter()
                .map(StructuredCommand::Bases),
        );
        commands
    }
}
