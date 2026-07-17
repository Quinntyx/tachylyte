use crate::{MediaIntent, MediaTokens};
use gpui::{div, prelude::*, Context, IntoElement, Render, Window};

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
    pub backend: PdfBackendState,
    /// Light-theme tokens used by the placeholder surface.
    pub tokens: MediaTokens,
    intents: Vec<MediaIntent>,
}
impl PdfModel {
    pub fn new(
        source: impl Into<String>,
        title: impl Into<String>,
        page_count: Option<u32>,
        backend: PdfBackendState,
    ) -> Self {
        Self {
            source: source.into(),
            title: title.into(),
            current_page: 1,
            page_count,
            backend,
            tokens: MediaTokens::light(),
            intents: Vec::new(),
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
}
