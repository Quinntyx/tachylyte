use crate::bases_formula::{display_property, summarize_formulas, FormulaDisplay, FormulaSummary};
use crate::bases_intents::BaseEvent;
use crate::bases_layout::{BasesLayoutState, CardCover, ColumnWidth, LayoutKind, PropertyColumn};
use gpui::{div, prelude::*, px, rgb, Context, Render, SharedString, Window};
use serde_json::Value;
use std::cmp::Ordering;
use std::collections::BTreeMap;
use tachylyte_structured::{evaluate, BaseDocument, Direction, Property, Record};

pub use crate::bases_formula::{format_value, FormulaError};
pub use crate::bases_intents::{
    BaseEvent as BasesEvent, BaseIntent as BasesIntent, EditCellIntent,
};
pub use crate::bases_layout::{ColumnVisibility, ToolbarAction};

/// Projection style for a Bases view.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum BaseLayout {
    #[default]
    Table,
    Cards,
    List,
    Map,
}

impl From<BaseLayout> for LayoutKind {
    fn from(layout: BaseLayout) -> Self {
        match layout {
            BaseLayout::Table => Self::Table,
            BaseLayout::Cards => Self::Cards,
            BaseLayout::List => Self::List,
            BaseLayout::Map => Self::MapPlaceholder,
        }
    }
}

impl From<LayoutKind> for BaseLayout {
    fn from(layout: LayoutKind) -> Self {
        match layout {
            LayoutKind::Table => Self::Table,
            LayoutKind::Cards => Self::Cards,
            LayoutKind::List => Self::List,
            LayoutKind::MapPlaceholder => Self::Map,
        }
    }
}

/// Commands emitted by Bases controls; persistence remains the host's responsibility.
#[derive(Clone, Debug, PartialEq)]
pub enum BaseCommand {
    Select(Option<usize>),
    Edit {
        row: usize,
        property: String,
        value: Value,
    },
    OpenRow {
        row: usize,
    },
    CreateRow {
        values: Record,
    },
    DeleteRow {
        row: usize,
    },
    Sort {
        property: String,
        direction: Direction,
    },
    Filter(String),
    Group(Option<String>),
    Layout(BaseLayout),
    ResizeColumn {
        property: String,
        width: ColumnWidth,
    },
    ReorderColumns(Vec<String>),
    SetColumnVisibility {
        property: String,
        visible: bool,
    },
    SelectMapPlaceholder(Option<usize>),
}

/// A stable row projection, preserving source index for commands.
#[derive(Clone, Debug, PartialEq)]
pub struct BaseRow {
    pub source_index: usize,
    pub cells: Vec<(String, Value)>,
}

/// Deterministic projection shared by table, cards, list, and map renderers.
#[derive(Clone, Debug, PartialEq)]
pub struct BaseProjection {
    pub columns: Vec<String>,
    pub rows: Vec<BaseRow>,
}

/// Counts displayed by the compact Bases footer.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BaseSummary {
    pub total_records: usize,
    pub visible_records: usize,
    pub selected_records: usize,
    pub formula: FormulaSummary,
}

/// State and interaction model for a Bases view.
#[derive(Clone, Debug)]
pub struct BaseModel {
    pub document: BaseDocument,
    pub records: Vec<Record>,
    pub layout: BaseLayout,
    pub selected: Option<usize>,
    pub filter: String,
    pub sort: Option<(String, Direction)>,
    pub group: Option<String>,
    pub disabled: bool,
    pub columns: BasesLayoutState,
    commands: Vec<BaseCommand>,
    events: Vec<BaseEvent>,
}

impl BaseModel {
    /// Build a model from a domain document and records loaded by the host.
    pub fn new(document: BaseDocument, records: Vec<Record>) -> Self {
        let mut model = Self {
            document,
            records,
            layout: BaseLayout::default(),
            selected: None,
            filter: String::new(),
            sort: None,
            group: None,
            disabled: false,
            columns: BasesLayoutState::default(),
            commands: Vec::new(),
            events: Vec::new(),
        };
        model.sync_columns();
        model
    }

