//! Registry conformance for the queue-v20 cycling keyword cohort.
//!
//! Every included card is a normal-layout creature with a printed body and one
//! `cycling {cost}` hand-zone activated ability and no other ability. Card
//! names, costs, types, stats and cycling costs are printed data; the shared
//! scenario executes every card. Each fingerprint is SHA-256 of the exact pinned
//! normalized Oracle text.

use tricerules_cards::primitives::{AbilityCost, AbilitySourceZone, Amount, SpellEffectKind};
use tricerules_cards::{AbilityPresentation, Layout};

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
        "desert_cerodon",
        "Desert Cerodon",
        "{5}{R}",
        &["Creature", "Beast"],
        6,
        4,
        "{R}",
        "dd222da4da5142187e079b2b0c1dae409f6fea22fbfdd1bb82d03e3884d9dae3",
    ),
    (
        "hundroog",
        "Hundroog",
        "{6}{G}",
        &["Creature", "Beast"],
        4,
        7,
        "{3}",
        "60ae32363500017272ad6c4ee3f55355784cc331c295b36b5b75153fd87d73fe",
    ),
    (
        "lurching_rotbeast",
        "Lurching Rotbeast",
        "{3}{B}",
        &["Creature", "Zombie", "Beast"],
        4,
        2,
        "{B}",
        "a465e0ef3df0c373204b03340f14c2fadb5a290ea5d53f6e651f57cc6a8b20c0",
    ),
];

fn check_card(
    id: &str,
    name: &str,
    mana: &str,
    types: &[&str],
    power: u32,
    toughness: u32,
    cycling: &str,
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
        panic!("{id}: one cycling ability");
    };
    assert_eq!(ability.ability_id.as_str(), "activated_01", "{id}");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1]),
        "{id} presentation"
    );
    assert_eq!(
        ability.source_zone,
        AbilitySourceZone::Hand,
        "{id}: cycling is a hand ability"
    );
    let [AbilityCost::Mana(mana_cost), AbilityCost::DiscardSelf] = ability.costs.as_slice() else {
        panic!("{id}: cycling mana plus discard-self costs");
    };
    assert_eq!(mana_cost.to_string(), cycling, "{id} cycling cost");
    assert!(
        matches!(
            ability.effect.as_slice(),
            [SpellEffectKind::Draw { count, .. }] if *count == Amount::Fixed(1)
        ),
        "{id}: {:?}",
        ability.effect
    );
    assert!(ability.targeting.is_none(), "{id}: cycling does not target");
}

#[test]
fn cycling_keyword_batch_canary_cards_match_their_typed_shape() {
    check_card(
        "desert_cerodon",
        "Desert Cerodon",
        "{5}{R}",
        &["Creature", "Beast"],
        6,
        4,
        "{R}",
    );
    check_card(
        "hundroog",
        "Hundroog",
        "{6}{G}",
        &["Creature", "Beast"],
        4,
        7,
        "{3}",
    );
}

#[test]
fn cycling_keyword_batch_maps_definitions() {
    for &(id, name, mana, types, power, toughness, cycling, _fingerprint) in CARDS {
        check_card(id, name, mana, types, power, toughness, cycling);
    }
}

#[test]
fn cycling_keyword_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for &(id, name, _mana, _types, _power, _toughness, _cycling, fingerprint) in CARDS {
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
