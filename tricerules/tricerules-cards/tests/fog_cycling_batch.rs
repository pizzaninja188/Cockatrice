//! Registry conformance for the queue-v19 combat-damage prevention cycling cohort.
//!
//! Every included card is a normal-layout instant that prevents all combat damage this
//! turn and has a hand-zone cycling activated ability. The shared scenario executes
//! every card. Each fingerprint is SHA-256 of the exact pinned normalized Oracle text.

use tricerules_cards::primitives::{AbilityCost, Amount, SpellEffectKind};
use tricerules_cards::{AbilityPresentation, AbilitySourceZone, ActivationTiming, Layout};

type Card = (
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    &'static str,
);

const CARDS: &[Card] = &[
    (
        "angelsong",
        "Angelsong",
        "{1}{W}",
        "{2}",
        "a996b35f1f393edd0a35e3193e851d2cc1ed529e6e3923d1979ef9eb6f1bfdac",
    ),
    (
        "haze_of_pollen",
        "Haze of Pollen",
        "{1}{G}",
        "{3}",
        "0b62232afb18f4db31c32cc8e3f05e163aae8441c4ab0582cba86ab5e1c96c95",
    ),
    (
        "lull",
        "Lull",
        "{1}{G}",
        "{2}",
        "a996b35f1f393edd0a35e3193e851d2cc1ed529e6e3923d1979ef9eb6f1bfdac",
    ),
];

#[test]
fn fog_cycling_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, mana, cycle, _fingerprint) in CARDS {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        assert_eq!(definition.name, name, "{id}");
        assert_eq!(registry.id_for_name(name), Some(id), "{id}");
        let face = definition.primary_face();
        assert_eq!(face.face_id.as_str(), id, "{id} face id");
        assert_eq!(face.mana_cost.to_string(), mana, "{id} mana cost");
        assert_eq!(face.types, ["Instant"], "{id} types");
        assert!(face.keywords.is_empty(), "{id} keywords");
        assert!(face.targeting.is_none(), "{id}");
        assert!(face.triggered_abilities.is_empty(), "{id}");
        assert!(face.static_abilities.is_empty(), "{id}");
        assert!(
            matches!(
                face.spell_effect.as_slice(),
                [SpellEffectKind::PreventAllCombatDamageTurn]
            ),
            "{id}: {:?}",
            face.spell_effect
        );

        let [ability] = face.activated_abilities.as_slice() else {
            panic!("{id}: one cycling ability");
        };
        assert_eq!(ability.ability_id.as_str(), "activated_01", "{id}");
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![2]),
            "{id} presentation"
        );
        assert_eq!(
            ability.source_zone,
            AbilitySourceZone::Hand,
            "{id} source zone"
        );
        assert_eq!(ability.timing, ActivationTiming::Normal, "{id} timing");
        assert!(
            matches!(
                ability.costs.as_slice(),
                [AbilityCost::Mana(cost), AbilityCost::DiscardSelf] if cost.to_string() == cycle
            ),
            "{id}: {:?}",
            ability.costs
        );
        assert!(
            matches!(
                ability.effect.as_slice(),
                [SpellEffectKind::Draw {
                    count: Amount::Fixed(1),
                    ..
                }]
            ),
            "{id}: {:?}",
            ability.effect
        );
        assert!(ability.targeting.is_none(), "{id}: cycling does not target");
    }
}

#[test]
fn fog_cycling_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for &(id, name, _mana, _cycle, fingerprint) in CARDS {
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
