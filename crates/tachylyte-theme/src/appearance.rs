//! Versioned persistence helpers for [`crate::AppearanceSettings`].

use crate::{AppearanceSettings, ThemeKind};
use serde_json::{Map, Value};

/// Version of the persisted appearance document emitted by [`encode`].
pub const CURRENT_VERSION: u32 = 1;

/// Encode settings as a small, stable JSON document.
pub fn encode(settings: &AppearanceSettings) -> String {
    let mut document = Map::new();
    document.insert("version".into(), CURRENT_VERSION.into());
    document.insert("theme".into(), theme_name(settings.theme).into());
    document.insert("font_size".into(), settings.font_size.into());
    document.insert("interface_scale".into(), settings.interface_scale.into());
    document.insert("reduced_motion".into(), settings.reduced_motion.into());
    Value::Object(document).to_string()
}

/// Decode a persisted document, migrating old fields and defaulting malformed or
/// missing values. Unknown fields and future versions are intentionally ignored.
pub fn decode(document: &str) -> AppearanceSettings {
    let mut settings = AppearanceSettings::default();
    let Ok(Value::Object(document)) = serde_json::from_str::<Value>(document) else {
        return settings;
    };
    if let Some(value) = document
        .get("theme")
        .and_then(Value::as_str)
        .or_else(|| document.get("name").and_then(Value::as_str))
    {
        settings.theme = parse_theme(value);
    } else if let Some(value) = document.get("dark").and_then(Value::as_bool) {
        settings.theme = if value {
            ThemeKind::Dark
        } else {
            ThemeKind::Light
        };
    }
    if let Some(value) = document
        .get("font_size")
        .and_then(Value::as_f64)
        .map(|v| v as f32)
    {
        if value.is_finite() && value > 0.0 {
            settings.font_size = value;
        }
    }
    if let Some(value) = document
        .get("interface_scale")
        .and_then(Value::as_f64)
        .map(|v| v as f32)
    {
        if value.is_finite() && value > 0.0 {
            settings.interface_scale = value;
        }
    }
    if let Some(value) = document.get("reduced_motion").and_then(Value::as_bool) {
        settings.reduced_motion = value;
    }
    settings
}

/// Alias for [`encode`], useful at persistence call sites.
pub fn encode_json(settings: &AppearanceSettings) -> String {
    encode(settings)
}

/// Alias for [`decode`], useful at persistence call sites.
pub fn decode_json(document: &str) -> AppearanceSettings {
    decode(document)
}

/// Encode settings using the explicit persistence-oriented name.
pub fn encode_appearance(settings: &AppearanceSettings) -> String {
    encode(settings)
}

/// Decode settings using the explicit persistence-oriented name.
pub fn decode_appearance(document: &str) -> AppearanceSettings {
    decode(document)
}

/// Migrate a serde-decoded legacy settings value into the current representation.
pub fn migrate(settings: AppearanceSettings) -> AppearanceSettings {
    settings
}

fn theme_name(theme: ThemeKind) -> &'static str {
    match theme {
        ThemeKind::Light => "Light",
        ThemeKind::Dark => "Dark",
        ThemeKind::System => "System",
    }
}

fn parse_theme(value: &str) -> ThemeKind {
    match value.to_ascii_lowercase().as_str() {
        "dark" => ThemeKind::Dark,
        "system" | "auto" => ThemeKind::System,
        _ => ThemeKind::Light,
    }
}
