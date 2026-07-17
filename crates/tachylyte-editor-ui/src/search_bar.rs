//! A renderer-independent find and replace bar.
//!
//! Offsets in this module are UTF-8 byte offsets into the supplied source.  The
//! model computes plans only; applying them remains the caller's responsibility.

use std::ops::Range;

/// One occurrence of the current query in the source.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResultSpan {
    pub span: Range<usize>,
}

/// A requested replacement, expressed as a source range and replacement text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReplacementPlan {
    pub span: Range<usize>,
    pub replacement: String,
}

/// State and operations for a find/replace bar.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchBar {
    query: String,
    replacement: String,
    source: String,
    case_sensitive: bool,
    results: Vec<ResultSpan>,
    active: Option<usize>,
}

impl Default for SearchBar {
    fn default() -> Self {
        Self {
            query: String::new(),
            replacement: String::new(),
            source: String::new(),
            // Match the editor's existing find behavior; users can opt into
            // case-insensitive matching from the bar.
            case_sensitive: true,
            results: Vec::new(),
            active: None,
        }
    }
}

impl SearchBar {
    pub fn new(source: impl Into<String>) -> Self {
        let mut bar = Self {
            source: source.into(),
            ..Self::default()
        };
        bar.recompute();
        bar
    }

    pub fn source(&self) -> &str {
        &self.source
    }
    pub fn query(&self) -> &str {
        &self.query
    }
    pub fn replacement(&self) -> &str {
        &self.replacement
    }
    pub fn case_sensitive(&self) -> bool {
        self.case_sensitive
    }
    pub fn results(&self) -> &[ResultSpan] {
        &self.results
    }
    pub fn active_result_index(&self) -> Option<usize> {
        self.active
    }
    pub fn active_result(&self) -> Option<ResultSpan> {
        self.active.map(|i| self.results[i].clone())
    }

    pub fn set_source(&mut self, source: impl Into<String>) {
        self.source = source.into();
        self.recompute();
    }
    pub fn set_query(&mut self, query: impl Into<String>) {
        self.query = query.into();
        self.recompute();
    }
    pub fn set_replacement(&mut self, replacement: impl Into<String>) {
        self.replacement = replacement.into();
    }
    pub fn set_case_sensitive(&mut self, value: bool) {
        if self.case_sensitive != value {
            self.case_sensitive = value;
            self.recompute();
        }
    }

    pub fn next_result(&mut self) -> Option<ResultSpan> {
        if self.results.is_empty() {
            self.active = None;
        } else {
            self.active = Some(self.active.map_or(0, |i| (i + 1) % self.results.len()));
        }
        self.active_result()
    }

    pub fn previous_result(&mut self) -> Option<ResultSpan> {
        if self.results.is_empty() {
            self.active = None;
        } else {
            self.active = Some(self.active.map_or(self.results.len() - 1, |i| {
                if i == 0 {
                    self.results.len() - 1
                } else {
                    i - 1
                }
            }));
        }
        self.active_result()
    }

    pub fn replace_current_plan(&self) -> Option<ReplacementPlan> {
        self.active_result().map(|r| ReplacementPlan {
            span: r.span,
            replacement: self.replacement.clone(),
        })
    }

    pub fn replace_all_plans(&self) -> Vec<ReplacementPlan> {
        self.results
            .iter()
            .map(|r| ReplacementPlan {
                span: r.span.clone(),
                replacement: self.replacement.clone(),
            })
            .collect()
    }

    fn recompute(&mut self) {
        self.results.clear();
        self.active = None;
        if self.query.is_empty() {
            return;
        }
        let query: Vec<char> = self.query.chars().collect();
        let chars: Vec<(usize, char)> = self.source.char_indices().collect();
        for start in 0..chars.len() {
            if start + query.len() > chars.len() {
                break;
            }
            let matches = query.iter().enumerate().all(|(i, wanted)| {
                let actual = chars[start + i].1;
                if self.case_sensitive {
                    actual == *wanted
                } else {
                    actual.to_lowercase().eq(wanted.to_lowercase())
                }
            });
            if matches {
                let begin = chars[start].0;
                let end = if start + query.len() < chars.len() {
                    chars[start + query.len()].0
                } else {
                    self.source.len()
                };
                self.results.push(ResultSpan { span: begin..end });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_byte_spans_and_wraps_navigation() {
        let mut bar = SearchBar::new("héllo hello");
        bar.set_query("hello");
        assert_eq!(bar.results(), &[ResultSpan { span: 7..12 }]);
        assert_eq!(bar.next_result().unwrap().span, 7..12);
        assert_eq!(bar.next_result().unwrap().span, 7..12);
        assert_eq!(bar.next_result().unwrap().span, 7..12);
    }

    #[test]
    fn plans_do_not_mutate_source_and_support_case() {
        let mut bar = SearchBar::new("One one ONE");
        bar.set_query("one");
        bar.set_replacement("two");
        assert_eq!(bar.results().len(), 1);
        bar.set_case_sensitive(false);
        assert_eq!(bar.results().len(), 3);
        bar.next_result();
        assert_eq!(bar.replace_current_plan().unwrap().replacement, "two");
        assert_eq!(bar.source(), "One one ONE");
        assert_eq!(bar.replace_all_plans().len(), 3);
    }
}
