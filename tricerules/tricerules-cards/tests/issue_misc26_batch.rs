//! Printed identity checks for five reviewed Standard Food, Treasure, and Clue cards.

use tricerules_cards::{CardRegistry, Layout};

#[test]
fn issue_misc26_batch_registers_five_exact_oracle_identities() {
    let registry = CardRegistry::global();
    for (id, name, mana, types, stats) in [
        (
            "dori,_bearer_of_friends",
            "Dori, Bearer of Friends",
            "{2}{R}",
            &["Creature", "Dwarf", "Warrior"][..],
            Some((3, 2)),
        ),
        (
            "skybeast_tracker",
            "Skybeast Tracker",
            "{3}{G}",
            &["Creature", "Giant", "Archer"][..],
            Some((2, 4)),
        ),
        (
            "reckless_lackey",
            "Reckless Lackey",
            "{R}",
            &["Creature", "Goblin", "Pirate"][..],
            Some((1, 2)),
        ),
        (
            "corsair_captain",
            "Corsair Captain",
            "{2}{U}",
            &["Creature", "Human", "Pirate"][..],
            Some((2, 2)),
        ),
        (
            "bumbleflowers_sharepot",
            "Bumbleflower's Sharepot",
            "{2}",
            &["Artifact"][..],
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
    assert_eq!(
        registry
            .get("dori,_bearer_of_friends")
            .unwrap()
            .primary_face()
            .supertypes,
        ["Legendary"]
    );
}
