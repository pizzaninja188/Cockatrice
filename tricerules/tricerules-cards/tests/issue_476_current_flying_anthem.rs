use tricerules_cards::primitives::CreatureScopeFilter;

#[test]
fn creature_scope_filter_can_require_a_current_keyword() {
    let parsed = ron::from_str::<CreatureScopeFilter>("(required_keyword: Some(Flying))")
        .expect("current-keyword filter RON");
    let serialized = ron::to_string(&parsed).expect("serialized creature scope filter");
    assert!(
        serialized.contains("required_keyword:Some(Flying)"),
        "a static creature scope must preserve its typed current-keyword predicate: {serialized}"
    );
}
