//! Small, deterministic, local-first foundations for an Obsidian-compatible vault.
//!
//! All filesystem mutations are confined to the vault root and use an atomic
//! temporary-file-and-rename strategy where applicable.

#[cfg(target_os = "linux")]
use rustix::fs::{
    fsync, mkdirat, openat, openat2, renameat, renameat_with, unlinkat, AtFlags, Mode, OFlags,
    RenameFlags, ResolveFlags,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
#[cfg(target_os = "linux")]
use std::sync::Arc;
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
    #[error("unknown feature: {0}")]
    UnknownFeature(String),
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
    #[cfg(target_os = "linux")]
    root_cap: Arc<fs::File>,
}
impl Vault {
    /// Open (or create) a vault directory.
    pub fn open(root: impl AsRef<Path>) -> Result<Self, CoreError> {
        let root = root.as_ref().to_path_buf();
        fs::create_dir_all(&root).map_err(|e| io_at(&root, e))?;
        let root = fs::canonicalize(&root).map_err(|e| io_at(&root, e))?;
        #[cfg(target_os = "linux")]
        let root_cap = Arc::new(fs::File::open(&root).map_err(|e| io_at(&root, e))?);
        Ok(Self {
            root,
            #[cfg(target_os = "linux")]
            root_cap,
        })
    }
    /// Absolute canonical vault root.
    pub fn root(&self) -> &Path {
        &self.root
    }
    fn checked(&self, p: &VaultPath, write: bool) -> Result<PathBuf, CoreError> {
        // Linux operations are first anchored to an O_PATH-like capability and
        // openat2's BENEATH|NO_SYMLINKS resolution. This is not a process-wide
        // mutex: replacing a directory or symlink concurrently cannot redirect
        // the capability lookup outside the vault. Other platforms retain the
        // lexical/symlink checks below; callers should treat those platforms as
        // lacking hostile concurrent-directory guarantees.
        #[cfg(target_os = "linux")]
        {
            let relative = if write {
                p.as_path().parent().unwrap_or(Path::new("."))
            } else {
                p.as_path()
            };
            let flags = OFlags::PATH
                | if write {
                    OFlags::DIRECTORY
                } else {
                    OFlags::empty()
                };
            let checked = openat2(
                &*self.root_cap,
                relative,
                flags,
                Mode::empty(),
                ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS,
            );
            if let Err(e) = checked {
                if !(write && e.raw_os_error() == 2) {
                    return Err(io_at(
                        &self.root,
                        io::Error::from_raw_os_error(e.raw_os_error()),
                    ));
                }
            }
        }
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
            if canonical.strip_prefix(&self.root).is_err() {
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
        #[cfg(target_os = "linux")]
        {
            let p = self.checked(path, true)?;
            let parent = p.parent().unwrap();
            let (dir, name) = linux_parent_mut(self, path)?;
            let tmp = format!(".tachylyte-{}.tmp", unique_suffix());
            let fd = openat(
                &dir,
                &tmp,
                OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL,
                Mode::from_raw_mode(0o600),
            )
            .map_err(|e| io_at(parent, io::Error::from_raw_os_error(e.raw_os_error())))?;
            let mut file: fs::File = fd.into();
            use std::io::Write;
            file.write_all(data)
                .and_then(|_| file.sync_all())
                .map_err(|e| io_at(parent, e))?;
            renameat(&dir, &tmp, &dir, name)
                .map_err(|e| io_at(&p, io::Error::from_raw_os_error(e.raw_os_error())))?;
            fsync(&dir)
                .map_err(|e| io_at(parent, io::Error::from_raw_os_error(e.raw_os_error())))?;
            return Ok(());
        }
        #[cfg(not(target_os = "linux"))]
        {
            return Err(CoreError::Unsupported(
                "secure mutations require Linux openat2 in this build".into(),
            ));
        }
        #[allow(unreachable_code)]
        let p = self.checked(path, true)?;
        let parent = p.parent().unwrap();
        return Err(CoreError::Unsupported(
            "secure mutations require Linux openat2 in this build".into(),
        ));
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
        })?;
        sync_parent(parent);
        Ok(())
    }
    /// Atomically create a new file, failing if it already exists.
    pub fn create(&self, path: &VaultPath, data: &[u8]) -> Result<(), CoreError> {
        #[cfg(target_os = "linux")]
        {
            let p = self.checked(path, true)?;
            let parent = p.parent().unwrap();
            let (dir, name) = linux_parent_mut(self, path)?;
            let tmp = format!(".tachylyte-{}.tmp", unique_suffix());
            let fd = openat(
                &dir,
                &tmp,
                OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL,
                Mode::from_raw_mode(0o600),
            )
            .map_err(|e| io_at(parent, io::Error::from_raw_os_error(e.raw_os_error())))?;
            let mut file: fs::File = fd.into();
            use std::io::Write;
            file.write_all(data)
                .and_then(|_| file.sync_all())
                .map_err(|e| io_at(parent, e))?;
            renameat_with(&dir, &tmp, &dir, name, RenameFlags::NOREPLACE).map_err(|e| {
                if e.raw_os_error() == 17 {
                    CoreError::Duplicate(path.0.clone())
                } else {
                    io_at(&p, io::Error::from_raw_os_error(e.raw_os_error()))
                }
            })?;
            fsync(&dir)
                .map_err(|e| io_at(parent, io::Error::from_raw_os_error(e.raw_os_error())))?;
            return Ok(());
        }
        #[cfg(not(target_os = "linux"))]
        {
            return Err(CoreError::Unsupported(
                "secure mutations require Linux openat2 in this build".into(),
            ));
        }
        #[allow(unreachable_code)]
        let p = self.checked(path, true)?;
        let parent = p.parent().unwrap();
        return Err(CoreError::Unsupported(
            "secure mutations require Linux openat2 in this build".into(),
        ));
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
        let result = fs::hard_link(&tmp, &p).map_err(|e| {
            let _ = fs::remove_file(&tmp);
            if e.kind() == io::ErrorKind::AlreadyExists {
                CoreError::Duplicate(path.0.clone())
            } else {
                io_at(&p, e)
            }
        });
        let _ = fs::remove_file(&tmp);
        if result.is_ok() {
            sync_parent(parent);
        }
        result
    }
    /// Rename a file or directory within this vault.
    pub fn rename(&self, from: &VaultPath, to: &VaultPath) -> Result<(), CoreError> {
        #[cfg(target_os = "linux")]
        {
            let a = self.checked(from, false)?;
            let b = self.checked(to, true)?;
            let _ = b.parent();
            let (src_dir, src_name) = linux_parent(self, from)?;
            let (dst_dir, dst_name) = linux_parent_mut(self, to)?;
            renameat_with(
                &src_dir,
                src_name,
                &dst_dir,
                dst_name,
                RenameFlags::NOREPLACE,
            )
            .map_err(|e| {
                if e.raw_os_error() == 17 {
                    CoreError::Duplicate(to.0.clone())
                } else {
                    io_at(&b, io::Error::from_raw_os_error(e.raw_os_error()))
                }
            })?;
            fsync(&src_dir)
                .map_err(|e| io_at(&a, io::Error::from_raw_os_error(e.raw_os_error())))?;
            fsync(&dst_dir)
                .map_err(|e| io_at(&b, io::Error::from_raw_os_error(e.raw_os_error())))?;
            return Ok(());
        }
        #[cfg(not(target_os = "linux"))]
        {
            return Err(CoreError::Unsupported(
                "secure mutations require Linux openat2 in this build".into(),
            ));
        }
        #[allow(unreachable_code)]
        let a = self.checked(from, false)?;
        let b = self.checked(to, true)?;
        if b.exists() {
            return Err(CoreError::Duplicate(to.0.clone()));
        }
        if let Some(parent) = b.parent() {
            return Err(CoreError::Unsupported(
                "secure mutations require Linux openat2 in this build".into(),
            ));
        }
        let metadata = fs::symlink_metadata(&a).map_err(|e| io_at(&a, e))?;
        if metadata.is_dir() {
            return Err(CoreError::Unsupported(
                "directory rename is unavailable without a platform no-replace primitive".into(),
            ));
        }
        fs::hard_link(&a, &b).map_err(|e| {
            if e.kind() == io::ErrorKind::AlreadyExists {
                CoreError::Duplicate(to.0.clone())
            } else {
                io_at(&b, e)
            }
        })?;
        fs::remove_file(&a).map_err(|e| {
            let _ = fs::remove_file(&b);
            io_at(&a, e)
        })?;
        sync_parent(a.parent().unwrap());
        sync_parent(b.parent().unwrap());
        Ok(())
    }
    /// Delete a vault entry permanently.
    pub fn delete(&self, path: &VaultPath) -> Result<(), CoreError> {
        #[cfg(target_os = "linux")]
        {
            let p = self.checked(path, false)?;
            let (dir, name) = linux_parent(self, path)?;
            let result = unlinkat(&dir, name, AtFlags::empty())
                .or_else(|_| unlinkat(&dir, name, AtFlags::REMOVEDIR));
            result.map_err(|e| io_at(&p, io::Error::from_raw_os_error(e.raw_os_error())))?;
            fsync(&dir).map_err(|e| io_at(&p, io::Error::from_raw_os_error(e.raw_os_error())))?;
            return Ok(());
        }
        #[cfg(not(target_os = "linux"))]
        {
            return Err(CoreError::Unsupported(
                "secure mutations require Linux openat2 in this build".into(),
            ));
        }
        #[allow(unreachable_code)]
        let p = self.checked(path, false)?;
        let m = fs::metadata(&p).map_err(|e| io_at(&p, e))?;
        if m.is_dir() {
            fs::remove_dir_all(&p)
        } else {
            fs::remove_file(&p)
        }
        .map_err(|e| io_at(&p, e))?;
        if let Some(parent) = p.parent() {
            sync_parent(parent);
        }
        Ok(())
    }
    /// Move an entry atomically into `.trash`, preserving its filename.
    pub fn trash(&self, path: &VaultPath) -> Result<VaultPath, CoreError> {
        #[cfg(target_os = "linux")]
        {
            let from = self.checked(path, false)?;
            let dir_path = self.root.join(".trash");
            if let Ok(meta) = fs::symlink_metadata(&dir_path) {
                if meta.file_type().is_symlink() {
                    return Err(CoreError::Symlink(dir_path));
                }
            }
            let name = path
                .as_path()
                .file_name()
                .ok_or_else(|| CoreError::InvalidPath(path.to_string()))?;
            let dest_path = PathBuf::from(".trash").join(name);
            let dest_vault_path = VaultPath::new(&dest_path).unwrap();
            let (src_dir, src_name) = linux_parent(self, path)?;
            let (dst_dir, dst_name) = linux_parent_mut(self, &dest_vault_path)?;
            renameat_with(
                &src_dir,
                src_name,
                &dst_dir,
                dst_name,
                RenameFlags::NOREPLACE,
            )
            .map_err(|e| {
                if e.raw_os_error() == 17 {
                    CoreError::Duplicate(dest_path.clone())
                } else {
                    io_at(&from, io::Error::from_raw_os_error(e.raw_os_error()))
                }
            })?;
            fsync(&src_dir)
                .map_err(|e| io_at(&from, io::Error::from_raw_os_error(e.raw_os_error())))?;
            fsync(&dst_dir)
                .map_err(|e| io_at(&dir_path, io::Error::from_raw_os_error(e.raw_os_error())))?;
            return Ok(VaultPath::new(dest_path).unwrap());
        }
        #[cfg(not(target_os = "linux"))]
        {
            return Err(CoreError::Unsupported(
                "secure mutations require Linux openat2 in this build".into(),
            ));
        }
        #[allow(unreachable_code)]
        let from = self.checked(path, false)?;
        let dir = self.root.join(".trash");
        match fs::symlink_metadata(&dir) {
            Ok(m) if m.file_type().is_symlink() => return Err(CoreError::Symlink(dir)),
            Ok(m) if !m.is_dir() => {
                return Err(io_at(&dir, io::Error::from(io::ErrorKind::NotADirectory)))
            }
            Ok(_) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                fs::create_dir(&dir).map_err(|x| io_at(&dir, x))?
            }
            Err(e) => return Err(io_at(&dir, e)),
        }
        let name = path
            .as_path()
            .file_name()
            .ok_or_else(|| CoreError::InvalidPath(path.to_string()))?;
        let mut dest = dir.join(name);
        if dest.exists() {
            dest.set_file_name(format!("{}-{}", name.to_string_lossy(), unique_suffix()));
        }
        let metadata = fs::symlink_metadata(&from).map_err(|e| io_at(&from, e))?;
        if metadata.is_dir() {
            return Err(CoreError::Unsupported(
                "directory trash is unavailable without a platform no-replace primitive".into(),
            ));
        }
        fs::hard_link(&from, &dest).map_err(|e| {
            if e.kind() == io::ErrorKind::AlreadyExists {
                CoreError::Duplicate(dest.clone())
            } else {
                io_at(&dest, e)
            }
        })?;
        fs::remove_file(&from).map_err(|e| {
            let _ = fs::remove_file(&dest);
            io_at(&from, e)
        })?;
        sync_parent(from.parent().unwrap());
        sync_parent(dest.parent().unwrap());
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
        let m = fs::symlink_metadata(&p).map_err(|x| io_at(&p, x))?;
        if m.file_type().is_symlink() {
            continue;
        }
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
fn sync_parent(parent: &Path) {
    #[cfg(unix)]
    if let Ok(file) = fs::File::open(parent) {
        let _ = file.sync_all();
    }
}

