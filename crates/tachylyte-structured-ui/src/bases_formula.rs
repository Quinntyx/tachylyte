//! UI-independent helpers for displaying Bases formula properties.
//!
//! This module intentionally returns data rather than GPUI elements.  Hosts can
//! therefore use the same evaluation, error, and count semantics in tables,
//! cards, and accessibility views.

use std::collections::BTreeMap;

use tachylyte_structured::{evaluate, Datum, Property, Record};

/// The result of evaluating a formula for display.
#[derive(Clone, Debug, PartialEq)]
pub enum FormulaDisplay {
    /// A successfully evaluated value, already formatted for presentation.
    Value(String),
    /// An evaluation failure, retaining the evaluator's complete error text.
    Error(FormulaError),
}

/// A formula error together with the source formula that produced it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FormulaError {
    pub formula: String,
    /// The raw error text from `tachylyte_structured::evaluate`.
    pub message: String,
}

impl std::fmt::Display for FormulaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

/// Counts useful when rendering a Bases formula summary.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FormulaSummary {
    pub total: usize,
    pub values: usize,
    pub errors: usize,
    pub nulls: usize,
}

/// Format evaluator values consistently, including all scalar JSON-like types
/// supported by the structured evaluator.
pub fn format_value(value: &Datum) -> String {
    match value {
        Datum::Null => "—".to_owned(),
        Datum::Bool(value) => value.to_string(),
        Datum::Number(value) => value.to_string(),
        Datum::Text(value) => value.clone(),
    }
}

/// Evaluate a formula and retain the evaluator's raw error text on failure.
pub fn evaluate_formula(formula: &str, record: &Record) -> FormulaDisplay {
    match evaluate(formula, record) {
        Ok(value) => FormulaDisplay::Value(format_value(&value)),
        Err(error) => FormulaDisplay::Error(FormulaError {
            formula: formula.to_owned(),
            message: error.to_string(),
        }),
    }
}

/// Display a property, evaluating it only when it is a formula.
pub fn display_property(property: &Property, record: &Record) -> FormulaDisplay {
    match property {
        Property::Formula { formula } => evaluate_formula(formula, record),
        Property::Text(value) => FormulaDisplay::Value(value.clone()),
        Property::Number(value) => FormulaDisplay::Value(value.to_string()),
        Property::Bool(value) => FormulaDisplay::Value(value.to_string()),
        Property::Other(value) => FormulaDisplay::Value(format!("{value:?}")),
    }
}

/// Summarize formula properties for one record in deterministic map order.
pub fn summarize_formulas(
    properties: &BTreeMap<String, Property>,
    record: &Record,
) -> FormulaSummary {
    let mut summary = FormulaSummary {
        total: properties
            .values()
            .filter(|property| matches!(property, Property::Formula { .. }))
            .count(),
        ..FormulaSummary::default()
    };
    for property in properties.values() {
        if let Property::Formula { .. } = property {
            match display_property(property, record) {
                FormulaDisplay::Value(value) => {
                    summary.values += 1;
                    if value == "—" {
                        summary.nulls += 1;
                    }
                }
                FormulaDisplay::Error(_) => summary.errors += 1,
            }
        }
    }
    summary
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_scalar_values() {
        assert_eq!(format_value(&Datum::Null), "—");
        assert_eq!(format_value(&Datum::Number(2.5)), "2.5");
        assert_eq!(format_value(&Datum::Bool(true)), "true");
        assert_eq!(format_value(&Datum::Text("hello".into())), "hello");
    }

    #[test]
    fn preserves_formula_errors_and_counts() {
        let mut properties = BTreeMap::new();
        properties.insert(
            "ok".into(),
            Property::Formula {
                formula: "1 + 2".into(),
            },
        );
        properties.insert(
            "empty".into(),
            Property::Formula {
                formula: "null".into(),
            },
        );
        properties.insert(
            "bad".into(),
            Property::Formula {
                formula: "1 +".into(),
            },
        );
        let record = Record::new();
        let summary = summarize_formulas(&properties, &record);
        assert_eq!(
            summary,
            FormulaSummary {
                total: 3,
                values: 2,
                errors: 1,
                nulls: 1
            }
        );
        match evaluate_formula("1 +", &record) {
            FormulaDisplay::Error(error) => assert!(error.message.contains("formula:")),
            other => panic!("expected error, got {other:?}"),
        }
    }
}
