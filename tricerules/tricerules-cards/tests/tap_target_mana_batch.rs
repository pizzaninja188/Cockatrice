//! Registry conformance for the queue-v20 mana-plus-tap tapper cohort.
//!
//! Every included card is a normal-layout creature or artifact whose only
//! printed ability is one `{cost}, {T}: Tap target creature.` activated ability.
//! Card names, costs, types and bodies are printed data; the shared scenario
//! executes every card. Each fingerprint is SHA-256 of the exact pinned
//! normalized Oracle text.

use tricerules_cards::primitives::{
    AbilityCost, EffectSubject, SpellEffectKind, TargetController, TargetKind,
};
use tricerules_cards::{AbilityPresentation, Layout};

type Card = (
    &'static str,
    &'static str,
    &'static str,
    &'static [&'static str],
    Option<(u32, u32)>,
    &'static str,
    &'static str,
);

const CARDS: &[Card] = &[
    (
        "akroan_jailer",
        "Akroan Jailer",
        "{W}",
        &["Creature", "Human", "Soldier"],
        Some((1, 1)),
        "{2}{W}",
        "560c7551471b8089cf3d507a9698525df20b69f3944797c236f5ed4c6af7b480",
    ),
    (
        "blinding_souleater",
        "Blinding Souleater",
        "{3}",
        &["Artifact", "Creature", "Phyrexian", "Cleric"],
        Some((1, 3)),
        "{W/P}",
        "c05678c0f3b58e0b4ef3a3df8aacf15dfa38345bd463306c7110128442d07eef",
    ),
    (
        "checkpoint_officer",
        "Checkpoint Officer",
        "{1}{W}",
        &["Creature", "Human", "Soldier"],
        Some((1, 2)),
        "{1}{W}",
        "d4a6526cc5f965f32891eb8878e54bd2e945eb203388b5215b4ca22710fe1ddd",
    ),
    (
        "elite_arrester",
        "Elite Arrester",
        "{W}",
        &["Creature", "Human", "Soldier"],
        Some((0, 3)),
        "{1}{U}",
        "595accbde7f6f7d10a8e0d03010d6cad1ca3f86dab8dab3bdd4c4a23a572faea",
    ),
    (
        "rathi_trapper",
        "Rathi Trapper",
        "{1}{B}",
        &["Creature", "Human", "Rebel", "Rogue"],
        Some((1, 2)),
        "{B}",
        "4a75d6080fe8989e70ee391067d8c39b1b0dac58cc2d017276422d043ed8a2c3",
    ),
    (
        "tyrants_machine",
        "Tyrant's Machine",
        "{2}",
        &["Artifact"],
        None,
        "{4}",
        "eac4c9014cfaace86c3e807e05c6c9b7fa3d3052104e0441768e55ffcc7949b7",
    ),
];

fn check_card(
    id: &str,
    name: &str,
    mana: &str,
    types: &[&str],
    body: Option<(u32, u32)>,
    cost: &str,
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
        body.map_or((None, None), |(p, t)| (Some(p), Some(t))),
        "{id} body"
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
    let [AbilityCost::Mana(mana_cost), AbilityCost::Tap] = ability.costs.as_slice() else {
        panic!("{id}: mana plus tap costs");
    };
    assert_eq!(mana_cost.to_string(), cost, "{id} activation mana");
    let [SpellEffectKind::Tap {
        subject: EffectSubject::Chosen(filter),
    }] = ability.effect.as_slice()
    else {
        panic!("{id}: one chosen tap effect");
    };
    assert_eq!(filter.kind, TargetKind::Creature, "{id}");
    assert!(filter.permanent_types.is_empty(), "{id}");
    assert_eq!(
        filter.controller,
        TargetController::Any,
        "{id}: any player's creature"
    );
}

#[test]
fn tap_target_mana_batch_canary_cards_match_their_typed_shape() {
    check_card(
        "rathi_trapper",
        "Rathi Trapper",
        "{1}{B}",
        &["Creature", "Human", "Rebel", "Rogue"],
        Some((1, 2)),
        "{B}",
    );
    check_card(
        "blinding_souleater",
        "Blinding Souleater",
        "{3}",
        &["Artifact", "Creature", "Phyrexian", "Cleric"],
        Some((1, 3)),
        "{W/P}",
    );
}

#[test]
fn tap_target_mana_batch_maps_definitions() {
    for &(id, name, mana, types, body, cost, _fingerprint) in CARDS {
        check_card(id, name, mana, types, body, cost);
    }
}

#[test]
fn tap_target_mana_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for &(id, name, _mana, _types, _body, _cost, fingerprint) in CARDS {
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
