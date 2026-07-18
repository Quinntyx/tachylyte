//! Data-only layout state used by Bases projections.
//!
//! This module deliberately contains no rendering or persistence concerns.  In
//! particular, `source_index` is retained on entries which can be reordered so
//! callers can address the original record rather than a rendered position.

use std::fmt;

/// The projections supported by a Bases view.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum LayoutKind {
    #[default]
    Table,
    Cards,
    List,
    MapPlaceholder,
}

/// Visibility of a property column.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ColumnVisibility {
    #[default]
    Visible,
    Hidden,
}

/// A property column and its deterministic display configuration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PropertyColumn {
    pub property: String,
    pub source_index: usize,
    pub visibility: ColumnVisibility,
    pub width: ColumnWidth,
}

impl PropertyColumn {
    pub fn new(property: impl Into<String>, source_index: usize) -> Self {
        Self {
            property: property.into(),
            source_index,
            visibility: ColumnVisibility::Visible,
            width: ColumnWidth::default(),
        }
    }

    pub fn hidden(mut self) -> Self {
        self.visibility = ColumnVisibility::Hidden;
        self
    }

    pub fn with_width(mut self, width: ColumnWidth) -> Self {
        self.width = width;
        self
    }

    pub fn is_visible(&self) -> bool {
        self.visibility == ColumnVisibility::Visible
    }
}

/// Width policy for a property column.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ColumnWidth {
    #[default]
    Auto,
    Fixed(u16),
}

/// Placeholder metadata for a card cover.  It describes intent, not an image.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CardCover {
    pub property: Option<String>,
    pub source_index: Option<usize>,
    pub aspect_ratio: Option<(u16, u16)>,
    pub placeholder_label: String,
}

impl CardCover {
    pub fn placeholder(label: impl Into<String>) -> Self {
        Self {
            placeholder_label: label.into(),
            ..Self::default()
        }
    }
}

/// Data-only configuration shared by all Bases layout projections.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BasesLayoutState {
    pub layout: LayoutKind,
    pub columns: Vec<PropertyColumn>,
    pub card_cover: CardCover,
}

impl BasesLayoutState {
    pub fn visible_columns(&self) -> impl Iterator<Item = &PropertyColumn> {
        self.columns.iter().filter(|column| column.is_visible())
    }
}

/// Compact labels for a Bases toolbar.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ToolbarAction {
    Layout,
    Properties,
    Filter,
    Sort,
    More,
}

impl ToolbarAction {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Layout => "Layout",
            Self::Properties => "Properties",
            Self::Filter => "Filter",
            Self::Sort => "Sort",
            Self::More => "More",
        }
    }
}

/// Return the compact label for a layout selector.
pub const fn layout_label(layout: LayoutKind) -> &'static str {
    match layout {
        LayoutKind::Table => "Table",
        LayoutKind::Cards => "Cards",
        LayoutKind::List => "List",
        LayoutKind::MapPlaceholder => "Map",
    }
}

impl fmt::Display for LayoutKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(layout_label(*self))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_deterministic() {
        let state = BasesLayoutState::default();
        assert_eq!(state.layout, LayoutKind::Table);
        assert!(state.columns.is_empty());
        assert_eq!(state.card_cover, CardCover::default());
    }

    #[test]
    fn columns_preserve_source_identity_and_visibility() {
        let state = BasesLayoutState {
            columns: vec![
                PropertyColumn::new("title", 7),
                PropertyColumn::new("tag", 2).hidden(),
            ],
            ..Default::default()
        };
        let visible: Vec<_> = state.visible_columns().collect();
        assert_eq!(visible[0].source_index, 7);
        assert_eq!(visible.len(), 1);
    }

    #[test]
    fn toolbar_labels_are_compact() {
        assert_eq!(ToolbarAction::Properties.label(), "Properties");
        assert_eq!(layout_label(LayoutKind::MapPlaceholder), "Map");
    }
}