    /// Replace records supplied by the host while retaining view and column state.
    pub fn update_records(&mut self, records: Vec<Record>) {
        self.records = records;
        self.selected = None;
        self.sync_columns();
    }

    /// Replace the definition while retaining the current record snapshot.
    pub fn update_document(&mut self, document: BaseDocument) {
        self.document = document;
        self.selected = None;
        self.sync_columns();
    }

    /// Enable or disable user interaction for this projection.
    pub fn set_disabled(&mut self, disabled: bool) {
        self.disabled = disabled;
    }

    fn property_names(&self) -> Vec<String> {
        let mut names = self.document.properties.keys().cloned().collect::<Vec<_>>();
        for record in &self.records {
            for key in record.keys() {
                if !names.contains(key) {
                    names.push(key.clone());
                }
            }
        }
        names.sort();
        names
    }

    fn sync_columns(&mut self) {
        let names = self.property_names();
        let mut known = self
            .columns
            .columns
            .drain(..)
            .filter(|column| names.contains(&column.property))
            .collect::<Vec<_>>();
        for name in names {
            if !known.iter().any(|column| column.property == name) {
                let source_index = known.len();
                known.push(PropertyColumn::new(name, source_index));
            }
        }
        self.columns.columns = known;
    }

    /// Build a projection with stable columns and source indices.
    pub fn projection(&self) -> BaseProjection {
        let mut rows: Vec<(usize, &Record)> = self.records.iter().enumerate().collect();
        if !self.filter.trim().is_empty() {
            rows.retain(|(_, record)| {
                evaluate(&self.filter, record)
                    .map(|value| value_truthy(&value))
                    .unwrap_or(false)
            });
        }
        if let Some((property, direction)) = &self.sort {
            rows.sort_by(|(_, left), (_, right)| {
                let ordering = compare_values(left.get(property), right.get(property));
                if matches!(direction, Direction::Desc) {
                    ordering.reverse()
                } else {
                    ordering
                }
            });
        }
        let columns = self
            .columns
            .visible_columns()
            .map(|column| column.property.clone())
            .collect::<Vec<_>>();
        let rows = rows
            .into_iter()
            .map(|(source_index, record)| BaseRow {
                source_index,
                cells: columns
                    .iter()
                    .map(|key| (key.clone(), self.cell_value(record, key)))
                    .collect(),
            })
            .collect();
        BaseProjection { columns, rows }
    }

    fn cell_value(&self, record: &Record, property: &str) -> Value {
        if let Some(Property::Formula { formula }) = self.document.properties.get(property) {
            return match display_property(
                &Property::Formula {
                    formula: formula.clone(),
                },
                record,
            ) {
                FormulaDisplay::Value(value) => Value::String(value),
                FormulaDisplay::Error(error) => Value::String(format!("⚠ {}", error.message)),
            };
        }
        record.get(property).cloned().unwrap_or(Value::Null)
    }

    /// Display one property using the same formula/error semantics as rendering.
    pub fn formula_display(&self, source_index: usize, property: &str) -> Option<FormulaDisplay> {
        let record = self.records.get(source_index)?;
        let definition = self.document.properties.get(property)?;
        Some(display_property(definition, record))
    }

    /// Return counts for the current filtered projection.
    pub fn summary(&self) -> BaseSummary {
        let projection = self.projection();
        let formula = self
            .records
            .iter()
            .fold(FormulaSummary::default(), |mut summary, record| {
                let current = summarize_formulas(&self.document.properties, record);
                summary.total += current.total;
                summary.values += current.values;
                summary.errors += current.errors;
                summary.nulls += current.nulls;
                summary
            });
        BaseSummary {
            total_records: self.records.len(),
            visible_records: projection.rows.len(),
            selected_records: usize::from(self.selected.is_some()),
            formula,
        }
    }

