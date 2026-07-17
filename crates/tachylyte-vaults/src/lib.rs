//! A small, filesystem-only registry for recently opened Tachylyte vaults.
//! The registry is deliberately UI and platform-launcher agnostic.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fmt, fs, io,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

pub type ExtraFields = BTreeMap<String, Value>;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct VaultEntry {
    pub id: String,
    pub name: String,
    pub path: String,
    pub last_opened: u64,
    #[serde(flatten)]
    pub extra: ExtraFields,
}

impl VaultEntry {
    pub fn status(&self) -> VaultStatus {
        status_for(Path::new(&self.path))
    }
    pub fn path_buf(&self) -> PathBuf {
        PathBuf::from(&self.path)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VaultStatus {
    Available,
    Stale,
    Missing,
}

fn status_for(path: &Path) -> VaultStatus {
    match fs::metadata(path) {
        Ok(m) if m.is_dir() => VaultStatus::Available,
        Ok(_) => VaultStatus::Stale,
        Err(_) => VaultStatus::Missing,
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
struct RegistryFile {
    #[serde(default)]
    vaults: Vec<VaultEntry>,
    #[serde(flatten)]
    extra: ExtraFields,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RevealIntent {
    pub path: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreateVaultPlan {
    pub name: String,
    pub parent: PathBuf,
    pub path: PathBuf,
}

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    Json(serde_json::Error),
    InvalidName,
    InvalidParent,
    DestinationExists(PathBuf),
    NotFound(String),
    DuplicatePath(PathBuf),
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "vault registry I/O error: {e}"),
            Error::Json(e) => write!(f, "invalid vault registry JSON: {e}"),
            Error::InvalidName => {
                f.write_str("vault name must be a non-empty single directory name")
            }
            Error::InvalidParent => f.write_str("vault parent must be a directory"),
            Error::DestinationExists(p) => {
                write!(f, "vault destination already exists: {}", p.display())
            }
            Error::NotFound(id) => write!(f, "vault not found: {id}"),
            Error::DuplicatePath(p) => write!(f, "vault is already registered: {}", p.display()),
        }
    }
}
impl std::error::Error for Error {}
impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}
impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e)
    }
}

pub struct VaultManager {
    config_path: PathBuf,
    file: RegistryFile,
}

