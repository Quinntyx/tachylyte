//! Small, deterministic, local-first foundations for an Obsidian-compatible vault.
//!
//! All filesystem mutations are confined to the vault root and use an atomic
//! temporary-file-and-rename strategy where applicable.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::{
    collections::BTreeMap,
    fmt, fs, io,
    path::{Component, Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use thiserror::Error;

/// Errors returned by core operations. User-controlled paths never panic.
#[derive(Debug, Error)]
pub enum CoreError {
    #[error("invalid vault-relative path: {0}")]
    InvalidPath(String),
    #[error("path escapes vault: {0}")]
    Escape(String),
    #[error("path is a symbolic link: {0}")]
    Symlink(PathBuf),
    #[error("not found: {0}")]
    NotFound(PathBuf),
    #[error("duplicate name: {0}")]
    Duplicate(PathBuf),
    #[error("unsupported file kind: {0}")]
    Unsupported(String),
    #[error("serialization error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("filesystem error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}
fn io_at(path: &Path, source: io::Error) -> CoreError {
    CoreError::Io {
        path: path.to_path_buf(),
        source,
    }
}

/// A validated path relative to a vault root.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct VaultPath(PathBuf);
impl VaultPath {
    /// Validate a relative path. Absolute paths, `..`, roots, and empty paths are rejected.
    pub fn new(path: impl AsRef<Path>) -> Result<Self, CoreError> {
        let p = path.as_ref();
        if p.as_os_str().is_empty() || p.is_absolute() {
            return Err(CoreError::InvalidPath(p.display().to_string()));
        }
        let mut out = PathBuf::new();
        for c in p.components() {
            match c {
                Component::Normal(x) => out.push(x),
                Component::CurDir => {}
                _ => return Err(CoreError::InvalidPath(p.display().to_string())),
            }
        }
        if out.as_os_str().is_empty() {
            return Err(CoreError::InvalidPath(p.display().to_string()));
        }
        Ok(Self(out))
    }
    /// Return the normalized relative path.
    pub fn as_path(&self) -> &Path {
        &self.0
    }
}
impl fmt::Display for VaultPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.display().fmt(f)
    }
}

