use crate::{IntentQueue, LeafState, MediaIntent, MediaTokens};
use gpui::{div, prelude::*, px, rgb, Context, Render, Window};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PlaybackKind {
    #[default]
    Audio,
    Video,
}

pub struct PlaybackModel {
    pub state: LeafState,
    pub tokens: MediaTokens,
    pub title: String,
    pub source: String,
    pub kind: PlaybackKind,
    pub playing: bool,
    pub position_ms: u64,
    pub duration_ms: u64,
    /// Output volume in the inclusive range 0.0..=1.0.
    pub volume: f32,
    /// Playback rate, mirrored to the host through [`MediaIntent::SetSpeed`].
    pub speed: f32,
    intents: IntentQueue,
}

impl PlaybackModel {
    pub fn new(title: impl Into<String>, source: impl Into<String>, kind: PlaybackKind) -> Self {
        Self {
            state: LeafState::default(),
            tokens: MediaTokens::light(),
            title: title.into(),
            source: source.into(),
            kind,
            playing: false,
            position_ms: 0,
            duration_ms: 0,
            volume: 1.0,
            speed: 1.0,
            intents: IntentQueue::default(),
        }
    }

    pub fn play(&mut self) {
        self.playing = true;
        self.intents.push(MediaIntent::Play);
    }
    pub fn pause(&mut self) {
        self.playing = false;
        self.intents.push(MediaIntent::Pause);
    }
    pub fn seek(&mut self, milliseconds: u64) {
        self.position_ms = milliseconds.min(self.duration_ms);
        self.intents.push(MediaIntent::Seek(self.position_ms));
    }

    pub fn set_volume(&mut self, volume: f32) {
        self.volume = if volume.is_nan() {
            0.0
        } else {
            volume.clamp(0.0, 1.0)
        };
        self.intents
            .push(MediaIntent::SetVolume((self.volume * 100.0).round() as u8));
    }

    pub fn set_speed(&mut self, speed: f32) {
        self.speed = if speed.is_nan() {
            1.0
        } else {
            speed.clamp(0.25, 4.0)
        };
        self.intents
            .push(MediaIntent::SetSpeed((self.speed * 100.0).round() as u16));
    }
    pub fn take_intents(&mut self) -> Vec<MediaIntent> {
        std::iter::from_fn(|| self.intents.take()).collect()
    }
}

pub struct PlaybackView {
    pub model: PlaybackModel,
}
impl PlaybackView {
    pub fn new(model: PlaybackModel) -> Self {
        Self { model }
    }
}
pub type AudioView = PlaybackView;
pub type VideoView = PlaybackView;

impl Render for PlaybackView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let kind = match self.model.kind {
            PlaybackKind::Audio => "Audio",
            PlaybackKind::Video => "Video",
        };
        div()
            .size_full()
            .flex()
            .flex_col()
            .gap_2()
            .p_3()
            .bg(rgb(0xffffffff))
            .child(
                div()
                    .text_lg()
                    .child(format!("{kind}: {}", self.model.title)),
            )
            .child(div().text_sm().child(self.model.source.clone()))
            .child(
                div()
                    .h(px(6.))
                    .bg(rgb(0xe5e7ebff))
                    .child(div().h(px(6.)).bg(rgb(0x4f8cff)).w(px(
                        if self.model.duration_ms == 0 {
                            0.
                        } else {
                            240. * self.model.position_ms as f32 / self.model.duration_ms as f32
                        },
                    ))),
            )
            .child(div().text_sm().child(format!(
                "Timeline: {} / {} ms",
                self.model.position_ms, self.model.duration_ms
            )))
            .child(div().text_sm().child(if self.model.playing {
                "Playing · Pause"
            } else {
                "Paused · Play"
            }))
            .child(div().text_sm().child(format!(
                "Volume: {:.0}% · Speed: {:.2}×",
                self.model.volume * 100.0,
                self.model.speed
            )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn controls_emit_intents() {
        let mut m = PlaybackModel::new("a", "x", PlaybackKind::Audio);
        m.play();
        m.pause();
        assert_eq!(
            m.take_intents(),
            vec![MediaIntent::Play, MediaIntent::Pause]
        );
    }
    #[test]
    fn seek_clamps() {
        let mut m = PlaybackModel::new("a", "x", PlaybackKind::Audio);
        m.duration_ms = 10;
        m.seek(99);
        assert_eq!(m.position_ms, 10);
        assert_eq!(m.take_intents(), vec![MediaIntent::Seek(10)]);
    }

    #[test]
    fn volume_and_speed_clamp_and_emit_intents() {
        let mut m = PlaybackModel::new("a", "x", PlaybackKind::Audio);
        m.set_volume(2.0);
        assert_eq!(m.volume, 1.0);
        m.set_volume(-1.0);
        assert_eq!(m.volume, 0.0);
        m.set_speed(9.0);
        assert_eq!(m.speed, 4.0);
        m.set_speed(0.0);
        assert_eq!(m.speed, 0.25);
        assert_eq!(
            m.take_intents(),
            vec![
                MediaIntent::SetVolume(100),
                MediaIntent::SetVolume(0),
                MediaIntent::SetSpeed(400),
                MediaIntent::SetSpeed(25)
            ]
        );
    }

    #[test]
    fn non_finite_controls_are_safe() {
        let mut m = PlaybackModel::new("a", "x", PlaybackKind::Audio);
        m.set_volume(f32::NAN);
        m.set_speed(f32::NAN);
        assert_eq!(m.volume, 0.0);
        assert_eq!(m.speed, 1.0);
    }
}
