//! Registry conformance for the reviewed "This creature can't block" cohort.
//!
//! Every included card is a normal-layout creature whose only rules text is the
//! unconditional, source-only can't-block restriction.
//! Each fingerprint is SHA-256 of the exact pinned normalized Oracle text.

use tricerules_cards::primitives::StaticAbilityDef;
use tricerules_cards::{AbilityPresentation, Layout};

type Card = (
    &'static str,
    &'static str,
    &'static str,
    &'static [&'static str],
    &'static [&'static str],
    Option<(u32, u32)>,
    &'static str,
);

const CARDS: &[Card] = &[
    (
        "ashenmoor_gouger",
        "Ashenmoor Gouger",
        "{B/R}{B/R}{B/R}",
        &["Creature", "Elemental", "Warrior"],
        &[],
        Some((4, 4)),
        "6789c3591d527fd5afdf867fd8025debf5f054e7c7b4f17a17a7b2153cbbb60c",
    ),
    (
        "craven_giant",
        "Craven Giant",
        "{2}{R}",
        &["Creature", "Giant"],
        &[],
        Some((4, 1)),
        "6789c3591d527fd5afdf867fd8025debf5f054e7c7b4f17a17a7b2153cbbb60c",
    ),
    (
        "craven_knight",
        "Craven Knight",
        "{1}{B}",
        &["Creature", "Human", "Knight"],
        &[],
        Some((2, 2)),
        "6789c3591d527fd5afdf867fd8025debf5f054e7c7b4f17a17a7b2153cbbb60c",
    ),
    (
        "goblin_raider",
        "Goblin Raider",
        "{1}{R}",
        &["Creature", "Goblin", "Warrior"],
        &[],
        Some((2, 2)),
        "6789c3591d527fd5afdf867fd8025debf5f054e7c7b4f17a17a7b2153cbbb60c",
    ),
    (
        "hulking_cyclops",
        "Hulking Cyclops",
        "{3}{R}{R}",
        &["Creature", "Cyclops"],
        &[],
        Some((5, 5)),
        "6789c3591d527fd5afdf867fd8025debf5f054e7c7b4f17a17a7b2153cbbb60c",
    ),
    (
        "hulking_goblin",
        "Hulking Goblin",
        "{1}{R}",
        &["Creature", "Goblin"],
        &[],
        Some((2, 2)),
        "6789c3591d527fd5afdf867fd8025debf5f054e7c7b4f17a17a7b2153cbbb60c",
    ),
    (
        "hulking_ogre",
        "Hulking Ogre",
        "{2}{R}",
        &["Creature", "Ogre"],
        &[],
        Some((3, 3)),
        "6789c3591d527fd5afdf867fd8025debf5f054e7c7b4f17a17a7b2153cbbb60c",
    ),
    (
        "jungle_lion",
        "Jungle Lion",
        "{G}",
        &["Creature", "Cat"],
        &[],
        Some((2, 1)),
        "6789c3591d527fd5afdf867fd8025debf5f054e7c7b4f17a17a7b2153cbbb60c",
    ),
    (
        "ogre_taskmaster",
        "Ogre Taskmaster",
        "{3}{R}",
        &["Creature", "Ogre"],
        &[],
        Some((4, 3)),
        "6789c3591d527fd5afdf867fd8025debf5f054e7c7b4f17a17a7b2153cbbb60c",
    ),
    (
        "scavenging_scarab",
        "Scavenging Scarab",
        "{3}{B}",
        &["Creature", "Insect"],
        &[],
        Some((3, 3)),
        "6789c3591d527fd5afdf867fd8025debf5f054e7c7b4f17a17a7b2153cbbb60c",
    ),
    (
        "spineless_thug",
        "Spineless Thug",
        "{1}{B}",
        &["Creature", "Phyrexian", "Zombie", "Mercenary"],
        &[],
        Some((2, 2)),
        "6789c3591d527fd5afdf867fd8025debf5f054e7c7b4f17a17a7b2153cbbb60c",
    ),
    (
        "yellow_scarves_troops",
        "Yellow Scarves Troops",
        "{1}{R}",
        &["Creature", "Human", "Soldier"],
        &[],
        Some((2, 2)),
        "6789c3591d527fd5afdf867fd8025debf5f054e7c7b4f17a17a7b2153cbbb60c",
    ),
    (
        "young_wei_recruits",
        "Young Wei Recruits",
        "{1}{B}",
        &["Creature", "Human", "Soldier"],
        &[],
        Some((2, 2)),
        "6789c3591d527fd5afdf867fd8025debf5f054e7c7b4f17a17a7b2153cbbb60c",
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
fn cant_block_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, mana, types, supertypes, body, _) in CARDS {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        assert_eq!(registry.id_for_name(name), Some(id), "{id}");
        let f = face(registry, id);
        assert_eq!(f.face_id.as_str(), id.replace('-', "_"), "{id} face id");
        assert_eq!(f.name, name, "{id} name");
        assert_eq!(f.mana_cost.to_string(), mana, "{id} mana cost");
        assert_eq!(
            f.types.iter().map(String::as_str).collect::<Vec<_>>(),
            types,
            "{id} types"
        );
        assert_eq!(
            f.supertypes.iter().map(String::as_str).collect::<Vec<_>>(),
            supertypes,
            "{id} supertypes"
        );
        assert_eq!(
            (f.power, f.toughness),
            body.map_or((None, None), |(p, t)| (Some(p), Some(t))),
            "{id} body"
        );
        assert!(f.spell_effect.is_empty(), "{id}");
        assert!(f.targeting.is_none(), "{id}");
        assert!(f.keywords.is_empty(), "{id}");
        assert!(f.protections.is_empty(), "{id}");
        assert!(f.triggered_abilities.is_empty(), "{id}");
        assert!(f.activated_abilities.is_empty(), "{id}");
        let [ability] = f.static_abilities.as_slice() else {
            panic!("{id}: exactly one static ability");
        };
        assert_eq!(ability.ability_id.as_str(), "static_01", "{id}");
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{id} presentation"
        );
        assert!(
            matches!(
                &ability.definition,
                StaticAbilityDef::SelfCombatRestriction {
                    restriction,
                    condition: None,
                } if restriction.cant_block && !restriction.cant_be_blocked
            ),
            "{id}: must publish the can't-block restriction"
        );
    }
}

#[test]
fn cant_block_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for &(id, name, _, _, _, _, expected) in CARDS {
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
