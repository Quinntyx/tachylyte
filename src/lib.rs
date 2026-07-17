//! Render-neutral, deterministic knowledge and navigation primitives.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Document {
    pub path: String,
    pub content: String,
    pub tags: Vec<String>,
    pub properties: BTreeMap<String, String>,
    pub tasks: Vec<Task>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Task {
    pub text: String,
    pub done: bool,
}

#[derive(Clone, Debug, Default)]
pub struct VaultIndex {
    docs: BTreeMap<String, Document>,
    revision: u64,
}
impl VaultIndex {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn upsert(&mut self, document: Document) -> u64 {
        self.docs.insert(document.path.clone(), document);
        self.revision += 1;
        self.revision
    }
    pub fn remove(&mut self, path: &str) -> u64 {
        self.docs.remove(path);
        self.revision += 1;
        self.revision
    }
    pub fn get(&self, path: &str) -> Option<&Document> {
        self.docs.get(path)
    }
    pub fn documents(&self) -> impl Iterator<Item = &Document> {
        self.docs.values()
    }
    pub fn snapshot(&self) -> (u64, Vec<Document>) {
        (self.revision, self.docs.values().cloned().collect())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Query {
    Term(String),
    Path(String),
    File(String),
    Content(String),
    Tag(String),
    Property(String, String),
    Task(Option<bool>),
    Not(Box<Query>),
    And(Vec<Query>),
    Or(Vec<Query>),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueryError {
    pub message: String,
    pub position: usize,
}

pub fn parse_query(input: &str) -> Result<Query, QueryError> {
    let mut tokens = Vec::new();
    let mut start = None;
    let mut quote = false;
    for (i, c) in input.char_indices() {
        if c == '"' {
            quote = !quote;
            if start.is_none() {
                start = Some(i + 1);
            }
        } else if c.is_whitespace() && !quote {
            if let Some(s) = start.take() {
                tokens.push(input[s..i].to_string());
            }
        } else if start.is_none() {
            start = Some(i);
        }
    }
    if quote {
        return Err(QueryError {
            message: "unterminated quote".into(),
            position: input.len(),
        });
    }
    if let Some(s) = start {
        tokens.push(input[s..].trim_end_matches('"').to_string());
    }
    if tokens.is_empty() {
        return Ok(Query::Term(String::new()));
    }
    let mut groups: Vec<Vec<Query>> = vec![Vec::new()];
    let mut is_or = false;
    for raw in tokens {
        if raw.eq_ignore_ascii_case("OR") {
            is_or = true;
            groups.push(Vec::new());
            continue;
        }
        let neg = raw.starts_with('-');
        let raw = if neg { &raw[1..] } else { &raw[..] };
        let q = parse_atom(raw)?;
        groups
            .last_mut()
            .unwrap()
            .push(if neg { Query::Not(Box::new(q)) } else { q });
    }
    let parts = groups
        .into_iter()
        .map(|g| {
            if g.len() == 1 {
                g.into_iter().next().unwrap()
            } else {
                Query::And(g)
            }
        })
        .collect::<Vec<_>>();
    if is_or {
        Ok(Query::Or(parts))
    } else if parts.len() == 1 {
        Ok(parts.into_iter().next().unwrap())
    } else {
        Ok(Query::And(parts))
    }
}
fn parse_atom(raw: &str) -> Result<Query, QueryError> {
    let (key, value) = raw.split_once(':').unwrap_or(("", raw));
    Ok(match key.to_ascii_lowercase().as_str() {
        "path" => Query::Path(value.into()),
        "file" => Query::File(value.into()),
        "content" => Query::Content(value.into()),
        "tag" => Query::Tag(value.trim_start_matches('#').into()),
        "task" => Query::Task(match value {
            "done" => Some(true),
            "todo" | "open" => Some(false),
            _ => None,
        }),
        "property" => {
            let (k, v) = value.split_once('=').unwrap_or((value, ""));
            Query::Property(k.into(), v.into())
        }
        _ => Query::Term(raw.into()),
    })
}
pub fn matches_query(q: &Query, d: &Document) -> bool {
    let contains = |a: &str, b: &str| a.to_lowercase().contains(&b.to_lowercase());
    match q {
        Query::Term(s) => {
            contains(&d.path, s) || contains(&d.content, s) || d.tags.iter().any(|x| contains(x, s))
        }
        Query::Path(s) => contains(&d.path, s),
        Query::File(s) => contains(d.path.rsplit('/').next().unwrap_or(&d.path), s),
        Query::Content(s) => contains(&d.content, s),
        Query::Tag(s) => d
            .tags
            .iter()
            .any(|x| contains(x.trim_start_matches('#'), s)),
        Query::Property(k, v) => d.properties.get(k).is_some_and(|x| contains(x, v)),
        Query::Task(done) => d.tasks.iter().any(|t| done.is_none_or(|x| t.done == x)),
        Query::Not(x) => !matches_query(x, d),
        Query::And(xs) => xs.iter().all(|x| matches_query(x, d)),
        Query::Or(xs) => xs.iter().any(|x| matches_query(x, d)),
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchResult {
    pub path: String,
    pub score: u32,
    pub snippet: String,
}
pub fn search(index: &VaultIndex, query: &str) -> Result<Vec<SearchResult>, QueryError> {
    let q = parse_query(query)?;
    let needle = query.to_lowercase();
    let mut out = index
        .documents()
        .filter(|d| matches_query(&q, d))
        .map(|d| {
            let score = relevance(d, &needle);
            SearchResult {
                path: d.path.clone(),
                score,
                snippet: snippet(&d.content, &needle),
            }
        })
        .collect::<Vec<_>>();
    out.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.path.cmp(&b.path)));
    Ok(out)
}
fn relevance(d: &Document, q: &str) -> u32 {
    let mut n = 0;
    if d.path.to_lowercase().contains(q) {
        n += 30;
    }
    n + d.content.to_lowercase().matches(q).count() as u32 * 10
}
pub fn snippet(text: &str, query: &str) -> String {
    let q = query.split_whitespace().next().unwrap_or("").to_lowercase();
    if q.is_empty() {
        return text.chars().take(160).collect();
    }
    let low = text.to_lowercase();
    let at = low.find(&q).unwrap_or(0);
    let start = text[..at]
        .char_indices()
        .rev()
        .nth(40)
        .map(|(i, _)| i)
        .unwrap_or(0);
    let end = text[at..]
        .char_indices()
        .nth(120)
        .map(|(i, _)| at + i)
        .unwrap_or(text.len());
    format!(
        "{}{}{}",
        if start > 0 { "…" } else { "" },
        &text[start..end],
        if end < text.len() { "…" } else { "" }
    )
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Link {
    pub source: String,
    pub target: String,
    pub alias: Option<String>,
    pub resolved: bool,
}
pub fn links(index: &VaultIndex, source: &str) -> Vec<Link> {
    let mut out = Vec::new();
    if let Some(d) = index.get(source) {
        for word in d.content.split_whitespace().filter(|x| x.starts_with("[[")) {
            let target = word
                .trim_matches(|c: char| "[ ]],.!?".contains(c))
                .split('|')
                .next()
                .unwrap_or("")
                .to_string();
            let resolved = index.get(&target).is_some()
                || index
                    .documents()
                    .any(|x| x.path.file_stem().is_some_and(|s| s == target));
            out.push(Link {
                source: source.into(),
                target,
                alias: word
                    .split('|')
                    .nth(1)
                    .map(|x| x.trim_end_matches("]]").into()),
                resolved,
            });
        }
    }
    out.sort_by(|a, b| a.target.cmp(&b.target));
    out
}
pub fn backlinks(index: &VaultIndex, target: &str) -> Vec<Link> {
    index
        .documents()
        .flat_map(|d| links(index, &d.path))
        .filter(|l| l.target == target || l.target == target.trim_end_matches(".md"))
        .collect()
}

/// Finds plain-text mentions that are not explicit wiki links.
pub fn unlinked_mentions(index: &VaultIndex, target: &str) -> Vec<(String, usize)> {
    let needle = target.trim_end_matches(".md").to_lowercase();
    let mut result = Vec::new();
    for document in index.documents() {
        for (line, text) in document.content.lines().enumerate() {
            if text.to_lowercase().contains(&needle)
                && !text.split("[[").skip(1).any(|part| part.contains(&needle))
            {
                result.push((document.path.clone(), line));
            }
        }
    }
    result.sort();
    result
}
trait PathStem {
    fn file_stem(&self) -> Option<&str>;
}
impl PathStem for str {
    fn file_stem(&self) -> Option<&str> {
        self.rsplit('/')
            .next()
            .and_then(|x| x.strip_suffix(".md"))
            .or_else(|| self.rsplit('/').next())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Heading {
    pub level: u8,
    pub text: String,
    pub line: usize,
}
pub fn outline(content: &str) -> Vec<Heading> {
    content
        .lines()
        .enumerate()
        .filter_map(|(line, s)| {
            let n = s.chars().take_while(|c| *c == '#').count();
            (n > 0 && s.chars().nth(n) == Some(' ')).then(|| Heading {
                level: n as u8,
                text: s[n + 1..].trim().into(),
                line,
            })
        })
        .collect()
}
pub fn tag_counts(index: &VaultIndex) -> BTreeMap<String, usize> {
    let mut m = BTreeMap::new();
    for d in index.documents() {
        for t in &d.tags {
            *m.entry(t.trim_start_matches('#').into()).or_default() += 1;
        }
    }
    m
}
pub fn property_counts(index: &VaultIndex) -> BTreeMap<String, usize> {
    let mut m = BTreeMap::new();
    for d in index.documents() {
        for k in d.properties.keys() {
            *m.entry(k.clone()).or_default() += 1;
        }
    }
    m
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum BookmarkTarget {
    File(String),
    Heading { path: String, heading: String },
    Block { path: String, id: String },
    Search(String),
    Url(String),
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Bookmark {
    pub id: String,
    pub title: String,
    pub target: BookmarkTarget,
    pub children: Vec<Bookmark>,
}
pub fn bookmark_roundtrip(b: &Bookmark) -> Result<Bookmark, serde_json::Error> {
    serde_json::from_str(&serde_json::to_string(b)?)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraphNode {
    pub id: String,
    pub group: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraphEdge {
    pub from: String,
    pub to: String,
    pub unresolved: bool,
}
#[derive(Clone, Debug, Default)]
pub struct GraphFilter {
    pub groups: BTreeSet<String>,
    pub include_unresolved: bool,
    pub query: Option<String>,
}
pub fn graph(index: &VaultIndex, filter: &GraphFilter) -> (Vec<GraphNode>, Vec<GraphEdge>) {
    let mut ns = index
        .documents()
        .map(|d| GraphNode {
            id: d.path.clone(),
            group: None,
        })
        .collect::<Vec<_>>();
    let mut es = Vec::new();
    for d in index.documents() {
        for l in links(index, &d.path) {
            if !l.resolved && !filter.include_unresolved {
                continue;
            }
            es.push(GraphEdge {
                from: d.path.clone(),
                to: l.target,
                unresolved: !l.resolved,
            });
        }
    }
    if let Some(q) = &filter.query {
        ns.retain(|n| n.id.to_lowercase().contains(&q.to_lowercase()));
    }
    ns.sort_by(|a, b| a.id.cmp(&b.id));
    es.sort_by(|a, b| a.from.cmp(&b.from).then(a.to.cmp(&b.to)));
    (ns, es)
}
pub fn random_note(index: &VaultIndex, seed: u64) -> Option<&Document> {
    let mut xs = index.documents().collect::<Vec<_>>();
    xs.sort_by(|a, b| a.path.cmp(&b.path));
    xs.get(if xs.is_empty() {
        0
    } else {
        (seed as usize) % xs.len()
    })
    .copied()
}
pub fn preview(index: &VaultIndex, path: &str) -> Option<String> {
    index
        .get(path)
        .map(|d| d.content.chars().take(300).collect())
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Command {
    pub id: String,
    pub title: String,
    pub usage: u32,
}
pub fn rank_commands(commands: &[Command], input: &str) -> Vec<Command> {
    let q = input.to_lowercase();
    let mut x = commands
        .iter()
        .filter(|c| c.title.to_lowercase().contains(&q) || c.id.to_lowercase().contains(&q))
        .cloned()
        .collect::<Vec<_>>();
    x.sort_by(|a, b| {
        let sa = if a.title.to_lowercase().starts_with(&q) {
            2
        } else {
            1
        };
        let sb = if b.title.to_lowercase().starts_with(&q) {
            2
        } else {
            1
        };
        sb.cmp(&sa)
            .then(b.usage.cmp(&a.usage))
            .then(a.id.cmp(&b.id))
    });
    x
}

/// Case-insensitive subsequence score used by quick switchers. Higher is better.
pub fn fuzzy_score(candidate: &str, input: &str) -> Option<u32> {
    if input.is_empty() {
        return Some(0);
    }
    let candidate = candidate.to_lowercase();
    let input = input.to_lowercase();
    let mut cursor = 0;
    let mut score = 0;
    let mut previous = None;
    for wanted in input.chars() {
        let offset = candidate[cursor..].find(wanted)?;
        let position = cursor + offset;
        score += 100u32.saturating_sub(position as u32);
        if previous == Some(position.saturating_sub(1)) {
            score += 25;
        }
        previous = Some(position);
        cursor = position + wanted.len_utf8();
    }
    Some(score)
}

pub fn quick_switch(paths: &[String], input: &str) -> Vec<(String, u32)> {
    let mut result = paths
        .iter()
        .filter_map(|path| fuzzy_score(path, input).map(|score| (path.clone(), score)))
        .collect::<Vec<_>>();
    result.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    result
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Sort {
    Name,
    Modified,
}
pub fn explore(index: &VaultIndex, prefix: &str, sort: Sort) -> Vec<String> {
    let mut x = index
        .documents()
        .filter(|d| d.path.starts_with(prefix))
        .map(|d| d.path.clone())
        .collect::<Vec<_>>();
    match sort {
        Sort::Name => x.sort(),
        Sort::Modified => x.sort(),
    };
    x
}

#[cfg(test)]
mod tests {
    use super::*;
    fn d(path: &str, content: &str) -> Document {
        Document {
            path: path.into(),
            content: content.into(),
            ..Default::default()
        }
    }
    #[test]
    fn query() {
        let x = d("a.md", "hello");
        assert!(matches_query(&parse_query("content:hello").unwrap(), &x));
        assert!(!matches_query(&parse_query("-content:hello").unwrap(), &x));
    }
    #[test]
    fn unicode_snippet() {
        assert!(snippet("αβγ hello", "hello").contains("hello"));
    }
    #[test]
    fn stale_revision() {
        let mut i = VaultIndex::new();
        let r = i.revision();
        i.upsert(d("a.md", "x"));
        assert!(i.revision() > r);
    }
    #[test]
    fn links_and_outline() {
        let mut i = VaultIndex::new();
        i.upsert(d("a.md", "# One\n[[b.md]]"));
        assert_eq!(outline(&i.get("a.md").unwrap().content)[0].text, "One");
        assert_eq!(links(&i, "a.md").len(), 1);
    }
    #[test]
    fn bookmark() {
        let b = Bookmark {
            id: "x".into(),
            title: "X".into(),
            target: BookmarkTarget::File("a".into()),
            children: vec![],
        };
        assert_eq!(bookmark_roundtrip(&b).unwrap(), b);
    }
    #[test]
    fn graph_unresolved() {
        let mut i = VaultIndex::new();
        i.upsert(d("a.md", "[[missing]]"));
        assert_eq!(graph(&i, &GraphFilter::default()).1.len(), 0);
        let f = GraphFilter {
            include_unresolved: true,
            ..Default::default()
        };
        assert_eq!(graph(&i, &f).1.len(), 1);
    }
}
