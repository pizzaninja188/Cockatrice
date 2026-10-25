use tricerules_cards::primitives::*;
use tricerules_cards::{
    AbilityDefinitionId, AbilityId, AbilityPresentation, CardRegistry, ProgramOwnerId,
};

#[test]
fn standstill_is_complete_one_face_enchantment_with_automatic_sacrifice_gated_opponent_draw() {
    let registry = tricerules_cards::registry::global();
    assert_eq!(registry.id_for_name("Standstill"), Some("standstill"));
    let card = registry.get("standstill").unwrap();
    assert_eq!(card.face_count(), 1);
    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), "standstill");
    assert_eq!(face.mana_cost.to_string(), "{1}{U}");
    assert_eq!(face.types, ["Enchantment"]);
    assert!(
        face.activated_abilities.is_empty()
            && face.static_abilities.is_empty()
            && face.spell_effect.is_empty()
    );
    let [trigger] = face.triggered_abilities.as_slice() else {
        panic!("one complete trigger")
    };
    assert_eq!(
        trigger.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(
        trigger.trigger,
        TriggerCondition::WheneverPlayerCastsSpell {
            caster: CastTriggerPlayer::AnyPlayer,
            filter: Default::default(),
            ordinal: None,
            ordinal_scope: Default::default(),
        }
    );
    assert!(
        trigger.effect.is_empty(),
        "Standstill uses its Program body"
    );

    let owner = ProgramOwnerId::ability(AbilityDefinitionId {
        card_id: "standstill".into(),
        face_id: face.face_id.clone(),
        ability_path: vec![trigger.ability_id.clone()],
    });
    let program = registry
        .compiled_program_for_owner(&owner)
        .expect("Standstill's exact trigger owner has a compiled Program");
    assert_eq!(
        program.required_inputs,
        vec![ProgramRequiredInput::TriggerAffectedPlayer]
    );
    let [sacrifice, count_gate] = program.body.steps.as_slice() else {
        panic!("Standstill sacrifices its source before testing the bound result")
    };
    assert_eq!(sacrifice.id, "sacrifice");
    assert!(sacrifice.input.is_none());
    let [sacrificed_result] = sacrifice.outputs.as_slice() else {
        panic!("the source sacrifice binds its typed card-result cohort")
    };
    let CompiledInstructionKind::Perform(effect) = &sacrifice.kind else {
        panic!("the first instruction sacrifices the source")
    };
    assert_eq!(
        **effect,
        SpellEffectKind::Sacrifice {
            subject: EffectSubject::Source,
        }
    );
    assert_eq!(
        program.slot(*sacrificed_result).unwrap().result_type,
        ResultType::CardResultCohort {
            action: CardResultAction::Sacrifice,
            who: PlayerRecipient::Controller,
            cardinality: Cardinality {
                min: 0,
                max: Some(1)
            },
        }
    );

    let CompiledInstructionKind::If { condition, yes, no } = &count_gate.kind else {
        panic!("the typed source-sacrifice count guards the draw")
    };
    let CompiledProgramCondition::Result(CompiledProgramResultCondition::CardResultCount {
        result,
        min,
        max,
        card_type,
    }) = condition.as_ref()
    else {
        panic!("the guard counts its named typed result")
    };
    assert_eq!(*result, *sacrificed_result);
    assert_eq!((*min, *max), (Some(1), None));
    assert_eq!(*card_type, None);
    let [draw] = yes.steps.as_slice() else {
        panic!("a successful sacrifice has one draw operation")
    };
    let CompiledInstructionKind::Perform(effect) = &draw.kind else {
        panic!("the successful sacrifice draws for the captured caster's opponents")
    };
    assert_eq!(
        **effect,
        SpellEffectKind::Draw {
            count: Amount::Fixed(3),
            who: PlayerRecipient::EachOtherPlayerThanAffectedPlayer,
        }
    );
    assert!(no.steps.is_empty() && no.yields.is_empty());
}

