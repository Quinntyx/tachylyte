//! Adapter for mounting the native graph surface in the workspace shell.

use gpui::{AppContext, Context, Entity};
use tachylyte_graph_ui::{GraphEvent, GraphView};
use tachylyte_knowledge::VaultIndex;

/// Owns a mounted [`GraphView`] and provides the shell lifecycle operations.
///
/// The shell can retain this value alongside its other entities, call
/// [`GraphMount::sync`] after replacing its vault snapshot, and drain user
/// interaction with [`GraphMount::drain_events`].
#[derive(Clone, Debug)]
pub struct GraphMount {
    view: Entity<GraphView>,
}

impl GraphMount {
    /// Construct and mount a graph entity from the current index snapshot.
    pub fn mount<T>(index: &VaultIndex, cx: &mut Context<T>) -> Self {
        Self {
            view: cx.new(|_| GraphView::from_index(index)),
        }
    }

    /// Return the entity so a shell can place it in its layout.
    pub fn entity(&self) -> Entity<GraphView> {
        self.view.clone()
    }

    /// Refresh graph data while retaining the graph view's UI state.
    pub fn sync<T>(&self, index: &VaultIndex, cx: &mut Context<T>) {
        self.view.update(cx, |view, cx| {
            view.update(index);
            cx.notify();
        });
    }

    /// Drain selection and open events emitted by the graph since the last call.
    pub fn drain_events<T>(&self, cx: &mut Context<T>) -> Vec<GraphEvent> {
        self.view.update(cx, |view, _| view.take_events())
    }
}
