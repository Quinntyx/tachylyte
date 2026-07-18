//! Pure, serialisable interactions emitted by a Bases projection.
//!
//! These values deliberately contain source indices rather than positions in a
//! filtered or sorted projection.  A host can therefore apply them to its
//! document without the view mutating that document or retaining a row handle.

use serde_json::Value;
use std::collections::BTreeMap;
use tachylyte_structured::Direction;

/// An interaction requested by a Bases control.
#[derive(Clone, Debug, PartialEq)]
pub enum BaseIntent {
    EditCell(EditCellIntent),
    OpenRow {
        source_index: usize,
    },
    CreateRow {
        values: BTreeMap<String, Value>,
    },
    DeleteRow {
        source_index: usize,
    },
    SetFilter {
        expression: String,
    },
    SetSort {
        property: String,
        direction: Direction,
    },
    SetGroup {
        property: Option<String>,
    },
    SelectMapPlaceholder {
        source_index: Option<usize>,
    },
}

/// An edit addressed to the stable source row identity.
#[derive(Clone, Debug, PartialEq)]
pub struct EditCellIntent {
    pub source_index: usize,
    pub property: String,
    pub value: Value,
}

/// A host-facing event produced after an intent is accepted or observed.
#[derive(Clone, Debug, PartialEq)]
pub enum BaseEvent {
    CellEdited(EditCellIntent),
    RowOpened {
        source_index: usize,
    },
    RowCreated {
        values: BTreeMap<String, Value>,
    },
    RowDeleted {
        source_index: usize,
    },
    FilterChanged {
        expression: String,
    },
    SortChanged {
        property: String,
        direction: Direction,
    },
    GroupChanged {
        property: Option<String>,
    },
    MapPlaceholderSelected {
        source_index: Option<usize>,
    },
}

impl EditCellIntent {
    pub fn new(source_index: usize, property: impl Into<String>, value: Value) -> Self {
        Self {
            source_index,
            property: property.into(),
            value,
        }
    }
}

impl BaseIntent {
    pub fn edit_cell(source_index: usize, property: impl Into<String>, value: Value) -> Self {
        Self::EditCell(EditCellIntent::new(source_index, property, value))
    }

    pub fn open_row(source_index: usize) -> Self {
        Self::OpenRow { source_index }
    }

    pub fn create_row(values: BTreeMap<String, Value>) -> Self {
        Self::CreateRow { values }
    }

    pub fn delete_row(source_index: usize) -> Self {
        Self::DeleteRow { source_index }
    }

    pub fn filter(expression: impl Into<String>) -> Self {
        Self::SetFilter {
            expression: expression.into(),
        }
    }

    pub fn sort(property: impl Into<String>, direction: Direction) -> Self {
        Self::SetSort {
            property: property.into(),
            direction,
        }
    }

    pub fn group(property: Option<impl Into<String>>) -> Self {
        Self::SetGroup {
            property: property.map(Into::into),
        }
    }

    pub fn select_map_placeholder(source_index: Option<usize>) -> Self {
        Self::SelectMapPlaceholder { source_index }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn edit_keeps_source_identity_and_json_value() {
        let intent = BaseIntent::edit_cell(7, "status", json!({"value": "done"}));
        assert_eq!(
            intent,
            BaseIntent::EditCell(EditCellIntent::new(7, "status", json!({"value": "done"}),))
        );
    }

    #[test]
    fn create_values_are_deterministically_ordered() {
        let mut values = BTreeMap::new();
        values.insert("z".into(), json!(1));
        values.insert("a".into(), json!(2));
        let BaseIntent::CreateRow { values } = BaseIntent::create_row(values) else {
            panic!("wrong intent")
        };
        assert_eq!(values.keys().collect::<Vec<_>>(), vec!["a", "z"]);
    }
}
