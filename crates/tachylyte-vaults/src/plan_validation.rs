//! Validation helpers for plans which create a vault directory.
//!
//! This module intentionally does not depend on the crate's public error type;
//! callers can map [`PlanValidationError`] into their own API error.

use std::{
    fs, io,
    path::{Component, Path, PathBuf},
};

#[derive(Debug)]
pub(crate) enum PlanValidationError {
    InvalidName,
    InvalidParent,
    InvalidPath,
    Io(io::Error),
}

/// Return whether `name` is a single, safe directory-name component.
pub(crate) fn valid_plan_name(name: &str) -> bool {
    !name.trim().is_empty()
        && name != "."
        && name != ".."
        && !name.chars().any(char::is_control)
        && !name.contains('/')
        && !name.contains('\\')
}

/// Return whether `parent` currently exists and is a directory.
pub(crate) fn valid_plan_parent(parent: &Path) -> bool {
    parent.is_dir()
}

/// Validate a path selected for opening an existing vault.
///
/// This deliberately does not canonicalize the path: callers can use the
/// original spelling in the UI, while the directory check still follows
/// symlinks in the same way as the eventual open operation.
pub(crate) fn validate_open_vault_path(path: &Path) -> Result<(), PlanValidationError> {
    if path.is_dir() {
        Ok(())
    } else {
        Err(PlanValidationError::InvalidPath)
    }
}

/// Return the conventional Obsidian configuration directory when present.
/// No directory or file is created by this helper.
pub(crate) fn discover_obsidian_metadata(path: &Path) -> io::Result<Option<PathBuf>> {
    validate_open_vault_path(path).map_err(|_| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "vault path is not an existing directory",
        )
    })?;
    let metadata = path.join(".obsidian");
    Ok(metadata.is_dir().then_some(metadata))
}

/// A filesystem-free description of the optional Welcome seed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct WelcomeSeedPlan {
    pub path: PathBuf,
    pub content: &'static str,
}

pub(crate) const WELCOME_SEED: &str = "# Welcome to Tachylyte\n\nTachylyte is a local Markdown workspace.\n\n## Getting started\n\n- Create notes and folders from the workspace shell.\n- Edit Markdown in the source view.\n- Use search to find notes in this vault.\n\nYour notes stay on disk in this vault directory.\n";

/// Plan a Welcome note only when the target does not already exist.
pub(crate) fn plan_welcome_seed(root: &Path) -> io::Result<Option<WelcomeSeedPlan>> {
    validate_open_vault_path(root).map_err(|_| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "vault path is not an existing directory",
        )
    })?;
    let path = root.join("Welcome.md");
    if path.exists() {
        Ok(None)
    } else {
        Ok(Some(WelcomeSeedPlan {
            path,
            content: WELCOME_SEED,
        }))
    }
}

/// Read Obsidian's app metadata without mutating the vault.
pub(crate) fn read_obsidian_app_metadata(path: &Path) -> io::Result<Option<Vec<u8>>> {
    let Some(metadata) = discover_obsidian_metadata(path)? else {
        return Ok(None);
    };
    let app = metadata.join("app.json");
    match fs::read(app) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

/// Validate the name, parent, and destination path of a create-vault plan.
///
/// Relative inputs are resolved against the current working directory and
/// normalized lexically (without filesystem canonicalization).  The
/// destination must then be exactly the normalized `parent.join(name)`.
pub(crate) fn validate_create_vault_plan(
    name: &str,
    parent: &Path,
    path: &Path,
) -> Result<(), PlanValidationError> {
    if !valid_plan_name(name) {
        return Err(PlanValidationError::InvalidName);
    }
    if !valid_plan_parent(parent) {
        return Err(PlanValidationError::InvalidParent);
    }

    let normalized_parent = lexical_absolute(parent).map_err(PlanValidationError::Io)?;
    let normalized_path = lexical_absolute(path).map_err(PlanValidationError::Io)?;
    if normalized_path != normalized_parent.join(name) {
        return Err(PlanValidationError::InvalidPath);
    }
    Ok(())
}

/// Make an absolute path and remove `.`/`..` components without touching the
/// filesystem.  `..` cannot escape the root/prefix, so the resulting path
/// remains within the same absolute path namespace.
pub(crate) fn lexical_absolute(path: &Path) -> io::Result<PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    let mut output = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                output.pop();
            }
            _ => output.push(component.as_os_str()),
        }
    }
    Ok(output)
}