/// A local vault rooted at `root`.
#[derive(Clone, Debug)]
pub struct Vault {
    root: PathBuf,
}
impl Vault {
    /// Open (or create) a vault directory.
    pub fn open(root: impl AsRef<Path>) -> Result<Self, CoreError> {
        let root = root.as_ref().to_path_buf();
        fs::create_dir_all(&root).map_err(|e| io_at(&root, e))?;
        let root = fs::canonicalize(&root).map_err(|e| io_at(&root, e))?;
        Ok(Self { root })
    }
    /// Absolute canonical vault root.
    pub fn root(&self) -> &Path {
        &self.root
    }
    fn checked(&self, p: &VaultPath, write: bool) -> Result<PathBuf, CoreError> {
        let full = self.root.join(p.as_path());
        let mut current = self.root.clone();
        let components: Vec<_> = p.as_path().components().collect();
        for (i, c) in components.iter().enumerate() {
            current.push(c.as_os_str());
            if let Ok(meta) = fs::symlink_metadata(&current) {
                if meta.file_type().is_symlink() {
                    return Err(CoreError::Symlink(current));
                }
                if write && i + 1 < components.len() && !meta.is_dir() {
                    return Err(io_at(
                        &current,
                        io::Error::from(io::ErrorKind::NotADirectory),
                    ));
                }
            } else if write {
                break;
            }
        }
        if !write {
            let canonical = fs::canonicalize(&full).map_err(|e| {
                if e.kind() == io::ErrorKind::NotFound {
                    CoreError::NotFound(p.0.clone())
                } else {
                    io_at(&full, e)
                }
            })?;
            if !canonical.starts_with(&self.root) {
                return Err(CoreError::Escape(p.to_string()));
            }
        }
        Ok(full)
    }
    /// Read UTF-8 bytes from a vault file.
    pub fn read(&self, path: &VaultPath) -> Result<Vec<u8>, CoreError> {
        let p = self.checked(path, false)?;
        fs::read(&p).map_err(|e| io_at(&p, e))
    }
    /// Atomically replace a file, refusing symlink targets and creating parents.
    pub fn write(&self, path: &VaultPath, data: &[u8]) -> Result<(), CoreError> {
        let p = self.checked(path, true)?;
        let parent = p.parent().unwrap();
        fs::create_dir_all(parent).map_err(|e| io_at(parent, e))?;
        let tmp = unique_temp(parent);
        {
            use std::io::Write;
            let mut f = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&tmp)
                .map_err(|e| io_at(&tmp, e))?;
            f.write_all(data)
                .and_then(|_| f.sync_all())
                .map_err(|e| io_at(&tmp, e))?;
        }
        fs::rename(&tmp, &p).map_err(|e| {
            let _ = fs::remove_file(&tmp);
            io_at(&p, e)
        })
    }
    /// Atomically create a new file, failing if it already exists.
    pub fn create(&self, path: &VaultPath, data: &[u8]) -> Result<(), CoreError> {
        let p = self.checked(path, true)?;
        if p.exists() {
            return Err(CoreError::Duplicate(path.0.clone()));
        }
        self.write(path, data)
    }
    /// Rename a file or directory within this vault.
    pub fn rename(&self, from: &VaultPath, to: &VaultPath) -> Result<(), CoreError> {
        let a = self.checked(from, false)?;
        let b = self.checked(to, true)?;
        if b.exists() {
            return Err(CoreError::Duplicate(to.0.clone()));
        }
        if let Some(parent) = b.parent() {
            fs::create_dir_all(parent).map_err(|e| io_at(parent, e))?;
        }
        fs::rename(&a, &b).map_err(|e| io_at(&b, e))
    }
    /// Delete a vault entry permanently.
    pub fn delete(&self, path: &VaultPath) -> Result<(), CoreError> {
        let p = self.checked(path, false)?;
        let m = fs::metadata(&p).map_err(|e| io_at(&p, e))?;
        if m.is_dir() {
            fs::remove_dir_all(&p)
        } else {
            fs::remove_file(&p)
        }
        .map_err(|e| io_at(&p, e))
    }
    /// Move an entry atomically into `.trash`, preserving its filename.
    pub fn trash(&self, path: &VaultPath) -> Result<VaultPath, CoreError> {
        let from = self.checked(path, false)?;
        let dir = self.root.join(".trash");
        fs::create_dir_all(&dir).map_err(|e| io_at(&dir, e))?;
        let name = path
            .as_path()
            .file_name()
            .ok_or_else(|| CoreError::InvalidPath(path.to_string()))?;
        let mut dest = dir.join(name);
        if dest.exists() {
            dest.set_file_name(format!("{}-{}", name.to_string_lossy(), unique_suffix()));
        }
        fs::rename(&from, &dest).map_err(|e| io_at(&dest, e))?;
        Ok(VaultPath::new(dest.strip_prefix(&self.root).unwrap()).expect("internal trash path"))
    }
    /// Scan supported Obsidian files, excluding hidden directories and `.trash`.
    pub fn scan(&self) -> Result<Vec<VaultEntry>, CoreError> {
        let mut out = Vec::new();
        scan_dir(&self.root, &self.root, &mut out)?;
        out.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(out)
    }
}
fn scan_dir(root: &Path, dir: &Path, out: &mut Vec<VaultEntry>) -> Result<(), CoreError> {
    for e in fs::read_dir(dir).map_err(|x| io_at(dir, x))? {
        let e = e.map_err(|x| io_at(dir, x))?;
        let p = e.path();
        let n = e.file_name();
        if n.to_string_lossy().starts_with('.') {
            continue;
        }
        let m = e.metadata().map_err(|x| io_at(&p, x))?;
        if m.is_dir() {
            scan_dir(root, &p, out)?
        } else if let Some(kind) = FileKind::from_path(&p) {
            out.push(VaultEntry {
                path: VaultPath::new(p.strip_prefix(root).unwrap()).unwrap(),
                kind,
                size: m.len(),
            })
        }
    }
    Ok(())
}
fn unique_suffix() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}
fn unique_temp(p: &Path) -> PathBuf {
    p.join(format!(".tachylyte-{}.tmp", unique_suffix()))
}

/// Supported Obsidian file categories.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum FileKind {
    Markdown,
    Canvas,
    Image,
    Audio,
    Video,
    Pdf,
}
impl FileKind {
    fn from_path(p: &Path) -> Option<Self> {
        match p.extension()?.to_str()?.to_ascii_lowercase().as_str() {
            "md" | "markdown" => Some(Self::Markdown),
            "canvas" => Some(Self::Canvas),
            "png" | "jpg" | "jpeg" | "gif" | "svg" | "webp" => Some(Self::Image),
            "mp3" | "wav" | "m4a" | "ogg" => Some(Self::Audio),
            "mp4" | "webm" | "mov" => Some(Self::Video),
            "pdf" => Some(Self::Pdf),
            _ => None,
        }
    }
}
/// A scanned supported file.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct VaultEntry {
    pub path: VaultPath,
    pub kind: FileKind,
    pub size: u64,
}
/// Cached metadata associated with a vault file.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct MetadataRecord {
    pub path: String,
    pub modified: u64,
    pub size: u64,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}
/// Persist a metadata cache record atomically.
pub fn save_metadata(
    vault: &Vault,
    path: &VaultPath,
    record: &MetadataRecord,
) -> Result<(), CoreError> {
    vault.write(path, &serde_json::to_vec_pretty(record)?)
}
/// Load a metadata cache record.
pub fn load_metadata(vault: &Vault, path: &VaultPath) -> Result<MetadataRecord, CoreError> {
    Ok(serde_json::from_slice(&vault.read(path)?)?)
}

