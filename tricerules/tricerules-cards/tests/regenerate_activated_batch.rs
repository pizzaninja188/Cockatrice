//! Registry conformance for the queue-v20 activated regeneration cohort.
//!
//! Every included card is a normal-layout creature or artifact creature with one
//! `{cost}: Regenerate this creature.` activated ability and no other ability.
//! Card names, costs, types and stats are printed data; the shared scenario
//! executes every card. Each fingerprint is SHA-256 of the exact pinned
//! normalized Oracle text.

use tricerules_cards::primitives::{AbilityCost, EffectSubject, SpellEffectKind};
use tricerules_cards::{AbilityPresentation, ActivationTiming, Layout};

type Card = (
    &'static str,
    &'static str,
    &'static str,
    &'static [&'static str],
    u32,
    u32,
    &'static str,
    &'static str,
);

const CARDS: &[Card] = &[
    (
        "asphodel_wanderer",
        "Asphodel Wanderer",
        "{B}",
        &["Creature", "Skeleton", "Soldier"],
        1,
        1,
        "{2}{B}",
        "33240089476fad1f4d01595299852c8fdceaae4dc5be6e359024ea60739568b8",
    ),
    (
        "odious_trow",
        "Odious Trow",
        "{B/G}",
        &["Creature", "Troll"],
        1,
        1,
        "{1}{B/G}",
        "8064b4b5dc626e0fd0eaae65e4550e49295017e160e0131fc436f20d21fc9cc8",
    ),
    (
        "pewter_golem",
        "Pewter Golem",
        "{5}",
        &["Artifact", "Creature", "Golem"],
        4,
        2,
        "{1}{B}",
        "9e95cd7d01f1398d2f1129e846277cf67f8f4b3f53861390c01b970e8b1bd853",
    ),
    (
        "selesnya_sentry",
        "Selesnya Sentry",
        "{2}{W}",
        &["Creature", "Elephant", "Soldier"],
        3,
        2,
        "{5}{G}",
        "ad64e9beb65fc18b9cf475398a0083e1094e81ed47d76437c5a2bf28ead11fb5",
    ),
    (
        "tangle_hulk",
        "Tangle Hulk",
        "{5}",
        &["Artifact", "Creature", "Phyrexian", "Beast"],
        5,
        3,
        "{2}{G}",
        "33294e2b00e51cbf970be2bfd9074cf6c2cb1858dfcd2528f3ad82ea9878cd62",
    ),
    (
        "votary_of_the_conclave",
        "Votary of the Conclave",
        "{W}",
        &["Creature", "Human", "Soldier"],
        1,
        1,
        "{2}{G}",
        "33294e2b00e51cbf970be2bfd9074cf6c2cb1858dfcd2528f3ad82ea9878cd62",
    ),
];

fn check_card(
    id: &str,
    name: &str,
    mana: &str,
    types: &[&str],
    power: u32,
    toughness: u32,
    regenerate: &str,
) {
    let registry = tricerules_cards::registry::global();
    let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
    assert_eq!(definition.layout, Layout::Normal, "{id}");
    assert_eq!(definition.name, name, "{id}");
    assert_eq!(registry.id_for_name(name), Some(id), "{id}");
    let face = definition.primary_face();
    assert_eq!(face.face_id.as_str(), id, "{id} face id");
    assert_eq!(face.mana_cost.to_string(), mana, "{id} mana cost");
    assert_eq!(face.types, types, "{id} types");
    assert_eq!(
        (face.power, face.toughness),
        (Some(power), Some(toughness)),
        "{id}"
    );
    assert!(face.keywords.is_empty(), "{id}");
    assert!(face.spell_effect.is_empty(), "{id}");
    assert!(face.targeting.is_none(), "{id}");
    assert!(face.triggered_abilities.is_empty(), "{id}");
    assert!(face.static_abilities.is_empty(), "{id}");

    let [ability] = face.activated_abilities.as_slice() else {
        panic!("{id}: one activated ability");
    };
    assert_eq!(ability.ability_id.as_str(), "activated_01", "{id}");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1]),
        "{id} presentation"
    );
    assert_eq!(ability.timing, ActivationTiming::Normal, "{id} timing");
    assert!(
        matches!(
            ability.costs.as_slice(),
            [AbilityCost::Mana(cost)] if cost.to_string() == regenerate
        ),
        "{id}: {:?}",
        ability.costs
    );
    assert!(
        matches!(
            ability.effect.as_slice(),
            [SpellEffectKind::Regenerate {
                subject: EffectSubject::Source,
            }]
        ),
        "{id}: {:?}",
        ability.effect
    );
    assert!(ability.targeting.is_none(), "{id}: no targets");
}

#[test]
fn regenerate_activated_batch_canary_cards_match_their_typed_shape() {
    check_card(
        "asphodel_wanderer",
        "Asphodel Wanderer",
        "{B}",
        &["Creature", "Skeleton", "Soldier"],
        1,
        1,
        "{2}{B}",
    );
    check_card(
        "odious_trow",
        "Odious Trow",
        "{B/G}",
        &["Creature", "Troll"],
        1,
        1,
        "{1}{B/G}",
    );
}

#[test]
fn regenerate_activated_batch_maps_definitions() {
    for &(id, name, mana, types, power, toughness, regenerate, _fingerprint) in CARDS {
        check_card(id, name, mana, types, power, toughness, regenerate);
    }
}

#[test]
fn regenerate_activated_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for &(id, name, _mana, _types, _power, _toughness, _regenerate, fingerprint) in CARDS {
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