#[cfg(target_os = "linux")]
fn linux_parent<'a>(
    vault: &Vault,
    path: &'a VaultPath,
) -> Result<(std::os::unix::io::OwnedFd, &'a Path), CoreError> {
    let parent = path
        .as_path()
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let name = path
        .as_path()
        .file_name()
        .ok_or_else(|| CoreError::InvalidPath(path.to_string()))?;
    let fd = openat2(
        &*vault.root_cap,
        parent,
        OFlags::RDONLY | OFlags::DIRECTORY,
        Mode::empty(),
        ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS,
    )
    .map_err(|e| io_at(parent, io::Error::from_raw_os_error(e.raw_os_error())))?;
    Ok((fd, Path::new(name)))
}

#[cfg(target_os = "linux")]
fn linux_parent_mut<'a>(
    vault: &Vault,
    path: &'a VaultPath,
) -> Result<(std::os::unix::io::OwnedFd, &'a Path), CoreError> {
    let mut dir = openat2(
        &*vault.root_cap,
        ".",
        OFlags::RDONLY | OFlags::DIRECTORY,
        Mode::empty(),
        ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS,
    )
    .map_err(|e| io_at(vault.root(), io::Error::from_raw_os_error(e.raw_os_error())))?;
    let mut components = path.as_path().components().peekable();
    while let Some(component) = components.next() {
        let name = component.as_os_str();
        if components.peek().is_none() {
            return Ok((dir, Path::new(name)));
        }
        match mkdirat(&dir, name, Mode::from_raw_mode(0o700)) {
            Ok(()) => {}
            Err(e) if e.raw_os_error() == 17 => {}
            Err(e) => {
                return Err(io_at(
                    path.as_path(),
                    io::Error::from_raw_os_error(e.raw_os_error()),
                ))
            }
        }
        dir = openat2(
            &dir,
            name,
            OFlags::RDONLY | OFlags::DIRECTORY,
            Mode::empty(),
            ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS,
        )
        .map_err(|e| {
            io_at(
                path.as_path(),
                io::Error::from_raw_os_error(e.raw_os_error()),
            )
        })?;
    }
    Err(CoreError::InvalidPath(path.to_string()))
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
    "metadata-cache",
    "attachments",
    "import-export",
    "themes",
    "plugins",
    "sync",
];
/// Feature toggle registry.
#[derive(Clone, Debug, Serialize)]
pub struct FeatureRegistry {
    enabled: BTreeMap<String, bool>,
}
impl Default for FeatureRegistry {
    fn default() -> Self {
        Self::defaults()
    }
}
impl<'de> Deserialize<'de> for FeatureRegistry {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Wire {
            enabled: Option<BTreeMap<String, bool>>,
        }
        let wire = Wire::deserialize(deserializer)?;
        let mut registry = Self::defaults();
        if let Some(values) = wire.enabled {
            for (id, state) in values {
                if Self::is_known(&id) {
                    registry.enabled.insert(id, state);
                }
            }
        }
        Ok(registry)
    }
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
    pub fn set(&mut self, id: &str, on: bool) -> Result<(), CoreError> {
        if !CORE_FEATURES.contains(&id) {
            return Err(CoreError::UnknownFeature(id.to_string()));
        }
        self.enabled.insert(id.to_string(), on);
        Ok(())
    }
    /// Return whether an identifier is a built-in feature.
    pub fn is_known(id: &str) -> bool {
        CORE_FEATURES.contains(&id)
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
        Command {
            id: "editor.open",
            feature: "editor",
        },
        Command {
            id: "graph.open",
            feature: "graph",
        },
        Command {
            id: "backlinks.list",
            feature: "backlinks",
        },
        Command {
            id: "daily-notes.open",
            feature: "daily-notes",
        },
        Command {
            id: "canvas.open",
            feature: "canvas",
        },
        Command {
            id: "templates.apply",
            feature: "templates",
        },
        Command {
            id: "properties.edit",
            feature: "properties",
        },
        Command {
            id: "markdown-preview.open",
            feature: "markdown-preview",
        },
        Command {
            id: "metadata-cache.refresh",
            feature: "metadata-cache",
        },
        Command {
            id: "attachments.open",
            feature: "attachments",
        },
        Command {
            id: "import-export.run",
            feature: "import-export",
        },
        Command {
            id: "themes.select",
            feature: "themes",
        },
        Command {
            id: "plugins.manage",
            feature: "plugins",
        },
        Command {
            id: "sync.run",
            feature: "sync",
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
        let occupied = VaultPath::new("occupied.md").unwrap();
        v.create(&occupied, b"keep").unwrap();
        assert!(matches!(
            v.rename(&p, &occupied),
            Err(CoreError::Duplicate(_))
        ));
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
        let mut got = load_settings(&v, &p).unwrap();
        assert_eq!(got.extra["future"]["a"], 1);
        got.extra.insert("new-future".into(), Value::Bool(true));
        save_settings(&v, &p, &got).unwrap();
        assert_eq!(load_settings(&v, &p).unwrap().extra["future"]["a"], 1);
        assert_eq!(load_settings(&v, &p).unwrap().extra["new-future"], true);
    }
    #[test]
    fn disabled_commands() {
        let mut r = FeatureRegistry::defaults();
        r.set("search", false).unwrap();
        assert!(!commands(&r).iter().any(|x| x.id == "search.query"));
    }

    #[test]
    fn registry_defaults_and_unknowns() {
        let r = FeatureRegistry::default();
        assert_eq!(CORE_FEATURES.len(), 20);
        assert!(CORE_FEATURES.iter().all(|id| r.is_enabled(id)));
        assert!(!r.is_enabled("not-a-feature"));
        let mut r = r;
        assert!(r.set("not-a-feature", true).is_err());
        assert_eq!(commands(&r).len(), CORE_FEATURES.len());
        let missing: FeatureRegistry = serde_json::from_str("{}").unwrap();
        assert!(missing.is_enabled("vault"));
        let with_unknown: FeatureRegistry =
            serde_json::from_str(r#"{"enabled":{"search":false,"future":true}}"#).unwrap();
        assert!(!with_unknown.is_enabled("search"));
        assert!(!with_unknown.is_enabled("future"));
    }

    #[cfg(unix)]
    #[test]
    fn scanner_and_trash_reject_symlinks() {
        use std::os::unix::fs::symlink;
        let d = tempdir().unwrap();
        let outside = tempdir().unwrap();
        fs::write(outside.path().join("outside.md"), b"x").unwrap();
        let v = Vault::open(d.path()).unwrap();
        symlink(outside.path(), d.path().join("linked")).unwrap();
        assert!(v.scan().unwrap().is_empty());
        symlink(outside.path(), d.path().join(".trash")).unwrap();
        let p = VaultPath::new("inside.md").unwrap();
        v.create(&p, b"x").unwrap();
        assert!(matches!(v.trash(&p), Err(CoreError::Symlink(_))));
    }
}
