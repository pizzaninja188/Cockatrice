//! Printed identities for five pinned Standard spells.

use tricerules_cards::{CardRegistry, Layout};

#[test]
fn issue_misc32_registers_five_exact_card_identities() {
    let registry = CardRegistry::global();
    for (id, name, mana, kind) in [
        ("gravkill", "Gravkill", "{3}{B}", "Instant"),
        (
            "darkness_descends",
            "Darkness Descends",
            "{2}{B}{B}",
            "Sorcery",
        ),
        (
            "preposterous_proportions",
            "Preposterous Proportions",
            "{5}{G}{G}",
            "Sorcery",
        ),
        (
            "bewildering_blizzard",
            "Bewildering Blizzard",
            "{4}{U}{U}",
            "Instant",
        ),
        (
            "stroke_of_midnight",
            "Stroke of Midnight",
            "{2}{W}",
            "Instant",
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
