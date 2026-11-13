//! Registry conformance for the reviewed Mountainwalk landwalk creature family.
//!
//! Every card is a normal-layout creature whose only rules text is Mountainwalk landwalk.
//! Each fingerprint is SHA-256 of the exact pinned normalized Oracle text.

use tricerules_cards::primitives::Evasion;
use tricerules_cards::Layout;

type Card = (
    &'static str,
    &'static str,
    &'static str,
    &'static [&'static str],
    u32,
    u32,
    &'static str,
);

const CARDS: &[Card] = &[
    (
        "canyon_wildcat",
        "Canyon Wildcat",
        "{1}{R}",
        &["Creature", "Cat"],
        2,
        1,
        "36648521c2b80dcf87dc9044df5c8db0bd48216b4a0a33623e6f07ebf574e0a6",
    ),
    (
        "cliff_threader",
        "Cliff Threader",
        "{1}{W}",
        &["Creature", "Kor", "Scout"],
        2,
        1,
        "36648521c2b80dcf87dc9044df5c8db0bd48216b4a0a33623e6f07ebf574e0a6",
    ),
    (
        "dwarven_grunt",
        "Dwarven Grunt",
        "{R}",
        &["Creature", "Dwarf"],
        1,
        1,
        "36648521c2b80dcf87dc9044df5c8db0bd48216b4a0a33623e6f07ebf574e0a6",
    ),
    (
        "glissas_courier",
        "Glissa's Courier",
        "{1}{G}{G}",
        &["Creature", "Phyrexian", "Horror"],
        2,
        3,
        "36648521c2b80dcf87dc9044df5c8db0bd48216b4a0a33623e6f07ebf574e0a6",
    ),
    (
        "goblin_mountaineer",
        "Goblin Mountaineer",
        "{R}",
        &["Creature", "Goblin", "Scout"],
        1,
        1,
        "36648521c2b80dcf87dc9044df5c8db0bd48216b4a0a33623e6f07ebf574e0a6",
    ),
    (
        "goblin_spelunkers",
        "Goblin Spelunkers",
        "{2}{R}",
        &["Creature", "Goblin", "Warrior"],
        2,
        2,
        "36648521c2b80dcf87dc9044df5c8db0bd48216b4a0a33623e6f07ebf574e0a6",
    ),
    (
        "hillcomber_giant",
        "Hillcomber Giant",
        "{2}{W}{W}",
        &["Creature", "Giant", "Scout"],
        3,
        3,
        "36648521c2b80dcf87dc9044df5c8db0bd48216b4a0a33623e6f07ebf574e0a6",
    ),
    (
        "mountain_goat",
        "Mountain Goat",
        "{R}",
        &["Creature", "Goat"],
        1,
        1,
        "36648521c2b80dcf87dc9044df5c8db0bd48216b4a0a33623e6f07ebf574e0a6",
    ),
    (
        "rock_badger",
        "Rock Badger",
        "{4}{R}",
        &["Creature", "Badger", "Beast"],
        3,
        3,
        "36648521c2b80dcf87dc9044df5c8db0bd48216b4a0a33623e6f07ebf574e0a6",
    ),
    (
        "sokenzan_bruiser",
        "Sokenzan Bruiser",
        "{4}{R}",
        &["Creature", "Ogre", "Warrior"],
        3,
        3,
        "36648521c2b80dcf87dc9044df5c8db0bd48216b4a0a33623e6f07ebf574e0a6",
    ),
    (
        "zodiac_dog",
        "Zodiac Dog",
        "{2}{R}",
        &["Creature", "Dog"],
        2,
        2,
        "36648521c2b80dcf87dc9044df5c8db0bd48216b4a0a33623e6f07ebf574e0a6",
    ),
    (
        "zodiac_goat",
        "Zodiac Goat",
        "{R}",
        &["Creature", "Goat"],
        1,
        1,
        "36648521c2b80dcf87dc9044df5c8db0bd48216b4a0a33623e6f07ebf574e0a6",
    ),
];

#[test]
fn mountainwalk_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, mana, types, power, toughness, _) in CARDS {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        assert_eq!(registry.id_for_name(name), Some(id), "{id}");
        let f = face(registry, id);
        assert_eq!(f.face_id.as_str(), id.replace('-', "_"), "{id} face id");
        assert_eq!(f.name, name, "{id} name");
        assert_eq!(f.mana_cost.to_string(), mana, "{id} mana cost");
        assert_eq!(f.types, types, "{id} types");
        assert_eq!(
            (f.power, f.toughness),
            (Some(power), Some(toughness)),
            "{id} body"
        );
        assert_eq!(
            f.evasions,
            [Evasion::Landwalk {
                land_subtype: "Mountain".to_string()
            }],
            "{id} Mountainwalk landwalk"
        );
        assert!(f.keywords.is_empty(), "{id}");
        assert!(f.static_abilities.is_empty(), "{id}");
        assert!(f.activated_abilities.is_empty(), "{id}");
        assert!(f.triggered_abilities.is_empty(), "{id}");
    }
}

fn face<'a>(
    registry: &'a tricerules_cards::CardRegistry,
    id: &str,
) -> &'a tricerules_cards::CardFace {
    registry
        .get(id)
        .unwrap_or_else(|| panic!("missing {id}"))
        .primary_face()
}

#[test]
fn mountainwalk_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for &(id, name, .., expected) in CARDS {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")))
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}"));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[0], id);
        assert_eq!(fields[1], name);
        assert_eq!(fields[2], id.replace('-', "_"));
        assert_eq!(fields[4], expected, "fingerprint drift for {id}");
    }
}
