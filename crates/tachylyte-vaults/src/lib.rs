//! A small, filesystem-only registry for recently opened Tachylyte vaults.
//! The registry is deliberately UI and platform-launcher agnostic.

mod atomic_persist;
mod plan_validation;

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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    selected: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    default: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    appearance: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    features: Option<Value>,
    #[serde(flatten)]
    extra: ExtraFields,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RevealIntent {
    pub path: PathBuf,
}

/// Records removed by stale pruning; retain this value to offer an undo action.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UndoPrune {
    pub removed: Vec<VaultEntry>,
    pub selected: Option<String>,
    pub default: Option<String>,
}

/// A validated existing directory. Constructing a plan never writes to disk.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenVaultPlan {
    pub path: PathBuf,
}

/// A filesystem-free plan for the optional Welcome note.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WelcomeSeedPlan {
    pub path: PathBuf,
    pub content: &'static str,
}

pub const WELCOME_SEED: &str = "# Welcome to Tachylyte\n\nTachylyte is a local Markdown workspace.\n\n## Getting started\n\n- Create notes and folders from the workspace shell.\n- Edit Markdown in the source view.\n- Use search to find notes in this vault.\n\nYour notes stay on disk in this vault directory.\n";

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
            parse_registry(&fs::read(&config_path)?)?
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
        if let Some(id) = self
            .file
            .default
            .as_deref()
            .or(self.file.selected.as_deref())
        {
            if let Some(v) = self.vaults().iter().find(|v| v.id == id) {
                return Some(v);
            }
        }
        self.recent()
            .into_iter()
            .find(|v| v.status() == VaultStatus::Available)
            .or_else(|| self.recent().into_iter().next())
    }
    pub fn selected_vault(&self) -> Option<&VaultEntry> {
        self.file
            .selected
            .as_deref()
            .and_then(|id| self.vaults().iter().find(|v| v.id == id))
    }
    pub fn selected_id(&self) -> Option<&str> {
        self.file.selected.as_deref()
    }
    pub fn set_default_vault(&mut self, id: &str) -> Result<(), Error> {
        if !self.file.vaults.iter().any(|v| v.id == id) {
            return Err(Error::NotFound(id.into()));
        }
        let mut c = self.file.clone();
        c.default = Some(id.into());
        c.selected = Some(id.into());
        self.commit(c)
    }
    pub fn set_selected_vault(&mut self, id: &str) -> Result<(), Error> {
        if !self.file.vaults.iter().any(|v| v.id == id) {
            return Err(Error::NotFound(id.into()));
        }
        let mut c = self.file.clone();
        c.selected = Some(id.into());
        self.commit(c)
    }
    /// Validate an existing directory for the Open action without mutating it.
    pub fn plan_open_vault(&self, path: impl Into<PathBuf>) -> Result<OpenVaultPlan, Error> {
        let path = path.into();
        if plan_validation::validate_open_vault_path(&path).is_err() {
            return Err(Error::InvalidParent);
        }
        Ok(OpenVaultPlan { path })
    }
    /// Register a previously validated Open plan.
    pub fn execute_open_vault(&mut self, plan: &OpenVaultPlan) -> Result<VaultEntry, Error> {
        if !plan.path.is_dir() {
            return Err(Error::InvalidParent);
        }
        self.import_folder(&plan.path)
    }
    /// Prepare the Welcome note without creating or overwriting a file.
    pub fn plan_welcome_seed(
        &self,
        root: impl AsRef<Path>,
    ) -> Result<Option<WelcomeSeedPlan>, Error> {
        let root = root.as_ref();
        let Some(plan) = plan_validation::plan_welcome_seed(root).map_err(Error::Io)? else {
            return Ok(None);
        };
        Ok(Some(WelcomeSeedPlan {
            path: plan.path,
            content: plan.content,
        }))
    }
    pub fn appearance(&self) -> Option<&Value> {
        self.file.appearance.as_ref()
    }
    pub fn feature_settings(&self) -> Option<&Value> {
        self.file.features.as_ref()
    }
    pub fn set_appearance(&mut self, value: Value) -> Result<(), Error> {
        let mut c = self.file.clone();
        c.appearance = Some(value);
        self.commit(c)
    }
    pub fn set_feature_settings(&mut self, value: Value) -> Result<(), Error> {
        let mut c = self.file.clone();
        c.features = Some(value);
        self.commit(c)
    }
    pub fn prune_stale(&mut self) -> Result<UndoPrune, Error> {
        let mut c = self.file.clone();
        let mut removed = Vec::new();
        let selected = c.selected.clone();
        let default = c.default.clone();
        c.vaults.retain(|v| {
            let keep = v.status() == VaultStatus::Available;
            if !keep {
                removed.push(v.clone());
            }
            keep
        });
        if let Some(id) = c.selected.as_deref() {
            if !c.vaults.iter().any(|v| v.id == id) {
                c.selected = None;
            }
        }
        if let Some(id) = c.default.as_deref() {
            if !c.vaults.iter().any(|v| v.id == id) {
                c.default = None;
            }
        }
        self.commit(c)?;
        Ok(UndoPrune {
            removed,
            selected,
            default,
        })
    }
    pub fn undo_prune(&mut self, undo: UndoPrune) -> Result<(), Error> {
        let mut c = self.file.clone();
        for v in undo.removed {
            if !c
                .vaults
                .iter()
                .any(|x| same_path(Path::new(&x.path), Path::new(&v.path)))
            {
                c.vaults.push(v);
            }
        }
        if let Some(id) = undo.selected {
            if c.vaults.iter().any(|v| v.id == id) {
                c.selected = Some(id);
            }
        }
        if let Some(id) = undo.default {
            if c.vaults.iter().any(|v| v.id == id) {
                c.default = Some(id);
            }
        }
        self.commit(c)
    }
    pub fn persist(&self) -> Result<(), Error> {
        self.persist_file(&self.file)
    }
    fn persist_file(&self, file: &RegistryFile) -> Result<(), Error> {
        if let Some(parent) = self.config_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let bytes = serde_json::to_vec_pretty(file)?;
        atomic_persist::persist_bytes(&self.config_path, &bytes)?;
        Ok(())
    }
    fn commit(&mut self, candidate: RegistryFile) -> Result<(), Error> {
        self.persist_file(&candidate)?;
        self.file = candidate;
        Ok(())
    }
    pub fn add(
        &mut self,
        name: impl Into<String>,
        path: impl AsRef<Path>,
    ) -> Result<VaultEntry, Error> {
        let name = name.into();
        if !valid_name(&name) {
            return Err(Error::InvalidName);
        }
        let path = canonical_or_normalized(path.as_ref())?;
        if self
            .file
            .vaults
            .iter()
            .find(|v| same_path(Path::new(&v.path), &path))
            .is_some()
        {
            let mut candidate = self.file.clone();
            let old = candidate
                .vaults
                .iter_mut()
                .find(|v| same_path(Path::new(&v.path), &path))
                .expect("duplicate was found in the source registry");
            old.last_opened = now();
            old.name = name;
            let out = old.clone();
            self.commit(candidate)?;
            return Ok(out);
        }
        let entry = VaultEntry {
            id: new_id(&path),
            name,
            path: path.to_string_lossy().into_owned(),
            last_opened: now(),
            extra: ExtraFields::new(),
        };
        let mut candidate = self.file.clone();
        candidate.vaults.push(entry.clone());
        self.commit(candidate)?;
        Ok(entry)
    }
    pub fn import_folder(&mut self, path: impl AsRef<Path>) -> Result<VaultEntry, Error> {
        let p = path.as_ref();
        if !p.is_dir() {
            return Err(Error::InvalidParent);
        }
        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("Vault");
        let mut entry = self.add(name, p)?;
        if p.join(".obsidian").is_dir() {
            let mut c = self.file.clone();
            if let Some(v) = c.vaults.iter_mut().find(|v| v.id == entry.id) {
                let metadata = plan_validation::read_obsidian_app_metadata(p)
                    .ok()
                    .flatten()
                    .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok());
                v.extra
                    .insert("obsidian".into(), metadata.unwrap_or(Value::Bool(true)));
                entry = v.clone();
            }
            self.commit(c)?;
        }
        Ok(entry)
    }
    pub fn open_vault(&mut self, id: &str) -> Result<VaultEntry, Error> {
        let mut candidate = self.file.clone();
        let v = candidate
            .vaults
            .iter_mut()
            .find(|v| v.id == id)
            .ok_or_else(|| Error::NotFound(id.into()))?;
        v.last_opened = now();
        candidate.selected = Some(id.to_owned());
        let out = v.clone();
        self.commit(candidate)?;
        Ok(out)
    }
    pub fn remove(&mut self, id: &str) -> Result<VaultEntry, Error> {
        let mut candidate = self.file.clone();
        let i = candidate
            .vaults
            .iter()
            .position(|v| v.id == id)
            .ok_or_else(|| Error::NotFound(id.into()))?;
        let out = candidate.vaults.remove(i);
        if candidate.selected.as_deref() == Some(id) {
            candidate.selected = None;
        }
        if candidate.default.as_deref() == Some(id) {
            candidate.default = None;
        }
        self.commit(candidate)?;
        Ok(out)
    }
    /// Restore a previously removed entry for an undo action.
    pub fn restore_entry(&mut self, entry: VaultEntry) -> Result<VaultEntry, Error> {
        if let Some(existing) = self
            .file
            .vaults
            .iter()
            .find(|v| same_path(Path::new(&v.path), Path::new(&entry.path)))
        {
            return Ok(existing.clone());
        }
        let mut candidate = self.file.clone();
        candidate.vaults.push(entry.clone());
        self.commit(candidate)?;
        Ok(entry)
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
        let mut candidate = self.file.clone();
        let v = candidate
            .vaults
            .iter_mut()
            .find(|v| v.id == id)
            .ok_or_else(|| Error::NotFound(id.into()))?;
        v.name = name;
        let out = v.clone();
        self.commit(candidate)?;
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
        validate_plan(plan)?;
        if plan.path.exists() {
            return Err(Error::DestinationExists(plan.path.clone()));
        }
        fs::create_dir(&plan.path)?;
        match self.add(plan.name.clone(), &plan.path) {
            Ok(entry) => Ok(entry),
            Err(error) => {
                let _ = fs::remove_dir(&plan.path);
                Err(error)
            }
        }
    }
    fn dedupe_in_memory(&mut self) {
        let mut out = Vec::new();
        for v in self.file.vaults.drain(..) {
            if let Some(existing) = out
                .iter_mut()
                .find(|x: &&mut VaultEntry| same_path(Path::new(&x.path), Path::new(&v.path)))
            {
                if self.file.selected.as_deref() == Some(&v.id) {
                    self.file.selected = Some(existing.id.clone());
                }
                if self.file.default.as_deref() == Some(&v.id) {
                    self.file.default = Some(existing.id.clone());
                }
                merge_entry(existing, v);
            } else {
                out.push(v);
            }
        }
        self.file.vaults = out;
    }
}

