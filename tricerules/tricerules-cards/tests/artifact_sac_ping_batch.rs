//! Registry conformance for the queue-v19 artifact sacrifice-ping cohort.
//!
//! Every included card is a normal-layout artifact with one activated ability that
//! sacrifices it as a cost to deal printed damage to a target creature. The shared
//! scenario executes every card. Each fingerprint is SHA-256 of the exact pinned
//! normalized Oracle text.

use tricerules_cards::primitives::{AbilityCost, Amount, SpellEffectKind, TargetKind};
use tricerules_cards::{AbilityPresentation, AbilitySourceZone, ActivationTiming, Layout};

type Card = (
    &'static str,
    &'static str,
    &'static str,
    u32,
    &'static str,
    &'static str,
);

const CARDS: &[Card] = &[
    (
        "disruptor_pistol",
        "Disruptor Pistol",
        "{2}",
        5,
        "{4}",
        "f609ab020fcaafba23629e8fa4fc350de0f419ebf729297d66d6592864842efc",
    ),
    (
        "flamecast_wheel",
        "Flamecast Wheel",
        "{1}",
        3,
        "{5}",
        "61fbc7bd3929d0b1ed9589a92135d738efde0848958d7f73de966a5670958514",
    ),
    (
        "scalding_cauldron",
        "Scalding Cauldron",
        "{1}",
        3,
        "{3}",
        "d7a037188cc0454ed6cb2953e1113cf647cd5d83fb2eaf04333fd253341a53de",
    ),
];

#[test]
fn artifact_sac_ping_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, mana, amount, activation, _fingerprint) in CARDS {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        assert_eq!(definition.name, name, "{id}");
        assert_eq!(registry.id_for_name(name), Some(id), "{id}");
        let face = definition.primary_face();
        assert_eq!(face.face_id.as_str(), id, "{id} face id");
        assert_eq!(face.mana_cost.to_string(), mana, "{id} mana cost");
        assert_eq!(face.types, ["Artifact"], "{id} types");
        assert!(face.keywords.is_empty(), "{id} keywords");
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
        assert_eq!(
            ability.source_zone,
            AbilitySourceZone::Battlefield,
            "{id} source zone"
        );
        assert_eq!(ability.timing, ActivationTiming::Normal, "{id} timing");
        assert!(
            matches!(
                ability.costs.as_slice(),
                [
                    AbilityCost::Mana(cost),
                    AbilityCost::Tap,
                    AbilityCost::SacrificeSelf
                ] if cost.to_string() == activation
            ),
            "{id}: {:?}",
            ability.costs
        );
        assert!(
            matches!(
                ability.effect.as_slice(),
                [SpellEffectKind::DamageTarget {
                    amount: Amount::Fixed(printed),
                    target,
                }] if *printed == amount && target.kind == TargetKind::Creature
            ),
            "{id}: {:?}",
            ability.effect
        );
        assert!(
            ability
                .targeting
                .as_ref()
                .is_some_and(|targeting| targeting.groups.len() == 1),
            "{id}: one declared target group"
        );
    }
}

#[test]
fn artifact_sac_ping_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for &(id, name, _mana, _amount, _activation, fingerprint) in CARDS {
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
