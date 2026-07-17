use gpui::{div, prelude::*, Context, IntoElement, Render, Window};

use crate::{MediaIntent, MediaTokens};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ImageFit {
    #[default]
    Contain,
    Cover,
    Actual,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ImageMetadata {
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub bytes: Option<u64>,
    pub mime: Option<String>,
}

pub struct ImageModel {
    pub source: String,
    pub title: String,
    pub tokens: MediaTokens,
    pub metadata: ImageMetadata,
    pub fit: ImageFit,
    pub zoom: f32,
    intents: Vec<MediaIntent>,
}

impl ImageModel {
    pub fn new(source: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            title: title.into(),
            tokens: MediaTokens::light(),
            metadata: ImageMetadata::default(),
            fit: ImageFit::Contain,
            zoom: 1.0,
            intents: Vec::new(),
        }
    }
    pub fn fit(&mut self, fit: ImageFit) {
        self.fit = fit;
    }
    pub fn zoom(&mut self, zoom: f32) {
        self.zoom = if zoom.is_finite() {
            zoom.clamp(0.1, 8.0)
        } else {
            1.0
        };
    }
    pub fn zoom_in(&mut self) {
        self.zoom(self.zoom * 1.25);
    }
    pub fn zoom_out(&mut self) {
        self.zoom(self.zoom / 1.25);
    }
    pub fn open_external(&mut self) {
        self.intents
            .push(MediaIntent::OpenExternal(self.source.clone()));
    }
    pub fn take_intents(&mut self) -> Vec<MediaIntent> {
        std::mem::take(&mut self.intents)
    }
}

pub struct ImageView {
    pub model: ImageModel,
}
impl ImageView {
    pub fn new(model: ImageModel) -> Self {
        Self { model }
    }
}

impl Render for ImageView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .bg(self.model.tokens.background)
            .text_color(self.model.tokens.foreground)
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .child(
                div()
                    .text_lg()
                    .child(format!("Image: {}", self.model.title)),
            )
            .child(div().mt_2().text_sm().child(format!(
                "[image preview unavailable] · {}% · {:?}",
                (self.model.zoom * 100.0) as u32,
                self.model.fit
            )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn zoom_is_bounded() {
        let mut m = ImageModel::new("x", "x");
        m.zoom(99.0);
        assert_eq!(m.zoom, 8.0);
        m.zoom(-1.0);
        assert_eq!(m.zoom, 0.1);
    }
    #[test]
    fn fit_and_intent() {
        let mut m = ImageModel::new("x", "x");
        m.fit(ImageFit::Cover);
        m.open_external();
        assert_eq!(m.fit, ImageFit::Cover);
        assert_eq!(m.take_intents().len(), 1);
    }
}
