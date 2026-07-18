use crate::{IntentQueue, LeafState, MediaIntent, MediaTokens};
use gpui::{div, prelude::*, px, rgb, Context, Render, Window};
use std::collections::VecDeque;
use url::{Host, Url};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WebPolicy {
    Offline,
    AllowList(Vec<String>),
}

/// Requests for the host browser.  The view only emits these requests; it
/// never performs navigation, downloads, or process/file-system work itself.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WebIntent {
    Navigate(String),
    Back,
    Forward,
    Reload(String),
    Download(String),
    OpenExternal(String),
}

impl WebPolicy {
    pub fn evaluate(&self, url: &str) -> Result<(), String> {
        if url.chars().any(char::is_control) {
            return Err("URL contains control characters".into());
        }
        let parsed = Url::parse(url).map_err(|_| "URL could not be parsed".to_string())?;
        if !matches!(parsed.scheme(), "http" | "https") {
            return Err("URL scheme is not HTTP(S)".into());
        }
        if !parsed.username().is_empty() || parsed.password().is_some() {
            return Err("URL contains credentials".into());
        }
        // Url::parse rejects malformed ports, and this host-only policy rejects
        // explicit ports too: accepting one would make an allow-list entry's
        // meaning ambiguous when it does not include a port.
        if parsed.port().is_some() {
            return Err("URL contains an explicit port".into());
        }
        let host =
            Self::normalize_url_host(&parsed).ok_or_else(|| "URL has no valid host".to_string())?;
        match self {
            Self::Offline => Err("offline policy blocks navigation".into()),
            // The policy is exact-host or dot-delimited subdomain matching;
            // this deliberately does not accept unrelated suffixes.
            Self::AllowList(list)
                if list
                    .iter()
                    .filter_map(|allowed| Self::normalize_allowed_host(allowed))
                    .any(|allowed| host == allowed || host.ends_with(&format!(".{allowed}"))) =>
            {
                Ok(())
            }
            Self::AllowList(_) => Err(format!("host is not on the allow-list: {host}")),
        }
    }

    fn normalize_allowed_host(entry: &str) -> Option<String> {
        // Entries are host names by default. A scheme is accepted only when
        // the entry is an otherwise bare absolute HTTP(S) URL, avoiding the
        // ambiguity of silently discarding paths, credentials, or ports.
        let candidate = if entry.starts_with("http://") || entry.starts_with("https://") {
            let parsed = Url::parse(entry).ok()?;
            if !matches!(parsed.scheme(), "http" | "https")
                || !parsed.username().is_empty()
                || parsed.password().is_some()
                || parsed.path() != "/"
                || parsed.query().is_some()
                || parsed.fragment().is_some()
                || parsed.port().is_some()
            {
                return None;
            }
            Self::normalize_url_host(&parsed)?
        } else {
            entry.to_owned()
        };
        if candidate.is_empty() || candidate.chars().any(char::is_control) {
            return None;
        }
        let parsed = Url::parse(&format!("http://{candidate}")).ok()?;
        if parsed.username() != ""
            || parsed.password().is_some()
            || parsed.path() != "/"
            || parsed.query().is_some()
            || parsed.fragment().is_some()
            || parsed.port().is_some()
        {
            return None;
        }
        Self::normalize_url_host(&parsed)
    }

    fn normalize_url_host(parsed: &Url) -> Option<String> {
        match parsed.host()? {
            Host::Domain(domain) => Self::normalize_domain(domain),
            Host::Ipv4(address) => Some(address.to_string()),
            Host::Ipv6(address) => Some(address.to_string()),
        }
    }

    fn normalize_domain(domain: &str) -> Option<String> {
        let domain = domain.trim_end_matches('.');
        if domain.is_empty()
            || domain.chars().any(char::is_control)
            || domain.split('.').any(|label| {
                label.is_empty()
                    || label.starts_with('-')
                    || label.ends_with('-')
                    || !label
                        .chars()
                        .all(|character| character.is_alphanumeric() || character == '-')
            })
        {
            return None;
        }
        Some(domain.to_ascii_lowercase())
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
    web_intents: VecDeque<WebIntent>,
    back: Vec<String>,
    forward: Vec<String>,
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
            web_intents: VecDeque::new(),
            back: Vec::new(),
            forward: Vec::new(),
        }
    }
    pub fn navigate(&mut self, url: impl Into<String>) {
        let url = url.into();
        match self.policy.evaluate(&url) {
            Ok(()) => {
                if !self.url.is_empty() && self.url != url {
                    self.back.push(self.url.clone());
                }
                self.forward.clear();
                self.url = url.clone();
                self.blocked_reason = None;
                self.web_intents.push_back(WebIntent::Navigate(url));
            }
            Err(reason) => self.blocked_reason = Some(reason),
        }
    }
    pub fn back(&mut self) {
        if let Some(url) = self.back.pop() {
            if self.policy.allows(&url) {
                self.forward.push(self.url.clone());
                self.url = url.clone();
                self.web_intents.push_back(WebIntent::Back);
            }
        }
    }
    pub fn forward(&mut self) {
        if let Some(url) = self.forward.pop() {
            if self.policy.allows(&url) {
                self.back.push(self.url.clone());
                self.url = url.clone();
                self.web_intents.push_back(WebIntent::Forward);
            }
        }
    }
    pub fn reload(&mut self) {
        if self.policy.allows(&self.url) {
            self.web_intents
                .push_back(WebIntent::Reload(self.url.clone()));
        }
    }
    pub fn download(&mut self) {
        if self.policy.allows(&self.url) {
            self.web_intents
                .push_back(WebIntent::Download(self.url.clone()));
        }
    }
    pub fn open_external(&mut self) {
        if self.policy.allows(&self.url) {
            self.web_intents
                .push_back(WebIntent::OpenExternal(self.url.clone()));
        }
    }
    pub fn take_web_intents(&mut self) -> Vec<WebIntent> {
        self.web_intents.drain(..).collect()
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
        let allowed = self.model.policy.allows(&self.model.url);
        let controls = format!(
            "[{} Back] [{} Forward] [{} Reload] [{} Download] [{} Open external]",
            if self.model.back.is_empty() {
                "disabled"
            } else {
                "enabled"
            },
            if self.model.forward.is_empty() {
                "disabled"
            } else {
                "enabled"
            },
            if allowed { "enabled" } else { "disabled" },
            if allowed { "enabled" } else { "disabled" },
            if allowed { "enabled" } else { "disabled" },
        );
        body = body.child(div().child(controls));
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

    #[test]
    fn navigation_history_and_host_requests_are_queued() {
        let mut m = WebViewModel::new(WebPolicy::AllowList(vec!["example.com".into()]));
        m.navigate("https://example.com/one");
        m.navigate("https://example.com/two");
        m.back();
        m.forward();
        m.reload();
        m.download();
        m.open_external();
        assert_eq!(
            m.take_web_intents(),
            vec![
                WebIntent::Navigate("https://example.com/one".into()),
                WebIntent::Navigate("https://example.com/two".into()),
                WebIntent::Back,
                WebIntent::Forward,
                WebIntent::Reload("https://example.com/two".into()),
                WebIntent::Download("https://example.com/two".into()),
                WebIntent::OpenExternal("https://example.com/two".into()),
            ]
        );
    }

    #[test]
    fn blocked_requests_have_no_side_effects() {
        let mut m = WebViewModel::new(WebPolicy::AllowList(vec!["example.com".into()]));
        m.navigate("https://evil.test");
        m.download();
        m.open_external();
        assert!(m.take_web_intents().is_empty());
        assert_eq!(m.url, "");
    }
}