    /// Count the current projection by the active grouping property.
    pub fn grouped_counts(&self) -> BTreeMap<String, usize> {
        let Some(property) = &self.group else {
            return BTreeMap::new();
        };
        let mut counts = BTreeMap::new();
        for row in self.projection().rows {
            let value = row
                .cells
                .iter()
                .find(|(name, _)| name == property)
                .map(|(_, value)| value_text(value))
                .unwrap_or_else(|| "—".into());
            *counts.entry(value).or_insert(0) += 1;
        }
        counts
    }

    /// Select a source row and emit a command.
    pub fn select(&mut self, row: Option<usize>) {
        if !self.disabled {
            self.selected = row;
            self.commands.push(BaseCommand::Select(row));
        }
    }

    /// Emit an editable-cell command addressed by source row identity.
    pub fn edit_cell(&mut self, row: usize, property: impl Into<String>, value: Value) {
        if !self.disabled {
            let property = property.into();
            let intent =
                crate::bases_intents::EditCellIntent::new(row, property.clone(), value.clone());
            self.commands.push(BaseCommand::Edit {
                row,
                property,
                value,
            });
            self.events.push(BaseEvent::CellEdited(intent));
        }
    }

    /// Emit a row-open intent without mutating the domain snapshot.
    pub fn open_row(&mut self, row: usize) {
        if !self.disabled {
            self.commands.push(BaseCommand::OpenRow { row });
            self.events.push(BaseEvent::RowOpened { source_index: row });
        }
    }

    /// Emit a row-create intent without mutating the domain snapshot.
    pub fn create_row(&mut self, values: Record) {
        if !self.disabled {
            self.commands.push(BaseCommand::CreateRow {
                values: values.clone(),
            });
            self.events.push(BaseEvent::RowCreated { values });
        }
    }

    /// Emit a row-delete intent addressed by source row identity.
    pub fn delete_row(&mut self, row: usize) {
        if !self.disabled {
            self.commands.push(BaseCommand::DeleteRow { row });
            self.events
                .push(BaseEvent::RowDeleted { source_index: row });
        }
    }

    /// Set sorting and emit a command.
    pub fn set_sort(&mut self, property: impl Into<String>, direction: Direction) {
        if !self.disabled {
            let property = property.into();
            self.sort = Some((property.clone(), direction.clone()));
            self.commands.push(BaseCommand::Sort {
                property: property.clone(),
                direction: direction.clone(),
            });
            self.events.push(BaseEvent::SortChanged {
                property,
                direction,
            });
        }
    }

    /// Set filtering and emit a command.
    pub fn set_filter(&mut self, expression: impl Into<String>) {
        if !self.disabled {
            let expression = expression.into();
            self.filter = expression.clone();
            self.commands.push(BaseCommand::Filter(expression.clone()));
            self.events.push(BaseEvent::FilterChanged { expression });
        }
    }

    /// Set grouping and emit a command.
    pub fn set_group(&mut self, property: Option<impl Into<String>>) {
        if !self.disabled {
            let property = property.map(Into::into);
            self.group = property.clone();
            self.commands.push(BaseCommand::Group(property.clone()));
            self.events.push(BaseEvent::GroupChanged { property });
        }
    }

    /// Change projection layout and emit a command.
    pub fn set_layout(&mut self, layout: BaseLayout) {
        if !self.disabled {
            self.layout = layout;
            self.columns.layout = layout.into();
            self.commands.push(BaseCommand::Layout(layout));
        }
    }

    /// Resize a visible or hidden property column while retaining its order.
    pub fn resize_column(&mut self, property: impl Into<String>, width: ColumnWidth) {
        if !self.disabled {
            let property = property.into();
            if let Some(column) = self
                .columns
                .columns
                .iter_mut()
                .find(|column| column.property == property)
            {
                column.width = width;
            }
            self.commands
                .push(BaseCommand::ResizeColumn { property, width });
        }
    }

