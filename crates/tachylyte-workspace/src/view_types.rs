//! Render-neutral descriptions of views.
//!
//! The string representation intentionally matches the existing `View.kind`
//! convention, so this module can be introduced without changing persisted
//! workspace data.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

/// The built-in kinds of workspace view, or an application-defined kind.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum ViewKind {
    Markdown,
    Graph,
    Settings,
    Media,
    Custom(String),
}

impl ViewKind {
    pub const fn markdown() -> Self {
        Self::Markdown
    }

    pub const fn graph() -> Self {
        Self::Graph
    }

    pub const fn settings() -> Self {
        Self::Settings
    }

    pub const fn media() -> Self {
        Self::Media
    }

    /// Returns the legacy string kind used by [`crate::View`].
    pub fn as_kind_str(&self) -> &str {
        match self {
            Self::Markdown => "markdown",
            Self::Graph => "graph",
            Self::Settings => "settings",
            Self::Media => "media",
            Self::Custom(kind) => kind,
        }
    }

    pub fn to_kind_string(&self) -> String {
        self.as_kind_str().to_owned()
    }

    pub fn from_kind_str(kind: impl AsRef<str>) -> Self {
        match kind.as_ref() {
            "markdown" => Self::Markdown,
            "graph" => Self::Graph,
            "settings" => Self::Settings,
            "media" => Self::Media,
            other => Self::Custom(other.to_owned()),
        }
    }
}

impl From<&str> for ViewKind {
    fn from(kind: &str) -> Self {
        Self::from_kind_str(kind)
    }
}

impl From<String> for ViewKind {
    fn from(kind: String) -> Self {
        Self::from_kind_str(kind)
    }
}

impl Serialize for ViewKind {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_kind_str())
    }
}

impl<'de> Deserialize<'de> for ViewKind {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(Self::from_kind_str(String::deserialize(deserializer)?))
    }
}

/// A serializable view kind and its optional path-backed or state-backed data.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct ViewDescriptor {
    pub kind: ViewKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub state: Value,
}

impl ViewDescriptor {
    pub fn new(kind: impl Into<ViewKind>) -> Self {
        Self {
            kind: kind.into(),
            path: None,
            state: Value::Null,
        }
    }

    pub fn from_path(kind: impl Into<ViewKind>, path: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            path: Some(path.into()),
            state: Value::Null,
        }
    }

    pub fn from_state(kind: impl Into<ViewKind>, state: Value) -> Self {
        Self {
            kind: kind.into(),
            path: None,
            state,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtins_and_custom_use_string_kind_convention() {
        assert_eq!(ViewKind::from_kind_str("markdown"), ViewKind::Markdown);
        assert_eq!(ViewKind::Graph.to_kind_string(), "graph");
        assert_eq!(
            ViewKind::from("plugin.view"),
            ViewKind::Custom("plugin.view".into())
        );
    }

    #[test]
    fn descriptor_constructors_round_trip_through_json() {
        let descriptor = ViewDescriptor::from_path(ViewKind::Markdown, "notes/readme.md");
        let encoded = serde_json::to_string(&descriptor).unwrap();
        assert_eq!(encoded, r#"{"kind":"markdown","path":"notes/readme.md"}"#);
        assert_eq!(
            serde_json::from_str::<ViewDescriptor>(&encoded).unwrap(),
            descriptor
        );

        let state = ViewDescriptor::from_state(
            ViewKind::Settings,
            serde_json::json!({"section":"general"}),
        );
        assert_eq!(
            serde_json::from_str::<ViewDescriptor>(&serde_json::to_string(&state).unwrap())
                .unwrap(),
            state
        );
    }
}
