//! Small, transport-free boundaries for first-party services and platform behavior.
//!
//! This crate deliberately does not contain a server client, credentials, or endpoint
//! defaults. Callers supply a transport and decide when network access is appropriate.
//! Unknown JSON fields are retained on wire-facing records where practical.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    path::Path,
};

pub type ExtraFields = BTreeMap<String, Value>;

/// A secret that cannot accidentally appear in logs or formatted errors.
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Secret(String);
impl Secret {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
    pub fn expose(&self) -> &str {
        &self.0
    }
}
impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[REDACTED]")
    }
}
impl fmt::Display for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[REDACTED]")
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Connectivity {
    Offline,
    Unauthenticated,
    Degraded,
    Online,
}

pub mod auth {
    use super::*;
    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
    pub enum SessionState {
        SignedOut,
        Authenticating,
        Authenticated { account_id: String },
        Expired,
        Degraded,
    }
    #[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub struct Session {
        pub state: SessionState,
        pub access_token: Option<Secret>,
        pub refresh_token: Option<Secret>,
        #[serde(flatten)]
        pub extra: ExtraFields,
    }
    impl fmt::Debug for Session {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("Session")
                .field("state", &self.state)
                .field(
                    "access_token",
                    &self.access_token.as_ref().map(|_| "[REDACTED]"),
                )
                .field(
                    "refresh_token",
                    &self.refresh_token.as_ref().map(|_| "[REDACTED]"),
                )
                .finish()
        }
    }
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum Error {
        InvalidTransition,
        MissingCredentials,
        TransportUnavailable,
    }
    impl fmt::Display for Error {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "auth error: {self:?}")
        }
    }
    impl std::error::Error for Error {}
    impl Session {
        pub fn signed_out() -> Self {
            Self {
                state: SessionState::SignedOut,
                access_token: None,
                refresh_token: None,
                extra: ExtraFields::new(),
            }
        }
        pub fn begin_login(&mut self) -> Result<(), Error> {
            if matches!(
                self.state,
                SessionState::SignedOut | SessionState::Expired | SessionState::Degraded
            ) {
                self.state = SessionState::Authenticating;
                Ok(())
            } else {
                Err(Error::InvalidTransition)
            }
        }
        pub fn authenticated(
            &mut self,
            account_id: impl Into<String>,
            access: Secret,
            refresh: Option<Secret>,
        ) -> Result<(), Error> {
            if matches!(self.state, SessionState::Authenticating) {
                self.state = SessionState::Authenticated {
                    account_id: account_id.into(),
                };
                self.access_token = Some(access);
                self.refresh_token = refresh;
                Ok(())
            } else {
                Err(Error::InvalidTransition)
            }
        }
        pub fn sign_out(&mut self) {
            self.state = SessionState::SignedOut;
            self.access_token = None;
            self.refresh_token = None;
        }
    }
}

pub mod sync {
    use super::*;
    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
    pub struct VersionVector(pub BTreeMap<String, u64>);
    impl VersionVector {
        pub fn increment(&mut self, actor: &str) {
            *self.0.entry(actor.into()).or_default() += 1;
        }
        pub fn dominates(&self, other: &Self) -> bool {
            other
                .0
                .iter()
                .all(|(k, v)| self.0.get(k).unwrap_or(&0) >= v)
        }
        pub fn concurrent(&self, other: &Self) -> bool {
            !self.dominates(other) && !other.dominates(self)
        }
    }
    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
    pub enum ConflictPolicy {
        KeepLocal,
        KeepRemote,
        RequireReview,
    }
    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
    pub struct SyncPlan {
        pub resources: Vec<String>,
        pub policy: ConflictPolicy,
        pub selective_settings: BTreeSet<String>,
        pub base: VersionVector,
        #[serde(flatten)]
        pub extra: ExtraFields,
    }
    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
    pub enum SyncState {
        Offline,
        Ready,
        Running,
        Conflicted,
        Degraded,
    }
    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
    pub struct Conflict {
        pub resource: String,
        pub local: VersionVector,
        pub remote: VersionVector,
    }
    impl SyncPlan {
        pub fn new(policy: ConflictPolicy) -> Self {
            Self {
                resources: vec![],
                policy,
                selective_settings: BTreeSet::new(),
                base: VersionVector::default(),
                extra: ExtraFields::new(),
            }
        }
        pub fn conflict(
            &self,
            resource: impl Into<String>,
            local: VersionVector,
            remote: VersionVector,
        ) -> Conflict {
            Conflict {
                resource: resource.into(),
                local,
                remote,
            }
        }
    }
}

