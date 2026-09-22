//! Printed identities for five pinned Standard spells.

use tricerules_cards::{CardRegistry, Layout};

#[test]
fn issue_misc44_registers_five_exact_card_identities() {
    let registry = CardRegistry::global();
    for (id, name, mana, types) in [
        (
            "mental_modulation",
            "Mental Modulation",
            "{1}{U}",
            &["Instant"][..],
        ),
        (
            "incinerating_blast",
            "Incinerating Blast",
            "{4}{R}",
            &["Sorcery"][..],
        ),
        (
            "slick_sequence",
            "Slick Sequence",
            "{U}{R}",
            &["Instant"][..],
        ),
        (
            "visions_of_villainy",
            "Visions of Villainy",
            "{2}{B}",
            &["Instant"][..],
        ),
        ("duel_tactics", "Duel Tactics", "{R}", &["Sorcery"][..]),
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
