//! Validation helpers for plans which create a vault directory.
//!
//! This module intentionally does not depend on the crate's public error type;
//! callers can map [`PlanValidationError`] into their own API error.

use std::{
    io,
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
