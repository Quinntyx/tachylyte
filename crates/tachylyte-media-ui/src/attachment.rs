use crate::{IntentQueue, LeafState, MediaIntent, MediaTokens};
use gpui::{div, prelude::*, Context, Render, Window};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AttachmentInfo {
    pub display_name: String,
    pub source: String,
    pub mime: String,
    pub byte_size: u64,
    pub modified: Option<String>,
}

pub struct AttachmentModel {
    pub state: LeafState,
    pub tokens: MediaTokens,
    pub info: AttachmentInfo,
    intents: IntentQueue,
}

impl AttachmentModel {
    pub fn new(info: AttachmentInfo) -> Self {
        Self {
            state: LeafState::default(),
            tokens: MediaTokens::light(),
            info,
            intents: IntentQueue::default(),
        }
    }
    pub fn open_external(&mut self) {
        self.intents
            .push(MediaIntent::OpenExternal(self.info.source.clone()));
    }
    pub fn take_intents(&mut self) -> Vec<MediaIntent> {
        std::iter::from_fn(|| self.intents.take()).collect()
    }
}

pub struct AttachmentView {
    pub model: AttachmentModel,
}
impl AttachmentView {
    pub fn new(model: AttachmentModel) -> Self {
        Self { model }
    }
}

impl Render for AttachmentView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let info = &self.model.info;
        div()
            .flex()
            .flex_col()
            .gap_1()
            .p_3()
            .bg(gpui::rgb(0xffffffff))
            .child(div().text_lg().child(info.display_name.clone()))
            .child(
                div()
                    .text_sm()
                    .child(format!("{} · {} bytes", info.mime, info.byte_size)),
            )
            .child(div().text_sm().child(info.source.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn metadata_and_external_intent() {
        let info = AttachmentInfo {
            display_name: "note.pdf".into(),
            source: "files/note.pdf".into(),
            mime: "application/pdf".into(),
            byte_size: 42,
            modified: Some("today".into()),
        };
        let mut m = AttachmentModel::new(info.clone());
        assert_eq!(m.info, info);
        m.open_external();
        assert_eq!(
            m.take_intents(),
            vec![MediaIntent::OpenExternal("files/note.pdf".into())]
        );
    }
}
