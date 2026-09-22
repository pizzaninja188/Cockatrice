//! Printed identities for five pinned Standard targeted removal spells.

use tricerules_cards::{CardRegistry, Layout};

#[test]
fn issue_misc37_registers_five_exact_card_identities() {
    let registry = CardRegistry::global();
    for (id, name, mana, types) in [
        (
            "harsh_annotation",
            "Harsh Annotation",
            "{1}{W}",
            &["Instant"][..],
        ),
        (
            "zukos_exile",
            "Zuko's Exile",
            "{5}",
            &["Instant", "Lesson"][..],
        ),
        (
            "shattered_wings",
            "Shattered Wings",
            "{2}{G}",
            &["Sorcery"][..],
        ),
        ("devout_decree", "Devout Decree", "{1}{W}", &["Sorcery"][..]),
        (
            "inevitable_defeat",
            "Inevitable Defeat",
            "{1}{R}{W}{B}",
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
