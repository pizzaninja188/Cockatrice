//! Registry conformance for the issue #359 reviewed direct-RON Spree pair.
//!
//! Explosive Derailment (`b23dc81d-01bb-4bf0-9932-5d32a6b22cf7`) and Unfortunate Accident
//! (`011558f3-3c23-430e-a08b-8f91238e7971`) were promoted after complete-definition review
//! against the pinned Scryfall snapshot. Both print `Spree` and two mutually exclusive modes,
//! each inseparably linked to its own additional mana cost (CR 702.171, CR 700.2h). The exact
//! Scryfall records and `rulings_uri` were fetched 2026-09-20; both returned the 2024-04-12
//! spree rulings, including mode-local targeting and mode-by-mode resolution order.

use tricerules_cards::primitives::{
    Amount, CastCostOptionDef, CastCostOptionRef, EffectSubject, ManaCostChoiceKind,
    PermanentTypeFilter, SpellEffectKind, TargetFilter, TargetGroupDef, TargetKind, TargetingDef,
};
use tricerules_cards::{AbilityPresentation, CardRegistry, ChoiceId, Color, Layout, ModeId};

const EXPLOSIVE_FINGERPRINT: &str =
    "cdccdd54cd9820bd251c15025c28a8ca9597cfdcd4f995418f401e299dee7e2e";
const ACCIDENT_FINGERPRINT: &str =
    "e2fd16a8a9ab84aea32c77126e37d2e08d9bb94908e372170160bed468ba1e0f";

fn single_group(targeting: &TargetingDef) -> &TargetGroupDef {
    let [group] = targeting.groups.as_slice() else {
        panic!("expected exactly one authored target group");
    };
    group
}

fn mana_option(
    group: &tricerules_cards::primitives::CastCostGroupDef,
    index: usize,
) -> (&ChoiceId, ManaCostChoiceKind, String) {
    let CastCostOptionDef::Mana {
        option_id,
        kind,
        cost,
        ..
    } = &group.options[index]
    else {
        panic!("expected a mana cast-cost option");
    };
    (option_id, *kind, cost.to_string())
}

fn linked(option_id: &str) -> CastCostOptionRef {
    CastCostOptionRef {
        group_id: ChoiceId::new("spree").expect("stable spree group"),
        option_id: ChoiceId::new(option_id).expect("stable spree option"),
    }
}

