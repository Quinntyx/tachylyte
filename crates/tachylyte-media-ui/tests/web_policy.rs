use tachylyte_media_ui::{WebPolicy, WebViewModel};

#[test]
fn allow_list_rejects_credential_bypass_without_allowed_intent() {
    let policy = WebPolicy::AllowList(vec!["example.com".into()]);
    let url = "https://example.com@evil.test";

    assert!(!policy.allows(url));

    let mut model = WebViewModel::new(policy);
    model.navigate(url);
    assert!(model.take_intents().is_empty());
}
