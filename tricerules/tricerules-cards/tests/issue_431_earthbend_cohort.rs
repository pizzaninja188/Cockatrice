//! Printed identities for the first #431 Earthbend cohort.

use tricerules_cards::Layout;

#[test]
fn issue_431_cohort_registers_exact_card_identities() {
    let registry = tricerules_cards::registry::global();
    for (id, name, mana, supertypes, types, body) in [
        (
            "earthbending_lesson",
            "Earthbending Lesson",
            "{3}{G}",
            &[][..],
            &["Sorcery", "Lesson"][..],
            None,
        ),
        (
            "earth_village_ruffians",
            "Earth Village Ruffians",
            "{2}{B/G}",
            &[][..],
            &["Creature", "Human", "Soldier", "Rogue"][..],
            Some((3, 1)),
        ),
        (
            "haru,_hidden_talent",
            "Haru, Hidden Talent",
            "{1}{G}",
            &["Legendary"][..],
            &["Creature", "Human", "Peasant", "Ally"][..],
            Some((1, 1)),
        ),
        (
            "toph,_the_first_metalbender",
            "Toph, the First Metalbender",
            "{1}{R}{G}{W}",
            &["Legendary"][..],
            &["Creature", "Human", "Warrior", "Ally"][..],
            Some((3, 3)),
        ),
        (
            "rockalanche",
            "Rockalanche",
            "{2}{G}",
            &[][..],
            &["Sorcery", "Lesson"][..],
            None,
        ),
        (
            "sandbenders_storm",
            "Sandbenders' Storm",
            "{3}{W}",
            &[][..],
            &["Instant"][..],
            None,
        ),
        (
            "fatal_fissure",
            "Fatal Fissure",
            "{1}{B}",
            &[][..],
            &["Instant"][..],
            None,
        ),
        (
            "the_cave_of_two_lovers",
            "The Cave of Two Lovers",
            "{3}{R}",
            &[][..],
            &["Enchantment", "Saga"][..],
            None,
        ),
    ] {
        let card = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(card.layout, Layout::Normal, "{id}");
        let face = card.primary_face();
        assert_eq!(face.name, name, "{id}");
        assert_eq!(face.mana_cost.to_string(), mana, "{id}");
        assert_eq!(face.supertypes, supertypes, "{id}");
        assert_eq!(face.types, types, "{id}");
        assert_eq!(face.power.zip(face.toughness), body, "{id}");
    }
}
