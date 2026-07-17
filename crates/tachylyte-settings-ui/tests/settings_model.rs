use tachylyte_settings_ui::{Category, Settings, SettingsEvent, Theme};

#[test]
fn defaults_and_categories_are_exposed() {
    let settings = Settings::default();

    assert_eq!(settings.category, Category::Appearance);
    assert_eq!(settings.theme, Theme::Light);
    assert!(settings.search().is_empty());
    assert_eq!(Category::ALL.len(), 6);
    assert_eq!(Category::ALL[0].label(), "About");
    assert_eq!(Category::ALL[5].label(), "Core plugins");
    assert!(!settings.plugins.is_empty());
    assert!(settings.plugins.iter().all(|plugin| plugin.enabled));
}

#[test]
fn search_filters_plugins_case_insensitively() {
    let mut settings = Settings::default();

    settings.set_search("GrApH");

    let plugins = settings.filtered_plugins();
    assert_eq!(plugins.len(), 1);
    assert_eq!(plugins[0].id, "graph");
}

#[test]
fn search_control_characters_are_ignored() {
    let mut settings = Settings::default();

    settings.set_search("graph\u{0000}");

    assert_eq!(settings.search(), "graph");
    assert_eq!(settings.filtered_plugins().len(), 1);
    assert_eq!(
        settings.drain_events(),
        vec![SettingsEvent::SearchChanged("graph".into())]
    );
}

#[test]
fn draining_without_new_changes_is_stably_neutral() {
    let mut settings = Settings::default();

    assert!(settings.drain_events().is_empty());
    settings.set_search("graph");
    assert_eq!(
        settings.drain_events(),
        vec![SettingsEvent::SearchChanged("graph".into())]
    );
    assert!(settings.drain_events().is_empty());

    settings.set_plugin_enabled("does-not-exist", false);
    assert!(settings.drain_events().is_empty());
}

#[test]
fn plugin_toggle_emits_a_neutral_event() {
    let mut settings = Settings::default();

    settings.set_plugin_enabled("graph", false);

    assert!(
        !settings
            .plugins
            .iter()
            .find(|plugin| plugin.id == "graph")
            .unwrap()
            .enabled
    );
    assert_eq!(
        settings.drain_events(),
        vec![SettingsEvent::PluginChanged {
            id: "graph".into(),
            enabled: false,
        }]
    );
}

#[test]
fn close_emits_close_event() {
    let mut settings = Settings::default();

    settings.request_close();

    assert_eq!(settings.drain_events(), vec![SettingsEvent::CloseRequested]);
}