fn validate_plan(plan: &CreateVaultPlan) -> Result<(), Error> {
    use plan_validation::{validate_create_vault_plan, PlanValidationError};
    validate_create_vault_plan(&plan.name, &plan.parent, &plan.path).map_err(|error| match error {
        PlanValidationError::InvalidName => Error::InvalidName,
        PlanValidationError::InvalidParent => Error::InvalidParent,
        PlanValidationError::InvalidPath => Error::InvalidParent,
        PlanValidationError::Io(error) => Error::Io(error),
    })
}

fn merge_entry(existing: &mut VaultEntry, duplicate: VaultEntry) {
    // Keep the first record's identity and display values for deterministic
    // conflict resolution, but retain the freshest activity and all unknown
    // fields from either record. Existing extension values win on conflicts.
    existing.last_opened = existing.last_opened.max(duplicate.last_opened);
    for (key, value) in duplicate.extra {
        existing.extra.entry(key).or_insert(value);
    }
    if existing.name.trim().is_empty() {
        existing.name = duplicate.name;
    }
    if existing.id.trim().is_empty() {
        existing.id = duplicate.id;
    }
}

fn parse_registry(bytes: &[u8]) -> Result<RegistryFile, Error> {
    let value: Value = serde_json::from_slice(bytes)?;
    // Older releases wrote the vault list directly, or used recentVaults/
    // recent_vaults. Normalize those forms while preserving object extensions.
    let mut file = match value {
        Value::Array(vaults) => RegistryFile {
            vaults: parse_entries(vaults)?,
            ..Default::default()
        },
        Value::Object(mut object) => {
            for (alias, canonical) in [
                ("selectedVault", "selected"),
                ("selected_vault", "selected"),
                ("defaultVault", "default"),
                ("default_vault", "default"),
            ] {
                if !object.contains_key(canonical) {
                    if let Some(value) = object.remove(alias) {
                        object.insert(canonical.into(), value);
                    }
                }
            }
            if !object.contains_key("vaults") {
                for key in ["recentVaults", "recent_vaults", "recent"] {
                    if let Some(v) = object.get(key).cloned() {
                        object.insert("vaults".into(), v);
                        break;
                    }
                }
            }
            if !object.contains_key("appearance") {
                if let Some(settings) = object.get("settings").cloned() {
                    object.insert("appearance".into(), settings);
                }
            }
            if !object.contains_key("features") {
                if let Some(Value::Object(settings)) = object.get("settings") {
                    if let Some(features) = settings.get("features") {
                        object.insert("features".into(), features.clone());
                    }
                }
            }
            let entries = object.remove("vaults").unwrap_or(Value::Array(Vec::new()));
            let mut file: RegistryFile = serde_json::from_value(Value::Object(object))?;
            file.vaults = match entries {
                Value::Array(v) => parse_entries(v)?,
                _ => Vec::new(),
            };
            file
        }
        _ => RegistryFile::default(),
    };
    for v in &mut file.vaults {
        if v.id.trim().is_empty() {
            v.id = new_id(Path::new(&v.path));
        }
        if v.name.trim().is_empty() {
            v.name = Path::new(&v.path)
                .file_name()
                .and_then(|x| x.to_str())
                .unwrap_or("Vault")
                .into();
        }
    }
    Ok(file)
}

fn parse_entries(values: Vec<Value>) -> Result<Vec<VaultEntry>, Error> {
    values
        .into_iter()
        .map(|mut value| {
            if let Value::Object(ref mut o) = value {
                if !o.contains_key("path") {
                    if let Some(v) = o.remove("location").or_else(|| o.remove("folder")) {
                        o.insert("path".into(), v);
                    }
                }
                o.entry("id")
                    .or_insert_with(|| Value::String(String::new()));
                o.entry("name")
                    .or_insert_with(|| Value::String(String::new()));
                if !o.contains_key("last_opened") {
                    let opened = o.remove("lastOpened").unwrap_or(Value::Number(0.into()));
                    o.insert("last_opened".into(), opened);
                }
            }
            Ok(serde_json::from_value(value)?)
        })
        .collect()
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
    // FNV-1a is deliberately specified here instead of relying on a
    // language/runtime hasher whose seed or algorithm may change between
    // platforms and releases.  The path itself is canonicalized by callers
    // before reaching this function, so the ID is stable across reloads.
    let portable_path = canonical_or_normalized(path).unwrap_or_else(|_| path.to_path_buf());
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in portable_path.to_string_lossy().as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}
