use tricerules_cards::{CardRegistry, Layout};

#[test]
fn issue_misc46_maps_definitions() {
    let registry = CardRegistry::global();
    for (id, name, mana, types) in [
        (
            "requisition_raid",
            "Requisition Raid",
            "{W}",
            &["Sorcery"][..],
        ),
        (
            "rustler_rampage",
            "Rustler Rampage",
            "{W}",
            &["Instant"][..],
        ),
        (
            "how_to_start_a_riot",
            "How to Start a Riot",
            "{2}{R}",
            &["Instant", "Lesson"][..],
        ),
        (
            "neutralize_the_guards",
            "Neutralize the Guards",
            "{2}{B}",
            &["Instant"][..],
        ),
    ] {
        let card = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(card.layout, Layout::Normal);
        let face = card.primary_face();
        assert_eq!(face.name, name);
        assert_eq!(face.mana_cost.to_string(), mana);
        assert_eq!(face.types, types);
    }
}
