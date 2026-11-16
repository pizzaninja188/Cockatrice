//! Registry conformance for the queue-v20 tap-to-grant-haste cohort.
//!
//! Every included card is a normal-layout creature whose only printed ability is
//! one `{T}: Target creature gains haste until end of turn.` activated ability.
//! Card names, costs, types and bodies are printed data; the shared scenario
//! executes every card. Each fingerprint is SHA-256 of the exact pinned
//! normalized Oracle text.

use tricerules_cards::primitives::{
    AbilityCost, EffectSubject, SpellEffectKind, TargetController, TargetKind,
};
use tricerules_cards::{AbilityPresentation, Keyword, Layout};

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
        "akki_drillmaster",
        "Akki Drillmaster",
        "{2}{R}",
        &["Creature", "Goblin", "Shaman"],
        2,
        2,
        "b8b07de2201b3b003b3a0141e76e5ca4ef1a875ac4ba98d8b77ea49f8d392ab9",
    ),
    (
        "bloodlust_inciter",
        "Bloodlust Inciter",
        "{R}",
        &["Creature", "Human", "Warrior"],
        1,
        1,
        "4ab734c15fcd75f97a3ed08a177b89a62946f112c86ebea2b5f8048ce89ee1b0",
    ),
    (
        "goblin_motivator",
        "Goblin Motivator",
        "{R}",
        &["Creature", "Goblin", "Warrior"],
        1,
        1,
        "4ab734c15fcd75f97a3ed08a177b89a62946f112c86ebea2b5f8048ce89ee1b0",
    ),
];

fn check_card(id: &str, name: &str, mana: &str, types: &[&str], power: u32, toughness: u32) {
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
    assert!(
        matches!(ability.costs.as_slice(), [AbilityCost::Tap]),
        "{id}: only the tap cost, got {:?}",
        ability.costs
    );
    let [SpellEffectKind::GrantKeywords {
        subject: EffectSubject::Chosen(filter),
        keywords,
    }] = ability.effect.as_slice()
    else {
        panic!("{id}: one chosen haste grant");
    };
    assert_eq!(keywords.as_slice(), [Keyword::Haste], "{id} keywords");
    assert_eq!(filter.kind, TargetKind::Creature, "{id}");
    assert_eq!(filter.controller, TargetController::Any, "{id}");
    let Some(targeting) = ability.targeting.as_ref() else {
        panic!("{id}: target group");
    };
    let [group] = targeting.groups.as_slice() else {
        panic!("{id}: one target group");
    };
    assert_eq!((group.min, group.max), (1, 1), "{id} target cardinality");
    assert_eq!(group.effect_indices, [0], "{id} effect index");
}

#[test]
fn tap_grant_haste_batch_canary_cards_match_their_typed_shape() {
    check_card(
        "bloodlust_inciter",
        "Bloodlust Inciter",
        "{R}",
        &["Creature", "Human", "Warrior"],
        1,
        1,
    );
    check_card(
        "akki_drillmaster",
        "Akki Drillmaster",
        "{2}{R}",
        &["Creature", "Goblin", "Shaman"],
        2,
        2,
    );
}

#[test]
fn tap_grant_haste_batch_maps_definitions() {
    for &(id, name, mana, types, power, toughness, _fingerprint) in CARDS {
        check_card(id, name, mana, types, power, toughness);
    }
}

#[test]
fn tap_grant_haste_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for &(id, name, _mana, _types, _power, _toughness, fingerprint) in CARDS {
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
