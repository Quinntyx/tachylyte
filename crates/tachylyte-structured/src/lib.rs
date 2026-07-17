//! Interoperable, UI-independent models for Obsidian Canvas and Bases files.
//! Unknown properties are retained in `extra` maps, so newer producers can be
//! read and written without data loss.
#![allow(clippy::items_after_test_module)]
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::cmp::Ordering;
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("invalid document: {0}")]
    Invalid(String),
    #[error("codec: {0}")]
    Codec(String),
    #[error("formula: {0}")]
    Formula(String),
}
pub type Result<T> = std::result::Result<T, Error>;

fn finite(n: f64, name: &str) -> Result<f64> {
    if n.is_finite() {
        Ok(n)
    } else {
        Err(Error::Invalid(format!("{name} must be finite")))
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct CanvasDocument {
    #[serde(default)]
    pub nodes: Vec<Node>,
    #[serde(default)]
    pub edges: Vec<Edge>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}
impl CanvasDocument {
    pub fn from_json(s: &str) -> Result<Self> {
        serde_json::from_str::<Self>(s)
            .map_err(|e| Error::Codec(e.to_string()))
            .and_then(|x: Self| {
                x.validate()?;
                Ok(x)
            })
    }
    pub fn to_json(&self) -> Result<String> {
        self.validate()?;
        serde_json::to_string_pretty(self).map_err(|e| Error::Codec(e.to_string()))
    }
    pub fn validate(&self) -> Result<()> {
        for n in &self.nodes {
            n.validate()?;
        }
        for e in &self.edges {
            e.validate()?;
        }
        Ok(())
    }
    pub fn node(&self, id: &str) -> Option<&Node> {
        self.nodes.iter().find(|n| n.id == id)
    }
    pub fn hit_test(&self, p: Point) -> Option<&Node> {
        self.nodes.iter().rev().find(|n| n.rect().contains(p))
    }
    pub fn move_node(&mut self, id: &str, p: Point) -> Result<()> {
        let n = self
            .nodes
            .iter_mut()
            .find(|n| n.id == id)
            .ok_or_else(|| Error::Invalid("unknown node".into()))?;
        n.x = finite(p.x, "x")?;
        n.y = finite(p.y, "y")?;
        Ok(())
    }
    pub fn resize_node(&mut self, id: &str, size: Size) -> Result<()> {
        let n = self
            .nodes
            .iter_mut()
            .find(|n| n.id == id)
            .ok_or_else(|| Error::Invalid("unknown node".into()))?;
        n.width = finite(size.width, "width")?;
        n.height = finite(size.height, "height")?;
        if n.width < 0.0 || n.height < 0.0 {
            return Err(Error::Invalid("size must be non-negative".into()));
        }
        Ok(())
    }
    pub fn connect(&mut self, edge: Edge) -> Result<()> {
        edge.validate()?;
        if self.node(&edge.from_node).is_none() || self.node(&edge.to_node).is_none() {
            return Err(Error::Invalid("edge references missing node".into()));
        }
        if self.edges.iter().any(|e| e.id == edge.id) {
            return Err(Error::Invalid("duplicate edge id".into()));
        }
        self.edges.push(edge);
        Ok(())
    }
    pub fn disconnect(&mut self, id: &str) -> bool {
        let n = self.edges.len();
        self.edges.retain(|e| e.id != id);
        n != self.edges.len()
    }
    pub fn bring_to_front(&mut self, id: &str) -> bool {
        if let Some(i) = self.nodes.iter().position(|n| n.id == id) {
            let n = self.nodes.remove(i);
            self.nodes.push(n);
            true
        } else {
            false
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct Size {
    pub width: f64,
    pub height: f64,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}
impl Rect {
    pub fn contains(&self, p: Point) -> bool {
        p.x >= self.x && p.y >= self.y && p.x <= self.x + self.width && p.y <= self.y + self.height
    }
}
impl Default for Rect {
    fn default() -> Self {
        Self {
            x: 0.,
            y: 0.,
            width: 0.,
            height: 0.,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Node {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}
impl Node {
    pub fn rect(&self) -> Rect {
        Rect {
            x: self.x,
            y: self.y,
            width: self.width,
            height: self.height,
        }
    }
    pub fn validate(&self) -> Result<()> {
        for (n, v) in [
            ("x", self.x),
            ("y", self.y),
            ("width", self.width),
            ("height", self.height),
        ] {
            finite(v, n)?;
        }
        if self.width < 0. || self.height < 0. {
            return Err(Error::Invalid(
                "node dimensions must be non-negative".into(),
            ));
        }
        if self.id.is_empty() {
            return Err(Error::Invalid("node id is empty".into()));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Edge {
    pub id: String,
    #[serde(rename = "fromNode")]
    pub from_node: String,
    #[serde(rename = "fromSide", default)]
    pub from_side: Option<String>,
    #[serde(rename = "toNode")]
    pub to_node: String,
    #[serde(rename = "toSide", default)]
    pub to_side: Option<String>,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}
impl Edge {
    fn validate(&self) -> Result<()> {
        if self.id.is_empty() {
            return Err(Error::Invalid("edge id is empty".into()));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct ViewState {
    pub pan: Point,
    pub zoom: f64,
    #[serde(default)]
    pub selection: Vec<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}
impl ViewState {
    pub fn validate(&self) -> Result<()> {
        finite(self.pan.x, "pan x")?;
        finite(self.pan.y, "pan y")?;
        finite(self.zoom, "zoom")?;
        if self.zoom <= 0. {
            return Err(Error::Invalid("zoom must be positive".into()));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default)]
pub struct History {
    undo: Vec<CanvasDocument>,
    redo: Vec<CanvasDocument>,
}
impl History {
    pub fn execute<F>(&mut self, doc: &mut CanvasDocument, f: F) -> Result<()>
    where
        F: FnOnce(&mut CanvasDocument) -> Result<()>,
    {
        let old = doc.clone();
        f(doc)?;
        self.undo.push(old);
        self.redo.clear();
        Ok(())
    }
    pub fn undo(&mut self, doc: &mut CanvasDocument) -> bool {
        if let Some(x) = self.undo.pop() {
            self.redo.push(doc.clone());
            *doc = x;
            true
        } else {
            false
        }
    }
    pub fn redo(&mut self, doc: &mut CanvasDocument) -> bool {
        if let Some(x) = self.redo.pop() {
            self.undo.push(doc.clone());
            *doc = x;
            true
        } else {
            false
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct BaseDocument {
    #[serde(default)]
    pub properties: BTreeMap<String, Property>,
    #[serde(default)]
    pub views: Vec<BaseView>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_yaml::Value>,
}
impl BaseDocument {
    pub fn from_yaml(s: &str) -> Result<Self> {
        serde_yaml::from_str(s).map_err(|e| Error::Codec(e.to_string()))
    }
    pub fn to_yaml(&self) -> Result<String> {
        serde_yaml::to_string(self).map_err(|e| Error::Codec(e.to_string()))
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum Property {
    Text(String),
    Number(f64),
    Bool(bool),
    Formula { formula: String },
    Other(serde_yaml::Value),
}
impl Default for Property {
    fn default() -> Self {
        Property::Text(String::new())
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct BaseView {
    pub name: Option<String>,
    pub filter: Option<String>,
    #[serde(default)]
    pub sort: Vec<Sort>,
    pub group_by: Option<String>,
    pub layout: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_yaml::Value>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Sort {
    pub property: String,
    #[serde(default)]
    pub direction: Direction,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    #[default]
    Asc,
    Desc,
}

pub type Record = BTreeMap<String, Value>;
#[derive(Clone, Debug, PartialEq)]
pub enum Datum {
    Null,
    Bool(bool),
    Number(f64),
    Text(String),
}
impl Datum {
    fn truthy(&self) -> bool {
        match self {
            Datum::Bool(x) => *x,
            Datum::Number(x) => *x != 0.,
            Datum::Text(x) => !x.is_empty(),
            Datum::Null => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn node(id: &str, x: f64) -> Node {
        Node {
            id: id.into(),
            kind: "text".into(),
            x,
            y: 0.,
            width: 10.,
            height: 10.,
            text: Some(id.into()),
            file: None,
            url: None,
            color: None,
            extra: BTreeMap::new(),
        }
    }

    #[test]
    fn canvas_round_trip_keeps_extensions_and_geometry() {
        let input = include_str!("../fixtures/canvas.json");
        let mut c = CanvasDocument::from_json(input).unwrap();
        assert_eq!(
            c.nodes[0].extra["vendorExtension"]["kept"],
            Value::Bool(true)
        );
        assert_eq!(c.extra["vendorCanvasExtension"], Value::from("preserved"));
        assert_eq!(c.hit_test(Point { x: 10., y: 5. }).unwrap().id, "welcome");
        c.move_node("welcome", Point { x: 20., y: 4. }).unwrap();
        assert!(c.to_json().unwrap().contains("vendorExtension"));
    }

    #[test]
    fn z_order_and_undo_are_deterministic() {
        let mut c = CanvasDocument {
            nodes: vec![node("a", 0.), node("b", 0.)],
            ..Default::default()
        };
        assert_eq!(c.hit_test(Point { x: 1., y: 1. }).unwrap().id, "b");
        let mut h = History::default();
        h.execute(&mut c, |d| d.move_node("a", Point { x: 3., y: 3. }))
            .unwrap();
        assert_eq!(c.node("a").unwrap().x, 3.);
        assert!(h.undo(&mut c));
        assert_eq!(c.node("a").unwrap().x, 0.);
        assert!(h.redo(&mut c));
    }

    #[test]
    fn safe_formulas_filters_and_sorts() {
        let mut a = Record::new();
        a.insert("score".into(), Value::from(2));
        a.insert("name".into(), Value::from("z"));
        let mut b = Record::new();
        b.insert("score".into(), Value::from(5));
        b.insert("name".into(), Value::from("a"));
        assert_eq!(evaluate("score + 3 > 4", &a).unwrap(), Datum::Bool(true));
        assert_eq!(evaluate("name = \"z\"", &a).unwrap(), Datum::Bool(true));
        assert_eq!(
            filter_records(&[a.clone(), b.clone()], "score > 2")
                .unwrap()
                .len(),
            1
        );
        let mut rows = vec![a, b];
        sort_records(&mut rows, "name", Direction::Asc);
        assert_eq!(rows[0]["name"], Value::from("a"));
        assert!(evaluate("__import__(x)", &rows[0]).is_err());
    }

    #[test]
    fn bases_unknown_yaml_survives() {
        let b =
            BaseDocument::from_yaml("properties:\n  score: 2\nviews: []\nnew-key: yes\n").unwrap();
        assert!(b.extra.contains_key("new-key"));
        assert!(b.to_yaml().unwrap().contains("new-key"));
    }

    #[test]
    fn malformed_and_non_finite_are_rejected() {
        assert!(CanvasDocument::from_json("not json").is_err());
        let mut c = CanvasDocument {
            nodes: vec![node("a", 0.)],
            ..Default::default()
        };
        assert!(c.move_node("a", Point { x: f64::NAN, y: 0. }).is_err());
        assert!(c
            .connect(Edge {
                id: "e".into(),
                from_node: "missing".into(),
                to_node: "a".into(),
                ..Default::default()
            })
            .is_err());
    }
}
pub fn evaluate(formula: &str, record: &Record) -> Result<Datum> {
    let mut p = Parser::new(formula, record);
    let v = p.expr()?;
    if p.peek().is_some() {
        return Err(Error::Formula("unexpected trailing input".into()));
    }
    Ok(v)
}
pub fn filter_records(records: &[Record], expression: &str) -> Result<Vec<Record>> {
    records
        .iter()
        .map(|r| Ok((evaluate(expression, r)?.truthy(), r)))
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .filter_map(|(yes, r)| yes.then_some(r.clone()))
        .collect::<Vec<_>>()
        .pipe(Ok)
}
trait Pipe: Sized {
    fn pipe<T>(self, f: impl FnOnce(Self) -> T) -> T {
        f(self)
    }
}
impl<T> Pipe for T {}
pub fn sort_records(records: &mut [Record], property: &str, direction: Direction) {
    records.sort_by(|a, b| {
        let x = a.get(property).cloned().unwrap_or(Value::Null);
        let y = b.get(property).cloned().unwrap_or(Value::Null);
        let o = x.to_string().cmp(&y.to_string());
        if matches!(direction, Direction::Desc) {
            o.reverse()
        } else {
            o
        }
    })
}

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
    r: &'a Record,
}
impl<'a> Parser<'a> {
    fn new(s: &'a str, r: &'a Record) -> Self {
        Self {
            s: s.as_bytes(),
            i: 0,
            r,
        }
    }
    fn ws(&mut self) {
        while self.s.get(self.i).is_some_and(|c| c.is_ascii_whitespace()) {
            self.i += 1
        }
    }
    fn peek(&mut self) -> Option<u8> {
        self.ws();
        self.s.get(self.i).copied()
    }
    fn eat(&mut self, c: u8) -> bool {
        if self.peek() == Some(c) {
            self.i += 1;
            true
        } else {
            false
        }
    }
    fn expr(&mut self) -> Result<Datum> {
        let mut x = self.term()?;
        loop {
            let op = if self.eat(b'=') {
                if self.eat(b'=') {
                    Some("==")
                } else {
                    Some("=")
                }
            } else if self.eat(b'!') {
                if self.eat(b'=') {
                    Some("!=")
                } else {
                    None
                }
            } else if self.eat(b'>') {
                Some(">")
            } else if self.eat(b'<') {
                Some("<")
            } else {
                None
            };
            let Some(op) = op else { break };
            let y = self.term()?;
            let c = x.cmp(&y);
            x = Datum::Bool(match op {
                "==" | "=" => c == Ordering::Equal,
                "!=" => c != Ordering::Equal,
                ">" => c == Ordering::Greater,
                "<" => c == Ordering::Less,
                _ => false,
            })
        }
        Ok(x)
    }
    fn term(&mut self) -> Result<Datum> {
        let mut x = self.atom()?;
        loop {
            let op = if self.eat(b'+') {
                Some(1)
            } else if self.eat(b'-') {
                Some(-1)
            } else {
                None
            };
            let Some(op) = op else { break };
            let y = self.atom()?;
            match (x, y) {
                (Datum::Number(a), Datum::Number(b)) => {
                    x = Datum::Number(if op == 1 { a + b } else { a - b })
                }
                _ => return Err(Error::Formula("arithmetic requires numbers".into())),
            }
        }
        Ok(x)
    }
    fn atom(&mut self) -> Result<Datum> {
        if self.eat(b'(') {
            let x = self.expr()?;
            if !self.eat(b')') {
                return Err(Error::Formula("missing )".into()));
            }
            return Ok(x);
        }
        if self.eat(b'"') {
            let start = self.i;
            while self.s.get(self.i).is_some_and(|c| *c != b'"') {
                self.i += 1;
            }
            if !self.eat(b'"') {
                return Err(Error::Formula("unterminated string".into()));
            }
            return Ok(Datum::Text(
                std::str::from_utf8(&self.s[start..self.i - 1])
                    .unwrap_or_default()
                    .to_owned(),
            ));
        }
        let start = self.i;
        while self
            .s
            .get(self.i)
            .is_some_and(|c| c.is_ascii_digit() || *c == b'.')
        {
            self.i += 1
        }
        if self.i > start {
            return Ok(Datum::Number(
                std::str::from_utf8(&self.s[start..self.i])
                    .unwrap()
                    .parse()
                    .map_err(|_| Error::Formula("invalid number".into()))?,
            ));
        }
        let start = self.i;
        while self
            .s
            .get(self.i)
            .is_some_and(|c| c.is_ascii_alphanumeric() || *c == b'_' || *c == b'.')
        {
            self.i += 1
        }
        if self.i == start {
            return Err(Error::Formula("expected value".into()));
        }
        let k = std::str::from_utf8(&self.s[start..self.i]).unwrap();
        Ok(match k {
            "true" => Datum::Bool(true),
            "false" => Datum::Bool(false),
            "null" => Datum::Null,
            _ => match self.r.get(k) {
                Some(Value::Bool(v)) => Datum::Bool(*v),
                Some(Value::Number(v)) => Datum::Number(v.as_f64().unwrap_or(0.)),
                Some(Value::String(v)) => Datum::Text(v.clone()),
                _ => Datum::Null,
            },
        })
    }
}
impl Datum {
    fn cmp(&self, o: &Self) -> Ordering {
        match (self, o) {
            (Datum::Number(a), Datum::Number(b)) => a.partial_cmp(b).unwrap_or(Ordering::Equal),
            (Datum::Text(a), Datum::Text(b)) => a.cmp(b),
            (Datum::Bool(a), Datum::Bool(b)) => a.cmp(b),
            _ => Ordering::Equal,
        }
    }
}
