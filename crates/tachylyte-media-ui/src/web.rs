use crate::{IntentQueue, LeafState, MediaIntent, MediaTokens};
use gpui::{div, prelude::*, px, rgb, Context, Render, Window};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WebPolicy {
    Offline,
    AllowList(Vec<String>),
}

impl WebPolicy {
    pub fn evaluate(&self, url: &str) -> Result<(), String> {
        let host = url
            .split("//")
            .nth(1)
            .and_then(|s| s.split('/').next())
            .unwrap_or("")
            .split(':')
            .next()
            .unwrap_or("");
        if host.is_empty() {
            return Err("URL has no host".into());
        }
        match self {
            Self::Offline => Err("offline policy blocks navigation".into()),
            Self::AllowList(list)
                if list
                    .iter()
                    .any(|allowed| host == allowed || host.ends_with(&format!(".{allowed}"))) =>
            {
                Ok(())
            }
            Self::AllowList(_) => Err(format!("host is not on the allow-list: {host}")),
        }
    }
    pub fn allows(&self, url: &str) -> bool {
        self.evaluate(url).is_ok()
    }
}

pub struct WebViewModel {
    pub state: LeafState,
    pub tokens: MediaTokens,
    pub policy: WebPolicy,
    pub url: String,
    pub blocked_reason: Option<String>,
    intents: IntentQueue,
}

impl WebViewModel {
    pub fn new(policy: WebPolicy) -> Self {
        Self {
            state: LeafState::default(),
            tokens: MediaTokens::light(),
            policy,
            url: String::new(),
            blocked_reason: None,
            intents: IntentQueue::default(),
        }
    }
    pub fn navigate(&mut self, url: impl Into<String>) {
        let url = url.into();
        self.url = url.clone();
        match self.policy.evaluate(&url) {
            Ok(()) => {
                self.blocked_reason = None;
                self.intents.push(MediaIntent::OpenExternal(url));
            }
            Err(reason) => self.blocked_reason = Some(reason),
        }
    }
    pub fn take_intents(&mut self) -> Vec<MediaIntent> {
        std::iter::from_fn(|| self.intents.take()).collect()
    }
}

pub struct WebView {
    pub model: WebViewModel,
}
impl WebView {
    pub fn new(model: WebViewModel) -> Self {
        Self { model }
    }
}
impl Render for WebView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let mut body = div().flex().flex_col().gap_2().p_2().bg(rgb(0xffffffff));
        body = body.child(
            div()
                .h(px(28.))
                .px_2()
                .border_1()
                .child(self.model.url.clone()),
        );
        if let Some(reason) = &self.model.blocked_reason {
            body = body.child(
                div()
                    .text_color(rgb(0xb42318ff))
                    .child(format!("Blocked: {reason}")),
            );
        }
        body
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn policy_blocks_and_allows() {
        let p = WebPolicy::AllowList(vec!["example.com".into()]);
        assert!(p.allows("https://www.example.com/a"));
        assert!(!p.allows("https://evil.test"));
    }
    #[test]
    fn blocked_navigation_is_observable() {
        let mut m = WebViewModel::new(WebPolicy::Offline);
        m.navigate("https://example.com");
        assert!(m.blocked_reason.is_some());
        assert!(m.take_intents().is_empty());
    }
}
