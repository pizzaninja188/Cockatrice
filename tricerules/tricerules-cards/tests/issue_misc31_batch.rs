//! Printed identities for five pinned Standard token cards.

use tricerules_cards::{CardRegistry, Layout};

#[test]
fn issue_misc31_registers_five_exact_card_identities() {
    let registry = CardRegistry::global();
    for (id, name, mana, types, stats) in [
        (
            "mine_raider",
            "Mine Raider",
            "{2}{R}",
            &["Creature", "Human", "Rogue"][..],
            Some((3, 2)),
        ),
        (
            "prompto_argentum",
            "Prompto Argentum",
            "{1}{R}",
            &["Creature", "Human", "Scout"][..],
            Some((2, 2)),
        ),
        (
            "smaug_the_magnificent",
            "Smaug the Magnificent",
            "{2}{R}{R}",
            &["Creature", "Dragon"][..],
            Some((4, 3)),
        ),
        ("fountainport", "Fountainport", "", &["Land"][..], None),
        (
            "baxter_stockman",
            "Baxter Stockman",
            "{3}{U}{R}",
            &["Creature", "Human", "Scientist"][..],
            Some((3, 3)),
        ),
    ] {
        let card = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(card.layout, Layout::Normal, "{id}");
        let face = card.primary_face();
        assert_eq!(face.name, name, "{id}");
        assert_eq!(face.mana_cost.to_string(), mana, "{id}");
        assert_eq!(face.types, types, "{id}");
        assert_eq!(
            face.supertypes.contains(&"Legendary".to_string()),
            matches!(
                id,
                "prompto_argentum" | "smaug_the_magnificent" | "baxter_stockman"
            ),
            "{id}"
        );
        assert_eq!(face.power.zip(face.toughness), stats, "{id}");
    }
}
