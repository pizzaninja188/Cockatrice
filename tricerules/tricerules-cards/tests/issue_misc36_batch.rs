//! Printed identities for three pinned Standard graveyard spells.

use tricerules_cards::{CardRegistry, Layout};

#[test]
fn issue_misc36_registers_three_exact_card_identities() {
    let registry = CardRegistry::global();
    for (id, name, mana, types) in [
        (
            "true_ancestry",
            "True Ancestry",
            "{1}{G}",
            &["Sorcery", "Lesson"][..],
        ),
        (
            "pull_through_the_weft",
            "Pull Through the Weft",
            "{3}{G}{G}",
            &["Sorcery"][..],
        ),
        (
            "badlands_revival",
            "Badlands Revival",
            "{3}{B}{G}",
            &["Sorcery"][..],
        ),
    ] {
        let card = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(card.layout, Layout::Normal, "{id}");
        let face = card.primary_face();
        assert_eq!(face.name, name, "{id}");
        assert_eq!(face.mana_cost.to_string(), mana, "{id}");
        assert_eq!(face.types, types, "{id}");
        assert!(face.power.is_none() && face.toughness.is_none(), "{id}");
    }
}
