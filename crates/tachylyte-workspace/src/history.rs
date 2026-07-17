//! Render-neutral navigation and closed-tab history.

use serde::{Deserialize, Serialize};

const DEFAULT_HISTORY_CAPACITY: usize = 50;

/// A bounded browser-like back/forward history.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NavigationHistory<T> {
    back: Vec<T>,
    current: Option<T>,
    forward: Vec<T>,
    #[serde(default = "default_capacity")]
    capacity: usize,
}

fn default_capacity() -> usize {
    DEFAULT_HISTORY_CAPACITY
}

impl<T> NavigationHistory<T> {
    pub fn new(capacity: usize) -> Self {
        Self {
            back: Vec::new(),
            current: None,
            forward: Vec::new(),
            capacity,
        }
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn current(&self) -> Option<&T> {
        self.current.as_ref()
    }

    pub fn can_back(&self) -> bool {
        !self.back.is_empty()
    }

    pub fn can_forward(&self) -> bool {
        !self.forward.is_empty()
    }

    pub fn clear_forward(&mut self) {
        self.forward.clear();
    }

    fn trim(stack: &mut Vec<T>, capacity: usize) {
        if stack.len() > capacity {
            let excess = stack.len() - capacity;
            stack.drain(..excess);
        }
    }
}

impl<T: Clone + PartialEq> NavigationHistory<T> {
    /// Visit a location, discarding forward history.
    pub fn visit(&mut self, location: T) {
        if self.current.as_ref() == Some(&location) {
            return;
        }
        if let Some(current) = self.current.replace(location) {
            self.back.push(current);
            Self::trim(&mut self.back, self.capacity);
        }
        self.clear_forward();
    }

    pub fn back(&mut self) -> Option<T> {
        let previous = self.back.pop()?;
        if let Some(current) = self.current.replace(previous.clone()) {
            self.forward.push(current);
            Self::trim(&mut self.forward, self.capacity);
        }
        Some(previous)
    }

    pub fn forward(&mut self) -> Option<T> {
        let next = self.forward.pop()?;
        if let Some(current) = self.current.replace(next.clone()) {
            self.back.push(current);
            Self::trim(&mut self.back, self.capacity);
        }
        Some(next)
    }
}

impl<T> Default for NavigationHistory<T> {
    fn default() -> Self {
        Self::new(DEFAULT_HISTORY_CAPACITY)
    }
}

/// A bounded newest-first stack of closed tabs (or other restorable state).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClosedTabStack<T> {
    items: Vec<T>,
    #[serde(default = "default_capacity")]
    capacity: usize,
}

impl<T> ClosedTabStack<T> {
    pub fn new(capacity: usize) -> Self {
        Self {
            items: Vec::new(),
            capacity,
        }
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

impl<T> Default for ClosedTabStack<T> {
    fn default() -> Self {
        Self::new(DEFAULT_HISTORY_CAPACITY)
    }
}

impl<T> ClosedTabStack<T> {
    pub fn push_closed(&mut self, tab: T) {
        if self.capacity == 0 {
            return;
        }
        self.items.push(tab);
        if self.items.len() > self.capacity {
            self.items.remove(0);
        }
    }

    pub fn reopen(&mut self) -> Option<T> {
        self.items.pop()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn navigation_discards_forward_on_new_visit_and_is_bounded() {
        let mut history = NavigationHistory::new(2);
        history.visit("a");
        history.visit("b");
        history.visit("c");
        assert_eq!(history.back(), Some("b"));
        assert_eq!(history.back(), Some("a"));
        assert_eq!(history.back(), None);
        assert_eq!(history.forward(), Some("b"));
        history.visit("new");
        assert_eq!(history.forward(), None);
        assert_eq!(history.current(), Some(&"new"));
    }

    #[test]
    fn closed_tabs_reopen_newest_and_drop_oldest() {
        let mut closed = ClosedTabStack::new(2);
        closed.push_closed(1);
        closed.push_closed(2);
        closed.push_closed(3);
        assert_eq!(closed.reopen(), Some(3));
        assert_eq!(closed.reopen(), Some(2));
        assert_eq!(closed.reopen(), None);
    }

    #[test]
    fn histories_round_trip_through_json() {
        let mut history = NavigationHistory::new(3);
        history.visit(serde_json::json!({"page": "home"}));
        history.visit(serde_json::json!({"page": "settings"}));
        let encoded = serde_json::to_string(&history).unwrap();
        let decoded: NavigationHistory<serde_json::Value> = serde_json::from_str(&encoded).unwrap();
        assert_eq!(history, decoded);
    }
}