    /// Reorder columns without changing their source identities.
    pub fn set_column_order(&mut self, order: Vec<String>) {
        if !self.disabled {
            let mut reordered = Vec::new();
            for property in &order {
                if let Some(column) = self
                    .columns
                    .columns
                    .iter()
                    .find(|column| &column.property == property)
                {
                    reordered.push(column.clone());
                }
            }
            for column in &self.columns.columns {
                if !order.contains(&column.property) {
                    reordered.push(column.clone());
                }
            }
            self.columns.columns = reordered;
            self.commands.push(BaseCommand::ReorderColumns(order));
        }
    }

    /// Toggle property visibility while keeping the property column state.
    pub fn set_column_visibility(&mut self, property: impl Into<String>, visible: bool) {
        if !self.disabled {
            let property = property.into();
            if let Some(column) = self
                .columns
                .columns
                .iter_mut()
                .find(|column| column.property == property)
            {
                column.visibility = if visible {
                    crate::bases_layout::ColumnVisibility::Visible
                } else {
                    crate::bases_layout::ColumnVisibility::Hidden
                };
            }
            self.commands
                .push(BaseCommand::SetColumnVisibility { property, visible });
        }
    }

    /// Set the card cover placeholder metadata.
    pub fn set_card_cover(&mut self, cover: CardCover) {
        if !self.disabled {
            self.columns.card_cover = cover;
        }
    }

    /// Emit a map-placeholder selection intent.
    pub fn select_map_placeholder(&mut self, source_index: Option<usize>) {
        if !self.disabled {
            self.commands
                .push(BaseCommand::SelectMapPlaceholder(source_index));
            self.events
                .push(BaseEvent::MapPlaceholderSelected { source_index });
        }
    }

    /// Drain emitted commands.
    pub fn take_commands(&mut self) -> Vec<BaseCommand> {
        std::mem::take(&mut self.commands)
    }

    /// Drain semantic events emitted alongside commands.
    pub fn take_events(&mut self) -> Vec<BaseEvent> {
        std::mem::take(&mut self.events)
    }
}

fn value_truthy(value: &tachylyte_structured::Datum) -> bool {
    match value {
        tachylyte_structured::Datum::Null => false,
        tachylyte_structured::Datum::Bool(value) => *value,
        tachylyte_structured::Datum::Number(value) => *value != 0.,
        tachylyte_structured::Datum::Text(value) => !value.is_empty(),
    }
}

fn compare_values(left: Option<&Value>, right: Option<&Value>) -> Ordering {
    fn rank(value: Option<&Value>) -> u8 {
        match value.unwrap_or(&Value::Null) {
            Value::Null => 0,
            Value::Bool(_) => 1,
            Value::Number(_) => 2,
            Value::String(_) => 3,
            _ => 4,
        }
    }
    let ordering = rank(left).cmp(&rank(right));
    if ordering != Ordering::Equal {
        return ordering;
    }
    match (left.unwrap_or(&Value::Null), right.unwrap_or(&Value::Null)) {
        (Value::Null, Value::Null) => Ordering::Equal,
        (Value::Bool(a), Value::Bool(b)) => a.cmp(b),
        (Value::Number(a), Value::Number(b)) => a
            .as_f64()
            .unwrap_or(0.)
            .partial_cmp(&b.as_f64().unwrap_or(0.))
            .unwrap_or(Ordering::Equal),
        (Value::String(a), Value::String(b)) => a.cmp(b),
        (a, b) => a.to_string().cmp(&b.to_string()),
    }
}

/// Native GPUI renderer for table, card, list, and map-placeholder projections.
pub struct BasesView {
    pub model: BaseModel,
}

impl BasesView {
    pub fn new(model: BaseModel) -> Self {
        Self { model }
    }
    pub fn from_document(document: BaseDocument, records: Vec<Record>) -> Self {
        Self::new(BaseModel::new(document, records))
    }
    pub fn update_records(&mut self, records: Vec<Record>) {
        self.model.update_records(records);
    }
    pub fn update_document(&mut self, document: BaseDocument) {
        self.model.update_document(document);
    }
    pub fn set_disabled(&mut self, disabled: bool) {
        self.model.set_disabled(disabled);
    }
    pub fn take_commands(&mut self) -> Vec<BaseCommand> {
        self.model.take_commands()
    }
    pub fn take_events(&mut self) -> Vec<BaseEvent> {
        self.model.take_events()
    }
}