#[test]
fn standstill_trigger_is_owned_by_a_compiled_program() {
    let card = tricerules_cards::registry::global()
        .get("standstill")
        .expect("Standstill is admitted as a complete card");
    let [trigger] = card.primary_face().triggered_abilities.as_slice() else {
        panic!("one Standstill trigger")
    };
    assert!(
        trigger.program.is_some() && trigger.effect.is_empty(),
        "Standstill's source-sacrifice gate and caster-relative draw belong to its Program"
    );
    let owner = ProgramOwnerId::ability(AbilityDefinitionId {
        card_id: "standstill".into(),
        face_id: card.primary_face().face_id.clone(),
        ability_path: vec![AbilityId::new("triggered_01").unwrap()],
    });
    assert_eq!(
        tricerules_cards::registry::global()
            .compiled_program_for_owner(&owner)
            .expect("the exact Standstill owner is registered")
            .owner,
        owner
    );
}

#[test]
fn standstill_named_count_uses_typed_cohorts_and_rejects_missing_or_wrong_results() {
    let source = include_str!("../data/standstill.ron");
    let source_step =
        "(id: \"sacrifice\", bind: [\"sacrificed\"], kind: Perform(Sacrifice(subject: Source)))";
    let mill_step =
        "(id: \"sacrifice\", bind: [\"sacrificed\"], kind: Perform(Mill(who: Controller, count: 1)))";
    assert!(
        source.contains(source_step),
        "the fixture names its source result"
    );
    let mill_cohort = source.replace(source_step, mill_step);
    assert_ne!(
        mill_cohort, source,
        "the producer replacement must take effect"
    );
    assert!(
        CardRegistry::from_chunks_and_tokens(&[&mill_cohort], &[]).is_ok(),
        "the generic count condition accepts another existing typed card-result producer"
    );

    let missing_binding = source.replace("bind: [\"sacrificed\"]", "bind: []");
    assert_ne!(
        missing_binding, source,
        "the missing-binding replacement must take effect"
    );
    let wrong_type = source.replace(
        "Sacrifice(subject: Source)",
        "Draw(who: Controller, count: 1)",
    );
    assert_ne!(
        wrong_type, source,
        "the wrong-type replacement must take effect"
    );
    let missing_slot = source.replace("result: \"sacrificed\"", "result: \"unbound\"");
    assert_ne!(
        missing_slot, source,
        "the out-of-scope replacement must take effect"
    );
    for invalid in [&missing_binding, &wrong_type, &missing_slot] {
        assert!(
            CardRegistry::from_chunks_and_tokens(&[invalid], &[]).is_err(),
            "an invalid or unavailable named cohort cannot authorize the conditional draw"
        );
    }
}

#[test]
fn flat_previous_effect_sacrifice_count_requires_a_bound_program_input() {
    // A compatible adjacent producer does not make a flat list a bound Program input.
    let fixture = r#"(
        id: "flat_sacrifice_result_fixture",
        name: "Flat Sacrifice Result Fixture",
        face_id: "flat_sacrifice_result_fixture",
        types: ["Artifact"],
        triggered_abilities: [(
            ability_id: "triggered_01",
            presentation: Fallback,
            trigger: WhenSelfEntersBattlefield,
            effect: [
                SacrificeAll(players: Controller, filter: (kind: AnyPermanent)),
                GainLife(amount: Count(CardsMatchingResult(filter: (
                    source: PreviousEffect, action: Sacrifice,
                    players: Controller, card_type: None,
                )))),
            ],
            modal: None,
            targeting: None,
            may: false,
            intervening_if: None,
            triggers_only_once: false,
            max_triggers_per_turn: None,
        )],
    )"#;
    let error = CardRegistry::from_chunks_and_tokens(&[fixture], &[])
        .expect_err("flat PreviousEffect count requires a bound Program input");
    assert!(
        error
            .to_string()
            .contains("PreviousEffect references require an authored Program"),
        "{error}"
    );
}
