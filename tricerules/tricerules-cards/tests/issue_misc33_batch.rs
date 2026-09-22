//! Printed identities for seven pinned Standard damage spells.

use tricerules_cards::{CardRegistry, Layout};

#[test]
fn issue_misc33_registers_seven_exact_card_identities() {
    let registry = CardRegistry::global();
    for (id, name, mana, kind) in [
        ("feed_the_flames", "Feed the Flames", "{3}{R}", "Instant"),
        ("elspeths_smite", "Elspeth's Smite", "{W}", "Instant"),
        (
            "obliterating_bolt",
            "Obliterating Bolt",
            "{1}{R}",
            "Sorcery",
        ),
        ("narsets_rebuke", "Narset's Rebuke", "{4}{R}", "Instant"),
        (
            "invasive_maneuvers",
            "Invasive Maneuvers",
            "{1}{R}",
            "Instant",
        ),
        (
            "rumbling_rockslide",
            "Rumbling Rockslide",
            "{3}{R}",
            "Sorcery",
        ),
        ("galvanize", "Galvanize", "{1}{R}", "Instant"),
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