impl Render for BasesView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();
        let projection = self.model.projection();
        let selected = self.model.selected;
        let disabled = self.model.disabled;
        let summary = self.model.summary();
        let layout = self.model.layout;
        let mut body = div().flex().flex_col().p_2().bg(rgb(0xffffffff));
        if layout == BaseLayout::Map {
            let map = entity.clone();
            body = body.child(
                div()
                    .id("bases-map-placeholder")
                    .p_4()
                    .border_1()
                    .border_color(rgb(0xe0e0e0ff))
                    .text_color(rgb(0x68645dff))
                    .child("Map view placeholder · select a row to locate it")
                    .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
                        map.update(cx, |view, cx| {
                            view.model.select_map_placeholder(view.model.selected);
                            cx.notify();
                        });
                    }),
            );
        } else if projection.columns.is_empty() {
            body = body.child(div().p_2().text_color(rgb(0x68645dff)).child(if disabled {
                "Unavailable while disabled"
            } else {
                "No properties yet"
            }));
        } else {
            if layout == BaseLayout::Table {
                let headings = projection
                    .columns
                    .iter()
                    .map(|column| format!("▦  {column}"))
                    .collect::<Vec<_>>()
                    .join("     ");
                body = body.child(
                    div()
                        .h(px(28.))
                        .flex()
                        .items_center()
                        .px_2()
                        .border_b_1()
                        .border_color(rgb(0xe0e0e0ff))
                        .child(headings),
                );
            }
            for row in &projection.rows {
                let active = selected == Some(row.source_index);
                let index = row.source_index;
                let text = row
                    .cells
                    .iter()
                    .map(|(key, value)| format!("{key}: {}", value_text(value)))
                    .collect::<Vec<_>>()
                    .join("   ");
                let e = entity.clone();
                let child = if layout == BaseLayout::Cards {
                    div()
                        .id(SharedString::from(format!("base-card-{index}")))
                        .w_full()
                        .p_2()
                        .m_1()
                        .border_1()
                        .border_color(rgb(0xe0e0e0ff))
                        .bg(rgb(if active { 0xf4f1ebff } else { 0xffffffff }))
                        .child(format!(
                            "▧ {}  {text}",
                            self.model.columns.card_cover.placeholder_label
                        ))
                } else {
                    div()
                        .id(SharedString::from(format!("base-row-{index}")))
                        .w_full()
                        .h(px(if layout == BaseLayout::List { 24. } else { 28. }))
                        .flex()
                        .items_center()
                        .px_2()
                        .border_b_1()
                        .border_color(rgb(0xe0e0e0ff))
                        .bg(rgb(if active { 0xf0f0f0ff } else { 0xffffffff }))
                        .text_color(rgb(0x222222ff))
                        .child(text)
                };
                body = body.child(
                    child.on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
                        e.update(cx, |view, cx| {
                            view.model.select(Some(index));
                            cx.notify();
                        });
                    }),
                );
            }
            if projection.rows.is_empty() {
                body = body.child(
                    div()
                        .p_2()
                        .text_color(rgb(0x68645dff))
                        .child("No records match this view"),
                );
            }
        }
        let table = entity.clone();
        let cards = entity.clone();
        let list = entity.clone();
        let map = entity.clone();
        let filter = entity.clone();
        let sort = entity.clone();
        let group = entity.clone();
        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(if disabled { 0xf1f1f1ff } else { 0xf6f6f6ff }))
            .text_color(rgb(0x222222ff))
            .child(
                div()
                    .h(px(36.))
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_2()
                    .bg(rgb(0xffffffff))
                    .border_b_1()
                    .border_color(rgb(0xe0e0e0ff))
                    .child("▦  Bases")
                    .child(toolbar_button(
                        "bases-table",
                        "▤ Table",
                        table,
                        BaseLayout::Table,
                    ))
                    .child(toolbar_button(
                        "bases-cards",
                        "▦ Cards",
                        cards,
                        BaseLayout::Cards,
                    ))
                    .child(toolbar_button(
                        "bases-list",
                        "☷ List",
                        list,
                        BaseLayout::List,
                    ))
                    .child(toolbar_button("bases-map", "⌖ Map", map, BaseLayout::Map))
                    .child(toolbar_action(
                        "bases-filter",
                        if self.model.filter.is_empty() {
                            "⌕ Filter"
                        } else {
                            "⌕ Filtered"
                        },
                        filter,
                        |model| model.set_filter(""),
                    ))
                    .child(toolbar_action("bases-sort", "↕ Sort", sort, |model| {
                        if let Some(property) = model.projection().columns.first().cloned() {
                            model.set_sort(property, Direction::Asc);
                        }
                    }))
                    .child(toolbar_action("bases-group", "⊞ Group", group, |model| {
                        model.set_group(model.projection().columns.first().cloned());
                    }))
                    .child(format!(
                        "  {} of {} records",
                        summary.visible_records, summary.total_records
                    )),
            )
            .child(body)
    }
}

