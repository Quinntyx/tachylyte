//! Pure pointer interaction state used by the Canvas view.
use std::collections::{BTreeMap, BTreeSet};
use tachylyte_structured::{Node, Point, Rect, Size};
use crate::canvas_geometry::contains_rect;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DragMoveState { pub origin: Point, pub current: Point }
impl DragMoveState {
    pub fn new(origin: Point) -> Self { Self { origin, current: origin } }
    pub fn update(&mut self, current: Point) { self.current = current; }
    pub fn delta(&self) -> Point { Point { x: self.current.x - self.origin.x, y: self.current.y - self.origin.y } }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResizeState { pub origin: Size, pub current: Size, pub minimum: Size }
impl ResizeState {
    pub fn new(origin: Size, minimum: Size) -> Self { Self { origin, current: origin, minimum } }
    pub fn update(&mut self, current: Size) {
        self.current = Size { width: current.width.max(self.minimum.width), height: current.height.max(self.minimum.height) };
    }
    pub fn size(&self) -> Size { self.current }
}

pub fn normalized_selection(a: Point, b: Point) -> Rect {
    Rect { x: a.x.min(b.x), y: a.y.min(b.y), width: (a.x - b.x).abs(), height: (a.y - b.y).abs() }
}

/// Return IDs whose rectangles are wholly contained by the selection rectangle.
pub fn nodes_in_selection<'a>(nodes: impl IntoIterator<Item = &'a Node>, selection: Rect) -> BTreeSet<String> {
    nodes.into_iter().filter(|n| contains_rect(selection, n.rect())).map(|n| n.id.clone()).collect()
}

/// Apply a click selection. Without additive mode the prior selection is replaced.
pub fn apply_selection(selected: &mut BTreeSet<String>, clicked: impl IntoIterator<Item = String>, additive: bool) {
    let clicked: BTreeSet<_> = clicked.into_iter().collect();
    if !additive { selected.clear(); }
    selected.extend(clicked);
}

/// Include a node's group and descendants, using conservative common metadata keys.
pub fn expand_group_selection(nodes: &[Node], selected: &BTreeSet<String>) -> BTreeSet<String> {
    let mut result = selected.clone();
    let mut children: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for n in nodes {
        for key in ["parent", "group"] {
            if let Some(parent) = n.extra.get(key).and_then(|v| v.as_str()) { children.entry(parent.into()).or_default().insert(n.id.clone()); }
        }
        if let Some(values) = n.extra.get("children").and_then(|v| v.as_array()) {
            for child in values.iter().filter_map(|v| v.as_str()) { children.entry(n.id.clone()).or_default().insert(child.into()); }
        }
    }
    let mut pending: Vec<_> = selected.iter().cloned().collect();
    while let Some(id) = pending.pop() {
        if let Some(kids) = children.get(&id) { for child in kids { if result.insert(child.clone()) { pending.push(child.clone()); } } }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn node(id: &str, x: f64, y: f64) -> Node { Node { id: id.into(), kind: "text".into(), x, y, width: 10., height: 10., text: None, file: None, url: None, color: None, extra: BTreeMap::new() } }
    #[test] fn geometry_and_resize() { assert_eq!(normalized_selection(Point{x:4.,y:8.}, Point{x:1.,y:2.}), Rect{x:1.,y:2.,width:3.,height:6.}); let mut r=ResizeState::new(Size{width:10.,height:8.},Size{width:20.,height:12.}); r.update(Size{width:1.,height:30.}); assert_eq!(r.size(), Size{width:20.,height:30.}); }
    #[test] fn additive_and_groups() { let mut s=BTreeSet::from(["a".into()]); apply_selection(&mut s, vec!["b".into()], true); assert_eq!(s.len(),2); let mut b=node("b",0.,0.); b.extra.insert("parent".into(),json!("a")); let mut c=node("c",0.,0.); c.extra.insert("group".into(),json!("b")); assert_eq!(expand_group_selection(&[node("a",0.,0.),b,c], &BTreeSet::from(["a".into()])).len(),3); }
}
