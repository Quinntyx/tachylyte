//! Data-only hierarchical vault explorer state.
use std::collections::BTreeSet;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileKind {
    Markdown,
    Canvas,
    Image,
    Pdf,
    Audio,
    Video,
    Other(String),
}
impl FileKind {
    pub fn glyph(&self) -> &'static str {
        match self {
            Self::Markdown => "M",
            Self::Canvas => "C",
            Self::Image => "I",
            Self::Pdf => "P",
            Self::Audio => "A",
            Self::Video => "V",
            Self::Other(_) => "•",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExplorerNodeKind {
    Folder,
    File(FileKind),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExplorerNode {
    pub path: String,
    pub name: String,
    pub kind: ExplorerNodeKind,
    pub modified: Option<i64>,
    pub created: Option<i64>,
    pub children: Vec<ExplorerNode>,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SortMode {
    #[default]
    Name,
    Modified,
    Created,
}
impl SortMode {
    #[allow(non_upper_case_globals)]
    pub const Alphabetical: Self = Self::Name;
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VisibleExplorerRow {
    pub path: String,
    pub name: String,
    pub kind: ExplorerNodeKind,
    pub depth: usize,
    pub expanded: bool,
    pub selected: bool,
    pub glyph: &'static str,
}
impl VisibleExplorerRow {
    pub fn path(&self) -> &str {
        &self.path
    }
    pub fn is_folder(&self) -> bool {
        matches!(self.kind, ExplorerNodeKind::Folder)
    }
}
pub type ExplorerRow = VisibleExplorerRow;
pub type FileExplorerTreeModel = ExplorerModel;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExplorerFileInput {
    pub path: String,
    pub modified: Option<i64>,
    pub created: Option<i64>,
}
impl From<String> for ExplorerFileInput {
    fn from(path: String) -> Self {
        Self {
            path,
            ..Self::default()
        }
    }
}
impl From<&str> for ExplorerFileInput {
    fn from(path: &str) -> Self {
        path.to_owned().into()
    }
}
impl ExplorerFileInput {
    pub fn new(path: impl Into<String>) -> Self {
        path.into().into()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExplorerModel {
    pub roots: Vec<ExplorerNode>,
    pub expanded: BTreeSet<String>,
    pub selected: Option<String>,
    pub active: Option<String>,
    pub sort_mode: SortMode,
    pub filter: String,
}
impl ExplorerModel {
    pub fn new(roots: Vec<ExplorerNode>) -> Self {
        let mut m = Self {
            roots,
            ..Self::default()
        };
        m.sort();
        m
    }
    pub fn from_vault_paths<I, P>(paths: I) -> Self
    where
        I: IntoIterator<Item = P>,
        P: Into<ExplorerFileInput>,
    {
        let mut m = Self::default();
        m.update_paths(paths);
        m
    }
    pub fn from_paths<I, P>(paths: I) -> Self
    where
        I: IntoIterator<Item = P>,
        P: Into<ExplorerFileInput>,
    {
        Self::from_vault_paths(paths)
    }
    pub fn update_paths<I, P>(&mut self, paths: I)
    where
        I: IntoIterator<Item = P>,
        P: Into<ExplorerFileInput>,
    {
        let old_expanded = self.expanded.clone();
        let old_selected = self.selected.clone();
        let old_active = self.active.clone();
        self.roots.clear();
        for input in paths.into_iter().map(Into::into) {
            let parts: Vec<_> = input
                .path
                .trim_matches('/')
                .split('/')
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .collect();
            if parts.is_empty() {
                continue;
            }
            self.insert(&parts, input.modified, input.created);
        }
        self.sort();
        self.expanded = if old_expanded.is_empty() {
            self.folder_paths()
        } else {
            old_expanded
                .into_iter()
                .filter(|p| self.node(p).is_some())
                .collect()
        };
        self.selected = old_selected.filter(|p| self.node(p).is_some());
        self.active = old_active.filter(|p| self.node(p).is_some());
    }
    fn insert(&mut self, parts: &[String], modified: Option<i64>, created: Option<i64>) {
        let mut list = &mut self.roots;
        let mut path = String::new();
        for (i, name) in parts.iter().enumerate() {
            if !path.is_empty() {
                path.push('/')
            }
            path.push_str(name);
            let is_file = i + 1 == parts.len();
            let pos = list.iter().position(|n| n.path == path);
            if let Some(p) = pos {
                if is_file {
                    list[p].modified = modified;
                    list[p].created = created;
                }
                list = &mut list[p].children;
            } else {
                list.push(ExplorerNode {
                    path: path.clone(),
                    name: name.clone(),
                    kind: if is_file {
                        ExplorerNodeKind::File(kind_for(name))
                    } else {
                        ExplorerNodeKind::Folder
                    },
                    modified: if is_file { modified } else { None },
                    created: if is_file { created } else { None },
                    children: Vec::new(),
                });
                let p = list.len() - 1;
                list = &mut list[p].children;
            }
        }
    }
    pub fn set_filter(&mut self, filter: impl Into<String>) {
        self.filter = filter.into();
    }
    pub fn search(&mut self, filter: impl Into<String>) {
        self.set_filter(filter);
    }
    pub fn set_sort_mode(&mut self, mode: SortMode) {
        self.sort_mode = mode;
        self.sort();
    }
    pub fn toggle(&mut self, path: impl Into<String>) {
        let p = path.into();
        if self
            .node(&p)
            .is_some_and(|n| matches!(n.kind, ExplorerNodeKind::Folder))
            && !self.expanded.insert(p.clone())
        {
            self.expanded.remove(&p);
        }
    }
    pub fn select(&mut self, path: impl Into<String>) {
        let p = path.into();
        if self.node(&p).is_some() {
            self.selected = Some(p);
        }
    }
    pub fn activate(&mut self) {
        self.active = self.selected.clone();
    }
    pub fn selected_node(&self) -> Option<&ExplorerNode> {
        self.selected.as_deref().and_then(|p| self.node(p))
    }
    pub fn active(&self) -> Option<&str> {
        self.active.as_deref()
    }
    pub fn selected_path(&self) -> Option<&str> {
        self.selected.as_deref()
    }
    pub fn active_path(&self) -> Option<&str> {
        self.active.as_deref()
    }
    pub fn visible(&self) -> Vec<VisibleExplorerRow> {
        self.visible_rows()
    }
    pub fn move_selection(&mut self, delta: isize) {
        let rows = self.visible_rows();
        if rows.is_empty() {
            return;
        }
        let old = self
            .selected
            .as_ref()
            .and_then(|p| rows.iter().position(|r| &r.path == p))
            .unwrap_or(0);
        let next = (old as isize + delta).clamp(0, rows.len() as isize - 1) as usize;
        self.select(rows[next].path.clone());
    }
    pub fn handle_key(&mut self, key: &str) {
        self.reduce_keyboard(&key.to_ascii_lowercase());
    }
    pub fn activate_selected(&mut self) {
        self.activate();
    }
    pub fn visible_rows(&self) -> Vec<VisibleExplorerRow> {
        let mut out = Vec::new();
        for n in &self.roots {
            self.walk(n, 0, &mut out);
        }
        out
    }
    fn walk(&self, n: &ExplorerNode, d: usize, out: &mut Vec<VisibleExplorerRow>) {
        let matching = self.filter.is_empty()
            || n.name.to_lowercase().contains(&self.filter.to_lowercase())
            || n.path.to_lowercase().contains(&self.filter.to_lowercase());
        let child_match = n.children.iter().any(|c| self.matches_tree(c));
        if matching || child_match {
            let ex = self.expanded.contains(&n.path);
            out.push(VisibleExplorerRow {
                path: n.path.clone(),
                name: n.name.clone(),
                kind: n.kind.clone(),
                depth: d,
                expanded: ex,
                selected: self.selected.as_deref() == Some(&n.path),
                glyph: match &n.kind {
                    ExplorerNodeKind::Folder => "D",
                    ExplorerNodeKind::File(k) => k.glyph(),
                },
            });
            if ex || !self.filter.is_empty() {
                for c in &n.children {
                    self.walk(c, d + 1, out);
                }
            }
        }
    }
    fn matches_tree(&self, n: &ExplorerNode) -> bool {
        self.filter.is_empty()
            || n.name.to_lowercase().contains(&self.filter.to_lowercase())
            || n.path.to_lowercase().contains(&self.filter.to_lowercase())
            || n.children.iter().any(|c| self.matches_tree(c))
    }
    fn folder_paths(&self) -> BTreeSet<String> {
        fn walk(xs: &[ExplorerNode], out: &mut BTreeSet<String>) {
            for n in xs {
                if matches!(n.kind, ExplorerNodeKind::Folder) {
                    out.insert(n.path.clone());
                    walk(&n.children, out);
                }
            }
        }
        let mut out = BTreeSet::new();
        walk(&self.roots, &mut out);
        out
    }
    pub fn reduce_keyboard(&mut self, key: &str) {
        let rows = self.visible_rows();
        if rows.is_empty() {
            return;
        }
        let i = self
            .selected
            .as_ref()
            .and_then(|p| rows.iter().position(|r| &r.path == p))
            .unwrap_or(0);
        match key {
            "up" => self.select(&rows[i.saturating_sub(1)].path),
            "down" => self.select(&rows[(i + 1).min(rows.len() - 1)].path),
            "home" => self.select(&rows[0].path),
            "end" => self.select(&rows[rows.len() - 1].path),
            "left" => {
                if rows[i].expanded {
                    self.toggle(&rows[i].path)
                } else if let Some(p) = rows[i].path.rfind('/') {
                    self.select(&rows[i].path[..p]);
                }
            }
            "right" => self.toggle(&rows[i].path),
            "enter" => self.activate(),
            "space" => self.toggle(&rows[i].path),
            _ => {}
        }
    }
    fn node(&self, path: &str) -> Option<&ExplorerNode> {
        fn f<'a>(xs: &'a [ExplorerNode], p: &str) -> Option<&'a ExplorerNode> {
            for n in xs {
                if n.path == p {
                    return Some(n);
                }
                if let Some(x) = f(&n.children, p) {
                    return Some(x);
                }
            }
            None
        }
        f(&self.roots, path)
    }
    fn sort(&mut self) {
        fn s(xs: &mut [ExplorerNode], mode: SortMode) {
            for n in xs.iter_mut() {
                s(&mut n.children, mode)
            }
            xs.sort_by(|a, b| {
                let k = matches!(b.kind, ExplorerNodeKind::Folder)
                    .cmp(&matches!(a.kind, ExplorerNodeKind::Folder));
                if k != std::cmp::Ordering::Equal {
                    return k;
                }
                let k = match mode {
                    SortMode::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                    SortMode::Modified => b.modified.cmp(&a.modified),
                    SortMode::Created => b.created.cmp(&a.created),
                };
                k.then(a.path.cmp(&b.path))
            })
        }
        s(&mut self.roots, self.sort_mode)
    }
}
fn kind_for(name: &str) -> FileKind {
    match name
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_lowercase()
        .as_str()
    {
        "md" | "markdown" => FileKind::Markdown,
        "canvas" => FileKind::Canvas,
        "png" | "jpg" | "jpeg" | "gif" | "webp" => FileKind::Image,
        "pdf" => FileKind::Pdf,
        "mp3" | "wav" | "ogg" => FileKind::Audio,
        "mp4" | "webm" | "mov" => FileKind::Video,
        x => FileKind::Other(x.to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn builds_hierarchy_and_folders_first() {
        let m = ExplorerModel::from_vault_paths(["z.md", "a/b.md", "a/c.png"]);
        assert_eq!(m.roots[0].name, "a");
        assert_eq!(m.roots[0].children.len(), 2);
    }
    #[test]
    fn filter_keeps_ancestors_and_navigation_is_bounded() {
        let mut m = ExplorerModel::from_vault_paths(["docs/readme.md", "other.md"]);
        m.set_filter("readme");
        assert_eq!(m.visible_rows().len(), 2);
        m.reduce_keyboard("end");
        assert_eq!(m.selected.as_deref(), Some("docs/readme.md"));
        m.reduce_keyboard("down");
        assert_eq!(m.selected.as_deref(), Some("docs/readme.md"));
    }
    #[test]
    fn metadata_sort_is_deterministic() {
        let mut m = ExplorerModel::from_vault_paths([
            ExplorerFileInput {
                path: "a.md".into(),
                modified: Some(1),
                created: None,
            },
            ExplorerFileInput {
                path: "b.md".into(),
                modified: Some(3),
                created: None,
            },
        ]);
        m.set_sort_mode(SortMode::Modified);
        assert_eq!(m.roots[0].name, "b.md");
    }
}
