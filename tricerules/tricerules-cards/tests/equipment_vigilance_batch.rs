//! Registry conformance for the queue-v19 Equipment vigilance cohort.
//!
//! Every included card is a normal-layout Equipment with one attached P/T modifier
//! that also grants vigilance and one `Equip` activated ability. The shared scenario
//! executes every card. Each fingerprint is SHA-256 of the exact pinned normalized
//! Oracle text.

use tricerules_cards::primitives::{
    AbilityCost, SpellEffectKind, StaticAbilityDef, TargetController, TargetKind,
};
use tricerules_cards::{AbilityPresentation, AbilitySourceZone, ActivationTiming, Keyword, Layout};

type Card = (
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    i32,
    i32,
    &'static str,
);

const CARDS: &[Card] = &[
    (
        "accorders_shield",
        "Accorder's Shield",
        "{0}",
        "{3}",
        0,
        3,
        "19a0ce8047cbabacd81471445f8845106c5f3c65d3dc0c85e3c3053a0f2b437b",
    ),
    (
        "cathars_shield",
        "Cathar's Shield",
        "{0}",
        "{3}",
        0,
        3,
        "8bfc825475a4bbd4d6901068df2f717658989d4eeeb41524192663c6442a7c9e",
    ),
    (
        "kor_halberd",
        "Kor Halberd",
        "{W}",
        "{1}",
        1,
        1,
        "cfa14e587c0bccc459fcc8814132c5a6be5c3ef9f3d114d7c3d1998527b894ad",
    ),
];

#[test]
fn equipment_vigilance_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, mana, equip, dp, dt, _fingerprint) in CARDS {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        assert_eq!(definition.name, name, "{id}");
        assert_eq!(registry.id_for_name(name), Some(id), "{id}");
        let face = definition.primary_face();
        assert_eq!(face.face_id.as_str(), id, "{id} face id");
        assert_eq!(face.mana_cost.to_string(), mana, "{id} mana cost");
        assert_eq!(face.types, ["Artifact", "Equipment"], "{id} types");
        assert!(face.keywords.is_empty(), "{id} keywords");
        assert!(face.spell_effect.is_empty(), "{id}");
        assert!(face.targeting.is_none(), "{id}");
        assert!(face.triggered_abilities.is_empty(), "{id}");

        let [static_ability] = face.static_abilities.as_slice() else {
            panic!("{id}: one static ability");
        };
        assert_eq!(
            static_ability.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{id} static presentation"
        );
        assert!(
            matches!(
                &static_ability.definition,
                StaticAbilityDef::AttachedModifier {
                    delta_power,
                    delta_toughness,
                    keywords,
                    ..
                } if *delta_power == dp
                    && *delta_toughness == dt
                    && keywords == &[Keyword::Vigilance]
            ),
            "{id}: {:?}",
            static_ability.definition
        );

        let [ability] = face.activated_abilities.as_slice() else {
            panic!("{id}: one activated ability");
        };
        assert_eq!(ability.ability_id.as_str(), "activated_01", "{id}");
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![2]),
            "{id} equip presentation"
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
                [AbilityCost::Mana(cost)] if cost.to_string() == equip
            ),
            "{id}: {:?}",
            ability.costs
        );
        assert!(
            matches!(
                ability.effect.as_slice(),
                [SpellEffectKind::Equip { target }]
                    if target.kind == TargetKind::Creature
                        && target.controller == TargetController::You
            ),
            "{id}: {:?}",
            ability.effect
        );
        assert!(
            ability.targeting.is_none(),
            "{id}: equip uses its effect target"
        );
    }
}

#[test]
fn equipment_vigilance_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for &(id, name, _mana, _equip, _dp, _dt, fingerprint) in CARDS {
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
