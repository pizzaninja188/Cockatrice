//! Printed identities for six pinned Standard modal spells.

use tricerules_cards::{CardRegistry, Layout};

#[test]
fn issue_misc41_registers_six_exact_card_identities() {
    let registry = CardRegistry::global();
    for (id, name, mana, types) in [
        ("slagstorm", "Slagstorm", "{1}{R}{R}", &["Sorcery"][..]),
        ("split_up", "Split Up", "{1}{W}{W}", &["Sorcery"][..]),
        (
            "thorins_last_stand",
            "Thorin's Last Stand",
            "{2}{W}{W}",
            &["Instant"][..],
        ),
        ("battle_menu", "Battle Menu", "{1}{W}", &["Instant"][..]),
        (
            "silverquill_charm",
            "Silverquill Charm",
            "{W}{B}",
            &["Instant"][..],
        ),
        (
            "glorious_decay",
            "Glorious Decay",
            "{1}{G}",
            &["Instant"][..],
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