pub mod publish {
    use super::*;
    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
    pub struct SiteConfig {
        pub title: String,
        pub base_path: String,
        #[serde(flatten)]
        pub extra: ExtraFields,
    }
    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
    pub struct PublishManifest {
        pub site: SiteConfig,
        pub files: BTreeMap<String, String>,
        #[serde(flatten)]
        pub extra: ExtraFields,
    }
    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
    pub enum DiffKind {
        Added,
        Changed,
        Removed,
    }
    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
    pub struct ManifestDiff {
        pub path: String,
        pub kind: DiffKind,
        pub digest: Option<String>,
    }
    pub fn diff(old: &PublishManifest, new: &PublishManifest) -> Vec<ManifestDiff> {
        let mut out = vec![];
        for (p, d) in &new.files {
            match old.files.get(p) {
                None => out.push(ManifestDiff {
                    path: p.clone(),
                    kind: DiffKind::Added,
                    digest: Some(d.clone()),
                }),
                Some(x) if x != d => out.push(ManifestDiff {
                    path: p.clone(),
                    kind: DiffKind::Changed,
                    digest: Some(d.clone()),
                }),
                _ => {}
            }
        }
        for p in old.files.keys().filter(|p| !new.files.contains_key(*p)) {
            out.push(ManifestDiff {
                path: p.clone(),
                kind: DiffKind::Removed,
                digest: None,
            });
        }
        out
    }
}

pub mod web {
    use super::*;
    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
    pub struct NavigationPolicy {
        pub allowed_hosts: BTreeSet<String>,
        pub allow_external: bool,
    }
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct SafeUrl(String);
    impl SafeUrl {
        pub fn parse(raw: &str, policy: &NavigationPolicy) -> Result<Self, UrlError> {
            let (scheme, rest) = raw.split_once("://").ok_or(UrlError::Malformed)?;
            if !matches!(scheme.to_ascii_lowercase().as_str(), "https" | "http") {
                return Err(UrlError::UnsafeScheme);
            }
            let host = rest
                .split(['/', '?', '#'])
                .next()
                .unwrap_or("")
                .to_ascii_lowercase();
            if host.is_empty() || (!policy.allow_external && !policy.allowed_hosts.contains(&host))
            {
                return Err(UrlError::HostDenied);
            }
            Ok(Self(raw.into()))
        }
        pub fn as_str(&self) -> &str {
            &self.0
        }
    }
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum UrlError {
        Malformed,
        UnsafeScheme,
        HostDenied,
    }
    impl fmt::Display for UrlError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "URL rejected: {self:?}")
        }
    }
    impl std::error::Error for UrlError {}
    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
    pub struct DownloadIntent {
        pub url: String,
        pub filename: String,
    }
    pub fn navigation(policy: &NavigationPolicy, raw: &str) -> Result<SafeUrl, UrlError> {
        SafeUrl::parse(raw, policy)
    }
}

pub mod files {
    use super::*;
    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
    pub struct RecoveryJob {
        pub path: String,
        pub attempts: u32,
    }
    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
    pub enum RecoveryState {
        Pending,
        Running,
        Recovered,
        Failed,
    }
    pub fn safe_relative(path: &str) -> Result<(), PathError> {
        let p = Path::new(path);
        if p.is_absolute()
            || p.components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            Err(PathError::Traversal)
        } else if path.is_empty() {
            Err(PathError::Empty)
        } else {
            Ok(())
        }
    }
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum PathError {
        Empty,
        Traversal,
    }
    pub struct RecoveryScheduler {
        queue: std::collections::VecDeque<RecoveryJob>,
    }
    impl RecoveryScheduler {
        pub fn new() -> Self {
            Self {
                queue: Default::default(),
            }
        }
        pub fn schedule(&mut self, job: RecoveryJob) -> Result<(), PathError> {
            safe_relative(&job.path)?;
            self.queue.push_back(job);
            Ok(())
        }
        pub fn pop(&mut self) -> Option<RecoveryJob> {
            self.queue.pop_front()
        }
    }
    impl Default for RecoveryScheduler {
        fn default() -> Self {
            Self::new()
        }
    }
}

