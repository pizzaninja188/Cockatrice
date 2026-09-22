//! Printed identity and complete ability-shape checks for five Standard entry/token cards.

use tricerules_cards::{CardRegistry, Layout};

#[test]
fn issue_misc25_batch_registers_five_exact_oracle_identities() {
    let registry = CardRegistry::global();
    for (id, name, mana, types, stats) in [
        (
            "news_helicopter",
            "News Helicopter",
            "{3}",
            &["Artifact", "Creature", "Construct"][..],
            Some((1, 1)),
        ),
        (
            "turtle_blimp",
            "Turtle Blimp",
            "{5}",
            &["Artifact", "Vehicle"][..],
            Some((3, 4)),
        ),
        (
            "hopeful_vigil",
            "Hopeful Vigil",
            "{1}{W}",
            &["Enchantment"][..],
            None,
        ),
        (
            "mechan_assembler",
            "Mechan Assembler",
            "{4}{U}",
            &["Artifact", "Creature", "Robot", "Artificer"][..],
            Some((4, 4)),
        ),
        (
            "mighty_mutanimals",
            "Mighty Mutanimals",
            "{2}{W}{W}",
            &["Creature", "Mutant", "Rebel"][..],
            Some((2, 1)),
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
