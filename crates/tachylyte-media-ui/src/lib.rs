//! Shared foundation and contracts for Tachylyte's native media leaves.

use std::collections::VecDeque;

/// Actions emitted by media leaves and consumed by their host.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MediaIntent {
    /// Open a source in the host's external application.
    OpenExternal(String),
    /// Start playback.
    Play,
    /// Pause playback.
    Pause,
    /// Seek to a position, expressed in milliseconds.
    Seek(u64),
    /// Advance to the next page or slide.
    NextPage,
    /// Return to the previous page or slide.
    PreviousPage,
}

/// A FIFO collection of intents produced by a media view.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IntentQueue(VecDeque<MediaIntent>);

impl IntentQueue {
    /// Add an intent to the end of the queue.
    pub fn push(&mut self, intent: MediaIntent) {
        self.0.push_back(intent);
    }

    /// Remove and return the oldest queued intent.
    pub fn take(&mut self) -> Option<MediaIntent> {
        self.0.pop_front()
    }
}

/// Common metadata and interactions for the chrome surrounding a media leaf.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LeafChrome {
    /// Human-readable leaf title.
    pub title: String,
    /// Source URI or vault-relative path.
    pub source: String,
    /// Intents waiting to be handled by the host.
    pub queued_intents: IntentQueue,
}

/// Mutable state shared by media leaf implementations.
pub type LeafState = LeafChrome;

/// Native light-theme tokens used by media views.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MediaTokens {
    /// The complete Tachylyte palette.
    pub palette: &'static tachylyte_theme::Palette,
    /// Background for the media surface.
    pub background: gpui::Hsla,
    /// Primary foreground color.
    pub foreground: gpui::Hsla,
    /// Accent used for controls and progress.
    pub accent: gpui::Hsla,
}

impl MediaTokens {
    /// Construct tokens matching the canonical native light theme.
    pub const fn light() -> Self {
        let palette = tachylyte_theme::light();
        Self {
            palette,
            background: palette.background.primary,
            foreground: palette.text.normal,
            accent: palette.accent,
        }
    }
}

pub mod attachment;
pub mod image;
pub mod pdf;
pub mod playback;
pub mod slides;
pub mod web;

pub use attachment::{AttachmentInfo, AttachmentModel, AttachmentView};
pub use image::{ImageFit, ImageMetadata, ImageModel, ImageView};
pub use pdf::{PdfBackendState, PdfModel, PdfView};
pub use playback::{AudioView, PlaybackKind, PlaybackModel, PlaybackView, VideoView};
pub use slides::{SlidesModel, SlidesView};
pub use web::{WebPolicy, WebView, WebViewModel};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intents_are_fifo() {
        let mut queue = IntentQueue::default();
        queue.push(MediaIntent::Play);
        queue.push(MediaIntent::Seek(42));
        assert_eq!(queue.take(), Some(MediaIntent::Play));
        assert_eq!(queue.take(), Some(MediaIntent::Seek(42)));
        assert_eq!(queue.take(), None);
    }

    #[test]
    fn light_tokens_use_theme_palette() {
        let tokens = MediaTokens::light();
        assert_eq!(tokens.palette, tachylyte_theme::light());
        assert_eq!(tokens.accent, tokens.palette.accent);
    }
}
