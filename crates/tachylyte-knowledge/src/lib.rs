//! Render-neutral, deterministic knowledge and navigation primitives.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Document {
    pub path: String,
    pub content: String,
    /// Caller-supplied monotonic timestamp used by navigation sorting.
    pub modified: u64,
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
        if self.docs.get(&document.path) != Some(&document) {
            self.docs.insert(document.path.clone(), document);
            self.revision += 1;
        }
        self.revision
    }
    pub fn remove(&mut self, path: &str) -> u64 {
        if self.docs.remove(path).is_some() {
            self.revision += 1;
        }
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
    let tokens = tokenize(input)?;
    if tokens.is_empty() {
        return Ok(Query::Term(String::new()));
    }
    let mut groups: Vec<Vec<Query>> = vec![Vec::new()];
    let mut is_or = false;
    for (raw, quoted) in tokens {
        if !quoted && raw.eq_ignore_ascii_case("OR") {
            if groups.last().is_none_or(Vec::is_empty) {
                return Err(QueryError {
                    message: "empty OR group".into(),
                    position: input.len(),
                });
            }
            is_or = true;
            groups.push(Vec::new());
            continue;
        }
        let neg = raw.starts_with('-');
        let raw = if neg { &raw[1..] } else { &raw[..] };
        if raw.is_empty() {
            return Err(QueryError {
                message: "bare negation".into(),
                position: 0,
            });
        }
        let q = parse_atom(raw)?;
        groups
            .last_mut()
            .unwrap()
            .push(if neg { Query::Not(Box::new(q)) } else { q });
    }
    if is_or && groups.last().is_none_or(Vec::is_empty) {
        return Err(QueryError {
            message: "empty OR group".into(),
            position: input.len(),
        });
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

fn tokenize(input: &str) -> Result<Vec<(String, bool)>, QueryError> {
    let mut result = Vec::new();
    let mut token = String::new();
    let mut quoted = false;
    let mut had_quote = false;
    let mut escaped = false;
    for (position, ch) in input.char_indices() {
        if escaped {
            token.push(ch);
            escaped = false;
        } else if ch == '\\' && quoted {
            escaped = true;
        } else if ch == '"' {
            quoted = !quoted;
            had_quote = true;
        } else if ch.is_whitespace() && !quoted {
            if !token.is_empty() {
                result.push((std::mem::take(&mut token), had_quote));
                had_quote = false;
            }
        } else {
            token.push(ch);
        }
        if position + ch.len_utf8() == input.len() && quoted {
            return Err(QueryError {
                message: "unterminated quote".into(),
                position: input.len(),
            });
        }
    }
    if escaped {
        token.push('\\');
    }
    if !token.is_empty() {
        result.push((token, had_quote));
    }
    Ok(result)
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
    let terms = positive_terms(&q);
    let mut out = index
        .documents()
        .filter(|d| matches_query(&q, d))
        .map(|d| {
            let score = relevance(d, &terms);
            SearchResult {
                path: d.path.clone(),
                score,
                snippet: terms
                    .iter()
                    .find(|term| matches!(term.kind, TermKind::Plain | TermKind::Content))
                    .map_or_else(String::new, |term| snippet(&d.content, &term.value)),
            }
        })
        .collect::<Vec<_>>();
    out.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.path.cmp(&b.path)));
    Ok(out)
}
#[derive(Clone, Copy)]
enum TermKind {
    Plain,
    Path,
    File,
    Content,
    Tag,
    Property,
}
struct SearchTerm {
    kind: TermKind,
    key: Option<String>,
    value: String,
}
fn positive_terms(query: &Query) -> Vec<SearchTerm> {
    match query {
        Query::Not(_) => Vec::new(),
        Query::And(xs) | Query::Or(xs) => xs.iter().flat_map(positive_terms).collect(),
        Query::Term(x) => vec![SearchTerm {
            kind: TermKind::Plain,
            key: None,
            value: x.clone(),
        }],
        Query::Path(x) => vec![SearchTerm {
            kind: TermKind::Path,
            key: None,
            value: x.clone(),
        }],
        Query::File(x) => vec![SearchTerm {
            kind: TermKind::File,
            key: None,
            value: x.clone(),
        }],
        Query::Content(x) => vec![SearchTerm {
            kind: TermKind::Content,
            key: None,
            value: x.clone(),
        }],
        Query::Tag(x) => vec![SearchTerm {
            kind: TermKind::Tag,
            key: None,
            value: x.clone(),
        }],
        Query::Property(k, x) => vec![SearchTerm {
            kind: TermKind::Property,
            key: Some(k.clone()),
            value: x.clone(),
        }],
        Query::Task(_) => Vec::new(),
    }
}
fn relevance(d: &Document, terms: &[SearchTerm]) -> u32 {
    terms
        .iter()
        .map(|term| {
            let needle = term.value.to_lowercase();
            let occurrences =
                |value: &str| value.to_lowercase().matches(&needle).count() as u32 * 10;
            match term.kind {
                TermKind::Path => occurrences(&d.path) + 30,
                TermKind::File => occurrences(d.path.rsplit('/').next().unwrap_or(&d.path)) + 25,
                TermKind::Content | TermKind::Plain => occurrences(&d.content),
                TermKind::Tag => d.tags.iter().map(|tag| occurrences(tag)).sum(),
                TermKind::Property => d
                    .properties
                    .iter()
                    .map(|(key, value)| {
                        let key_score = term.key.as_ref().map_or(0, |k| {
                            if key.to_lowercase().contains(&k.to_lowercase()) {
                                30
                            } else {
                                0
                            }
                        });
                        key_score + occurrences(value)
                    })
                    .sum(),
            }
        })
        .sum()
}
pub fn snippet(text: &str, query: &str) -> String {
    let q = query.split_whitespace().next().unwrap_or("").to_lowercase();
    if q.is_empty() {
        return text.chars().take(160).collect();
    }
    let (low, starts, ends) = folded_with_boundaries(text);
    let folded_start = low.find(&q).unwrap_or(0);
    let folded_end = folded_start + q.len();
    let at = starts.get(folded_start).copied().unwrap_or(0);
    let end_match = folded_end
        .checked_sub(1)
        .and_then(|i| ends.get(i))
        .copied()
        .unwrap_or(text.len());
    let start = text[..at]
        .char_indices()
        .rev()
        .nth(40)
        .map(|(i, _)| i)
        .unwrap_or(0);
    let end = text[end_match..]
        .char_indices()
        .nth(120)
        .map(|(i, _)| end_match + i)
        .unwrap_or(text.len());
    format!(
        "{}{}{}",
        if start > 0 { "…" } else { "" },
        &text[start..end],
        if end < text.len() { "…" } else { "" }
    )
}

