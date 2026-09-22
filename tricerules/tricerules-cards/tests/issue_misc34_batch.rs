//! Printed identities for five pinned Standard sequential spells.

use tricerules_cards::{CardRegistry, Layout};

#[test]
fn issue_misc34_registers_five_exact_card_identities() {
    let registry = CardRegistry::global();
    for (id, name, mana, kind) in [
        ("make_a_stand", "Make a Stand", "{2}{W}", "Instant"),
        (
            "heroic_reinforcements",
            "Heroic Reinforcements",
            "{2}{R}{W}",
            "Sorcery",
        ),
        ("on_the_job", "On the Job", "{2}{W}{W}", "Instant"),
        (
            "the_crystals_chosen",
            "The Crystal's Chosen",
            "{5}{W}{W}",
            "Sorcery",
        ),
        ("deduce", "Deduce", "{1}{U}", "Instant"),
    ] {
        let card = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(card.layout, Layout::Normal, "{id}");
        let face = card.primary_face();
        assert_eq!(face.name, name, "{id}");
        assert_eq!(face.mana_cost.to_string(), mana, "{id}");
        assert_eq!(face.types, &[kind], "{id}");
        assert!(face.power.is_none() && face.toughness.is_none(), "{id}");
    }
}
