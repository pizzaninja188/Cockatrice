use tricerules_cards::primitives::{
    CounterKind, EffectSubject, SpellEffectKind, TargetController, TargetKind,
};
use tricerules_cards::CardRegistry;

#[test]
fn issue_157_counter_effects_are_typed_and_validate_their_subjects() {
    for text in [
        "RemoveCounters(counter: MinusOneMinusOne, count: 1, subject: Source)",
        "PutCounterSnapshot(from: Source, subject: Chosen((kind: Creature, controller: You)))",
        "PutCounterSnapshot(from: TriggerObject, subject: Source)",
    ] {
        let effect: SpellEffectKind = ron::from_str(text).expect("typed counter effect");
        effect
            .validate(tricerules_cards::primitives::EffectContext::Ability)
            .unwrap();
    }
}

#[test]
fn issue_157_wither_and_absolute_untap_are_distinct_capabilities() {
    let _: tricerules_cards::Keyword = ron::from_str("Wither").unwrap();
    let _: tricerules_cards::primitives::StaticAbilityDef =
        ron::from_str("AttachedModifier(cant_untap: true)").unwrap();
}

#[test]
fn issue_157_counter_costs_distinguish_fixed_kind_from_any_one() {
    for text in [
        "RemoveCounters(counter: None, count: 1)",
        "RemoveCounters(counter: Some(PlusOnePlusOne), count: 2)",
    ] {
        let _: tricerules_cards::AbilityCost = ron::from_str(text).unwrap();
    }
}

#[test]
fn issue_157_departure_snapshot_requires_an_observed_object() {
    let result = CardRegistry::from_chunks_and_tokens(
        &[r#"(
        id: "invalid_snapshot", name: "Invalid snapshot", face_id: "invalid_snapshot", mana_cost: "{1}", types: ["Artifact"],
        triggered_abilities: [(ability_id: "triggered_01", presentation: Fallback, trigger: WhenSelfEntersBattlefield,
            effect: [PutCounterSnapshot(from: TriggerObject, subject: Source)], )]
    )"#],
        &[],
    );
    let error = result.expect_err("an ETB trigger cannot supply another object's departure bag");
    assert!(error.to_string().contains("observed object"), "{error}");
}

#[test]
fn issue_85_cards_share_the_grouped_creature_damage_primitive() {
    let registry = CardRegistry::global();

    let rabid_definition = registry
        .get("rabid_bite")
        .expect("Rabid Bite is registered");
    let rabid = rabid_definition.primary_face();
    assert_eq!(rabid.mana_cost.to_string(), "{1}{G}");
    assert_eq!(rabid.types, ["Sorcery"]);
    assert!(matches!(
        &rabid.spell_effect[..],
        [SpellEffectKind::CreatureDealsDamageEqualToPower { source, target }]
            if source.kind == TargetKind::Creature
                && source.controller == TargetController::You
                && target.kind == TargetKind::Creature
                && target.controller == TargetController::NotYou
    ));
    let rabid_groups = &rabid.targeting.as_ref().expect("grouped targeting").groups;
    assert_eq!(rabid_groups.len(), 2);
    assert_eq!(rabid_groups[0].effect_indices, [0]);
    assert_eq!(rabid_groups[1].effect_indices, [0]);
    assert_eq!(rabid_groups[1].distinct_from, [0]);

    let hunter_definition = registry
        .get("hunters_edge")
        .expect("Hunter's Edge is registered");
    let hunter = hunter_definition.primary_face();
    assert_eq!(hunter.mana_cost.to_string(), "{3}{G}");
    assert_eq!(hunter.types, ["Sorcery"]);
    assert!(matches!(
        &hunter.spell_effect[..],
        [
            SpellEffectKind::PutCounters {
                counter: CounterKind::PlusOnePlusOne,
                count: tricerules_cards::Amount::Fixed(1),
                subject: EffectSubject::Chosen(source_for_counter),
            },
            SpellEffectKind::CreatureDealsDamageEqualToPower { source, target },
        ] if source_for_counter.as_ref() == source
            && source.kind == TargetKind::Creature
            && source.controller == TargetController::You
            && target.kind == TargetKind::Creature
            && target.controller == TargetController::NotYou
    ));
    let hunter_groups = &hunter.targeting.as_ref().expect("grouped targeting").groups;
    assert_eq!(hunter_groups.len(), 2);
    assert_eq!(hunter_groups[0].effect_indices, [0, 1]);
    assert_eq!(hunter_groups[1].effect_indices, [1]);
    assert_eq!(hunter_groups[1].distinct_from, [0]);
}
