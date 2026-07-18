//! File-kind routing and a small GPUI mount point for document leaves.
//!
//! Routing is deliberately metadata-only. A [`DocumentView`] does not read a
//! file, fetch a URL, or launch an external process; the host owns those
//! capabilities and can mount the selected leaf when it has approved the
//! source.

use gpui::{div, prelude::*, AppContext, Context, Entity, Render, Window};
use std::path::Path;
use url::Url;

/// The leaf renderer selected for a document source.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FileKind {
    Markdown,
    Canvas,
    Image,
    Pdf,
    Audio,
    Video,
    Web,
    Attachment,
}

/// Route a safe local path or HTTP(S) URL without touching the filesystem or
/// network. Unknown extensions intentionally use the attachment surface.
pub fn file_kind(source: &str) -> FileKind {
    if is_safe_web_url(source) {
        return FileKind::Web;
    }
    if source.chars().any(char::is_control) || source.contains("://") {
        return FileKind::Attachment;
    }

    match Path::new(source)
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("md") | Some("markdown") => FileKind::Markdown,
        Some("canvas") => FileKind::Canvas,
        Some("png") | Some("jpg") | Some("jpeg") | Some("gif") | Some("svg") | Some("webp") => {
            FileKind::Image
        }
        Some("pdf") => FileKind::Pdf,
        Some("mp3") | Some("wav") | Some("m4a") | Some("ogg") => FileKind::Audio,
        Some("mp4") | Some("webm") | Some("mov") => FileKind::Video,
        _ => FileKind::Attachment,
    }
}

/// Alias emphasizing that this function is the host-facing file router.
pub fn route_file(source: &str) -> FileKind {
    file_kind(source)
}

fn is_safe_web_url(source: &str) -> bool {
    if source.chars().any(char::is_control) {
        return false;
    }
    let Ok(url) = Url::parse(source) else {
        return false;
    };
    matches!(url.scheme(), "http" | "https")
        && url.host().is_some()
        && url.username().is_empty()
        && url.password().is_none()
        && url.port().is_none()
}

/// A mountable, intentionally lightweight document shell.
pub struct DocumentView {
    pub source: String,
    pub kind: FileKind,
    pub title: String,
}

impl DocumentView {
    pub fn new(source: impl Into<String>) -> Self {
        let source = source.into();
        let title = Path::new(&source)
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| !name.is_empty())
            .unwrap_or(&source)
            .to_owned();
        Self {
            kind: file_kind(&source),
            source,
            title,
        }
    }

    /// Construct an entity that a workspace can place directly in its layout.
    pub fn mount<T>(source: impl Into<String>, cx: &mut Context<T>) -> Entity<Self> {
        let source = source.into();
        cx.new(|_| Self::new(source))
    }

    pub fn set_source(&mut self, source: impl Into<String>) {
        let next = Self::new(source);
        self.source = next.source;
        self.kind = next.kind;
        self.title = next.title;
    }

    fn boundary_label(&self) -> &'static str {
        match self.kind {
            FileKind::Markdown => "Markdown host/editor boundary",
            FileKind::Canvas => "Canvas host boundary",
            FileKind::Image => "GPUI image renderer",
            FileKind::Pdf => "PDF native backend boundary",
            FileKind::Audio => "Audio host playback boundary",
            FileKind::Video => "Video host playback boundary",
            FileKind::Web => "Web host policy boundary",
            FileKind::Attachment => "Attachment / external-app boundary",
        }
    }
}

impl Render for DocumentView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .gap_1()
            .p_3()
            .child(div().text_lg().child(self.title.clone()))
            .child(div().text_sm().child(format!("Kind: {:?}", self.kind)))
            .child(div().text_sm().child(self.source.clone()))
            .child(div().text_sm().child(self.boundary_label()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_supported_local_kinds_case_insensitively() {
        assert_eq!(file_kind("vault/PHOTO.JpG"), FileKind::Image);
        assert_eq!(file_kind("vault/clip.MP4"), FileKind::Video);
        assert_eq!(file_kind("vault/guide.pdf"), FileKind::Pdf);
        assert_eq!(file_kind("vault/readme.md"), FileKind::Markdown);
        assert_eq!(file_kind("vault/data.bin"), FileKind::Attachment);
    }

    #[test]
    fn routes_only_credential_free_http_urls_to_web() {
        assert_eq!(file_kind("https://example.test/docs"), FileKind::Web);
        assert_eq!(
            file_kind("https://user@example.test/docs"),
            FileKind::Attachment
        );
        assert_eq!(file_kind("javascript:alert(1)"), FileKind::Attachment);
        assert_eq!(
            file_kind("https://example.test:8443/docs"),
            FileKind::Attachment
        );
    }
}
