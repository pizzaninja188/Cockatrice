//! Printed identities for four pinned Standard graveyard spells.

use tricerules_cards::{CardRegistry, Layout};

#[test]
fn issue_misc35_registers_four_exact_card_identities() {
    let registry = CardRegistry::global();
    for (id, name, mana, kind) in [
        ("helping_hand", "Helping Hand", "{W}", "Sorcery"),
        ("hazels_nocturne", "Hazel's Nocturne", "{3}{B}", "Instant"),
        (
            "pull_from_the_grave",
            "Pull from the Grave",
            "{2}{B}",
            "Sorcery",
        ),
        (
            "mourners_surprise",
            "Mourner's Surprise",
            "{1}{B}",
            "Sorcery",
        ),
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
