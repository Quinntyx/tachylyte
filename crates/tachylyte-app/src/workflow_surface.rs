//! App-local mounting adapters for the workflow command and path surfaces.
//!
//! These types deliberately contain no application-controller policy.  They own
//! only the GPUI entity, while the host decides where to mount it and how to
//! interpret the [`WorkflowIntent`] values emitted by the UI crate.

use gpui::{AppContext, Context, Entity};
use tachylyte_workflow_ui::{
    CommandOption, CommandPalette, PathOption, QuickSwitcher, WorkflowIntent,
};

/// Owns a mounted [`CommandPalette`] and exposes its shell-facing operations.
#[derive(Clone, Debug)]
pub struct CommandPaletteSurface {
    entity: Entity<CommandPalette>,
}

impl CommandPaletteSurface {
    /// Mount a command palette from its display titles and command identifiers.
    pub fn new<T>(commands: Vec<CommandOption>, cx: &mut Context<T>) -> Self {
        Self {
            entity: cx.new(|_| CommandPalette::new(commands)),
        }
    }

    /// Construct a palette from `(command identifier, display title)` pairs.
    pub fn from_commands<I, C, D, T>(commands: I, cx: &mut Context<T>) -> Self
    where
        I: IntoIterator<Item = (C, D)>,
        C: Into<String>,
        D: Into<String>,
    {
        Self::new(
            commands
                .into_iter()
                .map(|(id, title)| CommandOption {
                    id: id.into(),
                    title: title.into(),
                })
                .collect(),
            cx,
        )
    }

    /// Return the entity for insertion into a GPUI layout.
    pub fn entity(&self) -> Entity<CommandPalette> {
        self.entity.clone()
    }

    /// Set the palette query and notify GPUI so the mounted view is refreshed.
    pub fn sync_query<T>(&self, query: impl Into<String>, cx: &mut Context<T>) {
        self.entity.update(cx, |palette, cx| {
            palette.set_query(query);
            cx.notify();
        });
    }

    /// Drain typed command intents emitted since the previous call.
    pub fn drain_intents<T>(&self, cx: &mut Context<T>) -> Vec<WorkflowIntent> {
        self.entity.update(cx, |palette, _| palette.take_intents())
    }
}

/// Owns a mounted [`QuickSwitcher`] and exposes its shell-facing operations.
#[derive(Clone, Debug)]
pub struct QuickSwitcherSurface {
    entity: Entity<QuickSwitcher>,
}

impl QuickSwitcherSurface {
    /// Mount a quick switcher from its path records.
    pub fn new<T>(paths: Vec<PathOption>, cx: &mut Context<T>) -> Self {
        Self {
            entity: cx.new(|_| QuickSwitcher::new(paths)),
        }
    }

    /// Construct a switcher from `(path, display label)` pairs.
    pub fn from_paths<I, P, L, T>(paths: I, cx: &mut Context<T>) -> Self
    where
        I: IntoIterator<Item = (P, L)>,
        P: Into<String>,
        L: Into<String>,
    {
        Self::new(
            paths
                .into_iter()
                .map(|(path, label)| PathOption {
                    path: path.into(),
                    label: label.into(),
                })
                .collect(),
            cx,
        )
    }

    /// Return the entity for insertion into a GPUI layout.
    pub fn entity(&self) -> Entity<QuickSwitcher> {
        self.entity.clone()
    }

    /// Set the switcher query and notify GPUI so the mounted view is refreshed.
    pub fn sync_query<T>(&self, query: impl Into<String>, cx: &mut Context<T>) {
        self.entity.update(cx, |switcher, cx| {
            switcher.set_query(query);
            cx.notify();
        });
    }

    /// Drain typed path-opening intents emitted since the previous call.
    pub fn drain_intents<T>(&self, cx: &mut Context<T>) -> Vec<WorkflowIntent> {
        self.entity
            .update(cx, |switcher, _| switcher.take_intents())
    }
}