impl VaultManager {
    /// Resolve the conventional per-user registry file without creating it.
    pub fn default_config_path() -> PathBuf {
        if let Some(p) = std::env::var_os("TACHYLYTE_CONFIG_HOME") {
            return PathBuf::from(p).join("vaults.json");
        }
        if let Some(p) = std::env::var_os("XDG_CONFIG_HOME") {
            return PathBuf::from(p).join("tachylyte/vaults.json");
        }
        if let Some(p) = std::env::var_os("APPDATA") {
            return PathBuf::from(p).join("Tachylyte/vaults.json");
        }
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".config/tachylyte/vaults.json")
    }

    pub fn open(path: impl Into<PathBuf>) -> Result<Self, Error> {
        let config_path = path.into();
        let file = if config_path.exists() {
            serde_json::from_slice(&fs::read(&config_path)?)?
        } else {
            RegistryFile::default()
        };
        let mut manager = Self { config_path, file };
        manager.dedupe_in_memory();
        Ok(manager)
    }
    pub fn open_default() -> Result<Self, Error> {
        Self::open(Self::default_config_path())
    }
    pub fn config_path(&self) -> &Path {
        &self.config_path
    }
    pub fn vaults(&self) -> &[VaultEntry] {
        &self.file.vaults
    }
    pub fn recent(&self) -> Vec<&VaultEntry> {
        let mut v: Vec<_> = self.vaults().iter().collect();
        v.sort_by_key(|x| std::cmp::Reverse(x.last_opened));
        v
    }
    pub fn default_vault(&self) -> Option<&VaultEntry> {
        self.recent()
            .into_iter()
            .find(|v| v.status() == VaultStatus::Available)
            .or_else(|| self.recent().into_iter().next())
    }
    pub fn persist(&self) -> Result<(), Error> {
        if let Some(parent) = self.config_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let bytes = serde_json::to_vec_pretty(&self.file)?;
        let tmp = self.config_path.with_extension("json.tmp");
        fs::write(&tmp, bytes)?;
        fs::rename(&tmp, &self.config_path)?;
        Ok(())
    }
    pub fn add(
        &mut self,
        name: impl Into<String>,
        path: impl AsRef<Path>,
    ) -> Result<VaultEntry, Error> {
        let path = canonical_or_normalized(path.as_ref())?;
        if let Some(old) = self
            .file
            .vaults
            .iter_mut()
            .find(|v| same_path(Path::new(&v.path), &path))
        {
            old.last_opened = now();
            old.name = name.into();
            let out = old.clone();
            self.persist()?;
            return Ok(out);
        }
        let name = name.into();
        if name.trim().is_empty() {
            return Err(Error::InvalidName);
        }
        let entry = VaultEntry {
            id: new_id(&path),
            name,
            path: path.to_string_lossy().into_owned(),
            last_opened: now(),
            extra: ExtraFields::new(),
        };
        self.file.vaults.push(entry.clone());
        self.persist()?;
        Ok(entry)
    }
    pub fn import_folder(&mut self, path: impl AsRef<Path>) -> Result<VaultEntry, Error> {
        let p = path.as_ref();
        if !p.is_dir() {
            return Err(Error::InvalidParent);
        }
        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("Vault");
        self.add(name, p)
    }
    pub fn open_vault(&mut self, id: &str) -> Result<VaultEntry, Error> {
        let v = self
            .file
            .vaults
            .iter_mut()
            .find(|v| v.id == id)
            .ok_or_else(|| Error::NotFound(id.into()))?;
        v.last_opened = now();
        let out = v.clone();
        self.persist()?;
        Ok(out)
    }
    pub fn remove(&mut self, id: &str) -> Result<VaultEntry, Error> {
        let i = self
            .file
            .vaults
            .iter()
            .position(|v| v.id == id)
            .ok_or_else(|| Error::NotFound(id.into()))?;
        let out = self.file.vaults.remove(i);
        self.persist()?;
        Ok(out)
    }
    pub fn rename_display_name(
        &mut self,
        id: &str,
        name: impl Into<String>,
    ) -> Result<VaultEntry, Error> {
        let name = name.into();
        if name.trim().is_empty() {
            return Err(Error::InvalidName);
        }
        let v = self
            .file
            .vaults
            .iter_mut()
            .find(|v| v.id == id)
            .ok_or_else(|| Error::NotFound(id.into()))?;
        v.name = name;
        let out = v.clone();
        self.persist()?;
        Ok(out)
    }
    pub fn reveal(&self, id: &str) -> Result<RevealIntent, Error> {
        self.file
            .vaults
            .iter()
            .find(|v| v.id == id)
            .map(|v| RevealIntent { path: v.path_buf() })
            .ok_or_else(|| Error::NotFound(id.into()))
    }
    pub fn plan_create_vault(
        &self,
        name: impl Into<String>,
        parent: impl Into<PathBuf>,
    ) -> Result<CreateVaultPlan, Error> {
        let name = name.into();
        if !valid_name(&name) {
            return Err(Error::InvalidName);
        }
        let parent = parent.into();
        if !parent.is_dir() {
            return Err(Error::InvalidParent);
        }
        let path = parent.join(&name);
        if path.exists() {
            return Err(Error::DestinationExists(path));
        }
        Ok(CreateVaultPlan { name, parent, path })
    }
    pub fn execute_create_vault(&mut self, plan: &CreateVaultPlan) -> Result<VaultEntry, Error> {
        if plan.path.exists() {
            return Err(Error::DestinationExists(plan.path.clone()));
        }
        fs::create_dir(&plan.path)?;
        self.add(plan.name.clone(), &plan.path)
    }
    fn dedupe_in_memory(&mut self) {
        let mut out = Vec::new();
        for v in self.file.vaults.drain(..) {
            if !out
                .iter()
                .any(|x: &VaultEntry| same_path(Path::new(&x.path), Path::new(&v.path)))
            {
                out.push(v);
            }
        }
        self.file.vaults = out;
    }
}

fn valid_name(s: &str) -> bool {
    !s.trim().is_empty()
        && s != "."
        && s != ".."
        && !s.contains('/')
        && !s.contains('\\')
        && !s.chars().any(char::is_control)
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
fn same_path(a: &Path, b: &Path) -> bool {
    canonical_or_normalized(a).ok() == canonical_or_normalized(b).ok()
}
fn canonical_or_normalized(path: &Path) -> Result<PathBuf, Error> {
    if path.exists() {
        Ok(fs::canonicalize(path)?)
    } else {
        let abs = if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir()?.join(path)
        };
        Ok(abs.components().collect())
    }
}
fn new_id(path: &Path) -> String {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    path.hash(&mut h);
    now().hash(&mut h);
    format!("{:016x}", h.finish())
}
