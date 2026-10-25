use tricerules_cards::{AbilityDefinitionId, AbilityId, CardFaceId, CardRegistry, ProgramOwnerId};

const KOYA_RON: &str = include_str!("../data/koya,_death_from_above.ron");

fn compiles(ron: &str) -> bool {
    CardRegistry::from_chunks_and_tokens(&[ron], &[]).is_ok()
}

fn replace_once(source: &str, from: &str, to: &str) -> String {
    assert!(source.contains(from), "fixture contains {from:?}");
    source.replacen(from, to, 1)
}

fn compile_owner() -> ProgramOwnerId {
    ProgramOwnerId::ability(AbilityDefinitionId {
        card_id: "koya,_death_from_above".into(),
        face_id: CardFaceId::new("koya_death_from_above").expect("canonical Koya face ID"),
        ability_path: vec![AbilityId::new("triggered_01").expect("canonical Koya ability ID")],
    })
}

#[test]
fn koya_triggered_program_compiles_from_the_actual_definition() {
    use tricerules_cards::primitives::{
        EffectContext, EffectSubject, InstructionKind, ProgramTargetRole, SpellEffectKind,
        TriggerCondition,
    };

    let registry = CardRegistry::from_chunks_and_tokens(&[KOYA_RON], &[])
        .expect("the actual Koya triggered Program compiles");
    let definition = registry
        .get("koya,_death_from_above")
        .expect("actual Koya card is registered");
    let [face] = definition.faces.as_slice() else {
        panic!("Koya has one printed face");
    };
    assert_eq!(face.supertypes, ["Legendary"]);
    assert_eq!(face.types, ["Creature", "Mutant", "Ninja", "Bird"]);
    assert!(face.is_legendary);
    let [outer] = face.triggered_abilities.as_slice() else {
        panic!("Koya has one printed triggered ability");
    };
    assert_eq!(outer.ability_id.as_str(), "triggered_01");
    assert_eq!(outer.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert!(outer.effect.is_empty());
    let Some(targeting) = outer.targeting.as_ref() else {
        panic!("Koya declares its optional target group");
    };
    let [group] = targeting.groups.as_slice() else {
        panic!("Koya has one target group");
    };
    assert_eq!((group.min, group.max), (0, 1));
    assert!(group.effect_indices.is_empty());
    let Some(program) = outer.program.as_ref() else {
        panic!("Koya's outer ability is an authored Program");
    };
    let [binding] = program.target_bindings.as_slice() else {
        panic!("the optional target is bound to the Exile producer");
    };
    assert_eq!(binding.instruction_id, "exile_target");
    assert_eq!(binding.role, ProgramTargetRole::Subject);
    assert_eq!(binding.target_group_index, 0);
    let [exile, schedule] = program.body.steps.as_slice() else {
        panic!("the actual outer Program has Exile followed by its delayed consumer");
    };
    assert_eq!(exile.bind, ["exiled"]);
    assert!(exile.input.is_none());
    assert_eq!(schedule.input.as_deref(), Some("exiled"));
    assert!(schedule.bind.is_empty());
    assert!(matches!(
        &exile.kind,
        InstructionKind::Perform(effect)
            if matches!(effect.as_ref(), SpellEffectKind::Exile {
                subject: EffectSubject::Chosen(filter),
                linked_exile_id: None,
            } if filter.kind == tricerules_cards::primitives::TargetKind::Creature
                && filter.excluded_objects == [tricerules_cards::primitives::TargetObjectExclusion::Source])
    ));
    let InstructionKind::Perform(effect) = &schedule.kind else {
        panic!("Koya's second instruction creates the delayed observer");
    };
    let SpellEffectKind::CreateDelayedTrigger {
        subject: Some(EffectSubject::PreviousEffectObject),
        affected_player: None,
        ability: delayed,
    } = effect.as_ref()
    else {
        panic!("the delayed consumer watches the exact Exile result");
    };
    assert_eq!(delayed.ability_id.as_str(), "delayed_01");
    assert_eq!(delayed.trigger, TriggerCondition::AtBeginningOfNextEndStep);
    assert!(
        delayed.program.is_some(),
        "the existing child Program remains present"
    );
    let compiled_outer = registry
        .compiled_program_for_owner(&compile_owner())
        .expect("the exact outer Koya owner has a compiled Program");
    assert_eq!(compiled_outer.owner, compile_owner());
    assert_eq!(compiled_outer.context, EffectContext::Ability);

    let child_owner = ProgramOwnerId::ability(AbilityDefinitionId {
        card_id: "koya,_death_from_above".into(),
        face_id: CardFaceId::new("koya_death_from_above").unwrap(),
        ability_path: vec![
            AbilityId::new("triggered_01").unwrap(),
            AbilityId::new("delayed_01").unwrap(),
        ],
    });
    assert!(
        registry.compiled_program_for_owner(&child_owner).is_some(),
        "the existing delayed child remains compiled under its full owner path"
    );
}

#[test]
fn koya_exile_producer_accepts_arbitrary_and_discarded_output_bindings() {
    let arbitrary = replace_once(
        KOYA_RON,
        "bind: [\"exiled\"]",
        "bind: [\"any_authored_name\"]",
    );
    let arbitrary = replace_once(
        &arbitrary,
        "input: Some(\"exiled\")",
        "input: Some(\"any_authored_name\")",
    );
    assert!(compiles(&arbitrary), "arbitrary output names are accepted");

    const PRODUCER_ONLY: &str = r#"(
      id: "koya,_death_from_above", name: "Koya, Death from Above",
      face_id: "koya_death_from_above", mana_cost: "{2}{W}",
      supertypes: ["Legendary"], types: ["Creature", "Mutant", "Ninja", "Bird"],
      power: 2, toughness: 1,
      triggered_abilities: [(
        ability_id: "triggered_01", presentation: OracleLines([2]),
        trigger: WhenSelfEntersBattlefield,
        targeting: Some((groups: [(
          min: 0, max: 1, prompt: "Choose up to one other target creature", effect_indices: [],
        )])),
        effect: [],
        program: Some((body: (steps: [
          (id: "exile_target", bind: [], input: None,
            kind: Perform(Exile(subject: Chosen((kind: Creature, excluded_objects: [Source])))))
        ], yields: []), target_bindings: [
          (instruction_id: "exile_target", role: Subject, target_group_index: 0)
        ])),
      )],
    )"#;
    let registry = CardRegistry::from_chunks_and_tokens(&[PRODUCER_ONLY], &[])
        .expect("the exact Exile operation may discard its typed output");
    assert!(registry
        .compiled_program_for_owner(&compile_owner())
        .is_some());
}

