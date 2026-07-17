use tachylyte_workflow_ui::{ListKey, RowListState};

#[test]
fn filtered_navigation_handles_unselected_edges_and_stale_enter() {
    let rows = vec!["before", "match-first", "match-last", "after"];

    let mut down = RowListState::with_query(rows.clone(), "match");
    down.key(ListKey::Down);
    assert_eq!(down.selected, Some(1));

    let mut up = RowListState::with_query(rows.clone(), "match");
    up.key(ListKey::Up);
    assert_eq!(up.selected, Some(2));

    let mut stale = RowListState::with_query(rows, "match");
    stale.selected = Some(0);
    assert_eq!(stale.key(ListKey::Enter), Some(&"match-first"));
    assert_eq!(stale.selected, Some(1));
}