#[test]
fn issue_359_explosive_derailment_maps_every_printed_clause() {
    let registry = CardRegistry::global();
    let definition = registry.get("explosive_derailment").expect("registered");
    assert_eq!(definition.name, "Explosive Derailment");
    assert_eq!(
        registry.id_for_name("Explosive Derailment"),
        Some("explosive_derailment")
    );
    assert_eq!(definition.layout, Layout::Normal);
    assert_eq!(definition.face_count(), 1);
    let face = definition.primary_face();
    assert_eq!(face.face_id.as_str(), "explosive_derailment");
    assert_eq!(face.mana_cost.to_string(), "{R}");
    assert_eq!(face.types, ["Instant"]);
    assert_eq!(face.colors(), vec![Color::Red]);
    assert!(face.keywords.is_empty());
    assert!(face.spell_effect.is_empty());
    assert!(face.triggered_abilities.is_empty());
    assert!(face.activated_abilities.is_empty());
    assert!(face.static_abilities.is_empty());
    assert!(face.custom_effect.is_none());
    assert!(face.targeting.is_none());

    // Oracle line 1: the Spree additional-cost group.
    let [group] = face.cast_cost_groups.as_slice() else {
        panic!("Explosive Derailment has exactly one spree group");
    };
    assert_eq!(group.group_id, ChoiceId::new("spree").unwrap());
    assert_eq!((group.min, group.max), (1, 2));
    assert_eq!(
        group.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    let (damage_cost, damage_kind, damage_pips) = mana_option(group, 0);
    assert_eq!(*damage_cost, ChoiceId::new("damage_creature_cost").unwrap());
    assert_eq!(damage_kind, ManaCostChoiceKind::AdditionalPayment);
    assert_eq!(damage_pips, "{2}");
    let (destroy_cost, destroy_kind, destroy_pips) = mana_option(group, 1);
    assert_eq!(
        *destroy_cost,
        ChoiceId::new("destroy_artifact_cost").unwrap()
    );
    assert_eq!(destroy_kind, ManaCostChoiceKind::AdditionalPayment);
    assert_eq!(destroy_pips, "{2}");

    // Oracle lines 2-3, in printed order, with mode-local targets.
    let modal = face.modal_spell.as_ref().expect("Spree is modal");
    assert_eq!((modal.min_modes, modal.max_modes), (1, 2));
    assert!(modal.all_modes_cast_cost.is_none());
    let [damage_mode, destroy_mode] = modal.modes.as_slice() else {
        panic!("two printed modes");
    };
    assert_eq!(damage_mode.mode_id, ModeId::new("damage_creature").unwrap());
    assert_eq!(
        damage_mode.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(
        damage_mode.linked_cast_cost,
        Some(linked("damage_creature_cost"))
    );
    assert_eq!(
        damage_mode.effects,
        [SpellEffectKind::DamageTarget {
            amount: Amount::Fixed(4),
            target: TargetFilter {
                kind: TargetKind::Creature,
                ..TargetFilter::default()
            },
        }]
    );
    let damage_targeting = damage_mode.targeting.as_ref().expect("creature target");
    let group = single_group(damage_targeting);
    assert_eq!((group.min, group.max), (1, 1));
    assert_eq!(group.prompt, "Choose target creature");
    assert_eq!(group.effect_indices, [0]);

    assert_eq!(
        destroy_mode.mode_id,
        ModeId::new("destroy_artifact").unwrap()
    );
    assert_eq!(
        destroy_mode.presentation,
        AbilityPresentation::OracleLines(vec![3])
    );
    assert_eq!(
        destroy_mode.linked_cast_cost,
        Some(linked("destroy_artifact_cost"))
    );
    assert_eq!(
        destroy_mode.effects,
        [SpellEffectKind::Destroy {
            subject: EffectSubject::Chosen(Box::new(TargetFilter {
                kind: TargetKind::AnyPermanent,
                permanent_types: vec![PermanentTypeFilter::Artifact],
                ..TargetFilter::default()
            })),
        }]
    );
    let destroy_targeting = destroy_mode.targeting.as_ref().expect("artifact target");
    let group = single_group(destroy_targeting);
    assert_eq!((group.min, group.max), (1, 1));
    assert_eq!(group.prompt, "Choose target artifact");
    assert_eq!(group.effect_indices, [0]);

    // Mode-local target contracts stay distinct: each mode compiles against its own targeting.
    for mode in &modal.modes {
        tricerules_cards::primitives::TargetSchema::compile(&mode.effects, mode.targeting.as_ref())
            .unwrap_or_else(|error| panic!("mode {} schema: {error}", mode.mode_id));
    }
}

#[test]
fn issue_359_unfortunate_accident_maps_every_printed_clause() {
    let registry = CardRegistry::global();
    let definition = registry.get("unfortunate_accident").expect("registered");
    assert_eq!(definition.name, "Unfortunate Accident");
    assert_eq!(
        registry.id_for_name("Unfortunate Accident"),
        Some("unfortunate_accident")
    );
    assert_eq!(definition.layout, Layout::Normal);
    assert_eq!(definition.face_count(), 1);
    let face = definition.primary_face();
    assert_eq!(face.face_id.as_str(), "unfortunate_accident");
    assert_eq!(face.mana_cost.to_string(), "{B}");
    assert_eq!(face.types, ["Instant"]);
    assert_eq!(face.colors(), vec![Color::Black]);
    assert!(face.keywords.is_empty());
    assert!(face.spell_effect.is_empty());
    assert!(face.triggered_abilities.is_empty());
    assert!(face.activated_abilities.is_empty());
    assert!(face.static_abilities.is_empty());
    assert!(face.custom_effect.is_none());
    assert!(face.targeting.is_none());

    let [group] = face.cast_cost_groups.as_slice() else {
        panic!("Unfortunate Accident has exactly one spree group");
    };
    assert_eq!(group.group_id, ChoiceId::new("spree").unwrap());
    assert_eq!((group.min, group.max), (1, 2));
    let (destroy_cost, _, destroy_pips) = mana_option(group, 0);
    assert_eq!(
        *destroy_cost,
        ChoiceId::new("destroy_creature_cost").unwrap()
    );
    assert_eq!(destroy_pips, "{2}{B}");
    let (mercenary_cost, _, mercenary_pips) = mana_option(group, 1);
    assert_eq!(*mercenary_cost, ChoiceId::new("mercenary_cost").unwrap());
    assert_eq!(mercenary_pips, "{1}");

    let modal = face.modal_spell.as_ref().expect("Spree is modal");
    assert_eq!((modal.min_modes, modal.max_modes), (1, 2));
    let [destroy_mode, mercenary_mode] = modal.modes.as_slice() else {
        panic!("two printed modes");
    };
    assert_eq!(
        destroy_mode.mode_id,
        ModeId::new("destroy_creature").unwrap()
    );
    assert_eq!(
        destroy_mode.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(
        destroy_mode.linked_cast_cost,
        Some(linked("destroy_creature_cost"))
    );
    assert_eq!(
        destroy_mode.effects,
        [SpellEffectKind::Destroy {
            subject: EffectSubject::Chosen(Box::new(TargetFilter {
                kind: TargetKind::Creature,
                ..TargetFilter::default()
            })),
        }]
    );
    let destroy_targeting = destroy_mode.targeting.as_ref().expect("creature target");
    assert_eq!(
        single_group(destroy_targeting).prompt,
        "Choose target creature"
    );

    assert_eq!(mercenary_mode.mode_id, ModeId::new("mercenary").unwrap());
    assert_eq!(
        mercenary_mode.presentation,
        AbilityPresentation::OracleLines(vec![3])
    );
    assert_eq!(
        mercenary_mode.linked_cast_cost,
        Some(linked("mercenary_cost"))
    );
    assert_eq!(
        mercenary_mode.effects,
        [SpellEffectKind::CreateTokens {
            token: "mercenary_r_1_1".into(),
            count: Amount::Fixed(1),
            who: tricerules_cards::primitives::PlayerRecipient::Controller,
            tapped: false,
            sacrifice_timing: None,
        }]
    );
    assert!(mercenary_mode.targeting.is_none());
    assert!(registry.is_token("mercenary_r_1_1"));
}

#[test]
fn issue_359_direct_ron_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, face_id, expected) in [
        (
            "explosive_derailment",
            "Explosive Derailment",
            "explosive_derailment",
            EXPLOSIVE_FINGERPRINT,
        ),
        (
            "unfortunate_accident",
            "Unfortunate Accident",
            "unfortunate_accident",
            ACCIDENT_FINGERPRINT,
        ),
    ] {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")))
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}"));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[0], id);
        assert_eq!(fields[1], name);
        assert_eq!(fields[2], face_id);
        assert_eq!(fields[3], name);
        assert_eq!(fields[4], expected, "fingerprint drift for {id}");
    }
}