#[test]
fn koya_exile_consumer_rejects_near_misses_and_producer_is_generic() {
    let wrong_owner = replace_once(
        KOYA_RON,
        "ability_id: \"triggered_01\"",
        "ability_id: \"different_trigger\"",
    );
    assert!(
        compiles(&wrong_owner),
        "the typed producer does not depend on the parent ability ID"
    );

    let generic_creature_filter = replace_once(
        KOYA_RON,
        "excluded_objects: [Source]",
        "excluded_objects: []",
    );
    assert!(
        compiles(&generic_creature_filter),
        "a valid Creature filter need not equal Koya's authored Source exclusion"
    );

    let linked_exile = replace_once(
        KOYA_RON,
        "kind: Perform(Exile(subject: Chosen((kind: Creature, excluded_objects: [Source]))))),",
        "kind: Perform(Exile(subject: Chosen((kind: Creature, excluded_objects: [Source])), linked_exile_id: Some(\"linked\"))),",
    );
    assert!(
        !compiles(&linked_exile),
        "the generic ExiledObject result excludes linked Exile receipts"
    );

    let wrong_max = replace_once(KOYA_RON, "max: 1,\n      prompt:", "max: 2,\n      prompt:");
    assert!(!compiles(&wrong_max), "the optional group is singleton");

    let duplicate_subject_binding = replace_once(
        KOYA_RON,
        "(instruction_id: \"exile_target\", role: Subject, target_group_index: 0)",
        "(instruction_id: \"exile_target\", role: Subject, target_group_index: 0),\n          (instruction_id: \"exile_target\", role: Subject, target_group_index: 0)",
    );
    assert!(
        !compiles(&duplicate_subject_binding),
        "the Exile instruction has exactly one owner-local binding"
    );

    let mixed_role_binding = replace_once(
        KOYA_RON,
        "(instruction_id: \"exile_target\", role: Subject, target_group_index: 0)",
        "(instruction_id: \"exile_target\", role: Subject, target_group_index: 0),\n          (instruction_id: \"exile_target\", role: Target, target_group_index: 0)",
    );
    assert!(
        !compiles(&mixed_role_binding),
        "an extra binding of another role cannot hide behind the Subject lookup"
    );

    let wrong_role_binding = replace_once(
        KOYA_RON,
        "instruction_id: \"exile_target\", role: Subject",
        "instruction_id: \"exile_target\", role: Target",
    );
    assert!(
        !compiles(&wrong_role_binding),
        "the Exile operation requires a Subject target binding"
    );

    let missing_subject_binding = replace_once(
        KOYA_RON,
        "(instruction_id: \"exile_target\", role: Subject, target_group_index: 0)",
        "",
    );
    assert!(
        !compiles(&missing_subject_binding),
        "the Exile operation cannot omit its authored target binding"
    );

    let wrong_filter = replace_once(KOYA_RON, "kind: Creature", "kind: AnyTarget");
    assert!(
        !compiles(&wrong_filter),
        "the ExiledObject producer rejects player-capable target filters"
    );

    let wrong_subject = replace_once(
        KOYA_RON,
        "subject: Some(PreviousEffectObject)",
        "subject: Some(Source)",
    );
    assert!(
        !compiles(&wrong_subject),
        "the observer watches the Exile result"
    );

    let wrong_affected_player = replace_once(
        KOYA_RON,
        "subject: Some(PreviousEffectObject),",
        "subject: Some(PreviousEffectObject),\n            affected_player: Some(Controller),",
    );
    assert!(
        !compiles(&wrong_affected_player),
        "the delayed consumer has no affected-player override"
    );

    let wrong_timing = replace_once(
        KOYA_RON,
        "trigger: AtBeginningOfNextEndStep",
        "trigger: AtBeginningOfNextTurnUpkeep",
    );
    assert!(
        !compiles(&wrong_timing),
        "the observer is next end step only"
    );

    let wrong_input = replace_once(
        KOYA_RON,
        "input: Some(\"exiled\")",
        "input: Some(\"missing_result\")",
    );
    assert!(
        !compiles(&wrong_input),
        "the input must name a bound Exile result"
    );

    let missing_input = replace_once(KOYA_RON, "input: Some(\"exiled\")", "input: None");
    assert!(
        !compiles(&missing_input),
        "the delayed observer requires an explicit input naming the Exile result"
    );

    let wrong_type = replace_once(
        KOYA_RON,
        "        (id: \"schedule_return\", input: Some(\"exiled\"),",
        "        (id: \"wrong_result_type\", bind: [\"other_result\"], input: None,\n          kind: Perform(Draw(who: Controller, count: 1))),\n        (id: \"schedule_return\", input: Some(\"other_result\"),",
    );
    assert!(
        !compiles(&wrong_type),
        "a DrawReceipt is not an ExiledObject"
    );

    let unrelated_instruction = replace_once(
        KOYA_RON,
        "        (id: \"schedule_return\", input: Some(\"exiled\"),",
        "        (id: \"unrelated_gap\", input: None, kind: Perform(GainLife(amount: 1))),\n        (id: \"schedule_return\", input: Some(\"exiled\"),",
    );
    assert!(
        compiles(&unrelated_instruction),
        "the exact named Exile result remains usable across an unrelated instruction"
    );
}
