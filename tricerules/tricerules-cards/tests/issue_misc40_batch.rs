//! Printed identities for six pinned Standard spells with grouped targets and modes.

use tricerules_cards::{CardRegistry, Layout};

#[test]
fn issue_misc40_registers_six_exact_card_identities() {
    let registry = CardRegistry::global();
    for (id, name, mana, types) in [
        (
            "mabels_mettle",
            "Mabel's Mettle",
            "{1}{W}",
            &["Instant"][..],
        ),
        ("skulduggery", "Skulduggery", "{B}", &["Instant"][..]),
        (
            "combat_tutorial",
            "Combat Tutorial",
            "{2}{U}",
            &["Sorcery"][..],
        ),
        (
            "cost_of_brilliance",
            "Cost of Brilliance",
            "{2}{B}",
            &["Sorcery"][..],
        ),
        (
            "mouser_attack!",
            "Mouser Attack!",
            "{1}{R}",
            &["Instant"][..],
        ),
        ("rat_out", "Rat Out", "{B}", &["Instant"][..]),
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
