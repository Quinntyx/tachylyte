use crate::{IntentQueue, LeafState, MediaIntent, MediaTokens};
use gpui::{div, prelude::*, px, rgb, Context, Render, Window};
use tachylyte_workflows::{parse_slides, Slide};

pub struct SlidesModel {
    pub state: LeafState,
    pub tokens: MediaTokens,
    pub slides: Vec<Slide>,
    pub current_page: usize,
    intents: IntentQueue,
}
impl SlidesModel {
    pub fn new(slides: Vec<Slide>) -> Self {
        Self {
            state: LeafState::default(),
            tokens: MediaTokens::light(),
            slides,
            current_page: 0,
            intents: IntentQueue::default(),
        }
    }
    pub fn from_markdown(markdown: &str) -> Self {
        Self::new(parse_slides(markdown))
    }
    pub fn next_page(&mut self) {
        if self.current_page + 1 < self.slides.len() {
            self.current_page += 1;
            self.intents.push(MediaIntent::NextPage);
        }
    }
    pub fn previous_page(&mut self) {
        if self.current_page > 0 {
            self.current_page -= 1;
            self.intents.push(MediaIntent::PreviousPage);
        }
    }
    pub fn take_intents(&mut self) -> Vec<MediaIntent> {
        std::iter::from_fn(|| self.intents.take()).collect()
    }
    pub fn current(&self) -> Option<&Slide> {
        self.slides.get(self.current_page)
    }
}
pub struct SlidesView {
    pub model: SlidesModel,
}
impl SlidesView {
    pub fn new(model: SlidesModel) -> Self {
        Self { model }
    }
}
impl Render for SlidesView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let mut body = div().flex().flex_col().gap_2().p_3().bg(rgb(0xffffffff));
        if let Some(slide) = self.model.current() {
            if let Some(title) = &slide.title {
                body = body.child(div().text_color(rgb(0x222222ff)).child(title.clone()));
            }
            body = body.child(div().child(slide.content.clone()));
        }
        body.child(div().mt_2().h(px(28.)).child(format!(
            "Slide {} / {}",
            self.model.current_page + 1,
            self.model.slides.len()
        )))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn paging_is_bounded_and_emits() {
        let mut m = SlidesModel::from_markdown("# A\n---\n# B");
        m.next_page();
        assert_eq!(m.current_page, 1);
        assert_eq!(m.take_intents(), vec![MediaIntent::NextPage]);
        m.next_page();
        assert_eq!(m.current_page, 1);
        m.previous_page();
        assert_eq!(m.take_intents(), vec![MediaIntent::PreviousPage]);
    }
}
