//! Deterministic, I/O-free plans for Tachylyte's Obsidian core workflows.
//!
//! Every operation in this crate describes a proposed change. A vault adapter is
//! responsible for applying that change; this keeps tests deterministic and
//! makes destructive operations impossible without an explicit plan.

use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
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
    #[error("conversion is lossy: {0}")]
    LossyConversion(String),
    #[error("restore precondition failed: {0}")]
    RestorePrecondition(String),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Sha256Digest(pub [u8; 32]);
impl Sha256Digest {
    pub fn of(content: &str) -> Self {
        Self(Sha256::digest(content.as_bytes()).into())
    }
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

fn existing_contains_normalized(existing: &BTreeSet<String>, candidate: &str) -> Result<bool> {
    let candidate = safe_path(candidate)?;
    existing.iter().try_fold(false, |found, path| {
        Ok(found || safe_path(path)? == candidate)
    })
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

/// Entry-point gate used by adapters so disabled features produce no plans,
/// commands, or background work.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkflowService {
    pub registry: FeatureRegistry,
}
impl WorkflowService {
    pub fn new(registry: FeatureRegistry) -> Self {
        Self { registry }
    }
    pub fn compose_note(
        &self,
        title: &str,
        template: Option<&str>,
        body: &str,
        now: DateTime<FixedOffset>,
    ) -> Result<NotePlan> {
        self.registry.require("note-composer")?;
        compose_note(title, template, body, now)
    }
    pub fn daily_note(
        &self,
        config: &DailyNoteConfig,
        now: DateTime<FixedOffset>,
        existing: &BTreeSet<String>,
        template: Option<&str>,
    ) -> Result<DailyNotePlan> {
        self.registry.require("daily-notes")?;
        daily_note_plan(config, now, existing, template)
    }
    pub fn audio_start(
        &self,
        id: &str,
        folder: &str,
        now: DateTime<FixedOffset>,
    ) -> Result<AudioSession> {
        self.registry.require("audio-recorder")?;
        audio_start(id, folder, now)
    }
    pub fn unique_note(
        &self,
        folder: &str,
        title: &str,
        extension: &str,
        existing: &BTreeSet<String>,
        now: DateTime<FixedOffset>,
    ) -> Result<UniqueNotePlan> {
        self.registry.require("unique-note")?;
        unique_note_plan(folder, title, extension, existing, now)
    }
    pub fn split_note(
        &self,
        source: &str,
        content: &str,
        digest: Sha256Digest,
        marker: &str,
        left: &str,
        right: &str,
    ) -> Result<(NotePlan, NotePlan)> {
        self.registry.require("note-composer")?;
        split_note(source, content, digest, marker, left, right)
    }
    pub fn merge_notes(
        &self,
        paths: &[&str],
        contents: &[&str],
        destination: &str,
    ) -> Result<NotePlan> {
        self.registry.require("note-composer")?;
        merge_notes(paths, contents, destination)
    }
    pub fn extract_note(
        &self,
        content: &str,
        start: usize,
        end: usize,
        destination: &str,
    ) -> Result<NotePlan> {
        self.registry.require("note-composer")?;
        extract_note(content, start, end, destination)
    }
    pub fn convert_format(&self, input: &str, from: &str, to: &str) -> Result<ConversionPlan> {
        self.registry.require("format-converter")?;
        convert_format(input, from, to)
    }
    pub fn recovery_retention(
        &self,
        snapshots: Vec<Snapshot>,
        keep: usize,
    ) -> Result<RecoveryPlan> {
        self.registry.require("file-recovery")?;
        Ok(retention_plan(snapshots, keep))
    }
    pub fn restore_checked(
        &self,
        snapshot: &Snapshot,
        current: &str,
        revision: u64,
        digest: Sha256Digest,
    ) -> Result<RestorePlan> {
        self.registry.require("file-recovery")?;
        restore_plan_checked(snapshot, current, revision, digest)
    }
    pub fn slides(&self, markdown: &str) -> Result<Vec<Slide>> {
        self.registry.require("slides")?;
        Ok(parse_slides(markdown))
    }
    pub fn word_status(&self, text: &str) -> Result<WordStatus> {
        self.registry.require("word-count")?;
        Ok(word_status(text))
    }
    pub fn commands(&self, definitions: &[CommandDefinition]) -> Result<Vec<CommandDefinition>> {
        self.registry.require("commands")?;
        Ok(command_definitions(&self.registry, definitions))
    }
    pub fn slash_commands<'a>(
        &self,
        query: &str,
        commands: &'a [CommandDefinition],
    ) -> Result<Vec<&'a CommandDefinition>> {
        self.registry.require("commands")?;
        Ok(rank_slash_commands(query, commands))
    }
    pub fn audio_transition(
        &self,
        session: &AudioSession,
        state: AudioState,
    ) -> Result<AudioSession> {
        self.registry.require("audio-recorder")?;
        audio_transition(session, state)
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
    let create = !existing_contains_normalized(existing, &path)?;
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

/// Variant for applications with a named timezone database. The caller resolves
/// the instant in that timezone (including DST rules) and passes the resulting
/// offset-aware instant here; this crate never consults process-global timezone
/// state.
pub fn daily_note_plan_resolved(
    config: &DailyNoteConfig,
    resolved_now: DateTime<FixedOffset>,
    existing: &BTreeSet<String>,
    template: Option<&str>,
) -> Result<DailyNotePlan> {
    daily_note_plan(config, resolved_now, existing, template)
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
    pub precondition: PlanPrecondition,
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
    if clean.is_empty() || clean == "." || clean == ".." || is_reserved_name(&clean) {
        return Err(WorkflowError::InvalidInput(
            "empty or reserved note title".into(),
        ));
    }
    if clean
        .chars()
        .any(|c| c.is_control() || "<>:\"/\\|?*".contains(c))
        || clean.ends_with('.')
        || clean.ends_with(' ')
    {
        return Err(WorkflowError::InvalidInput(
            "title contains platform-unsafe characters".into(),
        ));
    }
    let ext = extension.trim_start_matches('.');
    if ext.is_empty()
        || ext
            .chars()
            .any(|c| c.is_control() || "<>:\"/\\|?*".contains(c))
    {
        return Err(WorkflowError::InvalidInput("invalid note extension".into()));
    }
    let base = format!("{folder}/{clean}.{ext}");
    safe_path(&base)?;
    let path = if !existing_contains_normalized(existing, &base)? {
        base
    } else {
        let stamp = now
            .format("%Y%m%d-%H%M%S%3f%:z")
            .to_string()
            .replace(':', "");
        let candidate = format!("{folder}/{clean}-{stamp}.{ext}");
        if !existing_contains_normalized(existing, &candidate)? {
            candidate
        } else {
            let mut n = 2;
            loop {
                let p = format!("{folder}/{clean}-{stamp}-{n}.{ext}");
                if !existing_contains_normalized(existing, &p)? {
                    break p;
                }
                n += 1
            }
        }
    };
    Ok(UniqueNotePlan {
        path: path.clone(),
        title: clean,
        precondition: PlanPrecondition {
            path,
            condition: Precondition::MustNotExist,
        },
    })
}

fn is_reserved_name(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or(name).to_ascii_uppercase();
    matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.as_bytes()[3].is_ascii_digit()
            && stem.as_bytes()[3] != b'0')
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct LinkRewrite {
    pub from: String,
    pub to: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum SourceAction {
    Retain,
    Delete,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Precondition {
    MustNotExist,
    ContentDigest(Sha256Digest),
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlanPrecondition {
    pub path: String,
    pub condition: Precondition,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct NotePlan {
    pub content: String,
    pub rewrites: Vec<LinkRewrite>,
    pub source_action: SourceAction,
    pub preconditions: Vec<PlanPrecondition>,
    pub ordering: Vec<String>,
}
pub fn compose_note(
    title: &str,
    template: Option<&str>,
    body: &str,
    now: DateTime<FixedOffset>,
) -> Result<NotePlan> {
    let head = if let Some(t) = template {
        render_template(t, now, title, title)?
    } else {
        String::new()
    };
    Ok(NotePlan {
        content: format!("{head}{title}\n\n{body}"),
        rewrites: Vec::new(),
        source_action: SourceAction::Retain,
        preconditions: Vec::new(),
        ordering: Vec::new(),
    })
}
pub fn split_note(
    source_path: &str,
    content: &str,
    expected_source: Sha256Digest,
    marker: &str,
    left_path: &str,
    right_path: &str,
) -> Result<(NotePlan, NotePlan)> {
    let source_path = safe_path(source_path)?;
    let left_path = safe_path(left_path)?;
    let right_path = safe_path(right_path)?;
    if marker.is_empty()
        || left_path == right_path
        || source_path == left_path
        || source_path == right_path
    {
        return Err(WorkflowError::InvalidInput(
            "split marker and destinations must be distinct".into(),
        ));
    }
    if Sha256Digest::of(content) != expected_source {
        return Err(WorkflowError::RestorePrecondition(
            "split source digest mismatch".into(),
        ));
    }
    let mut parts = content.splitn(2, marker);
    let left = parts.next().unwrap_or("");
    let right = parts
        .next()
        .ok_or_else(|| WorkflowError::InvalidInput("split marker not found".into()))?;
    Ok((
        NotePlan {
            content: left.into(),
            // Rewrites are metadata for links outside the two new notes. They
            // must never be applied to either newly-created body.
            rewrites: Vec::new(),
            source_action: SourceAction::Retain,
            preconditions: vec![
                PlanPrecondition {
                    path: source_path.clone(),
                    condition: Precondition::ContentDigest(expected_source.clone()),
                },
                PlanPrecondition {
                    path: left_path.clone(),
                    condition: Precondition::MustNotExist,
                },
            ],
            ordering: vec!["check source".into(), "create left".into()],
        },
        NotePlan {
            content: right.into(),
            rewrites: Vec::new(),
            source_action: SourceAction::Retain,
            preconditions: vec![
                PlanPrecondition {
                    path: source_path,
                    condition: Precondition::ContentDigest(expected_source),
                },
                PlanPrecondition {
                    path: right_path,
                    condition: Precondition::MustNotExist,
                },
            ],
            ordering: vec![
                "check source".into(),
                "create left".into(),
                "create right".into(),
                "rewrite external links".into(),
            ],
        },
    ))
}
pub fn merge_notes(paths: &[&str], contents: &[&str], destination: &str) -> Result<NotePlan> {
    let destination = safe_path(destination)?;
    if paths.len() != contents.len() || paths.is_empty() {
        return Err(WorkflowError::InvalidInput(
            "paths and contents must match".into(),
        ));
    }
    let normalized_paths: Vec<String> = paths
        .iter()
        .map(|path| safe_path(path))
        .collect::<Result<_>>()?;
    for path in &normalized_paths {
        if *path == destination {
            return Err(WorkflowError::InvalidInput(
                "merge destination equals source".into(),
            ));
        }
    }
    let content = contents.join("\n\n");
    let rewrites = normalized_paths
        .iter()
        .map(|p| LinkRewrite {
            from: p.clone(),
            to: destination.clone(),
        })
        .collect();
    let mut preconditions: Vec<PlanPrecondition> = normalized_paths
        .iter()
        .zip(contents.iter())
        .map(|(p, content)| PlanPrecondition {
            path: p.clone(),
            condition: Precondition::ContentDigest(Sha256Digest::of(content)),
        })
        .collect();
    preconditions.push(PlanPrecondition {
        path: destination.clone(),
        condition: Precondition::MustNotExist,
    });
    Ok(NotePlan {
        content,
        rewrites,
        source_action: SourceAction::Delete,
        preconditions,
        ordering: vec![
            "check all source digests".into(),
            "check destination absent".into(),
            "create destination".into(),
            "rewrite links".into(),
            "delete sources".into(),
        ],
    })
}
pub fn extract_note(
    content: &str,
    start: usize,
    end: usize,
    destination: &str,
) -> Result<NotePlan> {
    let destination = safe_path(destination)?;
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
            to: destination.clone(),
        }],
        source_action: SourceAction::Retain,
        preconditions: vec![PlanPrecondition {
            path: destination,
            condition: Precondition::MustNotExist,
        }],
        ordering: vec!["create destination".into()],
    })
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConversionPlan {
    pub output: String,
    pub warnings: Vec<String>,
}
pub fn convert_format(input: &str, from: &str, to: &str) -> Result<ConversionPlan> {
    match (
        from.to_ascii_lowercase().as_str(),
        to.to_ascii_lowercase().as_str(),
    ) {
        (a, b) if a == b => Ok(ConversionPlan {
            output: input.into(),
            warnings: Vec::new(),
        }),
        ("markdown", "plain") => Ok(ConversionPlan {
            output: markdown_to_plain(input),
            warnings: vec!["markdown styling is removed outside literal code blocks".into()],
        }),
        ("plain", "markdown") => Ok(ConversionPlan {
            output: input.into(),
            warnings: Vec::new(),
        }),
        ("markdown", "html") => Ok(ConversionPlan {
            output: markdown_to_html(input),
            warnings: vec![
                "basic HTML conversion preserves code blocks but not all Markdown extensions"
                    .into(),
            ],
        }),
        _ => Err(WorkflowError::InvalidInput(format!(
            "unsupported conversion {from} -> {to}"
        ))),
    }
}
fn markdown_to_plain(input: &str) -> String {
    let mut code = false;
    input
        .lines()
        .map(|line| {
            if line.trim_start().starts_with("```") {
                code = !code;
                return line.to_string();
            }
            if code {
                line.to_string()
            } else {
                line.replace("**", "")
                    .replace("__", "")
                    .replace(['*', '_', '`'], "")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}
fn markdown_to_html(input: &str) -> String {
    let mut code = false;
    input
        .lines()
        .map(|line| {
            if line.trim_start().starts_with("```") {
                code = !code;
                return if code {
                    "<pre><code>".into()
                } else {
                    "</code></pre>".into()
                };
            }
            if code {
                escape_html(line)
            } else {
                format!("<p>{}</p>", escape_html(line))
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
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
    let _ = (snapshot, current);
    Err(WorkflowError::RestorePrecondition(
        "restore requires revision and current-content checksum; use restore_plan_checked".into(),
    ))
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RestorePlan {
    pub output: String,
    pub revision: u64,
    pub expected_current_checksum: Sha256Digest,
}
pub fn restore_plan_checked(
    snapshot: &Snapshot,
    current: &str,
    expected_revision: u64,
    expected_checksum: Sha256Digest,
) -> Result<RestorePlan> {
    if snapshot.revision != expected_revision {
        return Err(WorkflowError::RestorePrecondition(
            "revision identity mismatch".into(),
        ));
    }
    let actual = Sha256Digest::of(current);
    if actual != expected_checksum {
        return Err(WorkflowError::RestorePrecondition(
            "current content checksum mismatch".into(),
        ));
    }
    Ok(RestorePlan {
        output: snapshot.content.clone(),
        revision: snapshot.revision,
        expected_current_checksum: expected_checksum,
    })
}
pub fn diff_snapshots(old: &str, new: &str) -> Vec<String> {
    let old_lines: Vec<_> = old.lines().collect();
    let new_lines: Vec<_> = new.lines().collect();
    let mut lcs = vec![vec![0usize; new_lines.len() + 1]; old_lines.len() + 1];
    for i in (0..old_lines.len()).rev() {
        for j in (0..new_lines.len()).rev() {
            lcs[i][j] = if old_lines[i] == new_lines[j] {
                1 + lcs[i + 1][j + 1]
            } else {
                lcs[i + 1][j].max(lcs[i][j + 1])
            };
        }
    }
    let (mut i, mut j) = (0, 0);
    let mut result = Vec::new();
    while i < old_lines.len() || j < new_lines.len() {
        if i < old_lines.len() && j < new_lines.len() && old_lines[i] == new_lines[j] {
            i += 1;
            j += 1;
        } else if j < new_lines.len() && (i == old_lines.len() || lcs[i][j + 1] >= lcs[i + 1][j]) {
            result.push(format!("line {}: +{}", i + 1, new_lines[j]));
            j += 1;
        } else {
            result.push(format!("line {}: -{}", i + 1, old_lines[i]));
            i += 1;
        }
    }
    result
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
    let session = id.trim().replace(['/', '\\'], "-");
    if session.is_empty()
        || session
            .chars()
            .any(|c| c.is_control() || "<>:\"|?*".contains(c))
    {
        return Err(WorkflowError::InvalidInput(
            "invalid audio session id".into(),
        ));
    }
    let attachment = format!(
        "{folder}/audio-{}-{}.webm",
        now.format("%Y%m%d-%H%M%S%3f"),
        session
    );
    safe_path(&attachment)?;
    Ok(AudioSession {
        id: session,
        state: AudioState::Recording,
        attachment: Some(attachment),
    })
}
pub fn audio_transition(session: &AudioSession, state: AudioState) -> Result<AudioSession> {
    let valid = matches!(
        (&session.state, &state),
        (AudioState::Idle, AudioState::Recording)
            | (
                AudioState::Recording,
                AudioState::Paused | AudioState::Stopped | AudioState::Cancelled
            )
            | (
                AudioState::Paused,
                AudioState::Recording | AudioState::Stopped | AudioState::Cancelled
            )
    );
    if !valid {
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
    let mut slides = Vec::new();
    let mut current = String::new();
    let mut fenced = false;
    for line in markdown.replace("\r\n", "\n").lines() {
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
        }
        if !fenced && line.trim() == "---" {
            slides.push(current.clone());
            current.clear();
        } else {
            if !current.is_empty() {
                current.push('\n');
            }
            current.push_str(line);
        }
    }
    slides.push(current);
    slides
        .into_iter()
        .enumerate()
        .map(|(index, content)| {
            let title = content
                .lines()
                .find_map(|l| l.strip_prefix("# ").map(str::to_string));
            Slide {
                index,
                title,
                content,
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
            split_note(
                "source.md",
                "a\n---\nb",
                Sha256Digest::of("a\n---\nb"),
                "\n---\n",
                "a.md",
                "b.md"
            )
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
    #[test]
    fn plans_are_explicit_and_split_does_not_rewrite_new_bodies() {
        let (left, right) = split_note(
            "source.md",
            "a\n---\nb",
            Sha256Digest::of("a\n---\nb"),
            "\n---\n",
            "a.md",
            "right.md",
        )
        .unwrap();
        assert!(left.rewrites.is_empty() && right.rewrites.is_empty());
        assert_eq!(left.source_action, SourceAction::Retain);
        assert_eq!(
            left.preconditions[0].condition,
            Precondition::ContentDigest(Sha256Digest::of("a\n---\nb"))
        );
        assert!(
            merge_notes(&["a.md"], &["a"], "merged.md")
                .unwrap()
                .source_action
                == SourceAction::Delete
        );
        assert!(merge_notes(&["../a"], &["a"], "merged.md").is_err());
    }
    #[test]
    fn fidelity_and_recovery_regressions_are_visible() {
        let converted =
            convert_format("before\n```\n**literal**\n```", "markdown", "plain").unwrap();
        assert!(converted.output.contains("**literal**") && !converted.warnings.is_empty());
        assert_eq!(
            diff_snapshots("same", "same\nadded"),
            vec!["line 2: +added"]
        );
        assert_eq!(
            diff_snapshots("same\nremoved", "same"),
            vec!["line 2: -removed"]
        );
        let s = Snapshot {
            revision: 3,
            timestamp: now(),
            content: "old".into(),
        };
        assert!(restore_plan_checked(&s, "current", 2, Sha256Digest::of("current")).is_err());
        assert!(restore_plan_checked(&s, "current", 3, Sha256Digest::of("wrong")).is_err());
        assert_eq!(
            restore_plan_checked(&s, "current", 3, Sha256Digest::of("current"))
                .unwrap()
                .revision,
            3
        );
    }
    #[test]
    fn unsafe_names_audio_transitions_and_fenced_slides() {
        assert!(unique_note_plan("notes", "CON", "md", &BTreeSet::new(), now()).is_err());
        assert!(unique_note_plan("notes", "bad*name", "m:d", &BTreeSet::new(), now()).is_err());
        let audio = audio_start("session/42", "media", now()).unwrap();
        assert!(audio.attachment.as_ref().unwrap().contains("session-42"));
        assert!(audio_transition(&audio, AudioState::Idle).is_err());
        let paused = audio_transition(&audio, AudioState::Paused).unwrap();
        assert!(audio_transition(&paused, AudioState::Stopped).is_ok());
        assert_eq!(parse_slides("# A\n```\n---\n```\n---\r\n# B").len(), 2);
    }
    #[test]
    fn disabled_service_does_not_plan_work() {
        let mut registry = FeatureRegistry::new(["note-composer"]);
        registry.set_enabled("note-composer", false);
        let service = WorkflowService::new(registry);
        assert!(service.compose_note("x", None, "body", now()).is_err());
    }
    #[test]
    fn destination_guards_digests_and_lcs_are_explicit() {
        let unique = unique_note_plan("notes", "new", "md", &BTreeSet::new(), now()).unwrap();
        assert_eq!(unique.precondition.condition, Precondition::MustNotExist);
        let merged = merge_notes(&["a.md", "b.md"], &["A", "B"], "out.md").unwrap();
        assert!(merged
            .preconditions
            .iter()
            .any(|p| p.path == "out.md" && p.condition == Precondition::MustNotExist));
        assert!(merged
            .preconditions
            .iter()
            .any(|p| p.condition == Precondition::ContentDigest(Sha256Digest::of("A"))));
        assert!(merge_notes(&["out.md"], &["A"], "out.md").is_err());
        assert_eq!(diff_snapshots("a\nc", "a\nb\nc"), vec!["line 2: +b"]);
        assert!(unique_note_plan("notes", "COM9.txt", "md", &BTreeSet::new(), now()).is_err());
    }
    #[test]
    fn normalized_paths_cannot_bypass_collision_guards() {
        assert_eq!(safe_path(r"a\b"), safe_path("a/b"));
        assert_eq!(safe_path("a/./b"), safe_path("a/b"));
        assert!(merge_notes(&["a\\b.md"], &["A"], "a/b.md").is_err());
        assert!(merge_notes(&["a/./b.md"], &["A"], "a/b.md").is_err());
        assert!(split_note(
            "src\\note.md",
            "x---y",
            Sha256Digest::of("x---y"),
            "---",
            "src/note.md",
            "other.md"
        )
        .is_err());
        let mut existing = BTreeSet::new();
        existing.insert("notes\\new.md".into());
        let unique = unique_note_plan("notes", "new", "md", &existing, now()).unwrap();
        assert_ne!(unique.path, "notes/new.md");
    }
}
