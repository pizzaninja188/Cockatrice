//! Printed identities for five pinned Standard modal spells.

use tricerules_cards::{CardRegistry, Layout};

#[test]
fn issue_misc42_registers_five_exact_card_identities() {
    let registry = CardRegistry::global();
    for (id, name, mana, types) in [
        (
            "rydias_return",
            "Rydia's Return",
            "{3}{G}{G}",
            &["Sorcery"][..],
        ),
        (
            "splatter_technique",
            "Splatter Technique",
            "{1}{U}{U}{R}{R}",
            &["Sorcery"][..],
        ),
        ("suplex", "Suplex", "{1}{R}", &["Sorcery"][..]),
        ("agate_assault", "Agate Assault", "{2}{R}", &["Sorcery"][..]),
        ("school_daze", "School Daze", "{3}{U}{U}", &["Instant"][..]),
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
