//! Issue #342: graveyard card counts feed static P/T scaling and threshold conditions.
//!
//! These are the positive/negative calibration gates for widening
//! `CountExpression::validate_static_count` to public graveyard card counts. The cohort cards
//! that motivate the shape are Xande, Dark Mage (`+1/+1` per noncreature, nonland card) and
//! Moon-Vigil Adherents (battlefield creatures plus creature cards), with Avatar Destiny and
//! Song of Stupefaction for the attached scope.

use tricerules_cards::primitives::{
    CardTypeFilter, CountExpression, GameCondition, GraveyardAggregate, RelativePlayerSet,
    StaticAbilityDef, ZoneCardFilter,
};
use tricerules_cards::CardRegistry;

fn registry_with(card: &str) -> Result<CardRegistry, String> {
    CardRegistry::from_chunks_and_tokens(&[card], &[]).map_err(|error| error.to_string())
}

fn self_scaling(count: &str) -> String {
    format!(
        r#"(id: "gy_source", name: "Gy Source", face_id: "gy_source", types: ["Creature"], power: 1, toughness: 1,
            static_abilities: [(ability_id: "static_01", presentation: Fallback,
                definition: CountScaledSelfPt(count: {count}, power_per_match: 1, toughness_per_match: 1))])"#
    )
}

fn attached_scaling(count: &str) -> String {
    format!(
        r#"(id: "gy_aura", name: "Gy Aura", face_id: "gy_aura", types: ["Enchantment", "Aura"],
            spell_effect: [AuraAttach(target: (kind: Creature))],
            static_abilities: [(ability_id: "static_01", presentation: Fallback,
                definition: AttachedModifier(count: {count}, power_per_match: 1, toughness_per_match: 1))])"#
    )
}

fn creature_cards() -> ZoneCardFilter {
    ZoneCardFilter {
        card_type: Some(CardTypeFilter::Creature),
        ..Default::default()
    }
}

#[test]
fn issue_342_self_scaling_accepts_graveyard_card_counts() {
    let card = self_scaling(
        "GraveyardCards(owners: Controller, filter: Some((card_type: Some(Creature))))",
    );
    let registry = registry_with(&card).expect("graveyard card count must scale static P/T");
    let definition = registry.get("gy_source").expect("fixture registered");
    let StaticAbilityDef::CountScaledSelfPt { count, .. } =
        &definition.primary_face().static_abilities[0].definition
    else {
        panic!("expected CountScaledSelfPt");
    };
    assert_eq!(
        count,
        &CountExpression::GraveyardCards {
            owners: RelativePlayerSet::Controller,
            filter: Some(creature_cards()),
        },
        "the parsed count keeps owner scope and the printed creature filter"
    );
}

#[test]
fn issue_342_affine_scaling_combines_battlefield_and_graveyard_counts() {
    let card = self_scaling(
        "Affine(constant: 0, terms: [\
            (coefficient: 1, quantity: BattlefieldCreatures(filter: (controllers: Controller))), \
            (coefficient: 1, quantity: GraveyardCards(owners: Controller, filter: Some((card_type: Some(Creature)))))\
        ])",
    );
    assert!(
        registry_with(&card).is_ok(),
        "a battlefield-plus-graveyard affine count must validate for static scaling"
    );
}

#[test]
fn issue_342_attached_scaling_accepts_graveyard_card_counts() {
    let card = attached_scaling(
        "GraveyardCards(owners: Controller, filter: Some((card_type: Some(Creature))))",
    );
    let registry = registry_with(&card).expect("graveyard count must scale the attached object");
    let definition = registry.get("gy_aura").expect("fixture registered");
    let StaticAbilityDef::AttachedModifier {
        count,
        power_per_match,
        toughness_per_match,
        ..
    } = &definition.primary_face().static_abilities[0].definition
    else {
        panic!("expected AttachedModifier");
    };
    assert_eq!(
        count.as_ref(),
        Some(&CountExpression::GraveyardCards {
            owners: RelativePlayerSet::Controller,
            filter: Some(creature_cards()),
        })
    );
    assert_eq!((*power_per_match, *toughness_per_match), (1, 1));
}

#[test]
fn issue_342_self_scaling_still_rejects_event_and_source_counts() {
    for count in [
        "CreatureDeathsThisTurn",
        "SourcePower",
        "BattlefieldMaximum(filter: (controllers: Controller), characteristic: Power)",
    ] {
        assert!(
            registry_with(&self_scaling(count)).is_err(),
            "{count} must stay out of static P/T scaling"
        );
    }
}

#[test]
fn issue_342_attached_scaling_rejects_unscaled_and_event_counts() {
    assert!(
        registry_with(&attached_scaling("CreatureDeathsThisTurn")).is_err(),
        "event-dependent counts must stay out of attached static scaling"
    );
    let zero_multiplier = r#"(id: "gy_aura", name: "Gy Aura", face_id: "gy_aura", types: ["Enchantment", "Aura"],
        spell_effect: [AuraAttach(target: (kind: Creature))],
        static_abilities: [(ability_id: "static_01", presentation: Fallback,
            definition: AttachedModifier(count: GraveyardCards(owners: Controller), power_per_match: 0, toughness_per_match: 0))])"#;
    assert!(
        registry_with(zero_multiplier).is_err(),
        "count scaling must modify at least one P/T component"
    );
}

#[test]
fn issue_342_self_scaling_accepts_named_graveyard_counts() {
    let card = self_scaling("GraveyardCardsNamed(owners: Controller, name: \"Gy Creature\")");
    assert!(
        registry_with(&card).is_ok(),
        "a named public graveyard count must scale static P/T"
    );
}

#[test]
fn issue_342_affine_static_scaling_rejects_unsafe_leaves() {
    let card =
        self_scaling("Affine(constant: 0, terms: [(coefficient: 1, quantity: SourcePower)])");
    assert!(
        registry_with(&card).is_err(),
        "an affine count containing a source-relative leaf must stay out of static scaling"
    );
}

#[test]
fn issue_342_keyword_dependent_creature_scopes_stay_out_of_static_scaling() {
    let card = self_scaling(
        "BattlefieldCreatures(filter: (controllers: Controller, required_keywords: [Flying]))",
    );
    assert!(
        registry_with(&card).is_err(),
        "a keyword-dependent creature scope would need CR 613 dependency ordering"
    );
}

#[test]
fn issue_342_cost_modifier_accepts_graveyard_counts() {
    let card = r#"(id: "gy_spell", name: "Gy Spell", face_id: "gy_spell", mana_cost: "{3}{G}", types: ["Sorcery"],
        cost_modifiers: [GenericReduction(amount: Count(GraveyardCards(owners: Controller, filter: Some((card_type: Some(Creature))))))],
        spell_effect: [GainLife(amount: 1)])"#;
    assert!(
        registry_with(card).is_ok(),
        "a per-card graveyard cost reduction must validate"
    );
}

#[test]
fn issue_342_threshold_condition_matches_inclusive_bounds() {
    let condition = GameCondition::GraveyardAggregate {
        owners: RelativePlayerSet::Controller,
        aggregate: GraveyardAggregate::CardCount,
        filter: None,
        min: Some(4),
        max: None,
    };
    assert!(condition.validate().is_ok());
    assert!(condition.matches_value(4));
    assert!(condition.matches_value(9));
    assert!(!condition.matches_value(3));
}