/// Settings with unknown JSON keys retained across serialization.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Settings {
    #[serde(default)]
    pub toggles: BTreeMap<String, bool>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}
/// Persist settings as JSON atomically.
pub fn save_settings(
    vault: &Vault,
    path: &VaultPath,
    settings: &Settings,
) -> Result<(), CoreError> {
    vault.write(path, &serde_json::to_vec_pretty(settings)?)
}
/// Load settings, treating a missing file as defaults.
pub fn load_settings(vault: &Vault, path: &VaultPath) -> Result<Settings, CoreError> {
    match vault.read(path) {
        Ok(b) => Ok(serde_json::from_slice(&b)?),
        Err(CoreError::NotFound(_)) => Ok(Settings::default()),
        Err(e) => Err(e),
    }
}
/// Persist arbitrary workspace JSON atomically while retaining all fields.
pub fn save_workspace(vault: &Vault, path: &VaultPath, value: &Value) -> Result<(), CoreError> {
    vault.write(path, &serde_json::to_vec_pretty(value)?)
}
/// Load workspace JSON, returning an empty object when absent.
pub fn load_workspace(vault: &Vault, path: &VaultPath) -> Result<Value, CoreError> {
    match vault.read(path) {
        Ok(b) => Ok(serde_json::from_slice(&b)?),
        Err(CoreError::NotFound(_)) => Ok(Value::Object(Map::new())),
        Err(e) => Err(e),
    }
}

/// Stable built-in feature identifiers.
pub const CORE_FEATURES: &[&str] = &[
    "vault",
    "editor",
    "search",
    "graph",
    "backlinks",
    "file-explorer",
    "workspace",
    "commands",
    "settings",
    "daily-notes",
    "canvas",
    "templates",
    "properties",
    "markdown-preview",
];
/// Feature toggle registry.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct FeatureRegistry {
    #[serde(default)]
    enabled: BTreeMap<String, bool>,
}
impl FeatureRegistry {
    /// Defaults all known core features on.
    pub fn defaults() -> Self {
        Self {
            enabled: CORE_FEATURES
                .iter()
                .map(|x| (x.to_string(), true))
                .collect(),
        }
    }
    /// Set a feature state.
    pub fn set(&mut self, id: &str, on: bool) {
        self.enabled.insert(id.to_string(), on);
    }
    /// Query a feature (unknown features are disabled).
    pub fn is_enabled(&self, id: &str) -> bool {
        self.enabled.get(id).copied().unwrap_or(false)
    }
}
/// A command and the feature which gates it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Command {
    pub id: &'static str,
    pub feature: &'static str,
}
/// Commands available when their feature is enabled.
pub fn commands(registry: &FeatureRegistry) -> Vec<Command> {
    [
        Command {
            id: "vault.scan",
            feature: "vault",
        },
        Command {
            id: "file.create",
            feature: "file-explorer",
        },
        Command {
            id: "file.delete",
            feature: "file-explorer",
        },
        Command {
            id: "search.query",
            feature: "search",
        },
        Command {
            id: "workspace.save",
            feature: "workspace",
        },
        Command {
            id: "settings.save",
            feature: "settings",
        },
    ]
    .into_iter()
    .filter(|c| registry.is_enabled(c.feature))
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    #[test]
    fn paths_and_atomic_ops() {
        assert!(VaultPath::new("../x").is_err());
        assert!(VaultPath::new("/x").is_err());
        let d = tempdir().unwrap();
        let v = Vault::open(d.path()).unwrap();
        let p = VaultPath::new("a/n.md").unwrap();
        v.create(&p, b"one").unwrap();
        assert_eq!(v.read(&p).unwrap(), b"one");
        assert!(v.create(&p, b"two").is_err());
        v.write(&p, b"two").unwrap();
        v.rename(&p, &VaultPath::new("b.md").unwrap()).unwrap();
        let t = v.trash(&VaultPath::new("b.md").unwrap()).unwrap();
        assert!(t.as_path().starts_with(".trash"));
    }
    #[test]
    fn settings_unknown_round_trip() {
        let d = tempdir().unwrap();
        let v = Vault::open(d.path()).unwrap();
        let p = VaultPath::new("settings.json").unwrap();
        let s: Settings =
            serde_json::from_str(r#"{"toggles":{"x":false},"future":{"a":1}}"#).unwrap();
        save_settings(&v, &p, &s).unwrap();
        let got = load_settings(&v, &p).unwrap();
        assert_eq!(got.extra["future"]["a"], 1);
    }
    #[test]
    fn disabled_commands() {
        let mut r = FeatureRegistry::defaults();
        r.set("search", false);
        assert!(!commands(&r).iter().any(|x| x.id == "search.query"));
    }
}