fn toolbar_button(
    id: &'static str,
    label: &'static str,
    entity: gpui::Entity<BasesView>,
    layout: BaseLayout,
) -> impl IntoElement {
    div()
        .id(id)
        .h(px(28.))
        .px_2()
        .items_center()
        .hover(|style| style.bg(rgb(0xeeeeeeff)))
        .child(label)
        .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
            entity.update(cx, |view, cx| {
                view.model.set_layout(layout);
                cx.notify();
            });
        })
}

fn toolbar_action<F>(
    id: &'static str,
    label: &str,
    entity: gpui::Entity<BasesView>,
    action: F,
) -> impl IntoElement
where
    F: Fn(&mut BaseModel) + 'static,
{
    let label = label.to_owned();
    div()
        .id(id)
        .h(px(28.))
        .px_2()
        .items_center()
        .hover(|style| style.bg(rgb(0xeeeeeeff)))
        .child(label)
        .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
            entity.update(cx, |view, cx| {
                action(&mut view.model);
                cx.notify();
            });
        })
}

fn value_text(value: &Value) -> String {
    match value {
        Value::Null => "—".into(),
        Value::String(value) => value.clone(),
        _ => value.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn projection_and_commands_are_deterministic() {
        let mut a = Record::new();
        a.insert("name".into(), Value::String("A".into()));
        let mut b = Record::new();
        b.insert("name".into(), Value::String("B".into()));
        let mut model = BaseModel::new(BaseDocument::default(), vec![a, b]);
        assert_eq!(model.projection().columns, vec!["name"]);
        assert_eq!(model.projection().rows[0].source_index, 0);
        model.select(Some(1));
        model.edit_cell(1, "name", Value::String("C".into()));
        assert_eq!(model.take_commands().len(), 2);
        assert_eq!(model.take_events().len(), 1);
    }

    #[test]
    fn equal_records_keep_distinct_source_indices_through_filter_and_sort() {
        let mut first = Record::new();
        first.insert("name".into(), Value::String("same".into()));
        let second = first.clone();
        let mut model = BaseModel::new(BaseDocument::default(), vec![first, second]);
        model.set_filter("name = \"same\"");
        model.set_sort("name", Direction::Asc);
        assert_eq!(
            model
                .projection()
                .rows
                .iter()
                .map(|row| row.source_index)
                .collect::<Vec<_>>(),
            vec![0, 1]
        );
    }

    #[test]
    fn map_layout_and_column_state_are_commands_not_mutations() {
        let mut model = BaseModel::new(BaseDocument::default(), vec![]);
        model.set_layout(BaseLayout::Map);
        model.set_column_visibility("missing", false);
        assert_eq!(model.layout, BaseLayout::Map);
        assert!(matches!(
            model.take_commands()[0],
            BaseCommand::Layout(BaseLayout::Map)
        ));
    }
}
