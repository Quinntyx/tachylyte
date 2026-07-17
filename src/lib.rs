//! Deterministic, I/O-free plans for Tachylyte's Obsidian core workflows.
//!
//! Every operation in this crate describes a proposed change. A vault adapter is
//! responsible for applying that change; this keeps tests deterministic and
//! makes destructive operations impossible without an explicit plan.

use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub type Result<T> = std::result::Result<T, WorkflowError>;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum WorkflowError {
    #[error("path is not vault-relative and safe: {0}")]
    UnsafePath(String),
    #[error("invalid template token: {0}")]
    InvalidTemplate(String),
    #[error("feature is disabled: {0}")]
    Disabled(String),
    #[error("invalid workflow input: {0}")]
    InvalidInput(String),
}

/// Validate and normalize a vault-relative path. No absolute paths or `..` are
/// accepted, and platform separators are normalized to `/`.
pub fn safe_path(path: &str) -> Result<String> {
    let p = path.replace('\\', "/");
    if p.is_empty() || p.starts_with('/') || p.starts_with("//") || p.contains(':') {
        return Err(WorkflowError::UnsafePath(path.into()));
    }
    let mut out = Vec::new();
    for part in p.split('/') {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." || part.contains('\0') {
            return Err(WorkflowError::UnsafePath(path.into()));
        }
        out.push(part);
    }
    if out.is_empty() {
        return Err(WorkflowError::UnsafePath(path.into()));
    }
    Ok(out.join("/"))
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct FeatureSettings {
    pub enabled: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct FeatureRegistry {
    pub features: BTreeMap<String, FeatureSettings>,
}
impl FeatureRegistry {
    pub fn new(names: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            features: names
                .into_iter()
                .map(|n| (n.into(), FeatureSettings { enabled: true }))
                .collect(),
        }
    }
    pub fn is_enabled(&self, name: &str) -> bool {
        self.features.get(name).is_some_and(|f| f.enabled)
    }
    pub fn set_enabled(&mut self, name: impl Into<String>, enabled: bool) {
        self.features
            .entry(name.into())
            .or_insert(FeatureSettings { enabled })
            .enabled = enabled;
    }
    pub fn require(&self, name: &str) -> Result<()> {
        if self.is_enabled(name) {
            Ok(())
        } else {
            Err(WorkflowError::Disabled(name.into()))
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct DailyNoteConfig {
    pub folder: String,
    pub date_format: String,
    pub template: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct DailyNotePlan {
    pub path: String,
    pub date: String,
    pub content: Option<String>,
    pub create: bool,
}
pub fn daily_note_plan(
    config: &DailyNoteConfig,
    now: DateTime<FixedOffset>,
    existing: &BTreeSet<String>,
    template: Option<&str>,
) -> Result<DailyNotePlan> {
    let date = now.format(&config.date_format).to_string();
    let folder = if config.folder.is_empty() {
        String::new()
    } else {
        safe_path(&config.folder)?
    };
    let name = safe_path(&format!("{}.md", date))?;
    let path = if folder.is_empty() {
        name
    } else {
        format!("{folder}/{name}")
    };
    let create = !existing.contains(&path);
    let content = if create {
        template
            .map(|t| render_template(t, now, &date, &date))
            .transpose()?
    } else {
        None
    };
    Ok(DailyNotePlan {
        path,
        date,
        content,
        create,
    })
}

/// Supported tokens are `{{date}}`, `{{time}}`, and `{{title}}`; format tokens
/// are expanded by supplying the rendered date as title/format context.
pub fn render_template(
    template: &str,
    now: DateTime<FixedOffset>,
    title: &str,
    format: &str,
) -> Result<String> {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(start) = rest.find("{{") {
        out.push_str(&rest[..start]);
        let end = rest[start + 2..]
            .find("}}")
            .ok_or_else(|| WorkflowError::InvalidTemplate("unclosed token".into()))?
            + start
            + 2;
        let token = rest[start + 2..end].trim();
        let value = match token {
            "date" => now.format("%Y-%m-%d").to_string(),
            "time" => now.format("%H:%M").to_string(),
            "title" => title.to_string(),
            "format" => format.to_string(),
            _ => return Err(WorkflowError::InvalidTemplate(token.into())),
        };
        out.push_str(&value);
        rest = &rest[end + 2..];
    }
    if rest.contains("}}") {
        return Err(WorkflowError::InvalidTemplate(
            "closing token without opening token".into(),
        ));
    }
    out.push_str(rest);
    Ok(out)
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct UniqueNotePlan {
    pub path: String,
    pub title: String,
}
pub fn unique_note_plan(
    folder: &str,
    title: &str,
    extension: &str,
    existing: &BTreeSet<String>,
    now: DateTime<FixedOffset>,
) -> Result<UniqueNotePlan> {
    let folder = safe_path(folder)?;
    let clean = title.trim().replace(['/', '\\'], "-");
    if clean.is_empty() || clean == "." || clean == ".." {
        return Err(WorkflowError::InvalidInput("empty note title".into()));
    }
    let ext = extension.trim_start_matches('.');
    let base = format!("{folder}/{clean}.{ext}");
    let path = if !existing.contains(&base) {
        base
    } else {
        let stamp = now
            .format("%Y%m%d-%H%M%S%3f%:z")
            .to_string()
            .replace(':', "");
        let candidate = format!("{folder}/{clean}-{stamp}.{ext}");
        if !existing.contains(&candidate) {
            candidate
        } else {
            let mut n = 2;
            loop {
                let p = format!("{folder}/{clean}-{stamp}-{n}.{ext}");
                if !existing.contains(&p) {
                    break p;
                }
                n += 1
            }
        }
    };
    Ok(UniqueNotePlan { path, title: clean })
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct LinkRewrite {
    pub from: String,
    pub to: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct NotePlan {
    pub content: String,
    pub rewrites: Vec<LinkRewrite>,
}
pub fn compose_note(title: &str, template: Option<&str>, body: &str) -> Result<NotePlan> {
    let head = if let Some(t) = template {
        render_template(
            t,
            DateTime::parse_from_rfc3339("2000-01-01T00:00:00+00:00").unwrap(),
            title,
            title,
        )?
    } else {
        String::new()
    };
    Ok(NotePlan {
        content: format!("{head}{title}\n\n{body}"),
        rewrites: Vec::new(),
    })
}
pub fn split_note(
    content: &str,
    marker: &str,
    left_path: &str,
    right_path: &str,
) -> Result<(NotePlan, NotePlan)> {
    safe_path(left_path)?;
    safe_path(right_path)?;
    let mut parts = content.splitn(2, marker);
    let left = parts.next().unwrap_or("");
    let right = parts
        .next()
        .ok_or_else(|| WorkflowError::InvalidInput("split marker not found".into()))?;
    Ok((
        NotePlan {
            content: left.into(),
            rewrites: vec![LinkRewrite {
                from: left_path.into(),
                to: right_path.into(),
            }],
        },
        NotePlan {
            content: right.into(),
            rewrites: vec![LinkRewrite {
                from: left_path.into(),
                to: right_path.into(),
            }],
        },
    ))
}
pub fn merge_notes(paths: &[&str], contents: &[&str], destination: &str) -> Result<NotePlan> {
    safe_path(destination)?;
    if paths.len() != contents.len() || paths.is_empty() {
        return Err(WorkflowError::InvalidInput(
            "paths and contents must match".into(),
        ));
    }
    let content = contents.join("\n\n");
    let rewrites = paths
        .iter()
        .map(|p| LinkRewrite {
            from: (*p).into(),
            to: destination.into(),
        })
        .collect();
    Ok(NotePlan { content, rewrites })
}
pub fn extract_note(
    content: &str,
    start: usize,
    end: usize,
    destination: &str,
) -> Result<NotePlan> {
    safe_path(destination)?;
    if start > end
        || end > content.len()
        || !content.is_char_boundary(start)
        || !content.is_char_boundary(end)
    {
        return Err(WorkflowError::InvalidInput(
            "invalid character boundaries".into(),
        ));
    }
    Ok(NotePlan {
        content: content[start..end].into(),
        rewrites: vec![LinkRewrite {
            from: "(selection)".into(),
            to: destination.into(),
        }],
    })
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConversionPlan {
    pub output: String,
    pub warnings: Vec<String>,
}
pub fn convert_format(input: &str, from: &str, to: &str) -> Result<ConversionPlan> {
    let output = match (
        from.to_ascii_lowercase().as_str(),
        to.to_ascii_lowercase().as_str(),
    ) {
        (a, b) if a == b => input.into(),
        ("markdown", "plain") => input.replace("**", "").replace(['*', '`'], ""),
        ("plain", "markdown") => input.into(),
        ("markdown", "html") => input
            .lines()
            .map(|l| format!("<p>{}</p>", escape_html(l)))
            .collect::<Vec<_>>()
            .join("\n"),
        _ => {
            return Err(WorkflowError::InvalidInput(format!(
                "unsupported conversion {from} -> {to}"
            )))
        }
    };
    Ok(ConversionPlan {
        output,
        warnings: Vec::new(),
    })
}
fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommandDefinition {
    pub id: String,
    pub name: String,
    pub feature: String,
    pub enabled: bool,
}
pub fn command_definitions(
    registry: &FeatureRegistry,
    definitions: &[CommandDefinition],
) -> Vec<CommandDefinition> {
    definitions
        .iter()
        .filter(|c| registry.is_enabled(&c.feature))
        .cloned()
        .map(|mut c| {
            c.enabled = true;
            c
        })
        .collect()
}
pub fn rank_slash_commands<'a>(
    query: &str,
    commands: &'a [CommandDefinition],
) -> Vec<&'a CommandDefinition> {
    let q = query.trim_start_matches('/').to_ascii_lowercase();
    let mut out: Vec<_> = commands.iter().filter(|c| c.enabled).collect();
    out.sort_by_key(|c| {
        let n = c.name.to_ascii_lowercase();
        let id = c.id.to_ascii_lowercase();
        if n == q {
            (0, 0, n)
        } else if n.starts_with(&q) {
            (1, n.len(), n)
        } else if id.contains(&q) || n.contains(&q) {
            (2, n.len(), n)
        } else {
            (3, usize::MAX, n)
        }
    });
    out.retain(|c| {
        c.name.to_ascii_lowercase().contains(&q) || c.id.to_ascii_lowercase().contains(&q)
    });
    out
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Snapshot {
    pub revision: u64,
    pub timestamp: DateTime<FixedOffset>,
    pub content: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecoveryPlan {
    pub retain: Vec<Snapshot>,
    pub delete: Vec<u64>,
}
pub fn retention_plan(mut snapshots: Vec<Snapshot>, keep: usize) -> RecoveryPlan {
    snapshots.sort_by_key(|s| (s.timestamp, s.revision));
    let split = snapshots.len().saturating_sub(keep);
    let delete = snapshots[..split].iter().map(|s| s.revision).collect();
    RecoveryPlan {
        retain: snapshots[split..].to_vec(),
        delete,
    }
}
pub fn restore_plan(snapshot: &Snapshot, current: &str) -> Result<ConversionPlan> {
    if snapshot.content == current {
        return Ok(ConversionPlan {
            output: current.into(),
            warnings: vec!["snapshot is already current".into()],
        });
    }
    Ok(ConversionPlan {
        output: snapshot.content.clone(),
        warnings: vec![format!("restore revision {}", snapshot.revision)],
    })
}
pub fn diff_snapshots(old: &str, new: &str) -> Vec<String> {
    old.lines()
        .zip(new.lines())
        .enumerate()
        .filter(|(_, (a, b))| a != b)
        .map(|(i, (a, b))| format!("line {}: -{} +{}", i + 1, a, b))
        .collect()
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum AudioState {
    Idle,
    Recording,
    Paused,
    Stopped,
    Cancelled,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct AudioSession {
    pub id: String,
    pub state: AudioState,
    pub attachment: Option<String>,
}
pub fn audio_start(id: &str, folder: &str, now: DateTime<FixedOffset>) -> Result<AudioSession> {
    let folder = safe_path(folder)?;
    Ok(AudioSession {
        id: id.into(),
        state: AudioState::Recording,
        attachment: Some(format!(
            "{folder}/audio-{}.webm",
            now.format("%Y%m%d-%H%M%S")
        )),
    })
}
pub fn audio_transition(session: &AudioSession, state: AudioState) -> Result<AudioSession> {
    if matches!(
        (&session.state, &state),
        (AudioState::Idle, AudioState::Paused)
            | (AudioState::Stopped, _)
            | (AudioState::Cancelled, _)
    ) {
        return Err(WorkflowError::InvalidInput(
            "invalid audio transition".into(),
        ));
    }
    let mut next = session.clone();
    next.state = state;
    Ok(next)
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Slide {
    pub index: usize,
    pub title: Option<String>,
    pub content: String,
}
pub fn parse_slides(markdown: &str) -> Vec<Slide> {
    markdown
        .split("\n---\n")
        .enumerate()
        .map(|(index, content)| {
            let title = content
                .lines()
                .find_map(|l| l.strip_prefix("# ").map(str::to_string));
            Slide {
                index,
                title,
                content: content.into(),
            }
        })
        .collect()
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct WordStatus {
    pub words: usize,
    pub characters: usize,
    pub reading_minutes: usize,
}
pub fn word_status(text: &str) -> WordStatus {
    let words = text.split_whitespace().count();
    WordStatus {
        words,
        characters: text.chars().count(),
        reading_minutes: words.div_ceil(200),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    fn now() -> DateTime<FixedOffset> {
        FixedOffset::east_opt(0)
            .unwrap()
            .with_ymd_and_hms(2024, 3, 31, 1, 30, 0)
            .unwrap()
    }
    #[test]
    fn paths_and_templates_are_safe() {
        assert!(safe_path("../x").is_err());
        assert_eq!(
            render_template("{{title}} {{time}}", now(), "A", "x").unwrap(),
            "A 01:30"
        );
        assert!(render_template("{{bad}}", now(), "", "").is_err());
    }
    #[test]
    fn daily_is_idempotent_and_collision_safe() {
        let c = DailyNoteConfig {
            folder: "daily".into(),
            date_format: "%Y-%m-%d".into(),
            template: None,
        };
        let mut e = BTreeSet::new();
        let p = daily_note_plan(&c, now(), &e, None).unwrap();
        assert!(p.create);
        e.insert(p.path.clone());
        assert!(!daily_note_plan(&c, now(), &e, None).unwrap().create);
        let u = unique_note_plan("notes", "x", "md", &e, now()).unwrap();
        assert!(u.path.ends_with(".md"));
    }
    #[test]
    fn split_recovery_slides_and_status() {
        assert_eq!(
            split_note("a\n---\nb", "\n---\n", "a.md", "b.md")
                .unwrap()
                .0
                .content,
            "a"
        );
        let s = Snapshot {
            revision: 1,
            timestamp: now(),
            content: "a".into(),
        };
        assert_eq!(retention_plan(vec![s.clone()], 1).delete.len(), 0);
        assert_eq!(parse_slides("# A\n\n---\n# B").len(), 2);
        assert_eq!(word_status("one two").words, 2);
    }
}
