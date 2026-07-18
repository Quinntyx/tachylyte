use crate::{MediaIntent, MediaTokens};
use gpui::{div, prelude::*, Context, IntoElement, Render, Window};

/// Metadata needed to show a page in a thumbnail strip.  Rasterisation is
/// deliberately left to the host/native backend boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PdfThumbnail {
    pub page: u32,
    pub label: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PdfIntent {
    Zoom(f32),
    Rotate(i16),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PdfBackendState {
    Unavailable(String),
    Available,
}

pub struct PdfModel {
    pub source: String,
    pub title: String,
    pub current_page: u32,
    pub page_count: Option<u32>,
    pub thumbnails: Vec<PdfThumbnail>,
    pub zoom: f32,
    pub rotation: i16,
    pub backend: PdfBackendState,
    /// Light-theme tokens used by the placeholder surface.
    pub tokens: MediaTokens,
    intents: Vec<MediaIntent>,
    pdf_intents: Vec<PdfIntent>,
}
impl PdfModel {
    pub fn new(
        source: impl Into<String>,
        title: impl Into<String>,
        page_count: Option<u32>,
        backend: PdfBackendState,
    ) -> Self {
        let page_count = page_count.filter(|count| *count > 0);
        Self {
            source: source.into(),
            title: title.into(),
            current_page: 1,
            page_count,
            thumbnails: Vec::new(),
            zoom: 1.0,
            rotation: 0,
            backend,
            tokens: MediaTokens::light(),
            intents: Vec::new(),
            pdf_intents: Vec::new(),
        }
    }
    pub fn next_page(&mut self) {
        if self.page_count.is_none_or(|n| self.current_page < n) {
            self.current_page += 1;
            self.intents.push(MediaIntent::NextPage);
        }
    }
    pub fn previous_page(&mut self) {
        if self.current_page > 1 {
            self.current_page -= 1;
            self.intents.push(MediaIntent::PreviousPage);
        }
    }
    /// Select a page, clamping to the known document bounds.
    pub fn select_page(&mut self, page: u32) {
        let page = page.max(1);
        let page = self.page_count.map_or(page, |count| page.min(count.max(1)));
        if page != self.current_page {
            self.current_page = page;
        }
    }
    pub fn set_thumbnails(&mut self, thumbnails: Vec<PdfThumbnail>) {
        self.thumbnails = thumbnails
            .into_iter()
            .filter(|thumbnail| thumbnail.page >= 1)
            .collect();
        if let Some(count) = self.page_count {
            self.thumbnails
                .retain(|thumbnail| thumbnail.page >= 1 && thumbnail.page <= count);
        }
    }
    pub fn zoom(&mut self, zoom: f32) {
        self.zoom = if zoom.is_finite() {
            zoom.clamp(0.25, 4.0)
        } else {
            1.0
        };
        self.pdf_intents.push(PdfIntent::Zoom(self.zoom));
    }
    pub fn zoom_in(&mut self) {
        self.zoom(self.zoom * 1.25);
    }
    pub fn zoom_out(&mut self) {
        self.zoom(self.zoom / 1.25);
    }
    pub fn rotate(&mut self, degrees: i16) {
        let degrees = degrees.rem_euclid(360);
        self.rotation = (self.rotation + degrees).rem_euclid(360);
        self.pdf_intents.push(PdfIntent::Rotate(degrees));
    }
    pub fn take_pdf_intents(&mut self) -> Vec<PdfIntent> {
        std::mem::take(&mut self.pdf_intents)
    }
    pub fn open_external(&mut self) {
        self.intents
            .push(MediaIntent::OpenExternal(self.source.clone()));
    }
    pub fn take_intents(&mut self) -> Vec<MediaIntent> {
        std::mem::take(&mut self.intents)
    }
}
pub struct PdfView {
    pub model: PdfModel,
}
impl PdfView {
    pub fn new(model: PdfModel) -> Self {
        Self { model }
    }
}
impl Render for PdfView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let reason = match &self.model.backend {
            PdfBackendState::Unavailable(r) => format!("PDF backend unavailable: {r}"),
            PdfBackendState::Available => "PDF rendering is not enabled".into(),
        };
        div()
            .size_full()
            .bg(self.model.tokens.background)
            .text_color(self.model.tokens.foreground)
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .child(div().text_lg().child(self.model.title.clone()))
            .child(div().mt_2().child(reason))
            .child(
                div()
                    .mt_2()
                    .px_2()
                    .border_1()
                    .border_color(self.model.tokens.palette.borders.default)
                    .child("‹ Previous   ·   Page toolbar   ·   Next ›"),
            )
            .child(div().mt_2().text_sm().child(format!(
                    "Page {}{}",
                    self.model.current_page,
                    self.model
                        .page_count
                        .map(|n| format!(" of {n}"))
                        .unwrap_or_default()
                )))
            .child(div().mt_2().text_sm().child(format!(
                "Zoom {}% · Rotation {}° · {} thumbnail{}",
                (self.model.zoom * 100.0) as u32,
                self.model.rotation,
                self.model.thumbnails.len(),
                if self.model.thumbnails.len() == 1 {
                    ""
                } else {
                    "s"
                }
            )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unavailable_backend_and_page_toolbar_are_observable() {
        let mut model = PdfModel::new(
            "docs/guide.pdf",
            "Guide",
            Some(2),
            PdfBackendState::Unavailable("no PDF renderer configured".into()),
        );
        assert!(matches!(model.backend, PdfBackendState::Unavailable(_)));
        model.next_page();
        assert_eq!(model.current_page, 2);
        assert_eq!(model.take_intents(), vec![MediaIntent::NextPage]);
        model.next_page();
        assert!(model.take_intents().is_empty());
    }

    #[test]
    fn selection_zoom_rotation_and_thumbnails_are_bounded() {
        let mut model = PdfModel::new("guide.pdf", "Guide", Some(3), PdfBackendState::Available);
        model.set_thumbnails(vec![
            PdfThumbnail {
                page: 1,
                label: "one".into(),
            },
            PdfThumbnail {
                page: 4,
                label: "bad".into(),
            },
        ]);
        model.select_page(99);
        model.zoom(99.0);
        model.rotate(450);
        assert_eq!(model.current_page, 3);
        assert_eq!(model.thumbnails.len(), 1);
        assert_eq!(model.zoom, 4.0);
        assert_eq!(model.rotation, 90);
        assert_eq!(
            model.take_pdf_intents(),
            vec![PdfIntent::Zoom(4.0), PdfIntent::Rotate(90)]
        );
    }
}