pub mod intents {
    use super::*;
    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
    pub enum PrintIntent {
        Document { id: String },
        Selection { text: String },
    }
    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
    pub enum TransferIntent {
        Import { format: String },
        Export { format: String },
    }
    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
    pub enum UpdateState {
        Unknown,
        Available { version: String },
        Verified { version: String },
        Rejected,
    }
    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
    pub struct UpdateMetadata {
        pub version: String,
        pub digest: String,
        pub signature: Option<String>,
        #[serde(flatten)]
        pub extra: ExtraFields,
    }
}

pub mod uri {
    use super::*;
    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
    pub struct DeepLink {
        pub action: String,
        pub target: Option<String>,
        pub params: BTreeMap<String, String>,
    }
    pub fn parse(raw: &str) -> Option<DeepLink> {
        let (scheme, rest) = raw.split_once("://")?;
        if scheme != "tachylyte" {
            return None;
        };
        let (action, q) = rest.split_once('?').unwrap_or((rest, ""));
        let params = q
            .split('&')
            .filter_map(|p| p.split_once('='))
            .map(|(k, v)| (k.into(), v.into()))
            .collect();
        Some(DeepLink {
            action: action.into(),
            target: None,
            params,
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct TelemetryConsent {
    pub enabled: bool,
    pub explicit: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlatformCapabilities {
    pub network: bool,
    pub filesystem: bool,
    pub printing: bool,
    pub updates: bool,
    pub degraded: bool,
}

/// A deterministic transport useful in unit tests; it never performs I/O.
#[derive(Default)]
pub struct MockTransport {
    pub responses: BTreeMap<String, Vec<u8>>,
    pub requests: Vec<String>,
}
impl MockTransport {
    pub fn respond(&mut self, key: impl Into<String>, body: Vec<u8>) {
        self.responses.insert(key.into(), body);
    }
    pub fn request(&mut self, key: &str) -> Option<Vec<u8>> {
        self.requests.push(key.into());
        self.responses.get(key).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn secrets_redact() {
        let s = auth::Session {
            state: auth::SessionState::SignedOut,
            access_token: Some(Secret::new("token")),
            refresh_token: None,
            extra: ExtraFields::new(),
        };
        let output = format!("{s:?}");
        assert!(output.contains("[REDACTED]"));
        assert!(!output.contains("Secret(\"token\")"));
    }
    #[test]
    fn auth_states_are_explicit() {
        let mut s = auth::Session::signed_out();
        s.begin_login().unwrap();
        s.authenticated("a", Secret::new("x"), None).unwrap();
        s.sign_out();
        assert_eq!(s.state, auth::SessionState::SignedOut);
    }
    #[test]
    fn vectors_are_deterministic() {
        let mut a = sync::VersionVector::default();
        a.increment("a");
        let mut b = a.clone();
        b.increment("b");
        assert!(b.dominates(&a));
        assert!(!a.concurrent(&b));
    }
    #[test]
    fn unsafe_boundaries_rejected() {
        assert!(files::safe_relative("../x").is_err());
        let p = web::NavigationPolicy {
            allowed_hosts: ["example.test".into()].into_iter().collect(),
            allow_external: false,
        };
        assert!(web::navigation(&p, "file:///tmp/x").is_err());
        assert!(web::navigation(&p, "https://evil.test").is_err());
    }
    #[test]
    fn unknown_fields_survive() {
        let x: sync::SyncPlan = serde_json::from_str(
            r#"{"resources":[],"policy":"KeepLocal","selective_settings":[],"base":{},"future":7}"#,
        )
        .unwrap();
        assert_eq!(x.extra["future"], 7);
    }
    #[test]
    fn mock_has_no_network() {
        let mut t = MockTransport::default();
        t.respond("x", vec![1]);
        assert_eq!(t.request("x"), Some(vec![1]));
        assert_eq!(t.requests, ["x"]);
    }
}