fn folded_with_boundaries(text: &str) -> (String, Vec<usize>, Vec<usize>) {
    let mut folded = String::new();
    let mut starts = Vec::new();
    let mut ends = Vec::new();
    for (start, ch) in text.char_indices() {
        let end = start + ch.len_utf8();
        for lowered in ch.to_lowercase() {
            let bytes = lowered.len_utf8();
            starts.extend(std::iter::repeat_n(start, bytes));
            ends.extend(std::iter::repeat_n(end, bytes));
            folded.push(lowered);
        }
    }
    (folded, starts, ends)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Link {
    pub source: String,
    pub target: String,
    pub alias: Option<String>,
    pub resolved: bool,
}
fn resolve_target(index: &VaultIndex, target: &str) -> Option<String> {
    index.get(target).map(|d| d.path.clone()).or_else(|| {
        index
            .documents()
            .find(|d| d.path.file_stem() == Some(target))
            .map(|d| d.path.clone())
    })
}
pub fn links(index: &VaultIndex, source: &str) -> Vec<Link> {
    let mut out = Vec::new();
    if let Some(d) = index.get(source) {
        let mut rest = d.content.as_str();
        while let Some(open) = rest.find("[[") {
            let after_open = &rest[open + 2..];
            let Some(close) = after_open.find("]]") else {
                break;
            };
            let raw = &after_open[..close];
            let mut fields = raw.splitn(2, '|');
            let target = fields.next().unwrap_or("").trim().to_string();
            let alias = fields.next().map(|x| x.trim().to_string());
            let resolved = resolve_target(index, &target).is_some();
            out.push(Link {
                source: source.into(),
                target,
                alias,
                resolved,
            });
            rest = &after_open[close + 2..];
        }
    }
    out.sort_by(|a, b| a.target.cmp(&b.target));
    out
}
pub fn backlinks(index: &VaultIndex, target: &str) -> Vec<Link> {
    let canonical = resolve_target(index, target).unwrap_or_else(|| target.to_string());
    index
        .documents()
        .flat_map(|d| links(index, &d.path))
        .filter(|l| resolve_target(index, &l.target).as_deref() == Some(canonical.as_str()))
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
    pub unresolved: bool,
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
            group: d.path.split('/').next().map(str::to_string),
            unresolved: false,
        })
        .collect::<Vec<_>>();
    ns.retain(|n| {
        filter.groups.is_empty() || n.group.as_ref().is_some_and(|g| filter.groups.contains(g))
    });
    if let Some(q) = &filter.query {
        ns.retain(|n| n.id.to_lowercase().contains(&q.to_lowercase()));
    }
    let sources = ns.iter().map(|n| n.id.clone()).collect::<BTreeSet<_>>();
    if filter.include_unresolved {
        for document in index.documents().filter(|d| sources.contains(&d.path)) {
            for link in links(index, &document.path)
                .into_iter()
                .filter(|l| !l.resolved)
            {
                let id = unresolved_id(&link.target);
                if !ns.iter().any(|n| n.id == id) {
                    ns.push(GraphNode {
                        id,
                        group: None,
                        unresolved: true,
                    });
                }
            }
        }
    }
    let node_ids = ns.iter().map(|n| n.id.as_str()).collect::<BTreeSet<_>>();
    let mut es = Vec::new();
    for d in index.documents() {
        for l in links(index, &d.path) {
            if !l.resolved && !filter.include_unresolved {
                continue;
            }
            if !node_ids.contains(d.path.as_str()) {
                continue;
            }
            let target = if l.resolved {
                ns.iter()
                    .find(|n| n.id == l.target || n.id.file_stem() == Some(l.target.as_str()))
                    .map(|n| n.id.clone())
            } else {
                Some(unresolved_id(&l.target))
            };
            if target.is_none() {
                continue;
            }
            es.push(GraphEdge {
                from: d.path.clone(),
                to: target.unwrap(),
                unresolved: !l.resolved,
            });
        }
    }
    ns.sort_by(|a, b| a.id.cmp(&b.id));
    es.sort_by(|a, b| a.from.cmp(&b.from).then(a.to.cmp(&b.to)));
    (ns, es)
}
fn unresolved_id(target: &str) -> String {
    format!("unresolved:{target}")
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
    let mut docs = index
        .documents()
        .filter(|d| d.path.starts_with(prefix))
        .collect::<Vec<_>>();
    match sort {
        Sort::Name => docs.sort_by(|a, b| a.path.cmp(&b.path)),
        Sort::Modified => {
            docs.sort_by(|a, b| b.modified.cmp(&a.modified).then(a.path.cmp(&b.path)))
        }
    };
    docs.into_iter().map(|d| d.path.clone()).collect()
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
    fn unicode_case_expansion_snippet_is_safe() {
        let result = snippet("İstanbul notes", "i");
        assert!(result.is_char_boundary(result.len()));
    }
    #[test]
    fn parsed_terms_drive_field_scoring() {
        let mut i = VaultIndex::new();
        i.upsert(d("folder/needle.md", "unrelated"));
        i.upsert(d("other.md", "needle needle"));
        let results = search(&i, "file:needle").unwrap();
        assert_eq!(results[0].path, "folder/needle.md");
    }
    #[test]
    fn tags_and_properties_are_scored_in_their_own_fields() {
        let mut tagged = d("tagged.md", "no match");
        tagged.tags = vec!["important".into()];
        let mut property = d("property.md", "no match");
        property
            .properties
            .insert("status".into(), "important".into());
        let mut i = VaultIndex::new();
        i.upsert(tagged);
        i.upsert(property);
        assert_eq!(search(&i, "tag:important").unwrap()[0].path, "tagged.md");
        assert_eq!(
            search(&i, "property:status=important").unwrap()[0].path,
            "property.md"
        );
    }
    #[test]
    fn malformed_or_and_quoted_values() {
        assert!(parse_query("one OR").is_err());
        assert!(parse_query("OR one").is_err());
        assert!(parse_query("-").is_err());
        assert!(
            matches!(parse_query("content:\"two words\"").unwrap(), Query::Content(v) if v == "two words")
        );
        assert!(
            matches!(parse_query(r#"content:"two \"words\"""#).unwrap(), Query::Content(v) if v == "two \"words\"")
        );
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
        i.upsert(d("a.md", "# One\n[[Target Note|An alias]],"));
        assert_eq!(outline(&i.get("a.md").unwrap().content)[0].text, "One");
        assert_eq!(links(&i, "a.md").len(), 1);
        assert_eq!(links(&i, "a.md")[0].target, "Target Note");
        assert_eq!(links(&i, "a.md")[0].alias.as_deref(), Some("An alias"));
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
        let (nodes, edges) = graph(&i, &f);
        assert!(nodes
            .iter()
            .any(|n| n.unresolved && n.id == "unresolved:missing"));
        assert!(edges.iter().all(|e| nodes.iter().any(|n| n.id == e.to)));
    }
    #[test]
    fn backlinks_resolve_nested_basename_targets() {
        let mut i = VaultIndex::new();
        i.upsert(d("notes/Target.md", ""));
        i.upsert(d("source.md", "[[Target]]"));
        assert_eq!(backlinks(&i, "notes/Target.md")[0].source, "source.md");
        assert_eq!(backlinks(&i, "Target")[0].source, "source.md");
    }
    #[test]
    fn graph_groups_and_edges_are_consistent() {
        let mut i = VaultIndex::new();
        i.upsert(d("one/a.md", "[[one/b.md]]"));
        i.upsert(d("two/b.md", ""));
        let mut groups = BTreeSet::new();
        groups.insert("one".into());
        let (nodes, edges) = graph(
            &i,
            &GraphFilter {
                groups,
                ..Default::default()
            },
        );
        assert!(nodes.iter().all(|n| n.group.as_deref() == Some("one")));
        assert!(edges
            .iter()
            .all(|e| nodes.iter().any(|n| n.id == e.from) && nodes.iter().any(|n| n.id == e.to)));
    }
    #[test]
    fn modified_sort_and_noop_revision() {
        let mut i = VaultIndex::new();
        let mut old = d("old.md", "");
        old.modified = 1;
        let mut new = d("new.md", "");
        new.modified = 2;
        i.upsert(old.clone());
        i.upsert(new);
        let revision = i.revision();
        i.upsert(old);
        assert_eq!(i.revision(), revision);
        assert_eq!(explore(&i, "", Sort::Modified), vec!["new.md", "old.md"]);
        assert_eq!(i.remove("missing.md"), revision);
    }
}
