//! Registry conformance for the reviewed forestwalk creature family.
//!
//! Every card is a normal-layout creature whose only rules text is Forest landwalk. Card names,
//! costs, types and stats are printed data; the shared scenario executes every card. Each
//! fingerprint is SHA-256 of the exact pinned normalized Oracle text.

use tricerules_cards::primitives::Evasion;
use tricerules_cards::Layout;

type Card = (
    &'static str,
    &'static str,
    &'static str,
    &'static [&'static str],
    u32,
    u32,
);

const CARDS: &[Card] = &[
    (
        "cat_warriors",
        "Cat Warriors",
        "{1}{G}{G}",
        &["Creature", "Cat", "Warrior"],
        2,
        2,
    ),
    (
        "elite_cat_warrior",
        "Elite Cat Warrior",
        "{2}{G}",
        &["Creature", "Cat", "Warrior"],
        2,
        3,
    ),
    (
        "emerald_oryx",
        "Emerald Oryx",
        "{3}{G}",
        &["Creature", "Antelope"],
        2,
        3,
    ),
    (
        "heartwood_treefolk",
        "Heartwood Treefolk",
        "{2}{G}{G}",
        &["Creature", "Treefolk"],
        3,
        4,
    ),
    (
        "jukai_messenger",
        "Jukai Messenger",
        "{G}",
        &["Creature", "Human", "Monk"],
        1,
        1,
    ),
    (
        "koths_courier",
        "Koth's Courier",
        "{1}{R}{R}",
        &["Creature", "Human", "Rogue"],
        2,
        3,
    ),
    (
        "leaf_dancer",
        "Leaf Dancer",
        "{1}{G}{G}",
        &["Creature", "Centaur"],
        2,
        2,
    ),
    ("lynx", "Lynx", "{1}{G}", &["Creature", "Cat"], 2, 1),
    (
        "rushwood_dryad",
        "Rushwood Dryad",
        "{1}{G}",
        &["Creature", "Dryad"],
        2,
        1,
    ),
    (
        "slinking_serpent",
        "Slinking Serpent",
        "{2}{U}{B}",
        &["Creature", "Serpent"],
        2,
        3,
    ),
    (
        "somberwald_dryad",
        "Somberwald Dryad",
        "{1}{G}",
        &["Creature", "Dryad"],
        2,
        2,
    ),
    (
        "willow_dryad",
        "Willow Dryad",
        "{G}",
        &["Creature", "Dryad"],
        1,
        1,
    ),
    (
        "zendikar_farguide",
        "Zendikar Farguide",
        "{4}{G}",
        &["Creature", "Elemental"],
        3,
        3,
    ),
    (
        "zodiac_monkey",
        "Zodiac Monkey",
        "{1}{G}",
        &["Creature", "Monkey"],
        2,
        1,
    ),
    (
        "zodiac_rabbit",
        "Zodiac Rabbit",
        "{G}",
        &["Creature", "Rabbit"],
        1,
        1,
    ),
    (
        "zodiac_tiger",
        "Zodiac Tiger",
        "{2}{G}{G}",
        &["Creature", "Cat"],
        3,
        4,
    ),
];

type Fingerprint = (&'static str, &'static str, &'static str);

const FINGERPRINTS: &[Fingerprint] = &[
    (
        "cat_warriors",
        "Cat Warriors",
        "51079cf0d07f8f1627a7be856aa5fc573b57dacd83da36b54ef8dfd97abf321a",
    ),
    (
        "elite_cat_warrior",
        "Elite Cat Warrior",
        "51079cf0d07f8f1627a7be856aa5fc573b57dacd83da36b54ef8dfd97abf321a",
    ),
    (
        "emerald_oryx",
        "Emerald Oryx",
        "51079cf0d07f8f1627a7be856aa5fc573b57dacd83da36b54ef8dfd97abf321a",
    ),
    (
        "heartwood_treefolk",
        "Heartwood Treefolk",
        "51079cf0d07f8f1627a7be856aa5fc573b57dacd83da36b54ef8dfd97abf321a",
    ),
    (
        "jukai_messenger",
        "Jukai Messenger",
        "51079cf0d07f8f1627a7be856aa5fc573b57dacd83da36b54ef8dfd97abf321a",
    ),
    (
        "koths_courier",
        "Koth's Courier",
        "51079cf0d07f8f1627a7be856aa5fc573b57dacd83da36b54ef8dfd97abf321a",
    ),
    (
        "leaf_dancer",
        "Leaf Dancer",
        "51079cf0d07f8f1627a7be856aa5fc573b57dacd83da36b54ef8dfd97abf321a",
    ),
    (
        "lynx",
        "Lynx",
        "51079cf0d07f8f1627a7be856aa5fc573b57dacd83da36b54ef8dfd97abf321a",
    ),
    (
        "rushwood_dryad",
        "Rushwood Dryad",
        "51079cf0d07f8f1627a7be856aa5fc573b57dacd83da36b54ef8dfd97abf321a",
    ),
    (
        "slinking_serpent",
        "Slinking Serpent",
        "51079cf0d07f8f1627a7be856aa5fc573b57dacd83da36b54ef8dfd97abf321a",
    ),
    (
        "somberwald_dryad",
        "Somberwald Dryad",
        "51079cf0d07f8f1627a7be856aa5fc573b57dacd83da36b54ef8dfd97abf321a",
    ),
    (
        "willow_dryad",
        "Willow Dryad",
        "51079cf0d07f8f1627a7be856aa5fc573b57dacd83da36b54ef8dfd97abf321a",
    ),
    (
        "zendikar_farguide",
        "Zendikar Farguide",
        "51079cf0d07f8f1627a7be856aa5fc573b57dacd83da36b54ef8dfd97abf321a",
    ),
    (
        "zodiac_monkey",
        "Zodiac Monkey",
        "51079cf0d07f8f1627a7be856aa5fc573b57dacd83da36b54ef8dfd97abf321a",
    ),
    (
        "zodiac_rabbit",
        "Zodiac Rabbit",
        "51079cf0d07f8f1627a7be856aa5fc573b57dacd83da36b54ef8dfd97abf321a",
    ),
    (
        "zodiac_tiger",
        "Zodiac Tiger",
        "51079cf0d07f8f1627a7be856aa5fc573b57dacd83da36b54ef8dfd97abf321a",
    ),
];

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
fn forestwalk_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, mana, types, power, toughness) in CARDS {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        let f = face(registry, id);
        assert_eq!(f.face_id.as_str(), id, "{id} face id");
        assert_eq!(f.name, name, "{id} name");
        assert_eq!(f.mana_cost.to_string(), mana, "{id} mana cost");
        assert_eq!(f.types, types, "{id} types");
        assert_eq!(f.power, Some(power), "{id} power");
        assert_eq!(f.toughness, Some(toughness), "{id} toughness");
        assert_eq!(
            f.evasions,
            [Evasion::Landwalk {
                land_subtype: "Forest".to_string()
            }],
            "{id} forestwalk"
        );
        assert!(
            f.static_abilities.is_empty(),
            "{id}: no additional static abilities"
        );
        assert!(
            f.activated_abilities.is_empty(),
            "{id}: no activated abilities"
        );
        assert!(
            f.triggered_abilities.is_empty(),
            "{id}: no triggered abilities"
        );
    }
}

#[test]
fn forestwalk_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for &(id, name, fingerprint) in FINGERPRINTS {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")))
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}"));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[0], id);
        assert_eq!(fields[1], name);
        assert_eq!(fields[2], id);
        assert_eq!(fields[4], fingerprint, "fingerprint drift for {id}");
    }
}
