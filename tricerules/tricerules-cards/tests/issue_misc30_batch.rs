//! Printed identities for five reviewed pinned Standard token cards.

use tricerules_cards::{CardRegistry, Layout};

#[test]
fn issue_misc30_registers_five_exact_card_identities() {
    let registry = CardRegistry::global();
    for (id, name, mana, types, stats) in [
        (
            "involuntary_employment",
            "Involuntary Employment",
            "{3}{R}",
            &["Sorcery"][..],
            None,
        ),
        (
            "ant-mans_army",
            "Ant-Man's Army",
            "{2}{G}",
            &["Creature", "Insect"][..],
            Some((3, 2)),
        ),
        (
            "experimental_confectioner",
            "Experimental Confectioner",
            "{2}{B}",
            &["Creature", "Human", "Peasant"][..],
            Some((2, 3)),
        ),
        (
            "bakersbane_duo",
            "Bakersbane Duo",
            "{1}{G}",
            &["Creature", "Squirrel", "Raccoon"][..],
            Some((2, 2)),
        ),
        (
            "clachan_festival",
            "Clachan Festival",
            "{2}{W}",
            &["Kindred", "Enchantment", "Kithkin"][..],
            None,
        ),
    ] {
        let card = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(card.layout, Layout::Normal, "{id}");
        let face = card.primary_face();
        assert_eq!(face.name, name, "{id}");
        assert_eq!(face.mana_cost.to_string(), mana, "{id}");
        assert_eq!(face.types, types, "{id}");
        assert_eq!(face.power.zip(face.toughness), stats, "{id}");
    }
}
