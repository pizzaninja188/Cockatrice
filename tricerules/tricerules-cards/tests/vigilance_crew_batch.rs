//! Registry conformance for the queue-v20 vigilance Vehicle cohort.
//!
//! Every included card is a normal-layout Artifact — Vehicle with printed
//! power/toughness, the vigilance keyword and one crew ability; no other
//! ability. Card names, costs, bodies and crew thresholds are printed data;
//! the shared scenario executes every card. Each fingerprint is SHA-256 of the
//! exact pinned normalized Oracle text.

use tricerules_cards::primitives::{
    AbilityCost, EffectSubject, ObjectContributionKind, SpellEffectKind, TargetController,
    TargetKind,
};
use tricerules_cards::{AbilityPresentation, ActivationTiming, Keyword, Layout};

type Card = (
    &'static str,
    &'static str,
    &'static str,
    u32,
    u32,
    u32,
    &'static str,
);

const CARDS: &[Card] = &[
    (
        "brute_suit",
        "Brute Suit",
        "{3}",
        4,
        3,
        1,
        "380526d060b736abfbcbc68dc933fa8c90fda6c50f572e8d9e00484d328b8e2d",
    ),
    (
        "dusk_legion_dreadnought",
        "Dusk Legion Dreadnought",
        "{5}",
        4,
        6,
        2,
        "0ada12bc89e9434b19938a54f72af4425e424900313a17dd47d064226e86339f",
    ),
    (
        "funeral_longboat",
        "Funeral Longboat",
        "{2}",
        3,
        3,
        1,
        "380526d060b736abfbcbc68dc933fa8c90fda6c50f572e8d9e00484d328b8e2d",
    ),
];

fn check_card(id: &str, name: &str, mana: &str, power: u32, toughness: u32, crew: u32) {
    let registry = tricerules_cards::registry::global();
    let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
    assert_eq!(definition.layout, Layout::Normal, "{id}");
    assert_eq!(definition.name, name, "{id}");
    assert_eq!(registry.id_for_name(name), Some(id), "{id}");
    let face = definition.primary_face();
    assert_eq!(face.face_id.as_str(), id, "{id} face id");
    assert_eq!(face.mana_cost.to_string(), mana, "{id} mana cost");
    assert_eq!(face.types, ["Artifact", "Vehicle"], "{id} types");
    assert_eq!(
        (face.power, face.toughness),
        (Some(power), Some(toughness)),
        "{id}"
    );
    assert_eq!(face.keywords, [Keyword::Vigilance], "{id} keywords");
    assert!(face.spell_effect.is_empty(), "{id}");
    assert!(face.targeting.is_none(), "{id}");
    assert!(face.triggered_abilities.is_empty(), "{id}");
    assert!(face.static_abilities.is_empty(), "{id}");
    assert!(face.characteristic_defining_abilities.is_empty(), "{id}");

    let [ability] = face.activated_abilities.as_slice() else {
        panic!("{id}: one crew ability");
    };
    assert_eq!(ability.ability_id.as_str(), "crew", "{id}");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![2]),
        "{id} presentation"
    );
    assert_eq!(ability.timing, ActivationTiming::Normal, "{id} timing");
    let [AbilityCost::TapPermanents {
        constraint,
        filter,
        exclude_source,
    }] = ability.costs.as_slice()
    else {
        panic!("{id}: tap-permanents crew cost");
    };
    assert_eq!(
        constraint.aggregate_minimum(),
        Some((crew, ObjectContributionKind::CurrentPower)),
        "{id}: crew threshold"
    );
    assert_eq!(filter.kind, TargetKind::Creature, "{id}");
    assert_eq!(filter.controller, TargetController::You, "{id}");
    assert!(*exclude_source, "{id}: crew excludes the source");
    assert!(
        matches!(
            ability.effect.as_slice(),
            [SpellEffectKind::AddTypes {
                subject: EffectSubject::Source,
                ..
            }]
        ),
        "{id}: {:?}",
        ability.effect
    );
    assert!(ability.targeting.is_none(), "{id}: crew does not target");
}

#[test]
fn vigilance_crew_batch_canary_cards_match_their_typed_shape() {
    check_card("brute_suit", "Brute Suit", "{3}", 4, 3, 1);
    check_card(
        "dusk_legion_dreadnought",
        "Dusk Legion Dreadnought",
        "{5}",
        4,
        6,
        2,
    );
}

#[test]
fn vigilance_crew_batch_maps_definitions() {
    for &(id, name, mana, power, toughness, crew, _fingerprint) in CARDS {
        check_card(id, name, mana, power, toughness, crew);
    }
}

#[test]
fn vigilance_crew_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for &(id, name, _mana, _power, _toughness, _crew, fingerprint) in CARDS {
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
