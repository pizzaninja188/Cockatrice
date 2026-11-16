//! Registry conformance for the queue-v19 vanilla Vehicle cohort.
//!
//! Every included card is a normal-layout Artifact — Vehicle with printed power and
//! toughness, no other rules text and one crew activated ability. The shared scenario
//! executes every card. Each fingerprint is SHA-256 of the exact pinned normalized
//! Oracle text.

use tricerules_cards::primitives::{
    AbilityCost, EffectSubject, PermanentTypeFilter, SpellEffectKind, TargetController, TargetKind,
};
use tricerules_cards::{
    AbilityPresentation, AbilitySourceZone, ActivationTiming, Layout, ObjectContributionKind,
    ObjectPaymentConstraint,
};

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
        "consulate_dreadnought",
        "Consulate Dreadnought",
        "{1}",
        7,
        11,
        6,
        "3d660ff343f9e51a006504bf8c0384029c9a214de9ae0606670f4f842cd5c163",
    ),
    (
        "futurist_sentinel",
        "Futurist Sentinel",
        "{3}{U}",
        6,
        6,
        3,
        "d24821ed3bcf807a9e72522185cff9733d07ae4b276e2de20189349c3ece7ed4",
    ),
    (
        "irontread_crusher",
        "Irontread Crusher",
        "{4}",
        6,
        6,
        3,
        "d24821ed3bcf807a9e72522185cff9733d07ae4b276e2de20189349c3ece7ed4",
    ),
    (
        "sleek_schooner",
        "Sleek Schooner",
        "{3}",
        4,
        3,
        1,
        "21f9b3ac742bc8012ff1acede786491b724a3e13e88aeb652152d43fad7bb833",
    ),
];

#[test]
fn vanilla_vehicles_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, mana, power, toughness, crew, _fingerprint) in CARDS {
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
        assert!(face.keywords.is_empty(), "{id} keywords");
        assert!(face.spell_effect.is_empty(), "{id}");
        assert!(face.targeting.is_none(), "{id}");
        assert!(face.triggered_abilities.is_empty(), "{id}");
        assert!(face.static_abilities.is_empty(), "{id}");

        let [ability] = face.activated_abilities.as_slice() else {
            panic!("{id}: one crew ability");
        };
        assert_eq!(ability.ability_id.as_str(), "crew", "{id}");
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{id} presentation"
        );
        assert_eq!(
            ability.source_zone,
            AbilitySourceZone::Battlefield,
            "{id} source zone"
        );
        assert_eq!(ability.timing, ActivationTiming::Normal, "{id} timing");
        assert!(ability.targeting.is_none(), "{id}: crew does not target");
        let [taps] = ability.costs.as_slice() else {
            panic!("{id}: crew has one cost");
        };
        let AbilityCost::TapPermanents {
            constraint,
            filter,
            exclude_source,
        } = taps
        else {
            panic!("{id}: crew taps creatures");
        };
        assert_eq!(
            *constraint,
            ObjectPaymentConstraint::AggregateMinimum {
                minimum: crew,
                contribution: ObjectContributionKind::CurrentPower,
            },
            "{id}"
        );
        assert_eq!(filter.kind, TargetKind::Creature, "{id}");
        assert_eq!(filter.controller, TargetController::You, "{id}");
        assert!(*exclude_source, "{id}");
        assert!(
            matches!(
                ability.effect.as_slice(),
                [SpellEffectKind::AddTypes {
                    subject: EffectSubject::Source,
                    addition,
                }] if addition.card_types == [PermanentTypeFilter::Creature]
                    && addition.creature_types.is_empty()
            ),
            "{id}: {:?}",
            ability.effect
        );
    }
}

#[test]
fn vanilla_vehicles_batch_fingerprints_match_the_pinned_oracle_text() {
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
