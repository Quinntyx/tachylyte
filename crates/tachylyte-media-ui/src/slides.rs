use crate::{IntentQueue, LeafState, MediaIntent, MediaTokens};
use gpui::{div, prelude::*, px, rgb, Context, Render, Window};
use tachylyte_workflows::{parse_slides, Slide};

pub struct SlidesModel {
    pub state: LeafState,
    pub tokens: MediaTokens,
    pub slides: Vec<Slide>,
    pub current_page: usize,
    /// Whether the slide canvas should be presented without document chrome.
    pub presenter_mode: bool,
    /// Speaker notes for the currently selected slide (kept separate from the
    /// rendered slide content so notes never leak into the audience view).
    pub presenter_notes: Vec<String>,
    intents: IntentQueue,
}
impl SlidesModel {
    pub fn new(slides: Vec<Slide>) -> Self {
        Self {
            state: LeafState::default(),
            tokens: MediaTokens::light(),
            slides,
            current_page: 0,
            presenter_mode: false,
            presenter_notes: Vec::new(),
            intents: IntentQueue::default(),
        }
    }
    pub fn from_markdown(markdown: &str) -> Self {
        let mut model = Self::new(parse_slides(markdown));
        model.presenter_notes = model
            .slides
            .iter()
            .map(|slide| Self::notes_for_slide(&slide.content))
            .collect();
        model
    }
    fn notes_for_slide(source: &str) -> String {
        source
            .lines()
            .filter_map(|line| line.trim().strip_prefix("<!--")?.strip_suffix("-->"))
            .map(str::trim)
            .filter_map(|line| {
                line.strip_prefix("notes:")
                    .or_else(|| line.strip_prefix("note:"))
            })
            .map(str::trim)
            .collect::<Vec<_>>()
            .join("\n")
    }
    pub fn toggle_presenter_mode(&mut self) {
        self.presenter_mode = !self.presenter_mode;
    }
    pub fn set_presenter_notes(&mut self, notes: impl Into<String>) {
        if self.presenter_notes.len() <= self.current_page {
            self.presenter_notes
                .resize(self.current_page + 1, String::new());
        }
        self.presenter_notes[self.current_page] = notes.into();
    }
    pub fn progress(&self) -> (usize, usize) {
        (
            self.current_page.saturating_add(1).min(self.slides.len()),
            self.slides.len(),
        )
    }
    pub fn current_notes(&self) -> Option<&str> {
        self.presenter_notes
            .get(self.current_page)
            .map(String::as_str)
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
        let padding = if self.model.presenter_mode { 8. } else { 3. };
        let mut body = div()
            .size_full()
            .flex()
            .flex_col()
            .gap_2()
            .p(px(padding))
            .bg(rgb(0xffffffff));
        if let Some(slide) = self.model.current() {
            if let Some(title) = &slide.title {
                body = body.child(div().text_color(rgb(0x222222ff)).child(title.clone()));
            }
            body = body.child(div().flex_1().child(slide.content.clone()));
        }
        let (page, total) = self.model.progress();
        let progress = (page * 100).checked_div(total).unwrap_or(0);
        let mut footer = div()
            .mt_2()
            .h(px(28.))
            .child(format!("Slide {page} / {total} ({progress}%)"));
        if self.model.presenter_mode {
            let notes = self
                .model
                .presenter_notes
                .get(self.model.current_page)
                .cloned()
                .unwrap_or_else(|| "Notes: none".to_owned());
            footer = footer.child(div().text_color(rgb(0x555555ff)).child(notes));
        }
        body.child(footer)
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
    #[test]
    fn presenter_notes_and_mode_are_local_and_safe() {
        let mut m = SlidesModel::from_markdown(
            "# A\n<!-- notes: explain A -->\n```\n---\n```\n---\n# B\n<!-- note: explain B -->",
        );
        assert_eq!(m.current_notes(), Some("explain A"));
        m.toggle_presenter_mode();
        assert!(m.presenter_mode);
        m.next_page();
        assert_eq!(m.current_notes(), Some("explain B"));
        assert_eq!(m.progress(), (2, 2));
        m.set_presenter_notes("updated");
        assert_eq!(m.current_notes(), Some("updated"));
    }

    #[test]
    fn empty_deck_has_zero_progress() {
        let m = SlidesModel::new(Vec::new());
        assert_eq!(m.progress(), (0, 0));
        assert_eq!(m.current_notes(), None);
    }
}
