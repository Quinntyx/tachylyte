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
use url::Url;

pub type ExtraFields = BTreeMap<String, Value>;
fn secret_key(key: &str) -> bool {
    let k = key.to_ascii_lowercase();
    [
        "token",
        "secret",
        "password",
        "credential",
        "authorization",
        "private_key",
    ]
    .iter()
    .any(|x| k.contains(x))
}

/// A secret that cannot accidentally appear in logs or formatted errors.
#[derive(Clone, PartialEq, Eq)]
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
    #[derive(Clone, PartialEq, Eq)]
    pub struct Session {
        pub state: SessionState,
        pub access_token: Option<Secret>,
        pub refresh_token: Option<Secret>,
        pub extra: ExtraFields,
    }
    impl<'de> Deserialize<'de> for Session {
        fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
            #[derive(Deserialize)]
            struct Wire {
                state: SessionState,
                #[serde(flatten)]
                extra: ExtraFields,
            }
            let wire = Wire::deserialize(d)?;
            if wire.extra.keys().any(|k| super::secret_key(k)) {
                return Err(serde::de::Error::custom("secret-like session field"));
            }
            if matches!(wire.state, SessionState::Authenticated { .. }) {
                return Err(serde::de::Error::custom(
                    "authenticated sessions require runtime credentials",
                ));
            }
            Ok(Session {
                state: wire.state,
                access_token: None,
                refresh_token: None,
                extra: wire.extra,
            })
        }
    }
    impl serde::Serialize for Session {
        fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            use serde::ser::SerializeMap;
            let mut out = s.serialize_map(Some(1 + self.extra.len()))?;
            out.serialize_entry("state", &self.state)?;
            for (k, v) in &self.extra {
                if !super::secret_key(k) {
                    out.serialize_entry(k, v)?;
                }
            }
            out.end()
        }
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
        InvalidAccountId,
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
            self.access_token = None;
            self.refresh_token = None;
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
            let account_id = account_id.into();
            if account_id.trim().is_empty() || access.expose().is_empty() {
                return Err(Error::InvalidAccountId);
            }
            if matches!(self.state, SessionState::Authenticating) {
                self.state = SessionState::Authenticated { account_id };
                self.access_token = Some(access);
                self.refresh_token = refresh;
                Ok(())
            } else {
                Err(Error::InvalidTransition)
            }
        }
        pub fn failed(&mut self) {
            self.state = SessionState::Degraded;
            self.access_token = None;
            self.refresh_token = None;
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
    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
    pub enum Resolution {
        KeepLocal,
        KeepRemote,
    }
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum ConflictError {
        UnknownResource,
        NotConcurrent,
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
        pub fn resolve(
            &self,
            conflict: &Conflict,
            resolution: Resolution,
        ) -> Result<Resolution, ConflictError> {
            if !self.resources.iter().any(|r| r == &conflict.resource) {
                return Err(ConflictError::UnknownResource);
            }
            if !conflict.local.concurrent(&conflict.remote) {
                return Err(ConflictError::NotConcurrent);
            }
            Ok(resolution)
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
    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
    pub struct SiteConfigDiff {
        pub old: SiteConfig,
        pub new: SiteConfig,
    }
    pub fn diff(old: &PublishManifest, new: &PublishManifest) -> Vec<ManifestDiff> {
        let mut out = vec![];
        if old.site != new.site {
            out.push(ManifestDiff {
                path: "[site-config]".into(),
                kind: DiffKind::Changed,
                digest: None,
            });
        }
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
            if raw.chars().any(|c| c.is_control()) {
                return Err(UrlError::Malformed);
            }
            let parsed = Url::parse(raw).map_err(|_| UrlError::Malformed)?;
            if !matches!(parsed.scheme(), "https" | "http") {
                return Err(UrlError::UnsafeScheme);
            }
            if parsed.username() != "" || parsed.password().is_some() || parsed.host_str().is_none()
            {
                return Err(UrlError::Malformed);
            }
            let host = parsed.host_str().unwrap().to_ascii_lowercase();
            if parsed.port().is_some_and(|p| p == 0) {
                return Err(UrlError::Malformed);
            }
            if !policy.allow_external && !policy.allowed_hosts.contains(&host) {
                return Err(UrlError::HostDenied);
            }
            Ok(Self(parsed.to_string()))
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
            || path.contains('\\')
            || path.chars().any(char::is_control)
            || (path.len() >= 2 && path.as_bytes()[1] == b':')
            || path.starts_with("//")
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
    pub struct VerificationEvidence {
        algorithm: String,
        key_id: String,
        signature: String,
    }
    impl VerificationEvidence {
        pub fn algorithm(&self) -> &str {
            &self.algorithm
        }
        pub fn key_id(&self) -> &str {
            &self.key_id
        }
        pub fn signature(&self) -> &str {
            &self.signature
        }
    }
    pub trait SignatureVerifier {
        fn verify(&self, metadata: &UpdateMetadata) -> bool;
    }
    #[derive(Clone, Debug, Serialize, PartialEq, Eq)]
    pub enum PrintIntent {
        Document { id: String },
        Selection { text: String },
    }
    #[derive(Clone, Debug, Serialize, PartialEq, Eq)]
    pub enum TransferIntent {
        Import { format: String },
        Export { format: String },
    }
    #[derive(Clone, Debug, Serialize, PartialEq, Eq)]
    pub enum UpdateState {
        Unknown,
        Available {
            version: String,
        },
        Verified {
            version: String,
            evidence: VerificationEvidence,
        },
        Rejected,
    }
    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
    pub struct UpdateMetadata {
        pub version: String,
        pub digest: String,
        pub signature: Option<String>,
        pub signature_algorithm: Option<String>,
        pub key_id: Option<String>,
        #[serde(flatten)]
        pub extra: ExtraFields,
    }
    impl UpdateState {
        pub fn verify<V: SignatureVerifier>(
            metadata: &UpdateMetadata,
            verifier: &V,
        ) -> Option<Self> {
            if verifier.verify(metadata) {
                Some(Self::Verified {
                    version: metadata.version.clone(),
                    evidence: VerificationEvidence {
                        algorithm: metadata.signature_algorithm.clone()?,
                        key_id: metadata.key_id.clone()?,
                        signature: metadata.signature.clone()?,
                    },
                })
            } else {
                None
            }
        }
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
        let parsed = Url::parse(raw).ok()?;
        if parsed.scheme() != "tachylyte"
            || parsed.host_str()? != "app"
            || !parsed.username().is_empty()
            || parsed.password().is_some()
            || parsed.port().is_some()
        {
            return None;
        }
        let action = parsed.path_segments()?.next()?.to_string();
        if action.is_empty()
            || !action
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return None;
        }
        let params = parsed
            .query_pairs()
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();
        let target = parsed.fragment().and_then(|raw| {
            url::form_urlencoded::parse(format!("x={raw}").as_bytes())
                .next()
                .map(|(_, v)| v.into_owned())
        });
        if target
            .as_deref()
            .is_some_and(|t| t.is_empty() || t.chars().any(char::is_control))
        {
            return None;
        }
        Some(DeepLink {
            action,
            target,
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
pub struct TelemetryEvent {
    pub name: String,
    pub fields: ExtraFields,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TelemetryError {
    NotConsented,
    InvalidName,
}
pub fn record_telemetry(
    consent: &TelemetryConsent,
    mut event: TelemetryEvent,
) -> Result<TelemetryEvent, TelemetryError> {
    if !consent.explicit || !consent.enabled {
        return Err(TelemetryError::NotConsented);
    }
    if event.name.is_empty() || event.name.chars().any(|c| c.is_control()) {
        return Err(TelemetryError::InvalidName);
    }
    for key in ["token", "secret", "password", "credential"] {
        event.fields.remove(key);
    }
    Ok(event)
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
    responses: BTreeMap<String, Vec<u8>>,
    requests: Vec<String>,
}
impl MockTransport {
    pub fn respond(&mut self, key: impl Into<String>, body: Vec<u8>) {
        let key = key.into();
        if secret_key(&key) {
            return;
        }
        self.responses.insert(key, body);
    }
    pub fn request(&mut self, key: &str) -> Option<Vec<u8>> {
        let safe_key = if secret_key(key) { "[REDACTED]" } else { key };
        self.requests.push(safe_key.into());
        self.responses.get(key).cloned()
    }
    pub fn request_with_secret(&mut self, key: &str, _secret: &Secret) -> Option<Vec<u8>> {
        self.request(key)
    }
    pub fn recorded_requests(&self) -> &[String] {
        &self.requests
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
        assert_eq!(t.recorded_requests(), ["x"]);
    }
    #[test]
    fn secrets_are_not_serialized_or_recorded() {
        let s = auth::Session {
            state: auth::SessionState::Authenticated {
                account_id: "a".into(),
            },
            access_token: Some(Secret::new("raw")),
            refresh_token: Some(Secret::new("refresh")),
            extra: ExtraFields::new(),
        };
        let json = serde_json::to_string(&s).unwrap();
        assert!(!json.contains("raw"));
        assert!(serde_json::from_str::<auth::Session>(
            r#"{"state":{"Authenticated":{"account_id":"a"}}}"#
        )
        .is_err());
        assert!(serde_json::from_str::<auth::Session>(
            r#"{"state":"SignedOut","access_token":"raw"}"#
        )
        .is_err());
        let mut t = MockTransport::default();
        t.request_with_secret("token=raw", s.access_token.as_ref().unwrap());
        assert_eq!(t.recorded_requests(), ["[REDACTED]"]);
        t.respond("PASSWORD=raw", vec![2]);
        assert!(t.request("password=raw").is_none());
    }
    #[test]
    fn auth_clears_and_validates() {
        let mut s = auth::Session::signed_out();
        s.begin_login().unwrap();
        assert!(s.authenticated(" ", Secret::new("x"), None).is_err());
        s.failed();
        assert!(s.access_token.is_none());
    }
    #[test]
    fn strict_url_and_paths() {
        let p = web::NavigationPolicy {
            allowed_hosts: ["[::1]".into(), "example.test".into()]
                .into_iter()
                .collect(),
            allow_external: false,
        };
        assert!(web::navigation(&p, "https://user:pw@example.test").is_err());
        assert!(web::navigation(&p, "https://[::1]:443/x").is_ok());
        assert!(files::safe_relative(r"C:\\tmp\\x").is_err());
        assert!(files::safe_relative("a\0b").is_err());
    }
    #[test]
    fn conflict_site_telemetry_and_deeplink_boundaries() {
        let mut plan = sync::SyncPlan::new(sync::ConflictPolicy::RequireReview);
        plan.resources.push("doc".into());
        let mut a = sync::VersionVector::default();
        a.increment("a");
        let mut b = sync::VersionVector::default();
        b.increment("b");
        assert!(plan
            .resolve(&plan.conflict("doc", a, b), sync::Resolution::KeepLocal)
            .is_ok());
        let site = publish::SiteConfig {
            title: "x".into(),
            base_path: "/".into(),
            extra: ExtraFields::new(),
        };
        let mut newer = publish::PublishManifest {
            site: site.clone(),
            files: BTreeMap::new(),
            extra: ExtraFields::new(),
        };
        newer.site.title = "y".into();
        let old = publish::PublishManifest {
            site,
            files: BTreeMap::new(),
            extra: ExtraFields::new(),
        };
        assert_eq!(publish::diff(&old, &newer).len(), 1);
        let event = TelemetryEvent {
            name: "x".into(),
            fields: [("token".into(), Value::String("x".into()))]
                .into_iter()
                .collect(),
        };
        assert!(record_telemetry(&TelemetryConsent::default(), event.clone()).is_err());
        let c = TelemetryConsent {
            enabled: true,
            explicit: true,
        };
        assert!(!record_telemetry(&c, event)
            .unwrap()
            .fields
            .contains_key("token"));
        assert_eq!(
            uri::parse("tachylyte://app/open?target=a%20b#x")
                .unwrap()
                .params["target"],
            "a b"
        );
        assert!(uri::parse("tachylyte://app/bad%20x").is_none());
        assert!(uri::parse("tachylyte://user:pw@app/open").is_none());
        assert!(uri::parse("tachylyte://app:443/open").is_none());
    }
}
