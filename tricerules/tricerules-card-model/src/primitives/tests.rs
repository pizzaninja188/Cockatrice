#[test]
fn multikicker_counted_entry_schema_is_linked_and_fail_closed() {
    let definition = |kind: &str, cost: &str, affected: &str, option: &str| {
        format!(
            r#"(id: "repeated_entry", name: "Repeated Entry", face_id: "repeated_entry",
                mana_cost: "{{0}}", types: ["Artifact"],
                cast_cost_groups: [(group_id: "multikicker", presentation: Fallback,
                    options: [Mana(option_id: "kick", presentation: Fallback,
                        kind: {kind}, cost: "{cost}")])],
                static_abilities: [(ability_id: "entry", presentation: Fallback,
                    definition: EntersWithCounters(affected: {affected}, counter: Charge,
                        amount: Count(CastCostPaymentCount(cost: (
                            group_id: "multikicker", option_id: "{option}")))))])"#
        )
    };
    for fee in ["{1}", "{2}"] {
        let ron = definition("Multikicker", fee, "Self_", "kick");
        crate::CardRegistry::from_chunks_and_tokens(&[&ron], &[])
            .expect("Chalice and Comet Storm require repeated positive generic kicker fees");
    }
    for (kind, fee, affected, option) in [
        ("Multikicker", "{0}", "Self_", "kick"),
        ("Multikicker", "{G}", "Self_", "kick"),
        ("Multikicker", "{X}", "Self_", "kick"),
        ("Multikicker", "{2}", "Self_", "unknown"),
        ("Multikicker", "{2}", "Creatures(())", "kick"),
        ("Kicker", "{2}", "Self_", "kick"),
        ("AdditionalPayment", "{2}", "Self_", "kick"),
    ] {
        let ron = definition(kind, fee, affected, option);
        assert!(
            crate::CardRegistry::from_chunks_and_tokens(&[&ron], &[]).is_err(),
            "unsupported repeated-cost or entry context accepted: {ron}"
        );
    }
    let required = definition("Multikicker", "{2}", "Self_", "kick").replace(
        "presentation: Fallback,\n                    options:",
        "presentation: Fallback, min: 1,\n                    options:",
    );
    assert!(crate::CardRegistry::from_chunks_and_tokens(&[&required], &[]).is_err());
}

#[test]
fn conditional_permanent_transformation_rejects_later_layer_and_unpaired_shapes() {
    let definition = |filter: &str,
                      condition: &str,
                      base_power: &str,
                      base_toughness: &str,
                      triggered_abilities: &str| {
        format!(
            r#"(id: "conditional_transform_fixture", name: "Conditional Transform Fixture",
                face_id: "conditional_transform_fixture", types: ["Creature"],
                static_abilities: [(ability_id: "static_01", presentation: Fallback,
                    definition: ConditionalPermanentTransformation(
                        filter: {filter}, condition: {condition},
                        add_types: (card_types: [Creature]),
                        base_power: {base_power}, base_toughness: {base_toughness},
                        triggered_abilities: {triggered_abilities},
                    ))])"#
        )
    };
    let valid_filter = "(kind: AnyPermanent, controller: You, min_mana_value: Some(4))";
    let valid_condition = "ActivePlayer(players: Controller)";
    let valid_trigger = r#"[(ability_id: "triggered_01", presentation: Fallback,
        trigger: WheneverSelfDealsCombatDamageToPlayer, effect: [Draw(count: 1)])]"#;
    let valid = definition(
        valid_filter,
        valid_condition,
        "Some(4)",
        "Some(4)",
        valid_trigger,
    );
    crate::CardRegistry::from_chunks_and_tokens(&[&valid], &[])
        .expect("the supported early filter and paired base P/T are valid");

    for invalid in [
        definition(
            "(kind: AnyPermanent, controller: You, tapped: Some(false))",
            valid_condition,
            "Some(4)",
            "Some(4)",
            "[]",
        ),
        definition(
            valid_filter,
            "ObjectTapped(object: Source, tapped: true)",
            "Some(4)",
            "Some(4)",
            "[]",
        ),
        definition(valid_filter, valid_condition, "Some(4)", "None", "[]"),
        definition(
            valid_filter,
            valid_condition,
            "Some(4)",
            "Some(4)",
            r#"[(ability_id: "triggered_01", presentation: Fallback,
                trigger: WheneverSelfDealsCombatDamageToPlayer, effect: [ExileObservedObjects])]"#,
        ),
    ] {
        assert!(
            crate::CardRegistry::from_chunks_and_tokens(&[&invalid], &[]).is_err(),
            "unsupported transformation shape was accepted: {invalid}"
        );
    }
}

#[test]
fn mana_amount_ron_omits_zero_fields_and_round_trips() {
    let amount = super::ManaAmount {
        w: 1,
        ..Default::default()
    };

    let encoded = ron::ser::to_string(&amount).expect("mana amount should serialize");
    assert_eq!(encoded, "(w:1)");
    assert_eq!(
        ron::from_str::<super::ManaAmount>(&encoded).expect("compact mana amount should parse"),
        amount
    );
}

#[test]
fn permanent_event_filter_omits_unset_linked_creature_type_from_ron() {
    let mut filter = crate::primitives::PermanentEventFilter::default();
    let encoded_default = ron::ser::to_string(&filter).expect("filter should serialize");
    assert!(
        !encoded_default.contains("source_chosen_creature_type"),
        "an absent optional filter field must preserve existing generated RON"
    );
    assert_eq!(
        ron::from_str::<crate::primitives::PermanentEventFilter>(&encoded_default)
            .expect("filter should round-trip"),
        filter
    );

    filter.source_chosen_creature_type =
        Some(crate::AbilityLinkId::new("chosen_type").expect("stable link id should be valid"));
    let encoded_linked = ron::ser::to_string(&filter).expect("linked filter should serialize");
    assert!(encoded_linked.contains("source_chosen_creature_type"));
    assert_eq!(
        ron::from_str::<crate::primitives::PermanentEventFilter>(&encoded_linked)
            .expect("linked filter should round-trip"),
        filter
    );
}

#[test]
fn zone_card_filter_omits_unset_color_match_from_ron() {
    let filter = crate::primitives::ZoneCardFilter::default();
    let encoded = ron::ser::to_string(&filter).expect("zone card filter should serialize");
    assert!(
        !encoded.contains("is_colorless"),
        "an absent optional filter field must preserve existing generated RON"
    );
    assert_eq!(
        ron::from_str::<crate::primitives::ZoneCardFilter>(&encoded)
            .expect("zone card filter should round-trip"),
        filter
    );
}

#[test]
fn registry_compiles_authored_draw_then_discard_spell_program() {
    let ron = r#"(
        id: "whole_draw_discard_fixture", name: "Whole Draw Discard Fixture",
        face_id: "whole_draw_discard_fixture", types: ["Sorcery"],
        spell_effect: [],
        spell_program: Some((body: (steps: [
            (id: "draw", kind: Perform(Draw(who: Controller, count: 1))),
            (id: "discard", bind: ["discarded"], kind: Perform(Discard(
                who: Controller, quantity: Exact(1)
            )))
        ], yields: []), target_bindings: []))
    )"#;
    let registry = crate::CardRegistry::from_chunks_and_tokens(&[ron], &[])
        .expect("authored draw/discard fixture should load");
    let owner = crate::ProgramOwnerId::face(
        "whole_draw_discard_fixture",
        crate::CardFaceId::new("whole_draw_discard_fixture").unwrap(),
    );
    let program = registry
        .compiled_program_for_owner(&owner)
        .expect("the exact face owner should compile its authored program");
    assert_eq!(program.owner, owner);
    assert_eq!(program.body.steps.len(), 2);
    assert!(matches!(
        &program.body.steps[0].kind,
        crate::primitives::CompiledInstructionKind::Perform(effect)
            if matches!(effect.as_ref(), crate::primitives::SpellEffectKind::Draw { .. })
    ));
    assert!(matches!(
        &program.body.steps[1].kind,
        crate::primitives::CompiledInstructionKind::Perform(effect)
            if matches!(effect.as_ref(), crate::primitives::SpellEffectKind::Discard {
                quantity: crate::primitives::DiscardQuantity::Exact(1), ..
            })
    ));
}

#[test]
fn registry_rejects_retired_draw_discard_ron_variant() {
    let ron = r#"(
        id: "retired_draw_discard_syntax", name: "Retired Draw Discard Syntax",
        face_id: "retired_draw_discard_syntax", types: ["Sorcery"],
        spell_effect: [DrawDiscard(who: Controller, draw_count: 1,
            discard_count: 1, order: DrawThenDiscard, optional: false)]
    )"#;
    assert!(
        crate::CardRegistry::from_chunks_and_tokens(&[ron], &[]).is_err(),
        "retired DrawDiscard RON syntax must not be admitted"
    );
}

#[test]
fn optional_exact_discard_is_a_typed_exact_n_operation() {
    let effect = crate::primitives::SpellEffectKind::Discard {
        who: crate::primitives::PlayerRecipient::Controller,
        quantity: crate::primitives::DiscardQuantity::OptionalExact(2),
    };
    assert!(effect
        .validate(crate::primitives::EffectContext::Spell)
        .is_ok());
}

#[test]
fn registry_compiles_authored_modal_modes_by_stable_id_with_owner_local_targets() {
    let ron = r#"(
        id: "modal_draw_discard_fixture", name: "Modal Draw Discard Fixture",
        face_id: "modal_draw_discard_fixture", types: ["Sorcery"],
        modal_spell: (
            min_modes: 1, max_modes: 1,
            modes: [
                (mode_id: "mode_01", presentation: Fallback,
                    effects: [], program: Some((body: (steps: [
                        (id: "draw", kind: Perform(Draw(who: Controller, count: 1))),
                        (id: "discard", kind: Perform(Discard(
                            who: Controller, quantity: Exact(1)
                        )))
                    ], yields: [])))),
                (mode_id: "mode_02", presentation: Fallback,
                    effects: [], program: Some((body: (steps: [
                        (id: "draw_two", kind: Perform(Draw(who: Controller, count: 2)))
                    ], yields: [])))),
                (mode_id: "mode_03", presentation: Fallback,
                    effects: [], program: Some((body: (steps: [
                        (id: "counter_spell", kind: Perform(CounterTargetSpell(
                            spell_filter: (any_of: None, card_type: None, is_color: None,
                                min_mana_value: None, max_mana_value: None),
                            unless_controller_pays: None,
                            unless_controller_pays_by_cast_cost: None
                        )))
                    ], yields: []), target_bindings: [
                        (instruction_id: "counter_spell", role: Spell, target_group_index: 0)
                    ])),
                    targeting: Some((groups: [(min: 1, max: 1,
                        prompt: "Choose target spell", effect_indices: [])]))),
                (mode_id: "mode_04", presentation: Fallback,
                    effects: [], program: Some((body: (steps: [
                        (id: "copy_creature", kind: Perform(CreateTokenCopies(count: 1,
                            source: Chosen((kind: Creature, controller: You)))))
                    ], yields: []), target_bindings: [
                        (instruction_id: "copy_creature", role: Source, target_group_index: 0)
                    ])),
                    targeting: Some((groups: [(min: 1, max: 1,
                        prompt: "Choose target creature you control", effect_indices: [])])))
            ]
        )
    )"#;
    let registry = crate::CardRegistry::from_chunks_and_tokens(&[ron], &[])
        .expect("all-authored modal fixture should load");
    for (mode, expected_len) in [
        ("mode_01", 2),
        ("mode_02", 1),
        ("mode_03", 1),
        ("mode_04", 1),
    ] {
        let owner = crate::ProgramOwnerId::face_mode(
            "modal_draw_discard_fixture",
            crate::CardFaceId::new("modal_draw_discard_fixture").unwrap(),
            crate::ModeId::new(mode).unwrap(),
        );
        let program = registry
            .compiled_program_for_owner(&owner)
            .expect("every authored mode must compile for its stable owner id");
        assert_eq!(program.owner, owner);
        assert_eq!(program.body.steps.len(), expected_len);
        if mode == "mode_03" || mode == "mode_04" {
            assert_eq!(
                program.target_projection,
                crate::primitives::ProgramTargetProjection::ModeLocal {
                    mode_id: crate::ModeId::new(mode).unwrap(),
                }
            );
            assert_eq!(program.target_bindings.len(), 1);
            assert_eq!(program.target_bindings[0].instruction_address, 0);
            assert_eq!(program.target_bindings[0].target_group_index, 0);
            assert_eq!(program.target_bindings[0].role_index, 0);
        }
    }
}

#[test]
fn registry_compiles_authored_targeted_spell_siblings_in_instruction_order() {
    let ron = r#"(
        id: "targeted_draw_discard_fixture", name: "Targeted Draw Discard Fixture",
        face_id: "targeted_draw_discard_fixture", types: ["Sorcery"],
        spell_effect: [],
        spell_program: Some((body: (steps: [
            (id: "damage", kind: Perform(DamageTarget(amount: 5,
                target: (kind: Creature)))),
            (id: "return", kind: Perform(ReturnToOwnersHand(
                subject: Chosen((kind: Creature))))),
            (id: "draw", kind: Perform(Draw(who: Controller, count: 2))),
            (id: "discard", kind: Perform(Discard(who: Controller,
                quantity: Exact(1))))
        ], yields: []), target_bindings: [
            (instruction_id: "damage", role: Target, target_group_index: 0),
            (instruction_id: "return", role: Subject, target_group_index: 1)
        ])),
        targeting: Some((groups: [
            (min: 1, max: 1, prompt: "Choose target creature", effect_indices: []),
            (min: 0, max: 2, prompt: "Choose up to two target creatures", effect_indices: [])
        ]))
    )"#;
    let registry = crate::CardRegistry::from_chunks_and_tokens(&[ron], &[])
        .expect("authored target roles and source operations should pass their validators");
    let owner = crate::ProgramOwnerId::face(
        "targeted_draw_discard_fixture",
        crate::CardFaceId::new("targeted_draw_discard_fixture").unwrap(),
    );
    let program = registry
        .compiled_program_for_owner(&owner)
        .expect("the complete authored owner should compile as one body");
    assert_eq!(program.body.steps.len(), 4);
    assert!(matches!(
        &program.body.steps[0].kind,
        crate::primitives::CompiledInstructionKind::Perform(effect)
            if matches!(effect.as_ref(), crate::primitives::SpellEffectKind::DamageTarget { .. })
    ));
    assert!(matches!(
        &program.body.steps[1].kind,
        crate::primitives::CompiledInstructionKind::Perform(effect)
            if matches!(effect.as_ref(), crate::primitives::SpellEffectKind::ReturnToOwnersHand { .. })
    ));
    assert_eq!(program.target_bindings.len(), 2);
    assert_eq!(program.target_bindings[0].instruction_address, 0);
    assert_eq!(
        program.target_bindings[0].instruction_address,
        program.body.steps[0].instruction_address
    );
    assert_eq!(program.target_bindings[0].target_group_index, 0);
    assert_eq!(program.target_bindings[0].role_index, 0);
    assert_eq!(program.target_bindings[1].instruction_address, 1);
    assert_eq!(
        program.target_bindings[1].instruction_address,
        program.body.steps[1].instruction_address
    );
    assert_eq!(program.target_bindings[1].target_group_index, 1);
    assert_eq!(program.target_bindings[1].role_index, 0);
    assert!(matches!(
        &program.body.steps[2].kind,
        crate::primitives::CompiledInstructionKind::Perform(effect)
            if matches!(effect.as_ref(), crate::primitives::SpellEffectKind::Draw { count: crate::primitives::Amount::Fixed(2), .. })
    ));
    assert!(matches!(
        &program.body.steps[3].kind,
        crate::primitives::CompiledInstructionKind::Perform(effect)
            if matches!(effect.as_ref(), crate::primitives::SpellEffectKind::Discard {
                quantity: crate::primitives::DiscardQuantity::Exact(1), ..
            })
    ));
}

fn spell_target_damage_history_fixture(
    target_effect: &str,
    condition: &str,
    binding: &str,
    targeting: &str,
    rider: &str,
    exile_first: bool,
) -> String {
    let bindings = if binding.is_empty() {
        String::new()
    } else {
        format!("{binding},")
    };
    let target = format!("(id: \"target\", kind: Perform({target_effect}))");
    let condition = format!(
        r#"(id: "damage_check", kind: If(condition: {condition},
            yes: (steps: [(id: "rider", kind: Perform({rider}))], yields: []),
            no: (steps: [], yields: [])))"#
    );
    let steps = if exile_first {
        format!("{target}, {condition}")
    } else {
        format!("{condition}, {target}")
    };
    format!(
        r#"(
            id: "spell_target_history_fixture", name: "Spell Target History Fixture",
            face_id: "spell_target_history_fixture", types: ["Instant"], spell_effect: [],
            spell_program: Some((body: (steps: [{steps}], yields: []), target_bindings: [{bindings}])),
            targeting: {targeting}
        )"#
    )
}

fn modal_spell_target_damage_history_fixture() -> String {
    r#"(
        id: "modal_spell_target_history_fixture", name: "Modal Spell Target History Fixture",
        face_id: "modal_spell_target_history_fixture", types: ["Instant"], spell_effect: [],
        modal_spell: Some((min_modes: 1, max_modes: 1, modes: [(
            mode_id: "mode_01", presentation: Fallback, linked_cast_cost: None, effects: [],
            program: Some((body: (steps: [
                (id: "target", kind: Perform(Exile(subject: Chosen((kind: Creature))))),
                (id: "damage_check", kind: If(
                    condition: ObjectWasDealtDamageThisTurn(
                        object: ChosenTarget(group_index: 0, target_index: 0)),
                    yes: (steps: [(id: "rider", kind: Perform(CreateTokens(
                        token: "clue", count: 1, who: Controller,
                        tapped: false, sacrifice_timing: None)))], yields: []),
                    no: (steps: [], yields: [])
                )),
            ], yields: []), target_bindings: [
                (instruction_id: "target", role: Subject, target_group_index: 0),
            ])),
            targeting: Some((groups: [(
                min: 1, max: 1, prompt: "Choose target creature", effect_indices: []
            )]))
        )]))
    )"#
    .to_string()
}

fn compile_spell_target_history_fixture(ron: &str) -> Result<crate::CardRegistry, String> {
    const CLUE: &str = r#"(id: "clue", name: "Clue", face_id: "clue", types: ["Artifact"] )"#;
    const FOOD: &str = r#"(id: "food", name: "Food", face_id: "food", types: ["Artifact"] )"#;
    crate::CardRegistry::from_chunks_and_tokens(&[ron], &[CLUE, FOOD])
        .map_err(|error| error.to_string())
}

#[test]
fn spell_program_compiles_a_subject_bound_creature_exile_target_history_profile() {
    let ron = spell_target_damage_history_fixture(
        "Exile(subject: Chosen((kind: Creature)))",
        "ObjectWasDealtDamageThisTurn(object: ChosenTarget(group_index: 0, target_index: 0))",
        "(instruction_id: \"target\", role: Subject, target_group_index: 0)",
        "Some((groups: [(min: 1, max: 1, prompt: \"Choose target creature\", effect_indices: [])]))",
        "CreateTokens(token: \"clue\", count: 1, who: Controller, tapped: false, sacrifice_timing: None)",
        true,
    );
    let registry = compile_spell_target_history_fixture(&ron)
        .expect("the Subject-bound Creature Exile and damage-history profile compiles");
    let owner = crate::ProgramOwnerId::face(
        "spell_target_history_fixture",
        crate::CardFaceId::new("spell_target_history_fixture").unwrap(),
    );
    let program = registry
        .compiled_program_for_owner(&owner)
        .expect("the complete spell face owns its compiled Program");
    assert_eq!(program.body.steps.len(), 2);
    assert_eq!(program.target_bindings.len(), 1);
    assert_eq!(program.target_bindings[0].target_group_index, 0);
    assert_eq!(program.target_bindings[0].role_index, 0);
    assert!(matches!(
        &program.body.steps[0].kind,
        crate::primitives::CompiledInstructionKind::Perform(effect)
            if matches!(effect.as_ref(), crate::primitives::SpellEffectKind::Exile {
                subject: crate::primitives::EffectSubject::Chosen(filter),
                linked_exile_id: None,
            } if filter.kind == crate::primitives::TargetKind::Creature)
    ));
    assert!(matches!(
        &program.body.steps[1].kind,
        crate::primitives::CompiledInstructionKind::If { condition, .. }
            if matches!(condition.as_ref(), crate::primitives::CompiledProgramCondition::Game(
                crate::primitives::GameCondition::ObjectWasDealtDamageThisTurn {
                    object: crate::primitives::ConditionObjectRef::ChosenTarget {
                        group_index: 0, target_index: 0,
                    }
                }
            ))
    ));
}

#[test]
fn modal_spell_program_target_history_rejects_unsupported_mode_local_context() {
    let ron = modal_spell_target_damage_history_fixture();
    let error = compile_spell_target_history_fixture(&ron)
        .expect_err("mode-local targets are outside the face-owned history profile");
    let diagnostic = error.to_string();
    assert!(
        diagnostic.contains("If condition uses context outside"),
        "modal target-history use must receive the Program unsupported-context diagnostic: {diagnostic}"
    );
}

#[test]
fn spell_program_target_history_profile_accepts_a_non_clue_token_rider_and_count() {
    let ron = spell_target_damage_history_fixture(
        "Exile(subject: Chosen((kind: Creature)))",
        "ObjectWasDealtDamageThisTurn(object: ChosenTarget(group_index: 0, target_index: 0))",
        "(instruction_id: \"target\", role: Subject, target_group_index: 0)",
        "Some((groups: [(min: 1, max: 1, prompt: \"Choose target creature\", effect_indices: [])]))",
        "CreateTokens(token: \"food\", count: 2, who: Controller, tapped: false, sacrifice_timing: None)",
        true,
    );
    let registry = compile_spell_target_history_fixture(&ron)
        .expect("the target-history profile composes with an ordinary two-Food rider");
    let owner = crate::ProgramOwnerId::face(
        "spell_target_history_fixture",
        crate::CardFaceId::new("spell_target_history_fixture").unwrap(),
    );
    let program = registry.compiled_program_for_owner(&owner).unwrap();
    assert!(matches!(
        &program.body.steps[1].kind,
        crate::primitives::CompiledInstructionKind::If { yes, .. }
            if matches!(
                &yes.steps[0].kind,
                crate::primitives::CompiledInstructionKind::Perform(effect)
                    if matches!(effect.as_ref(), crate::primitives::SpellEffectKind::CreateTokens {
                        token, count: crate::primitives::Amount::Fixed(2), ..
                    } if token == "food")
            )
    ));
}

#[test]
fn spell_program_target_damage_history_requires_the_bound_singleton_creature_exile() {
    let condition =
        "ObjectWasDealtDamageThisTurn(object: ChosenTarget(group_index: 0, target_index: 0))";
    let subject_binding = "(instruction_id: \"target\", role: Subject, target_group_index: 0)";
    let singleton =
        "Some((groups: [(min: 1, max: 1, prompt: \"Choose target creature\", effect_indices: [])]))";
    let clue =
        "CreateTokens(token: \"clue\", count: 1, who: Controller, tapped: false, sacrifice_timing: None)";
    let cases = [
        spell_target_damage_history_fixture(
            "Destroy(subject: Chosen((kind: Creature)))",
            condition,
            subject_binding,
            singleton,
            clue,
            true,
        ),
        spell_target_damage_history_fixture(
            "Exile(subject: Chosen((kind: Creature)))",
            condition,
            "",
            singleton,
            clue,
            true,
        ),
        spell_target_damage_history_fixture(
            "Exile(subject: Chosen((kind: Creature)))",
            condition,
            "(instruction_id: \"target\", role: Target, target_group_index: 0)",
            singleton,
            clue,
            true,
        ),
        spell_target_damage_history_fixture(
            "Exile(subject: Chosen((kind: AnyPermanent)))",
            condition,
            subject_binding,
            singleton,
            clue,
            true,
        ),
        spell_target_damage_history_fixture(
            "Exile(subject: Chosen((kind: Creature)))",
            condition,
            subject_binding,
            "Some((groups: [(min: 1, max: 2, prompt: \"Choose target creatures\", effect_indices: [])]))",
            clue,
            true,
        ),
        spell_target_damage_history_fixture(
            "Exile(subject: Chosen((kind: Creature)))",
            condition,
            subject_binding,
            "Some((groups: [(min: 0, max: 1, prompt: \"Choose a target creature\", effect_indices: [])]))",
            clue,
            true,
        ),
        spell_target_damage_history_fixture(
            "Exile(subject: Chosen((kind: Creature)))",
            "ObjectWasDealtDamageThisTurn(object: ChosenTarget(group_index: 0, target_index: 1))",
            subject_binding,
            singleton,
            clue,
            true,
        ),
        spell_target_damage_history_fixture(
            "Exile(subject: Chosen((kind: Creature)))",
            "ObjectWasDealtDamageThisTurn(object: ChosenTarget(group_index: 1, target_index: 0))",
            subject_binding,
            singleton,
            clue,
            true,
        ),
        spell_target_damage_history_fixture(
            "Exile(subject: Chosen((kind: Creature)))",
            "AllOf([ObjectWasDealtDamageThisTurn(object: ChosenTarget(group_index: 0, target_index: 0))])",
            subject_binding,
            singleton,
            clue,
            true,
        ),
        spell_target_damage_history_fixture(
            "Exile(subject: Chosen((kind: Creature)))",
            condition,
            subject_binding,
            singleton,
            clue,
            false,
        ),
        spell_target_damage_history_fixture(
            "Exile(subject: Chosen((kind: Creature)), linked_exile_id: \"other\")",
            condition,
            subject_binding,
            singleton,
            clue,
            true,
        ),
    ];
    for ron in cases {
        assert!(
            compile_spell_target_history_fixture(&ron).is_err(),
            "unsupported target-history profile unexpectedly compiled: {ron}"
        );
    }
}

#[test]
fn spell_program_target_history_condition_without_authored_target_group_is_rejected() {
    let ron = spell_target_damage_history_fixture(
        "Destroy(subject: Chosen((kind: Creature)))",
        "ObjectWasDealtDamageThisTurn(object: ChosenTarget(group_index: 0, target_index: 0))",
        "(instruction_id: \"target\", role: Subject, target_group_index: 0)",
        "None",
        "CreateTokens(token: \"clue\", count: 1, who: Controller, tapped: false, sacrifice_timing: None)",
        true,
    );
    assert!(compile_spell_target_history_fixture(&ron).is_err());
}

#[test]
fn registry_compiles_resultless_return_all_to_owners_hand_program_leaf() {
    let ron = r#"(
        id: "return_all_to_owners_hand_program_fixture",
        name: "Return All To Owners Hand Program Fixture",
        face_id: "return_all_to_owners_hand_program_fixture",
        types: ["Sorcery"],
        spell_program: Some((body: (steps: [
            (id: "return_blue", kind: Perform(ReturnAllToOwnersHand(
                kind: (kind: AnyPermanent, is_color: Some(Blue))))),
        ], yields: []), target_bindings: [])),
    )"#;
    let registry = crate::CardRegistry::from_chunks_and_tokens(&[ron], &[])
        .expect("the existing mass-return leaf is valid inside a resolution program");
    let owner = crate::ProgramOwnerId::face(
        "return_all_to_owners_hand_program_fixture",
        crate::CardFaceId::new("return_all_to_owners_hand_program_fixture").unwrap(),
    );
    let program = registry
        .compiled_program_for_owner(&owner)
        .expect("the complete authored owner should compile as one body");
    assert_eq!(program.body.steps.len(), 1);
    assert!(matches!(
        &program.body.steps[0].kind,
        crate::primitives::CompiledInstructionKind::Perform(effect)
            if matches!(effect.as_ref(), crate::primitives::SpellEffectKind::ReturnAllToOwnersHand { kind }
                if kind.is_color == Some(crate::Color::Blue))
    ));
}

#[test]
fn registry_compiles_nested_authored_target_roles_by_instruction_id() {
    let ron = r#"(
        id: "nested_target_program_fixture", name: "Nested Target Program Fixture",
        face_id: "nested_target_program_fixture", types: ["Sorcery"],
        spell_effect: [],
        spell_program: Some((body: (steps: [
            (id: "conditional_damage", kind: If(condition: ControllerLibraryEmpty,
                yes: (steps: [(id: "damage", kind: Perform(DamageTarget(
                    amount: 3, target: (kind: Creature))))], yields: []),
                no: (steps: [], yields: [])))
        ], yields: []), target_bindings: [
            (instruction_id: "damage", role: Target, target_group_index: 0)
        ])),
        targeting: Some((groups: [
            (min: 1, max: 1, prompt: "Choose target creature", effect_indices: [])
        ]))
    )"#;
    let registry = crate::CardRegistry::from_chunks_and_tokens(&[ron], &[])
        .expect("the authored compiler binds targets inside nested branches by instruction id");
    let owner = crate::ProgramOwnerId::face(
        "nested_target_program_fixture",
        crate::CardFaceId::new("nested_target_program_fixture").unwrap(),
    );
    let program = registry
        .compiled_program_for_owner(&owner)
        .expect("authored face program is registered");
    assert!(
        matches!(
            &program.body.steps[0].kind,
            crate::primitives::CompiledInstructionKind::If { yes, .. }
                if matches!(yes.steps[0].kind, crate::primitives::CompiledInstructionKind::Perform(_))
        ),
        "nested target instruction remains inside its explicit branch"
    );
    assert_eq!(program.target_bindings.len(), 1);
    assert_eq!(program.target_bindings[0].instruction_address, 1);
    assert_eq!(program.target_bindings[0].target_group_index, 0);
    assert_eq!(program.target_bindings[0].role_index, 0);
}

#[test]
fn registry_compiles_authored_discard_count_branch_to_its_typed_result_slot() {
    let card = r#"(
        id: "discard_branch_fixture", name: "Discard Branch Fixture",
        face_id: "discard_branch_fixture", types: ["Creature"], power: 2, toughness: 2,
        triggered_abilities: [(ability_id: "triggered_01", presentation: Fallback,
            trigger: WhenSelfEntersBattlefield, effect: [],
            program: Some((body: (steps: [
                (id: "draw", kind: Perform(Draw(who: Controller, count: 1))),
                (id: "discard", bind: ["discarded"], kind: Perform(Discard(
                    who: Controller, quantity: Exact(1)
                ))),
                (id: "create_soldier_if_nonland", kind: IfResult(
                    condition: DiscardCount(result: "discarded", players: Controller,
                        card_type: Some(Nonland), min: Some(1), max: None),
                    yes: (steps: [(id: "create_soldier", kind: Perform(CreateTokens(
                        token: "soldier_w_1_1", count: 1, who: Controller,
                        tapped: false, sacrifice_timing: None
                    )))], yields: []),
                    no: (steps: [], yields: [])
                ))
            ], yields: []))),
            modal: None, targeting: None, may: false, intervening_if: None,
            triggers_only_once: false, max_triggers_per_turn: None
        )]
    )"#;
    let token = r#"(id: "soldier_w_1_1", name: "Soldier", face_id: "soldier_w_1_1",
        types: ["Creature", "Soldier"], colors: [White], power: 1, toughness: 1)"#;
    let registry = crate::CardRegistry::from_chunks_and_tokens(&[card], &[token])
        .expect("authored typed-discard branch and registered token should load");
    let owner = crate::ProgramOwnerId::ability(crate::AbilityDefinitionId {
        card_id: "discard_branch_fixture".into(),
        face_id: crate::CardFaceId::new("discard_branch_fixture").unwrap(),
        ability_path: vec![crate::AbilityId::new("triggered_01").unwrap()],
    });
    let program = registry
        .compiled_program_for_owner(&owner)
        .expect("the authored triggered owner should compile");
    assert_eq!(program.body.steps.len(), 3);
    let crate::primitives::CompiledInstructionKind::If { condition, yes, no } =
        &program.body.steps[2].kind
    else {
        panic!("Recruit branch must consume the typed discard result");
    };
    let crate::primitives::CompiledProgramCondition::Result(
        crate::primitives::CompiledProgramResultCondition::DiscardCount { result, min, .. },
    ) = condition.as_ref()
    else {
        panic!("Recruit branch must use the typed discard count condition");
    };
    assert_eq!(*min, Some(1));
    assert_eq!(
        program.slot(*result).unwrap().result_type,
        crate::primitives::ResultType::DiscardResult {
            who: crate::primitives::PlayerRecipient::Controller,
            cardinality: crate::primitives::Cardinality {
                min: 0,
                max: Some(1)
            },
            optional_payment: false,
        }
    );
    assert!(matches!(
        &yes.steps[0].kind,
        crate::primitives::CompiledInstructionKind::Perform(effect)
            if matches!(effect.as_ref(), crate::primitives::SpellEffectKind::CreateTokens { .. })
    ));
    assert_eq!(yes.steps[0].id, "create_soldier");
    assert!(no.steps.is_empty());
}

#[test]
fn registry_requires_a_draw_step_producer_for_affected_player_program_input() {
    let make_card = |trigger| {
        format!(
            r#"(id: "affected_draw_fixture", name: "Affected Draw Fixture",
                face_id: "affected_draw_fixture", types: ["Creature"], power: 1, toughness: 1,
                triggered_abilities: [(ability_id: "triggered_01", presentation: Fallback,
                    trigger: {trigger}, effect: [], program: Some((body: (steps: [
                        (id: "draw", kind: Perform(Draw(who: AffectedPlayer, count: 1))),
                        (id: "discard", kind: Perform(Discard(who: AffectedPlayer,
                            quantity: Exact(1))))
                    ], yields: []))), modal: None, targeting: None,
                    may: false, intervening_if: None, triggers_only_once: false,
                    max_triggers_per_turn: None)])"#
        )
    };
    let wrong_trigger = make_card("WhenSelfEntersBattlefield");
    let error = crate::CardRegistry::from_chunks_and_tokens(&[&wrong_trigger], &[])
        .expect_err("an unrelated event does not supply an affected player");
    assert!(format!("{error}").contains("trigger does not supply"));

    let draw_step = make_card("AtBeginningOfDrawStep(player: AnyPlayer)");
    let registry = crate::CardRegistry::from_chunks_and_tokens(&[&draw_step], &[])
        .expect("the generic draw-step trigger supplies its event player");
    let owner = crate::ProgramOwnerId::ability(crate::AbilityDefinitionId {
        card_id: "affected_draw_fixture".into(),
        face_id: crate::CardFaceId::new("affected_draw_fixture").unwrap(),
        ability_path: vec![crate::AbilityId::new("triggered_01").unwrap()],
    });
    assert_eq!(
        registry
            .compiled_program_for_owner(&owner)
            .unwrap()
            .required_inputs,
        vec![crate::primitives::ProgramRequiredInput::TriggerAffectedPlayer]
    );
}

fn named_card_result_count_card(id: &str, producer_step: &str, condition: &str) -> String {
    format!(
        r#"(id: "{id}", name: "Named Card Result Count Fixture",
            face_id: "{id}", types: ["Creature"], power: 1, toughness: 1,
            triggered_abilities: [(ability_id: "triggered_01", presentation: Fallback,
                trigger: WhenSelfEntersBattlefield, effect: [],
                program: Some((body: (steps: [
                    {producer_step},
                    (id: "count", kind: IfResult(condition: {condition},
                        yes: (steps: [], yields: []), no: (steps: [], yields: [])))
                ], yields: []))), modal: None, targeting: None,
                may: false, intervening_if: None, triggers_only_once: false,
                max_triggers_per_turn: None)])"#
    )
}

fn expect_named_count_rejection(id: &str, producer_step: &str, condition: &str, reason: &str) {
    let card = named_card_result_count_card(id, producer_step, condition);
    let error = crate::CardRegistry::from_chunks_and_tokens(&[&card], &[])
        .expect_err("invalid typed card-result count predicate must be rejected");
    assert!(
        format!("{error}").contains(reason),
        "expected {reason:?} in {error}; card: {card}"
    );
}

#[test]
fn registry_compiles_named_card_result_count_for_source_sacrifice_and_mill_cohorts() {
    let cases = [
        (
            "named_count_source_sacrifice",
            r#"(id: "produce", bind: ["cohort"], kind: Perform(Sacrifice(subject: Source)))"#,
            r#"CardResultCount(result: "cohort", min: Some(1), max: None)"#,
            crate::primitives::ResultType::CardResultCohort {
                action: crate::primitives::CardResultAction::Sacrifice,
                who: crate::primitives::PlayerRecipient::Controller,
                cardinality: crate::primitives::Cardinality {
                    min: 0,
                    max: Some(1),
                },
            },
            "min: Some(1), max: None",
        ),
        (
            "named_count_source_sacrifice_bounded",
            r#"(id: "produce", bind: ["cohort"], kind: Perform(Sacrifice(subject: Source)))"#,
            r#"CardResultCount(result: "cohort", min: Some(1), max: Some(1))"#,
            crate::primitives::ResultType::CardResultCohort {
                action: crate::primitives::CardResultAction::Sacrifice,
                who: crate::primitives::PlayerRecipient::Controller,
                cardinality: crate::primitives::Cardinality {
                    min: 0,
                    max: Some(1),
                },
            },
            "min: Some(1), max: Some(1)",
        ),
        (
            "named_count_mill_bounded",
            r#"(id: "produce", bind: ["cohort"], kind: Perform(Mill(who: Controller, count: 3)))"#,
            r#"CardResultCount(result: "cohort", min: Some(1), max: Some(3))"#,
            crate::primitives::ResultType::CardResultCohort {
                action: crate::primitives::CardResultAction::Mill,
                who: crate::primitives::PlayerRecipient::Controller,
                cardinality: crate::primitives::Cardinality {
                    min: 0,
                    max: Some(3),
                },
            },
            "min: Some(1), max: Some(3)",
        ),
        (
            "named_count_mill_nonland",
            r#"(id: "produce", bind: ["cohort"], kind: Perform(Mill(who: Controller, count: 3)))"#,
            r#"CardResultCount(result: "cohort", card_type: Some(Nonland), min: Some(1), max: None)"#,
            crate::primitives::ResultType::CardResultCohort {
                action: crate::primitives::CardResultAction::Mill,
                who: crate::primitives::PlayerRecipient::Controller,
                cardinality: crate::primitives::Cardinality {
                    min: 0,
                    max: Some(3),
                },
            },
            "min: Some(1), max: None",
        ),
        (
            "named_count_mill_land_bounded",
            r#"(id: "produce", bind: ["cohort"], kind: Perform(Mill(who: Controller, count: 3)))"#,
            r#"CardResultCount(result: "cohort", card_type: Some(Land), min: Some(1), max: Some(2))"#,
            crate::primitives::ResultType::CardResultCohort {
                action: crate::primitives::CardResultAction::Mill,
                who: crate::primitives::PlayerRecipient::Controller,
                cardinality: crate::primitives::Cardinality {
                    min: 0,
                    max: Some(3),
                },
            },
            "min: Some(1), max: Some(2)",
        ),
        (
            "named_count_mill_higher_minimum",
            r#"(id: "produce", bind: ["cohort"], kind: Perform(Mill(who: Controller, count: 3)))"#,
            r#"CardResultCount(result: "cohort", min: Some(2), max: Some(3))"#,
            crate::primitives::ResultType::CardResultCohort {
                action: crate::primitives::CardResultAction::Mill,
                who: crate::primitives::PlayerRecipient::Controller,
                cardinality: crate::primitives::Cardinality {
                    min: 0,
                    max: Some(3),
                },
            },
            "min: Some(2), max: Some(3)",
        ),
        (
            "named_count_mill_max_only",
            r#"(id: "produce", bind: ["cohort"], kind: Perform(Mill(who: Controller, count: 3)))"#,
            r#"CardResultCount(result: "cohort", min: None, max: Some(1))"#,
            crate::primitives::ResultType::CardResultCohort {
                action: crate::primitives::CardResultAction::Mill,
                who: crate::primitives::PlayerRecipient::Controller,
                cardinality: crate::primitives::Cardinality {
                    min: 0,
                    max: Some(3),
                },
            },
            "min: None, max: Some(1)",
        ),
    ];

    for (id, producer_step, condition, expected_type, expected_bounds) in cases {
        let card = named_card_result_count_card(id, producer_step, condition);
        let expects_type_filter = condition.contains("card_type: Some(");
        let registry = crate::CardRegistry::from_chunks_and_tokens(&[&card], &[])
            .expect("a named count over a typed card-result cohort should compile");
        let owner = crate::ProgramOwnerId::ability(crate::AbilityDefinitionId {
            card_id: id.into(),
            face_id: crate::CardFaceId::new(id).unwrap(),
            ability_path: vec![crate::AbilityId::new("triggered_01").unwrap()],
        });
        let program = registry
            .compiled_program_for_owner(&owner)
            .expect("the triggered owner should compile");
        assert_eq!(
            program
                .slot(crate::primitives::ResultSlotId(0))
                .unwrap()
                .result_type,
            expected_type
        );
        let crate::primitives::CompiledInstructionKind::If {
            condition: compiled_condition,
            ..
        } = &program.body.steps[1].kind
        else {
            panic!("named result count should compile as an IfResult condition");
        };
        let compiled = format!("{compiled_condition:?}");
        assert!(compiled.contains("CardResultCount"), "{compiled}");
        assert!(compiled.contains("result: ResultSlotId(0)"), "{compiled}");
        assert!(compiled.contains(expected_bounds), "{compiled}");
        if expects_type_filter {
            assert!(
                compiled.contains("card_type: Some("),
                "typed count keeps its exact type predicate: {compiled}"
            );
        }
    }
}

#[test]
fn registry_rejects_invalid_named_card_result_count_bounds() {
    let producer =
        r#"(id: "produce", bind: ["cohort"], kind: Perform(Mill(who: Controller, count: 3)))"#;
    expect_named_count_rejection(
        "named_count_empty_bounds",
        producer,
        r#"CardResultCount(result: "cohort", min: None, max: None)"#,
        "requires a minimum or maximum",
    );
    expect_named_count_rejection(
        "named_count_inverted_bounds",
        producer,
        r#"CardResultCount(result: "cohort", min: Some(2), max: Some(1))"#,
        "minimum cannot exceed its maximum",
    );
    expect_named_count_rejection(
        "named_count_above_typed_upper_bound",
        r#"(id: "produce", bind: ["cohort"], kind: Perform(Sacrifice(subject: Source)))"#,
        r#"CardResultCount(result: "cohort", min: Some(2), max: None)"#,
        "exceeds its typed cohort cardinality",
    );
    expect_named_count_rejection(
        "named_count_max_above_typed_upper_bound",
        r#"(id: "produce", bind: ["cohort"], kind: Perform(Sacrifice(subject: Source)))"#,
        r#"CardResultCount(result: "cohort", min: None, max: Some(2))"#,
        "exceeds its typed cohort cardinality",
    );
    expect_named_count_rejection(
        "named_count_filtered_empty_bounds",
        producer,
        r#"CardResultCount(result: "cohort", card_type: Some(Nonland), min: None, max: None)"#,
        "requires a minimum or maximum",
    );
    expect_named_count_rejection(
        "named_count_filtered_inverted_bounds",
        producer,
        r#"CardResultCount(result: "cohort", card_type: Some(Land), min: Some(2), max: Some(1))"#,
        "minimum cannot exceed its maximum",
    );
    expect_named_count_rejection(
        "named_count_filtered_minimum_above_typed_upper_bound",
        producer,
        r#"CardResultCount(result: "cohort", card_type: Some(Nonland), min: Some(4), max: None)"#,
        "exceeds its typed cohort cardinality",
    );
    expect_named_count_rejection(
        "named_count_filter_wrong_result_type",
        r#"(id: "produce", bind: ["cohort"], kind: Perform(Draw(who: Controller, count: 1)))"#,
        r#"CardResultCount(result: "cohort", card_type: Some(Nonland), min: Some(1), max: None)"#,
        "not a compatible card-result cohort",
    );

    let legacy_filter = named_card_result_count_card(
        "named_count_rejects_ambient_legacy_filter",
        producer,
        r#"CardResultCount(result: "cohort",
            filter: (source: PreviousEffect, action: Sacrifice, players: Controller),
            min: Some(1), max: None)"#,
    );
    assert!(
        crate::CardRegistry::from_chunks_and_tokens(&[&legacy_filter], &[]).is_err(),
        "a named typed-result count does not reintroduce ambient legacy filter arguments"
    );
}

#[test]
fn registry_rejects_unbound_and_out_of_scope_named_card_result_count_slots() {
    expect_named_count_rejection(
        "named_count_unbound_slot",
        r#"(id: "produce", kind: Perform(Sacrifice(subject: Source)))"#,
        r#"CardResultCount(result: "cohort", min: Some(1), max: None)"#,
        "result 'cohort' is not in scope",
    );
    expect_named_count_rejection(
        "named_count_out_of_scope_slot",
        r#"(id: "branch", kind: If(condition: Void,
            yes: (steps: [(id: "produce", bind: ["cohort"],
                kind: Perform(Sacrifice(subject: Source)))], yields: []),
            no: (steps: [], yields: [])))"#,
        r#"CardResultCount(result: "cohort", min: Some(1), max: None)"#,
        "result 'cohort' is not in scope",
    );
    expect_named_count_rejection(
        "named_count_filtered_unbound_slot",
        r#"(id: "produce", bind: ["cohort"], kind: Perform(Mill(who: Controller, count: 3)))"#,
        r#"CardResultCount(result: "missing", card_type: Some(Land), min: Some(1), max: None)"#,
        "result 'missing' is not in scope",
    );
}

#[test]
fn registry_compiles_cast_payment_import_late_and_inside_a_branch() {
    let spell = |id: &str, body: &str| {
        format!(
            r#"(id: "{id}", name: "Cast Payment Import Fixture", face_id: "{id}",
                mana_cost: "{{1}}{{R}}", types: ["Sorcery"],
                additional_costs: [DiscardCard],
                spell_program: Some((body: {body}, target_bindings: [])))"#
        )
    };
    let late_import = spell(
        "cast_payment_import_late",
        r#"(steps: [
            (id: "draw", bind: [], input: None,
                kind: Perform(Draw(who: Controller, count: 2))),
            (id: "import_payment", bind: ["paid"], input: None,
                kind: ImportCastPayment),
            (id: "count_nonland", bind: [], input: None,
                kind: IfResult(
                    condition: CardResultCount(result: "paid", card_type: Some(Nonland),
                        min: Some(1), max: None),
                    yes: (steps: [], yields: []), no: (steps: [], yields: []),
                )),
        ], yields: [])"#,
    );
    let registry = crate::CardRegistry::from_chunks_and_tokens(&[&late_import], &[])
        .expect("the exact mandatory discard profile compiles a named cast-payment count");
    let owner = crate::ProgramOwnerId::face(
        "cast_payment_import_late",
        crate::CardFaceId::new("cast_payment_import_late").unwrap(),
    );
    let program = registry
        .compiled_program_for_owner(&owner)
        .expect("the exact spell face owns its compiled Program");
    assert_eq!(
        program
            .body
            .steps
            .iter()
            .map(|step| step.id.as_str())
            .collect::<Vec<_>>(),
        ["draw", "import_payment", "count_nonland"],
        "import need not be the first authored instruction"
    );
    let payment_slot = program
        .slots
        .iter()
        .find(|slot| slot.name == "paid")
        .expect("the imported payment has a named typed result slot");
    let payment_type = format!("{:?}", payment_slot.result_type);
    assert!(payment_type.contains("CastPaymentCohort"), "{payment_type}");
    assert!(payment_type.contains("Discard"), "{payment_type}");
    assert!(payment_type.contains("min: 1"), "{payment_type}");
    assert!(payment_type.contains("max: Some(1)"), "{payment_type}");
    assert!(
        !payment_type.contains("who:"),
        "the imported payer is not reinterpreted as an effect recipient: {payment_type}"
    );
    let count = format!("{:?}", program.body.steps[2].kind);
    assert!(count.contains("CardResultCount"), "{count}");
    assert!(count.contains("card_type: Some(Nonland)"), "{count}");
    assert!(count.contains("min: Some(1), max: None"), "{count}");

    let nested_import = spell(
        "cast_payment_import_nested",
        r#"(steps: [(id: "route", bind: [], input: None, kind: If(
            condition: Void,
            yes: (steps: [(id: "import_payment", bind: ["paid"], input: None,
                kind: ImportCastPayment)], yields: []),
            no: (steps: [], yields: []),
        ))], yields: [])"#,
    );
    let registry = crate::CardRegistry::from_chunks_and_tokens(&[&nested_import], &[])
        .expect("a typed import inside a nested block is admitted for its exact spell owner");
    let owner = crate::ProgramOwnerId::face(
        "cast_payment_import_nested",
        crate::CardFaceId::new("cast_payment_import_nested").unwrap(),
    );
    assert!(registry.compiled_program_for_owner(&owner).is_some());
}

#[test]
fn registry_rejects_cast_payment_import_outside_exact_mandatory_discard_spell_profile() {
    let body = r#"(steps: [(id: "import_payment", bind: ["paid"], input: None,
        kind: ImportCastPayment)], yields: [])"#;
    let spell = |id: &str, types: &str, face_fields: &str, cost_fields: &str, owner_field: &str| {
        format!(
            r#"(id: "{id}", name: "Invalid Cast Payment Import Fixture", face_id: "{id}",
                mana_cost: "{{1}}{{R}}", types: {types}, {face_fields} {cost_fields}
                {owner_field})"#
        )
    };
    let program_owner_field = format!("spell_program: Some((body: {body}, target_bindings: []))");
    let invalid_cost_profiles = [
        ("cast_payment_import_without_cost", "[\"Instant\"]", "", ""),
        (
            "cast_payment_import_mixed_additional_costs",
            "[\"Instant\"]",
            "",
            r#"additional_costs: [DiscardCard, SacrificePermanent(
                filter: (kind: AnyPermanent, controller: You, permanent_types: [Artifact]))],"#,
        ),
        (
            "cast_payment_import_with_cast_cost_group",
            "[\"Instant\"]",
            "",
            r#"additional_costs: [DiscardCard], cast_cost_groups: [(group_id: "kicker",
                presentation: Fallback, min: 0, max: 1, options: [Mana(option_id: "kick",
                    presentation: Fallback, kind: Kicker, cost: "{1}")])],"#,
        ),
        (
            "cast_payment_import_with_flashback_profile",
            "[\"Instant\"]",
            "",
            r#"additional_costs: [DiscardCard], flashback_cost: Some("{R}"),"#,
        ),
        (
            "cast_payment_import_with_harmonize_profile",
            "[\"Instant\"]",
            "",
            r#"additional_costs: [DiscardCard], harmonize_cost: Some("{R}"),"#,
        ),
        (
            "cast_payment_import_with_sneak_profile",
            "[\"Instant\"]",
            "",
            r#"additional_costs: [DiscardCard], sneak_cost: Some("{R}"),"#,
        ),
        (
            "cast_payment_import_with_instant_speed_receipt_profile",
            "[\"Instant\"]",
            "",
            r#"additional_costs: [DiscardCard], instant_speed_cast_cost: Some((
                group_id: "kicker", option_id: "kick", expected_selected: true)),
                cast_cost_groups: [(group_id: "kicker", presentation: Fallback,
                    min: 0, max: 1, options: [Mana(option_id: "kick",
                        presentation: Fallback, kind: Kicker, cost: "{1}")])],"#,
        ),
        (
            "cast_payment_import_with_cost_modifier_profile",
            "[\"Instant\"]",
            "",
            r#"additional_costs: [DiscardCard], cost_modifiers: [GenericReduction(amount: 1)],"#,
        ),
    ];
    for (id, types, face_fields, cost_fields) in invalid_cost_profiles {
        let definition = spell(id, types, face_fields, cost_fields, &program_owner_field);
        let _: crate::card_def::RawCardDefinition =
            ron::from_str(&definition).expect("invalid owner profiles still parse as card data");
        let error = crate::CardRegistry::from_chunks_and_tokens(&[&definition], &[])
            .expect_err("unsupported cast-payment owner profile must be rejected");
        assert!(
            error
                .to_string()
                .contains("ImportCastPayment requires a validated mandatory single-discard spell cost profile"),
            "{id} must reach the closed-profile compiler diagnostic, got {error}"
        );
    }

    // Prototype and Warp require permanent faces, while this registry currently admits authored
    // spell Programs only on Instant or Sorcery faces. These owners therefore cannot reach the
    // same compiler profile check as the remaining alternative-cost profiles.
    let prior_owner_profiles = [
        (
            "cast_payment_import_prototype_permanent_owner",
            "[\"Artifact\", \"Creature\"]",
            "power: Some(2), toughness: Some(2), prototype: Some((mana_cost: \"{R}\", power: 2, toughness: 2)),",
            "additional_costs: [DiscardCard],",
        ),
        (
            "cast_payment_import_warp_permanent_owner",
            "[\"Artifact\"]",
            "",
            "additional_costs: [DiscardCard], warp_cost: Some(\"{R}\"),",
        ),
    ];
    for (id, types, face_fields, cost_fields) in prior_owner_profiles {
        let definition = spell(id, types, face_fields, cost_fields, &program_owner_field);
        let _: crate::card_def::RawCardDefinition =
            ron::from_str(&definition).expect("permanent profile fixtures parse as card data");
        let error = crate::CardRegistry::from_chunks_and_tokens(&[&definition], &[])
            .expect_err("permanent-face Programs are outside the spell owner contract");
        assert!(
            error
                .to_string()
                .contains("spell_program requires an instant or sorcery face"),
            "{id} fails at the prior spell-owner validation, got {error}"
        );
    }

    let modal = spell(
        "cast_payment_import_modal_owner",
        "[\"Instant\"]",
        "",
        "additional_costs: [DiscardCard],",
        &format!(
            r#"modal_spell: Some((min_modes: 1, max_modes: 1, modes: [(
                mode_id: "mode_01", presentation: Fallback, effects: [],
                program: Some((body: {body}, target_bindings: [])),
            )]))"#
        ),
    );
    let _: crate::card_def::RawCardDefinition =
        ron::from_str(&modal).expect("modal payment fixture parses");
    let modal_error = crate::CardRegistry::from_chunks_and_tokens(&[&modal], &[])
        .expect_err("modal spell-mode owners are outside the initial payment import contract");
    assert!(
        modal_error.to_string().contains(
            "ImportCastPayment requires a validated mandatory single-discard spell cost profile"
        ),
        "the modal owner reaches the closed-profile compiler diagnostic: {modal_error}"
    );

    let ability = format!(
        r#"(id: "cast_payment_import_ability_owner", name: "Invalid Cast Payment Ability",
            face_id: "cast_payment_import_ability_owner", types: ["Creature"],
            power: Some(1), toughness: Some(1),
            triggered_abilities: [(ability_id: "triggered_01", presentation: Fallback,
                trigger: WhenSelfEntersBattlefield, effect: [],
                program: Some((body: {body}, target_bindings: [])), modal: None,
                targeting: None, may: false, intervening_if: None,
                triggers_only_once: false, max_triggers_per_turn: None)])"#
    );
    let _: crate::card_def::RawCardDefinition =
        ron::from_str(&ability).expect("ability payment fixture parses");
    let ability_error = crate::CardRegistry::from_chunks_and_tokens(&[&ability], &[])
        .expect_err("ability owners cannot import cast payment");
    assert!(
        ability_error.to_string().contains(
            "ImportCastPayment requires a validated mandatory single-discard spell cost profile"
        ),
        "the ability owner reaches the closed-profile compiler diagnostic: {ability_error}"
    );
}

#[test]
fn registry_rejects_named_card_result_count_on_non_cohort_slot() {
    expect_named_count_rejection(
        "named_count_wrong_result_type",
        r#"(id: "produce", bind: ["cohort"], kind: Perform(Draw(who: Controller, count: 1)))"#,
        r#"CardResultCount(result: "cohort", min: Some(1), max: None)"#,
        "not a compatible card-result cohort",
    );
}

#[test]
fn registry_admits_draw_to_each_other_player_from_captured_ability_actor_only() {
    let make_card = |id: &str, trigger: &str, effect: &str| {
        format!(
            r#"(id: "{id}", name: "Each Other Draw Fixture", face_id: "{id}",
                types: ["Creature"], power: 1, toughness: 1,
                triggered_abilities: [(ability_id: "triggered_01", presentation: Fallback,
                    trigger: {trigger}, effect: [],
                    program: Some((body: (steps: [(id: "effect", kind: Perform({effect}))],
                        yields: []))), modal: None, targeting: None,
                    may: false, intervening_if: None, triggers_only_once: false,
                    max_triggers_per_turn: None)])"#
        )
    };
    let effect = "Draw(who: EachOtherPlayerThanAffectedPlayer, count: 1)";
    let valid = make_card(
        "each_other_draw_valid",
        "WheneverPlayerCastsSpell(caster: AnyPlayer)",
        effect,
    );
    let registry = crate::CardRegistry::from_chunks_and_tokens(&[&valid], &[])
        .expect("spell-cast ability can draw for each player other than its captured caster");
    let owner = crate::ProgramOwnerId::ability(crate::AbilityDefinitionId {
        card_id: "each_other_draw_valid".into(),
        face_id: crate::CardFaceId::new("each_other_draw_valid").unwrap(),
        ability_path: vec![crate::AbilityId::new("triggered_01").unwrap()],
    });
    assert_eq!(
        registry
            .compiled_program_for_owner(&owner)
            .unwrap()
            .required_inputs,
        vec![crate::primitives::ProgramRequiredInput::TriggerAffectedPlayer]
    );

    let unsupported_trigger = make_card(
        "each_other_draw_unsupported_trigger",
        "WhenSelfEntersBattlefield",
        effect,
    );
    let error = crate::CardRegistry::from_chunks_and_tokens(&[&unsupported_trigger], &[])
        .expect_err("draw recipient requires a trigger with a captured affected player");
    assert!(format!("{error}").contains("trigger does not supply that input"));

    let spell = r#"(id: "each_other_draw_spell_stays_unsupported",
        name: "Each Other Draw Spell Fixture", face_id: "each_other_draw_spell_stays_unsupported",
        types: ["Sorcery"], spell_effect: [],
        spell_program: Some((body: (steps: [
            (id: "draw", kind: Perform(Draw(
                who: EachOtherPlayerThanAffectedPlayer, count: 1
            )))
        ], yields: []), target_bindings: [])))"#;
    assert!(spell.contains("EachOtherPlayerThanAffectedPlayer"));
    let error = crate::CardRegistry::from_chunks_and_tokens(&[spell], &[])
        .expect_err("ability-only Draw exception must not widen spell recipients");
    assert!(
        format!("{error}").contains("spells cannot reference triggering-spell context"),
        "{error}"
    );

    for (id, other_effect, reason) in [
        (
            "each_other_discard_stays_unsupported",
            "Discard(who: EachOtherPlayerThanAffectedPlayer, quantity: Exact(1))",
            "EachOtherPlayerThanAffectedPlayer is supported only by Draw",
        ),
        (
            "each_other_mill_stays_unsupported",
            "Mill(who: EachOtherPlayerThanAffectedPlayer, count: 1)",
            "EachOtherPlayerThanAffectedPlayer is supported only by Draw",
        ),
    ] {
        let card = make_card(
            id,
            "WheneverPlayerCastsSpell(caster: AnyPlayer)",
            other_effect,
        );
        let error = crate::CardRegistry::from_chunks_and_tokens(&[&card], &[])
            .expect_err("only Draw receives the new affected-player recipient exception");
        assert!(format!("{error}").contains(reason), "{error}");
    }
}

#[test]
fn resultless_program_can_copy_its_trigger_observed_object() {
    let program: crate::CardProgram = ron::from_str(
        r#"(body: (steps: [
            (id: "copy_observed", kind: Perform(CreateTokenCopies(
                count: 1, source: TriggerObject,
            )))
        ], yields: []))"#,
    )
    .expect("the observed-object program fixture parses");
    let owner = crate::ProgramOwnerId::ability(crate::AbilityDefinitionId {
        card_id: "observed_copy_fixture".into(),
        face_id: crate::CardFaceId::new("observed_copy_fixture").unwrap(),
        ability_path: vec![crate::AbilityId::new("triggered_01").unwrap()],
    });

    let compiled = crate::primitives::CompiledProgram::compile(
        owner,
        crate::primitives::EffectContext::Ability,
        &program,
    )
    .expect("an ability program may consume its captured entering-object snapshot");
    assert_eq!(
        compiled.required_inputs,
        vec![crate::primitives::ProgramRequiredInput::TriggerObservedObject]
    );
    assert!(compiled.target_bindings.is_empty());
    assert!(compiled.slots.is_empty());
    assert!(compiled.body.steps[0].outputs.is_empty());
    assert!(matches!(
        &compiled.body.steps[0].kind,
        crate::primitives::CompiledInstructionKind::Perform(effect)
            if matches!(effect.as_ref(), crate::primitives::SpellEffectKind::CreateTokenCopies {
                count: crate::primitives::Amount::Fixed(1),
                source: crate::primitives::TokenCopySource::TriggerObject,
            })
    ));
}

const FLAMESHADOW_TOKEN_BATCH_PROGRAM_RON: &str = r#"(body: (steps: [
            (id: "choose_copy", kind: Choose(
                choice: "copy", chooser: Controller, optional: true,
                branches: [(branch_id: "pay_to_copy", presentation: OracleLines([1]),
                    cost: Mana("{R}"), requirement: Always,
                    body: (steps: [
                        (id: "copy_entry", bind: ["tokens"], kind: Perform(
                            CreateTokenCopies(count: 2, source: TriggerObject)
                        )),
                        (id: "grant_haste", input: Some("tokens"), kind: Perform(
                            ApplyPermanentModifier(
                                subject: PreviousEffectObjects,
                                modifier: GrantKeywords([Haste]), duration: Indefinite,
                            )
                        )),
                        (id: "schedule_exile", input: Some("tokens"), kind: Perform(
                            CreateDelayedTrigger(
                                subject: Some(PreviousEffectObjects),
                                ability: (
                                    ability_id: "delayed_01",
                                    presentation: OracleLines([1]),
                                    trigger: AtBeginningOfNextEndStep,
                                    effect: [ExileObservedObjects],
                                ),
                            )
                        )),
                    ], yields: []))],
                otherwise: (steps: [], yields: []),
            ))
        ], yields: []))"#;

fn compile_flameshadow_token_batch_program(
    ron_source: &str,
) -> Result<crate::primitives::CompiledProgram, String> {
    let program: crate::CardProgram =
        ron::from_str(ron_source).expect("the Flameshadow-like token-batch program fixture parses");
    let owner = crate::ProgramOwnerId::ability(crate::AbilityDefinitionId {
        card_id: "flameshadow_batch_fixture".into(),
        face_id: crate::CardFaceId::new("flameshadow_batch_fixture").unwrap(),
        ability_path: vec![crate::AbilityId::new("triggered_01").unwrap()],
    });

    crate::primitives::CompiledProgram::compile(
        owner,
        crate::primitives::EffectContext::Ability,
        &program,
    )
}

#[test]
fn flameshadow_trigger_object_token_batch_program_compiles() {
    let compiled = compile_flameshadow_token_batch_program(FLAMESHADOW_TOKEN_BATCH_PROGRAM_RON)
        .expect("a TriggerObject copy batch can feed the matching Haste and delayed-exile steps");
    assert_eq!(
        compiled.required_inputs,
        vec![crate::primitives::ProgramRequiredInput::TriggerObservedObject]
    );
    assert_eq!(compiled.slots.len(), 1);
    let crate::primitives::CompiledInstructionKind::Choose { branches, .. } =
        &compiled.body.steps[0].kind
    else {
        panic!("the paid copy is a Choose branch");
    };
    assert_eq!(branches.len(), 1);
    assert_eq!(branches[0].body.steps.len(), 3);
    let copy = &branches[0].body.steps[0];
    assert_eq!(copy.outputs.len(), 1);
    assert!(matches!(
        compiled.slot(copy.outputs[0]).map(|slot| &slot.result_type),
        Some(crate::primitives::ResultType::TokenCopyBatch)
    ));
    assert_eq!(
        branches[0].body.steps[1].input,
        copy.outputs.first().copied()
    );
    assert_eq!(
        branches[0].body.steps[2].input,
        copy.outputs.first().copied()
    );
}

#[test]
fn program_compiler_rejects_unapproved_token_copy_batch_shapes() {
    for (label, old, new, expected_error) in [
        (
            "chosen copies stay resultless",
            "source: TriggerObject",
            "source: Chosen((kind: Creature))",
            "binds 1 names for 0 results",
        ),
        (
            "wrong result type",
            "CreateTokenCopies(count: 2, source: TriggerObject)",
            "Draw(count: 1)",
            "wrong type for this instruction",
        ),
        (
            "haste keyword is closed",
            "modifier: GrantKeywords([Haste])",
            "modifier: GrantKeywords([Flying])",
            "does not accept an input result",
        ),
        (
            "haste duration is closed",
            "duration: Indefinite",
            "duration: UntilEndOfTurn",
            "does not accept an input result",
        ),
        (
            "delayed timing is closed",
            "trigger: AtBeginningOfNextEndStep",
            "trigger: AtBeginningOfNextTurnUpkeep",
            "does not accept an input result",
        ),
        (
            "delayed body is closed",
            "effect: [ExileObservedObjects]",
            "effect: [ExileWarpedObject]",
            "does not accept an input result",
        ),
        (
            "delayed affected player is closed",
            "subject: Some(PreviousEffectObjects),\n                                ability:",
            "subject: Some(PreviousEffectObjects),\n                                affected_player: Some(Controller),\n                                ability:",
            "does not accept an input result",
        ),
        (
            "delayed may is closed",
            "trigger: AtBeginningOfNextEndStep,\n                                    effect:",
            "trigger: AtBeginningOfNextEndStep,\n                                    may: true,\n                                    effect:",
            "does not accept an input result",
        ),
        (
            "delayed target is closed",
            "trigger: AtBeginningOfNextEndStep,\n                                    effect:",
            "trigger: AtBeginningOfNextEndStep,\n                                    targeting: Some((groups: [])),\n                                    effect:",
            "does not accept an input result",
        ),
    ] {
        let malformed = FLAMESHADOW_TOKEN_BATCH_PROGRAM_RON.replace(old, new);
        assert_ne!(malformed, FLAMESHADOW_TOKEN_BATCH_PROGRAM_RON, "{label} fixture changed");
        let error = compile_flameshadow_token_batch_program(&malformed)
            .expect_err("unsupported token-batch source, type, or consumer shape must fail closed");
        assert!(
            error.contains(expected_error),
            "{label}: expected {expected_error:?}, got {error:?}"
        );
    }

    let unavailable = FLAMESHADOW_TOKEN_BATCH_PROGRAM_RON.replace(
        "input: Some(\"tokens\"), kind: Perform(\n                            ApplyPermanentModifier",
        "input: Some(\"missing\"), kind: Perform(\n                            ApplyPermanentModifier",
    );
    assert_ne!(unavailable, FLAMESHADOW_TOKEN_BATCH_PROGRAM_RON);
    let error = compile_flameshadow_token_batch_program(&unavailable)
        .expect_err("a consumer cannot read a missing batch slot");
    assert!(
        error.contains("result 'missing' is not in scope"),
        "{error}"
    );
}

#[test]
fn trigger_observed_object_program_input_is_ability_only_and_trigger_bound() {
    let program = r#"(body: (steps: [
        (id: "copy_observed", kind: Perform(CreateTokenCopies(
            count: 1, source: TriggerObject,
        )))
    ], yields: []))"#;
    let spell_program: crate::CardProgram = ron::from_str(program).unwrap();
    let spell_error = crate::primitives::CompiledProgram::compile(
        crate::ProgramOwnerId::face(
            "observed_spell",
            crate::CardFaceId::new("observed_spell").unwrap(),
        ),
        crate::primitives::EffectContext::Spell,
        &spell_program,
    )
    .expect_err("a spell cannot read a trigger event object");
    assert!(
        spell_error.contains("spells cannot reference a trigger object"),
        "{spell_error}"
    );

    let triggered = format!(
        r#"(id: "observed_trigger", name: "Observed Trigger", face_id: "observed_trigger",
            types: ["Artifact"], triggered_abilities: [(
                ability_id: "triggered_01", presentation: Fallback,
                trigger: WheneverYouProliferate, effect: [], program: Some({program}),
            )])"#
    );
    let trigger_error = crate::CardRegistry::from_chunks_and_tokens(&[&triggered], &[])
        .expect_err("a non-entry trigger does not supply a captured observed permanent")
        .to_string();
    assert!(
        trigger_error.contains("TriggerObservedObject"),
        "{trigger_error}"
    );
    assert!(
        trigger_error.contains("trigger does not supply"),
        "{trigger_error}"
    );

    let activated = format!(
        r#"(id: "observed_activated", name: "Observed Activated", face_id: "observed_activated",
            types: ["Artifact"], activated_abilities: [(
                ability_id: "activated_01", presentation: Fallback,
                costs: [], effect: [], program: Some({program}),
            )])"#
    );
    let activation_error = crate::CardRegistry::from_chunks_and_tokens(&[&activated], &[])
        .expect_err("an activated ability has no observed trigger object")
        .to_string();
    assert!(
        activation_error.contains("TriggerObservedObject"),
        "{activation_error}"
    );
    assert!(
        activation_error.contains("trigger does not supply"),
        "{activation_error}"
    );

    for source in [
        "Source",
        "LinkedExile(producer_ability_id: \"producer\", linked_exile_id: \"linked\")",
    ] {
        let unsupported: crate::CardProgram = ron::from_str(&format!(
            r#"(body: (steps: [
                (id: "copy", kind: Perform(CreateTokenCopies(count: 1, source: {source})))
            ], yields: []))"#
        ))
        .expect("unsupported source fixture parses");
        let error = crate::primitives::CompiledProgram::compile(
            crate::ProgramOwnerId::ability(crate::AbilityDefinitionId {
                card_id: "unsupported_copy".into(),
                face_id: crate::CardFaceId::new("unsupported_copy").unwrap(),
                ability_path: vec![crate::AbilityId::new("triggered_01").unwrap()],
            }),
            crate::primitives::EffectContext::Ability,
            &unsupported,
        )
        .expect_err("unapproved token-copy sources remain outside program scope");
        assert!(error.contains("source is not supported"), "{error}");
    }
}

#[test]
fn land_type_additions_and_replacements_validate_distinct_subtype_rules() {
    let forest: super::TypeLineAddition = ron::from_str("(land_types: [Forest])").unwrap();
    assert!(
        !forest.is_empty(),
        "a land subtype is a real type-line addition"
    );
    assert!(forest.validate().is_ok());
    let duplicate: super::TypeLineAddition =
        ron::from_str("(land_types: [Forest, Forest])").unwrap();
    assert!(
        duplicate.validate().is_err(),
        "duplicate land subtypes are rejected"
    );
    let replacement: super::TypeLineReplacement =
        ron::from_str("(card_types: [Land], land_types: [Forest])").unwrap();
    assert!(replacement.validate().is_ok());
    let malformed: super::TypeLineReplacement =
        ron::from_str("(card_types: [Creature], land_types: [Forest])").unwrap();
    assert!(
        malformed.validate().is_err(),
        "a replacement land subtype requires Land"
    );
}

#[test]
fn storm_is_a_spell_keyword_and_static_emblems_require_typed_effects() {
    let face: crate::CardFace = ron::from_str(
        r#"(
            face_id: "storm_spell",
            name: "Storm Spell",
            mana_cost: "{1}{U}",
            types: ["Instant"],
            spell_keywords: [Storm],
        )"#,
    )
    .expect("spell keywords parse independently from permanent keywords");
    assert_eq!(face.spell_keywords, vec![super::SpellKeyword::Storm]);

    let cascade: super::SpellKeyword =
        ron::from_str("Cascade").expect("Cascade parses as a spell keyword");
    assert_eq!(format!("{cascade:?}"), "Cascade");

    let _wildsear: crate::IdentifiedStaticAbility = ron::from_str(
        r#"(
            ability_id: "wildsear_cascade",
            presentation: Fallback,
            definition: GrantSpellKeyword(
                filter: (card_type: Some(Enchantment), origin: Some(Hand)),
                keyword: Cascade,
            ),
        )"#,
    )
    .expect("a permanent static ability can grant a filtered spell keyword");

    let emblem: super::SpellEffectKind = ron::from_str(
        r#"CreateStaticEmblem(
            emblem_id: "ral_crackling_wit_emblem",
            display_name: "Ral, Crackling Wit Emblem",
            effects: [GrantSpellKeyword(
                filter: (card_type: Some(InstantOrSorcery)),
                keyword: Storm,
            )],
        )"#,
    )
    .expect("typed spell-keyword emblem parses");
    assert!(emblem.validate(super::EffectContext::Ability).is_ok());

    let empty: super::SpellEffectKind = ron::from_str(
        r#"CreateStaticEmblem(
            emblem_id: "empty",
            display_name: "Empty Emblem",
            effects: [],
        )"#,
    )
    .expect("shape parses before semantic validation");
    assert!(empty.validate(super::EffectContext::Ability).is_err());

    let zero_anthem: super::SpellEffectKind = ron::from_str(
        r#"CreateStaticEmblem(
            emblem_id: "zero",
            display_name: "Zero Emblem",
            effects: [CreaturePt(power: 0, toughness: 0)],
        )"#,
    )
    .expect("shape parses before semantic validation");
    assert!(zero_anthem.validate(super::EffectContext::Ability).is_err());
}

#[test]
fn presentation_only_choice_prose_is_rejected() {
    assert!(ron::from_str::<super::CastCostGroupDef>(
        r#"(prompt: "Pay kicker?", options: [Mana(label: "Kicker {2}", cost: "{2}")])"#,
    )
    .is_err());
    assert!(ron::from_str::<super::ResolutionBranchDef>(
        r#"(label: "Draw a card", cost: None, effects: [Draw(count: 1)])"#,
    )
    .is_err());
    assert!(ron::from_str::<super::SearchSelectionSlot>(
        r#"(label: "a basic land card", filter: (card_type: Some(BasicLand)))"#,
    )
    .is_err());
    assert!(ron::from_str::<super::ManaSpendingRestriction>(
        r#"(label: "Spend only to cast a creature spell.", cast_spell: [(card_type: Some(Creature))])"#,
    )
    .is_err());
    let rules_only = r#"(id: "tracking_in_rules_data", name: "Tracking in Rules Data", face_id: "tracking_in_rules_data", mana_cost: "{1}", types: ["Creature"], power: 1, toughness: 1)"#;
    assert!(crate::CardRegistry::from_chunks_and_tokens(&[rules_only], &[]).is_ok());
    let with_tracking = rules_only.replace(")", ", partial: \"not a rules field\")");
    assert!(crate::CardRegistry::from_chunks_and_tokens(&[&with_tracking], &[]).is_err());
}

#[test]
fn structured_choice_metadata_is_stable_nonmechanical_and_unique() {
    let group: super::CastCostGroupDef = ron::from_str(
        r#"(
            group_id: "cast_cost_01",
            presentation: Fallback,
            options: [Mana(
                option_id: "option_01",
                presentation: OracleLines([1]),
                kind: Kicker,
                cost: "{2}",
            )],
        )"#,
    )
    .expect("structured cast-cost presentation");
    assert!(group.validate().is_ok());
    assert_eq!(group.fallback_prompt(), "Choose cast cost (cast_cost_01)");
    assert_eq!(group.options[0].fallback_label(), "Kicker {2}");

    let mut duplicate_options = group.clone();
    duplicate_options.options.push(group.options[0].clone());
    assert!(duplicate_options.validate().is_err());

    let bounded_group: super::CastCostGroupDef = ron::from_str(
        r#"(
            group_id: "spree",
            presentation: Fallback,
            min: 1,
            max: 2,
            options: [
                Mana(option_id: "first", presentation: Fallback, kind: AdditionalPayment, cost: "{1}"),
                Mana(option_id: "second", presentation: Fallback, kind: AdditionalPayment, cost: "{2}"),
            ],
        )"#,
    )
    .expect("bounded cast-cost group parses");
    assert!(bounded_group.validate().is_ok());

    let branch: super::ResolutionBranchDef = ron::from_str(
        r#"(branch_id: "draw", presentation: Fallback, cost: None, effects: [Draw(count: 1)])"#,
    )
    .expect("structured branch presentation");
    let mut externally_mapped = branch.clone();
    externally_mapped.presentation = crate::AbilityPresentation::OracleLines(vec![1]);
    assert_eq!(branch.fallback_label(), "Draw a card.");
    assert_eq!(branch.fallback_label(), externally_mapped.fallback_label());

    let affected_draw_labels = [
        (0, "The affected player draws no cards."),
        (1, "The affected player draws a card."),
        (2, "The affected player draws 2 cards."),
    ];
    for (count, expected) in affected_draw_labels {
        let branch: super::ResolutionBranchDef = ron::from_str(&format!(
            r#"(branch_id: "draw_{count}", presentation: OracleLines([1]), cost: None,
                effects: [Draw(count: {count}, who: AffectedPlayer)])"#
        ))
        .expect("affected-player draw branch");
        assert_eq!(branch.fallback_label(), expected);
    }

    let duplicate_slots: super::SpellEffectKind = ron::from_str(
        r#"SearchLibrary(
            slots: [
                (slot_id: "slot_01", presentation: Fallback, filter: (card_type: Some(BasicLand))),
                (slot_id: "slot_01", presentation: Fallback, filter: (required_subtypes: ["Shrine"])),
            ],
            destination: Hand,
        )"#,
    )
    .expect("structured search slots");
    assert!(duplicate_slots
        .validate(super::EffectContext::Spell)
        .is_err());

    let duplicate_groups = r#"(
        id: "duplicate_groups", name: "Duplicate Groups", face_id: "duplicate_groups",
        mana_cost: "{1}", types: ["Sorcery"],
        cast_cost_groups: [
            (group_id: "cast_cost_01", presentation: Fallback, options: [Mana(option_id: "option_01", presentation: Fallback, kind: AdditionalPayment, cost: "{1}")]),
            (group_id: "cast_cost_01", presentation: Fallback, options: [Mana(option_id: "option_01", presentation: Fallback, kind: AdditionalPayment, cost: "{2}")]),
        ],
    )"#;
    assert!(crate::CardRegistry::from_chunks_and_tokens(&[duplicate_groups], &[]).is_err());
}

#[test]
fn issue_204_self_animation_schema_is_source_bound_and_fail_closed() {
    let ability_card = |effect: &str| {
        format!(
            r#"(
                id: "test_land", name: "Test Land", face_id: "test_land", types: ["Land"],
                activated_abilities: [(
                    ability_id: "activated_01", presentation: Fallback,
                    costs: [Mana("{{1}}")], effect: [{effect}],
                )],
            )"#
        )
    };
    let valid =
        ability_card("AnimateSelf(base_power: 2, base_toughness: 2, duration: UntilEndOfTurn)");
    crate::CardRegistry::from_chunks_and_tokens(&[&valid], &[])
        .expect("subtype-preserving animation is reusable for Crew");

    for (effect, expected_reason) in [
        (
            "AnimateSelf(base_power: 2, base_toughness: 2, duration: WhileSourceOnBattlefield)",
            "duration",
        ),
        (
            "AnimateSelf(base_power: 2, base_toughness: 2, colors: Some([Blue, Blue]), duration: Indefinite)",
            "colors must be distinct",
        ),
        (
            "AnimateSelf(base_power: 2, base_toughness: 2, creature_types: Replace([\"Shark\", \"Shark\"]), duration: Indefinite)",
            "creature types must be distinct",
        ),
        (
            "AnimateSelf(base_power: 2, base_toughness: 2, creature_types: Replace([\" \"]), duration: Indefinite)",
            "cannot be empty",
        ),
        (
            "AnimateSelf(base_power: 2, base_toughness: 2, keywords: [Vigilance, Vigilance], duration: Indefinite)",
            "keywords must be distinct",
        ),
    ] {
        let invalid = ability_card(effect);
        let error = crate::CardRegistry::from_chunks_and_tokens(&[&invalid], &[])
            .expect_err("invalid animation schema must fail closed");
        assert!(
            error.to_string().contains(expected_reason),
            "expected {expected_reason:?} in {error}"
        );
    }

    let spell = r#"(
        id: "test_spell", name: "Test Spell", face_id: "test_spell", types: ["Sorcery"],
        spell_effect: [AnimateSelf(base_power: 2, base_toughness: 2, duration: UntilEndOfTurn)],
    )"#;
    let error = crate::CardRegistry::from_chunks_and_tokens(&[spell], &[])
        .expect_err("AnimateSelf requires a source permanent");
    assert!(error.to_string().contains("source-bound"));
}

#[test]
fn issue_176_graveyard_cost_reduction_is_a_typed_target_filter() {
    let card = r#"(id: "test", name: "Test", face_id: "test", mana_cost: "{4}{B}", types: ["Sorcery"], cost_modifiers: [TargetMatchGenericReduction(amount: 3, filter: Graveyard((card: Some((card_type: Some(Creature), max_mana_value: Some(3))))))], spell_effect: [MoveGraveyardCards(filter: (card: Some((card_type: Some(Creature)))), destination: Battlefield())])"#;
    assert!(crate::CardRegistry::from_chunks_and_tokens(&[card], &[]).is_ok());
}
#[test]
fn issue_153_cast_cost_amount_roundtrips_and_checks_references() {
    let source =
        "CastCost(cast_cost: (group_id: \"cast_cost_01\", option_id: \"option_01\"), when_true: 4, otherwise: 2)";
    let amount: super::Amount = ron::from_str(source).unwrap();
    let encoded = ron::to_string(&amount).unwrap();
    assert_eq!(ron::from_str::<super::Amount>(&encoded).unwrap(), amount);
    let invalid = format!(
        r#"(id: "test", name: "Test", face_id: "test", types: ["Instant"], spell_effect: [DamageAll(amount: {source})])"#
    );
    assert!(crate::CardRegistry::from_chunks_and_tokens(&[&invalid], &[]).is_err());
    let ability = format!(
        r#"(id: "test", name: "Test", face_id: "test", types: ["Creature"], power: 1, toughness: 1, activated_abilities: [(ability_id: "activated_01", presentation: Fallback, costs: [Tap], effect: [Draw(count: {source})])])"#
    );
    assert!(
        crate::CardRegistry::from_chunks_and_tokens(&[&ability], &[]).is_err(),
        "cast cost receipts are not an ability context"
    );
    let zero = r#"(id: "test", name: "Test", face_id: "test", types: ["Creature"], power: 1, toughness: 1, activated_abilities: [(ability_id: "activated_01", presentation: Fallback, costs: [Blight(count: 0)], effect: [Draw(count: 1)])])"#;
    assert!(crate::CardRegistry::from_chunks_and_tokens(&[zero], &[]).is_err());
}

#[test]
fn issue_159_retires_legacy_counter_receipt_and_current_target_conditions_deserialize() {
    assert!(ron::from_str::<super::ResolutionBranchRequirement>(
        "PreviousResultReceipt(CounterUnlessPaid(paid: true))"
    )
    .is_err());

    let effect: super::SpellEffectKind = ron::from_str(
        r#"Conditional(
            condition: ObjectMatches(
                object: ChosenTarget(group_index: 0, target_index: 0),
                filter: (kind: Creature, required_subtypes: ["Ally"]),
            ),
            effect: GrantKeywords(
                subject: Chosen((kind: Creature, controller: You)),
                keywords: [Flying],
            ),
        )"#,
    )
    .expect("a later effect can inspect and reuse an earlier chosen target");
    assert!(matches!(effect, super::SpellEffectKind::Conditional { .. }));
}

#[test]
fn issue_211_counter_reflexive_receipts_require_an_adjacent_matching_producer() {
    let valid = r#"(
        id: "test", name: "Test", face_id: "test", types: ["Enchantment"],
        triggered_abilities: [(
            ability_id: "triggered_01", presentation: Fallback,
            trigger: WheneverPermanentEntersBattlefield(
                controller: Controller,
                filter: (permanent_type: Some(Land)),
            ),
            effect: [
                PutCounters(counter: Quest, count: 1, subject: Source),
                CreateReflexiveTrigger(
                    when: Some(CountersPlaced(counter: Quest, object: Source)),
                    ability: (
                        ability_id: "reflexive_01", presentation: Fallback,
                        intervening_if: Some(SourceCounterCount(counter: Quest, min: Some(4))),
                        effect: [GainLife(amount: 1)],
                    ),
                ),
            ],
        )],
    )"#;
    assert!(crate::CardRegistry::from_chunks_and_tokens(&[valid], &[]).is_ok());

    let missing_producer = valid.replace(
        "PutCounters(counter: Quest, count: 1, subject: Source)",
        "Draw(count: 1)",
    );
    let error = crate::CardRegistry::from_chunks_and_tokens(&[&missing_producer], &[])
        .expect_err("a receipt cannot inspect a nonadjacent or absent counter result");
    assert!(error
        .to_string()
        .contains("immediately preceding PutCounters effect with the same counter kind"));

    let mismatched_counter = valid.replace(
        "PutCounters(counter: Quest, count: 1, subject: Source)",
        "PutCounters(counter: Charge, count: 1, subject: Source)",
    );
    assert!(crate::CardRegistry::from_chunks_and_tokens(&[&mismatched_counter], &[]).is_err());
}

#[test]
fn issue_159_exact_cohorts_grouped_permissions_and_second_from_top_deserialize() {
    let retired_graveyard_syntax = ron::from_str::<super::SpellEffectKind>(
        "ChooseGraveyardCard(filter: (card_type: Some(NonlandPermanent)), destination: Hand, optional: true, from_result: Some((source: PreviousEffect, action: Mill, players: Controller)))",
    );
    assert!(
        retired_graveyard_syntax.is_err(),
        "the legacy ChooseGraveyardCard/from_result syntax must be rejected"
    );

    let exile: super::SpellEffectKind = ron::from_str(
        "ExileTopWithPlayPermission(player: Controller, count: 2, count_by_cast_cost: Some((condition: (group_id: \"cast_cost_01\", option_id: \"option_01\", expected_selected: true), if_selected: 3, otherwise: 2)))",
    )
    .expect("one permission instruction can carry a receipt-conditioned result cohort");
    assert!(matches!(
        exile,
        super::SpellEffectKind::ExileTopWithPlayPermission { count: 2, .. }
    ));

    let placement: super::LibraryPlacement = ron::from_str("OwnerChoiceSecondFromTopOrBottom")
        .expect("Lost Days uses the owner's second-from-top or bottom choice");
    assert_eq!(
        placement,
        super::LibraryPlacement::OwnerChoiceSecondFromTopOrBottom
    );
}

#[test]
fn issue_159_heterogeneous_library_search_slots_deserialize() {
    let effect: super::SpellEffectKind = ron::from_str(
        r#"SearchLibrary(
            slots: [
                (slot_id: "slot_01", presentation: Fallback, filter: (card_type: Some(BasicLand))),
                (slot_id: "slot_02", presentation: Fallback, filter: (required_subtypes: ["Shrine"]), enabled_by_cast_cost: Some((group_id: "cast_cost_01", option_id: "option_01", expected_selected: true))),
            ],
            destination: Hand,
            reveal: true,
        )"#,
    )
    .expect("Aang's Journey authors distinct engine-owned search slots");
    assert!(matches!(
        effect,
        super::SpellEffectKind::SearchLibrary { slots, .. } if slots.len() == 2
    ));
}
#[test]
fn issue_166_cast_quantity_and_filter_validate() {
    let amount: super::Amount = ron::from_str(
        "Count(SpellsCastThisTurn(players: AffectedPlayer, filter: (any_of: Some([(card_type: Some(Noncreature)), (required_subtypes: [\"Otter\"])]))))"
    ).expect("cast history is a shared quantity");
    let effects = [super::SpellEffectKind::DamagePlayer {
        amount,
        who: super::PlayerRecipient::AffectedPlayer,
    }];
    assert!(super::SpellEffectKind::validate_list(&effects).is_ok());
}

#[test]
fn issue_166_invalid_cast_filters_are_rejected() {
    for filter in [
        "(min_mana_value: Some(4), max_mana_value: Some(2))",
        "(required_subtypes: [\"\"])",
        "(any_of: Some([]))",
        "(any_of: Some([()]))",
    ] {
        let trigger: super::TriggerCondition =
            ron::from_str(&format!("WheneverPlayerCastsSpell(filter: {filter})")).unwrap();
        assert!(trigger.validate().is_err(), "{filter}");
    }
}

#[test]
fn issue_191_cast_filter_accepts_targeted_permanent_type() {
    let trigger: super::TriggerCondition = ron::from_str(
        "WheneverPlayerCastsSpell(filter: (card_type: Some(InstantOrSorcery), targeted_permanent_type: Some(Creature)))",
    )
    .expect("Repartee needs a typed existential target predicate");
    assert!(trigger.validate().is_ok());
}

#[test]
fn issue_236_cast_filter_accepts_derived_color() {
    let trigger: super::TriggerCondition =
        ron::from_str("WheneverPlayerCastsSpell(filter: (is_color: Some(Blue)))")
            .expect("Hydro-Man needs a typed cast-color predicate");
    assert!(trigger.validate().is_ok());
}

#[test]
fn issue_236_composable_permanent_modifiers_parse_and_validate() {
    let card = r#"(
        id: "test_hydro",
        name: "Test Hydro",
        face_id: "test_hydro",
        mana_cost: "{U}{U}",
        types: ["Legendary", "Creature", "Elemental", "Villain"],
        power: 2,
        toughness: 2,
        triggered_abilities: [(
            ability_id: "end_step_land",
            presentation: Fallback,
            trigger: AtBeginningOfEndStep(player: Controller),
            effect: [
                ApplyPermanentModifier(
                    subject: Source,
                    modifier: SetTypeLine((card_types: [Land])),
                    duration: UntilControllerNextTurn,
                ),
                ApplyPermanentModifier(
                    subject: Source,
                    modifier: GrantActivatedAbility((
                        ability_id: "land_mana",
                        presentation: Fallback,
                        costs: [Tap],
                        effect: [ProduceMana(options: [(u: 1)])],
                    )),
                    duration: UntilControllerNextTurn,
                ),
            ],
        )],
    )"#;
    let wrenn = r#"(
        id: "test_wrenn",
        name: "Test Wrenn",
        face_id: "test_wrenn",
        mana_cost: "{1}{G}{G}",
        types: ["Planeswalker", "Wrenn"],
        loyalty: 4,
        activated_abilities: [(
            ability_id: "animate_land",
            presentation: Fallback,
            costs: [Loyalty(1)],
            effect: [
                ApplyPermanentModifier(
                    subject: Chosen((kind: AnyPermanent, permanent_types: [Land])),
                    modifier: AddTypes((card_types: [Creature], creature_types: ["Treefolk"])),
                    duration: UntilControllerNextTurn,
                ),
                ApplyPermanentModifier(
                    subject: Chosen((kind: AnyPermanent, permanent_types: [Land])),
                    modifier: SetBasePowerToughness(power: 3, toughness: 3),
                    duration: UntilControllerNextTurn,
                ),
                ApplyPermanentModifier(
                    subject: Chosen((kind: AnyPermanent, permanent_types: [Land])),
                    modifier: GrantKeywords([Vigilance, Hexproof, Haste]),
                    duration: UntilControllerNextTurn,
                ),
            ],
            targeting: Some((groups: [(
                min: 1,
                max: 1,
                prompt: "Choose target land you control",
                effect_indices: [0, 1, 2],
            )])),
        )],
    )"#;
    crate::CardRegistry::from_chunks_and_tokens(&[card, wrenn], &[])
        .expect("Hydro-Man's modifiers should use the reusable resolving vocabulary");
}

#[test]
fn issue_236_permanent_modifier_validation_fails_closed() {
    let invalid_effects = [
        "ApplyPermanentModifier(subject: AttachedObject, modifier: SetTypeLine((card_types: [Land])), duration: UntilControllerNextTurn)",
        "ApplyPermanentModifier(subject: Source, modifier: GrantKeywords([]), duration: UntilControllerNextTurn)",
        "ApplyPermanentModifier(subject: Source, modifier: GrantActivatedAbility((ability_id: \"bad\", presentation: Fallback, source_zone: Hand, costs: [DiscardSelf], effect: [ProduceMana(options: [(u: 1)])])), duration: UntilControllerNextTurn)",
    ];
    for effect in invalid_effects {
        let card = format!(
            r#"(id: "invalid", name: "Invalid", face_id: "invalid", types: ["Creature"], power: 1, toughness: 1,
            triggered_abilities: [(ability_id: "test", presentation: Fallback, trigger: WhenSelfEntersBattlefield, effect: [{effect}])])"#,
        );
        assert!(
            crate::CardRegistry::from_chunks_and_tokens(&[&card], &[]).is_err(),
            "must reject {effect}"
        );
    }

    let source_in_spell = r#"(
        id: "invalid_spell", name: "Invalid Spell", face_id: "invalid_spell", types: ["Sorcery"],
        spell_effect: [ApplyPermanentModifier(
            subject: Source,
            modifier: SetTypeLine((card_types: [Land])),
            duration: UntilControllerNextTurn,
        )],
    )"#;
    assert!(crate::CardRegistry::from_chunks_and_tokens(&[source_in_spell], &[]).is_err());

    let source_linked_spell = r#"(
        id: "invalid_lifetime", name: "Invalid Lifetime", face_id: "invalid_lifetime", types: ["Sorcery"],
        spell_effect: [ApplyPermanentModifier(
            subject: Chosen((kind: AnyPermanent)),
            modifier: SetTypeLine((card_types: [Land])),
            duration: WhileSourceOnBattlefield,
        )],
        targeting: Some((groups: [(min: 1, max: 1, prompt: "Choose target permanent", effect_indices: [0])])),
    )"#;
    assert!(crate::CardRegistry::from_chunks_and_tokens(&[source_linked_spell], &[]).is_err());
}

#[test]
fn issue_234_linked_exile_schema_is_paired_and_fail_closed() {
    let valid = r#"(
        id: "test_vacuum",
        name: "Test Vacuum",
        face_id: "test_vacuum",
        mana_cost: "{1}",
        types: ["Artifact"],
        activated_abilities: [(
            ability_id: "exile_card",
            presentation: Fallback,
            costs: [Tap],
            effect: [MoveGraveyardCards(
                filter: (owner: AnyPlayer),
                destination: Exile,
                linked_exile_id: Some("exiled_cards"),
            )],
        ), (
            ability_id: "return_creatures",
            presentation: Fallback,
            costs: [Mana("{6}"), Tap, SacrificeSelf],
            timing: SorcerySpeed,
            effect: [ReturnLinkedExiledCards(
                producer_ability_id: "exile_card",
                linked_exile_id: "exiled_cards",
                filter: (card_type: Some(Creature)),
                entry_counters: [(counter: Keyword(Flying), count: 1)],
                entry_modifiers: [
                    AddTypes((creature_types: ["Spirit"])),
                    SetBasePowerToughness(power: 1, toughness: 1),
                ],
            )],
        )],
    )"#;
    crate::CardRegistry::from_chunks_and_tokens(&[valid], &[])
        .expect("Ghost Vacuum needs a paired linked-exile vocabulary");

    for (needle, invalid_effects) in [
        (
            "requires an activated or triggered ability",
            r#"spell_effect: [MoveGraveyardCards(filter: (owner: AnyPlayer), destination: Exile, linked_exile_id: Some("exiled_cards"))],
            activated_abilities: [(ability_id: "return", presentation: Fallback, costs: [], effect: [ReturnLinkedExiledCards(producer_ability_id: "return", linked_exile_id: "exiled_cards", filter: (card_type: Some(Creature)))])]"#,
        ),
        (
            "requires the exile destination",
            r#"activated_abilities: [
                (ability_id: "bad", presentation: Fallback, costs: [], effect: [MoveGraveyardCards(filter: (owner: AnyPlayer), destination: Hand, linked_exile_id: Some("exiled_cards"))]),
                (ability_id: "return", presentation: Fallback, costs: [], effect: [ReturnLinkedExiledCards(producer_ability_id: "bad", linked_exile_id: "exiled_cards", filter: (card_type: Some(Creature)))])
            ]"#,
        ),
        (
            "linked exile return requires an activated or triggered ability",
            r#"spell_effect: [ReturnLinkedExiledCards(producer_ability_id: "producer", linked_exile_id: "exiled_cards", filter: (card_type: Some(Creature)))],
            activated_abilities: [(ability_id: "producer", presentation: Fallback, costs: [], effect: [MoveGraveyardCards(filter: (owner: AnyPlayer), destination: Exile, linked_exile_id: Some("exiled_cards"))])]"#,
        ),
        (
            "must name the linked-exile producer with matching ability and link ids",
            r#"activated_abilities: [
                (ability_id: "exile_card", presentation: Fallback, costs: [Tap], effect: [MoveGraveyardCards(filter: (owner: AnyPlayer), destination: Exile, linked_exile_id: Some("exiled_cards"))]),
                (ability_id: "return", presentation: Fallback, costs: [], effect: [ReturnLinkedExiledCards(producer_ability_id: "other_exile", linked_exile_id: "exiled_cards", filter: (card_type: Some(Creature)))])
            ]"#,
        ),
        (
            "ability link id 'Bad' must be canonical snake_case beginning with a letter",
            r#"activated_abilities: [
                (ability_id: "first", presentation: Fallback, costs: [], effect: [MoveGraveyardCards(filter: (owner: AnyPlayer), destination: Exile, linked_exile_id: Some("Bad"))]),
                (ability_id: "second", presentation: Fallback, costs: [], effect: [ReturnLinkedExiledCards(producer_ability_id: "first", linked_exile_id: "Bad", filter: (card_type: Some(Creature)))])
            ]"#,
        ),
        (
            "requires exactly one producer and one consumer",
            r#"activated_abilities: [(ability_id: "bad", presentation: Fallback, costs: [], effect: [MoveGraveyardCards(filter: (owner: AnyPlayer), destination: Exile, linked_exile_id: Some("exiled_cards"))])]"#,
        ),
        (
            "requires exactly one producer and one consumer",
            r#"activated_abilities: [
                (ability_id: "first", presentation: Fallback, costs: [], effect: [MoveGraveyardCards(filter: (owner: AnyPlayer), destination: Exile, linked_exile_id: Some("exiled_cards"))]),
                (ability_id: "duplicate", presentation: Fallback, costs: [], effect: [MoveGraveyardCards(filter: (owner: AnyPlayer), destination: Exile, linked_exile_id: Some("exiled_cards"))]),
                (ability_id: "return", presentation: Fallback, costs: [], effect: [ReturnLinkedExiledCards(producer_ability_id: "first", linked_exile_id: "exiled_cards", filter: (card_type: Some(Creature)))])
            ]"#,
        ),
        (
            "zone card filter requires at least one predicate",
            r#"activated_abilities: [
                (ability_id: "first", presentation: Fallback, costs: [], effect: [MoveGraveyardCards(filter: (owner: AnyPlayer), destination: Exile, linked_exile_id: Some("exiled_cards"))]),
                (ability_id: "second", presentation: Fallback, costs: [], effect: [ReturnLinkedExiledCards(producer_ability_id: "first", linked_exile_id: "exiled_cards", filter: ())]),
            ]"#,
        ),
        (
            "entry counter placement count must be positive",
            r#"activated_abilities: [
                (ability_id: "first", presentation: Fallback, costs: [], effect: [MoveGraveyardCards(filter: (owner: AnyPlayer), destination: Exile, linked_exile_id: Some("exiled_cards"))]),
                (ability_id: "second", presentation: Fallback, costs: [], effect: [ReturnLinkedExiledCards(producer_ability_id: "first", linked_exile_id: "exiled_cards", filter: (card_type: Some(Creature)), entry_counters: [(counter: Keyword(Flying), count: 0)])]),
            ]"#,
        ),
        (
            "entry counter placements cannot repeat a counter kind",
            r#"activated_abilities: [
                (ability_id: "first", presentation: Fallback, costs: [], effect: [MoveGraveyardCards(filter: (owner: AnyPlayer), destination: Exile, linked_exile_id: Some("exiled_cards"))]),
                (ability_id: "second", presentation: Fallback, costs: [], effect: [ReturnLinkedExiledCards(producer_ability_id: "first", linked_exile_id: "exiled_cards", filter: (card_type: Some(Creature)), entry_counters: [(counter: Keyword(Flying), count: 1), (counter: Keyword(Flying), count: 1)])]),
            ]"#,
        ),
        (
            "type-line addition must add a card type or subtype",
            r#"activated_abilities: [
                (ability_id: "first", presentation: Fallback, costs: [], effect: [MoveGraveyardCards(filter: (owner: AnyPlayer), destination: Exile, linked_exile_id: Some("exiled_cards"))]),
                (ability_id: "second", presentation: Fallback, costs: [], effect: [ReturnLinkedExiledCards(producer_ability_id: "first", linked_exile_id: "exiled_cards", filter: (card_type: Some(Creature)), entry_modifiers: [AddTypes(())])]),
            ]"#,
        ),
    ] {
        let card = format!(
            r#"(id: "invalid_link", name: "Invalid Link", face_id: "invalid_link", mana_cost: "{{1}}", types: ["Artifact"], {invalid_effects})"#
        );
        let error = crate::CardRegistry::from_chunks_and_tokens(&[&card], &[])
            .expect_err("invalid linked-exile schema must fail registry load");
        assert!(
            error.to_string().contains(needle),
            "expected {needle:?}, got {error}"
        );
    }
}

#[test]
fn linked_exile_token_copy_source_names_its_paired_ability_and_link() {
    let fixture = r#"(
        id: "test_linked_copy",
        name: "Test Linked Copy",
        face_id: "test_linked_copy",
        types: ["Artifact"],
        activated_abilities: [(
            ability_id: "exile_card",
            presentation: Fallback,
            costs: [Tap],
            effect: [MoveGraveyardCards(
                filter: (owner: AnyPlayer),
                destination: Exile,
                linked_exile_id: Some("exiled_card"),
            )],
        ), (
            ability_id: "make_copy",
            presentation: Fallback,
            costs: [],
            effect: [CreateTokenCopies(
                count: 1,
                source: LinkedExile(
                    producer_ability_id: "exile_card",
                    linked_exile_id: "exiled_card",
                ),
            )],
        ), (
            ability_id: "return_card",
            presentation: Fallback,
            costs: [SacrificeSelf],
            effect: [ReturnLinkedExiledCards(
                producer_ability_id: "exile_card",
                linked_exile_id: "exiled_card",
                filter: (card_type: Some(Creature)),
            )],
        )],
    )"#;
    crate::CardRegistry::from_chunks_and_tokens(&[fixture], &[])
        .expect("copy and return abilities share the exact linked exile producer");
}

#[test]
fn token_copy_cohort_chain_uses_plural_previous_objects_and_one_exile_trigger() {
    let fixture = r#"(
        id: "test_copy_cohort",
        name: "Test Copy Cohort",
        face_id: "test_copy_cohort",
        types: ["Enchantment"],
        triggered_abilities: [(
            ability_id: "copy_creature",
            presentation: Fallback,
            trigger: WheneverPermanentEntersBattlefield(
                controller: Controller,
                filter: (permanent_type: Some(Creature), token: Some(false)),
            ),
            effect: [
                CreateTokenCopies(count: 1, source: TriggerObject),
                ApplyPermanentModifier(
                    subject: PreviousEffectObjects,
                    modifier: GrantKeywords([Haste]),
                    duration: Indefinite,
                ),
                CreateDelayedTrigger(
                    subject: Some(PreviousEffectObjects),
                    ability: (
                        ability_id: "exile_copies",
                        presentation: Fallback,
                        trigger: AtBeginningOfNextEndStep,
                        effect: [ExileObservedObjects],
                    ),
                ),
            ],
        )],
    )"#;
    crate::CardRegistry::from_chunks_and_tokens(&[fixture], &[])
        .expect("one typed token-copy cohort can feed Haste and a delayed Exile effect");
}

#[test]
fn token_copy_cohort_exile_rejects_unscoped_and_wrong_consumers() {
    let direct_consumer = r#"(
        id: "test_unscoped_cohort_exile",
        name: "Test Unscoped Cohort Exile",
        face_id: "test_unscoped_cohort_exile",
        types: ["Enchantment"],
        triggered_abilities: [(
            ability_id: "exile_cohort",
            presentation: Fallback,
            trigger: WheneverPermanentEntersBattlefield(
                controller: Controller,
                filter: (permanent_type: Some(Creature), token: Some(false)),
            ),
            effect: [ExileObservedObjects],
        )],
    )"#;
    assert!(crate::CardRegistry::from_chunks_and_tokens(&[direct_consumer], &[]).is_err());

    let wrong_delayed_consumer = r#"(
        id: "test_wrong_cohort_consumer",
        name: "Test Wrong Cohort Consumer",
        face_id: "test_wrong_cohort_consumer",
        types: ["Enchantment"],
        triggered_abilities: [(
            ability_id: "copy_creature",
            presentation: Fallback,
            trigger: WheneverPermanentEntersBattlefield(
                controller: Controller,
                filter: (permanent_type: Some(Creature), token: Some(false)),
            ),
            effect: [
                CreateTokenCopies(count: 1, source: TriggerObject),
                ApplyPermanentModifier(
                    subject: PreviousEffectObjects,
                    modifier: GrantKeywords([Haste]),
                    duration: Indefinite,
                ),
                CreateDelayedTrigger(
                    subject: Some(PreviousEffectObjects),
                    ability: (
                        ability_id: "wrong_exile",
                        presentation: Fallback,
                        trigger: AtBeginningOfNextEndStep,
                        effect: [Exile(subject: TriggerObject)],
                    ),
                ),
            ],
        )],
    )"#;
    assert!(crate::CardRegistry::from_chunks_and_tokens(&[wrong_delayed_consumer], &[]).is_err());

    let wrong_modifier = r#"(
        id: "test_wrong_cohort_modifier",
        name: "Test Wrong Cohort Modifier",
        face_id: "test_wrong_cohort_modifier",
        types: ["Enchantment"],
        triggered_abilities: [(
            ability_id: "copy_creature",
            presentation: Fallback,
            trigger: WheneverPermanentEntersBattlefield(
                controller: Controller,
                filter: (permanent_type: Some(Creature), token: Some(false)),
            ),
            effect: [
                CreateTokenCopies(count: 1, source: TriggerObject),
                ApplyPermanentModifier(
                    subject: PreviousEffectObjects,
                    modifier: GrantKeywords([Flying]),
                    duration: Indefinite,
                ),
                CreateDelayedTrigger(
                    subject: Some(PreviousEffectObjects),
                    ability: (
                        ability_id: "exile_copies",
                        presentation: Fallback,
                        trigger: AtBeginningOfNextEndStep,
                        effect: [ExileObservedObjects],
                    ),
                ),
            ],
        )],
    )"#;
    assert!(crate::CardRegistry::from_chunks_and_tokens(&[wrong_modifier], &[]).is_err());
}

#[test]
fn linked_exile_choice_requires_its_exact_trigger_object_or_graveyard_exile_producer() {
    let choose = r#"ChooseLinkedExiledCard(
        producer_ability_id: "exile_cards",
        linked_exile_id: "linked_cards",
        filter: (any_of: Some([
            (card_type: Some(Creature)),
            (card_type: Some(Noncreature)),
        ])),
        destination: OwnerGraveyard,
        optional: false,
    )"#;
    let valid_trigger_object = format!(
        r#"(
            id: "linked_choice_trigger_object", name: "Linked Choice Fixture", face_id: "linked_choice_fixture",
            types: ["Artifact"],
            triggered_abilities: [(
                ability_id: "exile_cards", presentation: Fallback,
                trigger: WheneverPlayerDiscardsCard(player: Controller),
                effect: [Exile(subject: TriggerObject, linked_exile_id: Some("linked_cards"))],
            )],
            activated_abilities: [(
                ability_id: "choose_card", presentation: Fallback, costs: [Tap],
                effect: [{choose}],
            )],
        )"#
    );
    crate::CardRegistry::from_chunks_and_tokens(&[&valid_trigger_object], &[])
        .expect("a discard-bound linked Exile producer can feed one linked-card chooser");

    let valid_graveyard_cohort = format!(
        r#"(
            id: "linked_choice_graveyard", name: "Linked Choice Fixture", face_id: "linked_choice_fixture",
            types: ["Artifact"],
            triggered_abilities: [(
                ability_id: "exile_cards", presentation: Fallback,
                trigger: WhenSelfEntersBattlefield,
                effect: [ExileGraveyards(players: All, linked_exile_id: Some("linked_cards"))],
            )],
            activated_abilities: [(
                ability_id: "choose_card", presentation: Fallback, costs: [Tap],
                effect: [{choose}],
            )],
        )"#
    );
    crate::CardRegistry::from_chunks_and_tokens(&[&valid_graveyard_cohort], &[])
        .expect("linked graveyard exile can feed the same linked-card chooser");

    let mismatched_producer = valid_trigger_object.replace(
        "producer_ability_id: \"exile_cards\"",
        "producer_ability_id: \"other_ability\"",
    );
    let error = crate::CardRegistry::from_chunks_and_tokens(&[&mismatched_producer], &[])
        .expect_err("the chooser must name the exact linked-exile producer ability");
    assert!(
        error.to_string().contains(
            "linked Exile choice must name its matching TriggerObject or graveyard-exile producer"
        ),
        "unexpected linked-choice validation error: {error}"
    );
}

#[test]
fn mandatory_program_owner_graveyard_linked_choice_rejects_hideaway_look_permission() {
    let hideaway_owner_graveyard_return = r#"(
        id: "hideaway_linked_program_return", name: "Hideaway Linked Program Return",
        face_id: "hideaway_linked_program_return", types: ["Artifact"],
        triggered_abilities: [(
            ability_id: "hideaway", presentation: Fallback,
            trigger: WhenSelfEntersBattlefield,
            effect: [Hideaway(count: 1, linked_exile_id: "exiled_cards")],
        )],
        activated_abilities: [(
            ability_id: "return_linked_card", presentation: Fallback, costs: [Tap],
            effect: [], program: Some((body: (steps: [
                (id: "choose_linked_card", bind: ["chosen"], input: None,
                    kind: Perform(ChooseLinkedExiledCard(
                        producer_ability_id: "hideaway",
                        linked_exile_id: "exiled_cards",
                        filter: (card_type: Some(Creature)),
                        destination: OwnerGraveyard,
                        optional: false,
                    ))),
            ], yields: []), target_bindings: [])),
        )],
    )"#;

    let error = crate::CardRegistry::from_chunks_and_tokens(
        &[hideaway_owner_graveyard_return],
        &[],
    )
    .expect_err(
        "a delegated Program chooser must not inherit a prior Hideaway viewer's look permission",
    );
    assert!(
        error.to_string().contains(
            "mandatory Program OwnerGraveyard linked Exile choices cannot use Hideaway producers"
        ),
        "unexpected hidden linked-choice admission error: {error}"
    );

    let legacy_hideaway_play = r#"(
        id: "legacy_hideaway_play", name: "Legacy Hideaway Play", face_id: "legacy_hideaway_play",
        types: ["Artifact"],
        triggered_abilities: [(
            ability_id: "hideaway", presentation: Fallback,
            trigger: WhenSelfEntersBattlefield,
            effect: [Hideaway(count: 1, linked_exile_id: "exiled_cards")],
        )],
        activated_abilities: [(
            ability_id: "play_linked_card", presentation: Fallback, costs: [Tap],
            effect: [PlayLinkedExiledCard(
                producer_ability_id: "hideaway",
                linked_exile_id: "exiled_cards",
                filter: None,
                play_kind: Play,
                return_source_to_owner_hand_if_cast: false,
            )],
        )],
    )"#;
    crate::CardRegistry::from_chunks_and_tokens(&[legacy_hideaway_play], &[])
        .expect("legacy Hideaway play permissions remain supported");
}

#[test]
fn currency_converter_linked_choice_program_binds_a_typed_land_and_nonland_result() {
    let currency = r#"(
        id: "currency_converter", name: "Currency Converter",
        face_id: "currency_converter", mana_cost: "{1}", types: ["Artifact"],
        triggered_abilities: [(
            ability_id: "discard_to_exile", presentation: OracleLines([1]),
            trigger: WheneverPlayerDiscardsCard(player: Controller),
            effect: [Exile(subject: TriggerObject, linked_exile_id: Some("exiled_cards"))],
        )],
        activated_abilities: [(
            ability_id: "return_linked_card", presentation: OracleLines([3]),
            source_zone: Battlefield, costs: [Tap], effect: [],
            program: Some((body: (steps: [
                (id: "choose_linked_card", bind: ["chosen"], input: None,
                    kind: Perform(ChooseLinkedExiledCard(
                        producer_ability_id: "discard_to_exile",
                        linked_exile_id: "exiled_cards",
                        filter: (any_of: Some([
                            (card_type: Some(Creature)),
                            (card_type: Some(Noncreature)),
                        ])),
                        destination: OwnerGraveyard,
                        optional: false,
                    ))),
                (id: "create_treasure_for_land", bind: [], input: None,
                    kind: IfResult(
                        condition: CardResultCount(result: "chosen", card_type: Some(Land),
                            min: Some(1), max: None),
                        yes: (steps: [(id: "create_treasure", bind: [], input: None,
                            kind: Perform(CreateTokens(token: "treasure", count: 1,
                                who: Controller, tapped: false, sacrifice_timing: None)))],
                            yields: []),
                        no: (steps: [], yields: []),
                    )),
                (id: "create_rogue_for_nonland", bind: [], input: None,
                    kind: IfResult(
                        condition: CardResultCount(result: "chosen", card_type: Some(Nonland),
                            min: Some(1), max: None),
                        yes: (steps: [(id: "create_rogue", bind: [], input: None,
                            kind: Perform(CreateTokens(token: "rogue_b_2_2", count: 1,
                                who: Controller, tapped: false, sacrifice_timing: None)))],
                            yields: []),
                        no: (steps: [], yields: []),
                    )),
            ], yields: []), target_bindings: [])),
        )],
    )"#;
    let registry = crate::CardRegistry::from_chunks_and_tokens(
        &[currency],
        &[
            include_str!("../../../tricerules-cards/data/tokens/treasure.ron"),
            include_str!("../../../tricerules-cards/data/tokens/rogue_b_2_2.ron"),
        ],
    )
    .unwrap_or_else(|error| {
        panic!("Currency Converter's linked-choice Program must compile: {error}")
    });
    let owner = crate::ProgramOwnerId::ability(crate::AbilityDefinitionId {
        card_id: "currency_converter".into(),
        face_id: crate::CardFaceId::new("currency_converter").unwrap(),
        ability_path: vec![crate::AbilityId::new("return_linked_card").unwrap()],
    });
    let program = registry
        .compiled_program_for_owner(&owner)
        .expect("the actual return-linked-card ability has a compiled Program owner");
    assert_eq!(program.owner, owner);
    assert_eq!(program.context, crate::primitives::EffectContext::Ability);
    let choose = &program.body.steps[0];
    assert_eq!(choose.id, "choose_linked_card");
    assert_eq!(choose.outputs.len(), 1);
    let result_type = &program.slot(choose.outputs[0]).unwrap().result_type;
    let crate::primitives::ResultType::LinkedExileCardResultCohort {
        producer_ability_id,
        linked_exile_id,
        cardinality,
    } = result_type
    else {
        panic!("linked choice retains its exact origin and owner-local cohort facts");
    };
    assert_eq!(
        producer_ability_id,
        &crate::AbilityId::new("discard_to_exile").unwrap()
    );
    assert_eq!(
        linked_exile_id,
        &crate::AbilityLinkId::new("exiled_cards").unwrap()
    );
    assert_eq!(
        *cardinality,
        crate::primitives::Cardinality {
            min: 0,
            max: Some(1),
        }
    );
    assert_eq!(program.body.steps.len(), 3);
}

#[test]
fn linked_choice_program_link_ids_and_producer_abilities_are_validated() {
    let definition = |named_producer: &str, linked_id: &str| {
        format!(
            r#"(
                id: "linked_choice_program_link_fixture", name: "Linked Choice Program Link Fixture",
                face_id: "linked_choice_program_link_fixture", types: ["Artifact"],
                triggered_abilities: [(
                    ability_id: "exile_cards", presentation: Fallback,
                    trigger: WheneverPlayerDiscardsCard(player: Controller),
                    effect: [Exile(subject: TriggerObject, linked_exile_id: Some("linked_cards"))],
                )],
                activated_abilities: [(
                    ability_id: "choose_card", presentation: Fallback, costs: [Tap], effect: [],
                    program: Some((body: (steps: [
                        (id: "choose_linked_card", bind: ["chosen"], kind: Perform(
                            ChooseLinkedExiledCard(
                                producer_ability_id: "{named_producer}",
                                linked_exile_id: "{linked_id}",
                                filter: (card_type: Some(Creature)),
                                destination: OwnerGraveyard,
                                optional: false,
                            )
                        )),
                    ], yields: []), target_bindings: [])),
                )],
            )"#
        )
    };

    let wrong_producer = definition("another_ability", "linked_cards");
    let error = crate::CardRegistry::from_chunks_and_tokens(&[&wrong_producer], &[])
        .expect_err("Program chooser must name the unique exact producer ability");
    assert!(
        error.to_string().contains(
            "linked Exile choice must name its matching TriggerObject or graveyard-exile producer"
        ),
        "unexpected producer error: {error}"
    );

    let wrong_link = definition("exile_cards", "another_link");
    let error = crate::CardRegistry::from_chunks_and_tokens(&[&wrong_link], &[])
        .expect_err("Program consumer must share the producer's link ID");
    assert!(
        error
            .to_string()
            .contains("requires exactly one producer and one consumer"),
        "unexpected linked-pair error: {error}"
    );
}

#[test]
fn currency_linked_choice_leaf_is_admitted_as_a_typed_program_result() {
    let body = r#"(steps: [
        (id: "choose_linked_card", bind: ["chosen"], kind: Perform(
            ChooseLinkedExiledCard(
                producer_ability_id: "discard_to_exile",
                linked_exile_id: "exiled_cards",
                filter: (any_of: Some([
                    (card_type: Some(Creature)),
                    (card_type: Some(Noncreature)),
                ])),
                destination: OwnerGraveyard,
                optional: false,
            )
        )),
    ], yields: [])"#;
    let authored: crate::CardProgram =
        ron::from_str(&format!("(body: {body})")).expect("linked choice body parses");
    let owner = crate::ProgramOwnerId::ability(crate::AbilityDefinitionId {
        card_id: "currency_converter".into(),
        face_id: crate::CardFaceId::new("currency_converter").unwrap(),
        ability_path: vec![crate::AbilityId::new("return_linked_card").unwrap()],
    });
    let compiled = crate::primitives::CompiledProgram::compile(
        owner,
        crate::primitives::EffectContext::Ability,
        &authored,
    );
    assert!(
        compiled.is_ok(),
        "the exact mandatory OwnerGraveyard linked-card move needs a typed Program result: {compiled:?}"
    );

    for (invalid, reason) in [
        (
            body.replace("optional: false", "optional: true"),
            "optional linked-card choices are outside the admitted result profile",
        ),
        (
            body.replace(
                "destination: OwnerGraveyard",
                "destination: ControllerBattlefield",
            ),
            "controller battlefield moves have a different result profile",
        ),
    ] {
        let authored: crate::CardProgram =
            ron::from_str(&format!("(body: {invalid})")).expect("variant body parses");
        let error = crate::primitives::CompiledProgram::compile(
            crate::ProgramOwnerId::ability(crate::AbilityDefinitionId {
                card_id: "currency_converter".into(),
                face_id: crate::CardFaceId::new("currency_converter").unwrap(),
                ability_path: vec![crate::AbilityId::new("return_linked_card").unwrap()],
            }),
            crate::primitives::EffectContext::Ability,
            &authored,
        )
        .expect_err(reason);
        assert!(
            error.contains("mandatory ability-owned OwnerGraveyard"),
            "{error}"
        );
    }

    let spell: crate::CardProgram =
        ron::from_str(&format!("(body: {body})")).expect("spell body parses");
    let error = crate::primitives::CompiledProgram::compile(
        crate::ProgramOwnerId::face(
            "linked_choice_spell_fixture",
            crate::CardFaceId::new("linked_choice_spell_fixture").unwrap(),
        ),
        crate::primitives::EffectContext::Spell,
        &spell,
    )
    .expect_err("linked card choice result is ability-owned");
    assert!(
        error.contains("mandatory ability-owned OwnerGraveyard"),
        "{error}"
    );
}

#[test]
fn linked_choice_program_result_joins_only_the_same_producer_and_link() {
    let owner = crate::ProgramOwnerId::ability(crate::AbilityDefinitionId {
        card_id: "currency_converter".into(),
        face_id: crate::CardFaceId::new("currency_converter").unwrap(),
        ability_path: vec![crate::AbilityId::new("return_linked_card").unwrap()],
    });
    let body = |right_producer: &str, right_link: &str| {
        format!(
            r#"(body: (steps: [(id: "branch", bind: ["chosen"], kind: If(
                condition: Void,
                yes: (steps: [(id: "left_choice", bind: ["left"], kind: Perform(
                    ChooseLinkedExiledCard(
                        producer_ability_id: "discard_to_exile",
                        linked_exile_id: "exiled_cards",
                        filter: (card_type: Some(Creature)),
                        destination: OwnerGraveyard,
                        optional: false,
                    )
                ))], yields: ["left"]),
                no: (steps: [(id: "right_choice", bind: ["right"], kind: Perform(
                    ChooseLinkedExiledCard(
                        producer_ability_id: "{right_producer}",
                        linked_exile_id: "{right_link}",
                        filter: (card_type: Some(Creature)),
                        destination: OwnerGraveyard,
                        optional: false,
                    )
                ))], yields: ["right"]),
            ))], yields: []), target_bindings: [])"#
        )
    };

    let same_origin: crate::CardProgram =
        ron::from_str(&body("discard_to_exile", "exiled_cards")).unwrap();
    let compiled = crate::primitives::CompiledProgram::compile(
        owner.clone(),
        crate::primitives::EffectContext::Ability,
        &same_origin,
    )
    .expect("same linked result origin may join across branches");
    let joined = &compiled
        .slot(compiled.body.steps[0].outputs[0])
        .unwrap()
        .result_type;
    assert!(matches!(
        joined,
        crate::primitives::ResultType::LinkedExileCardResultCohort {
            producer_ability_id,
            linked_exile_id,
            cardinality: crate::primitives::Cardinality {
                min: 0,
                max: Some(1),
            },
        } if producer_ability_id.as_str() == "discard_to_exile"
            && linked_exile_id.as_str() == "exiled_cards"
    ));

    for (producer, link) in [
        ("another_producer", "exiled_cards"),
        ("discard_to_exile", "another_link"),
    ] {
        let different_origin: crate::CardProgram = ron::from_str(&body(producer, link)).unwrap();
        let error = crate::primitives::CompiledProgram::compile(
            owner.clone(),
            crate::primitives::EffectContext::Ability,
            &different_origin,
        )
        .expect_err("distinct linked result origins cannot join");
        assert!(
            error.contains("mismatched explicit yield types"),
            "unexpected error for producer={producer}, link={link}: {error}"
        );
    }
}

#[test]
fn duplicant_targeted_linked_exile_pair_is_exact_and_fail_closed() {
    let valid = r#"(
        id: "duplicant",
        name: "Duplicant",
        face_id: "duplicant",
        mana_cost: "{6}",
        types: ["Artifact", "Creature", "Shapeshifter"],
        power: 2,
        toughness: 4,
        triggered_abilities: [(
            ability_id: "imprint",
            presentation: OracleLines([1]),
            trigger: WhenSelfEntersBattlefield,
            may: true,
            targeting: Some((groups: [(min: 1, max: 1, prompt: "Choose target nontoken creature", effect_indices: [0])])),
            effect: [Exile(
                subject: Chosen((kind: Creature, token: Some(false))),
                linked_exile_id: Some("imprinted_creature"),
            )],
        )],
        static_abilities: [(ability_id: "imprint_static", presentation: OracleLines([2]), definition: DuplicantImprint(
            producer_ability_id: "imprint",
            linked_exile_id: "imprinted_creature",
        ))],
    )"#;
    crate::CardRegistry::from_chunks_and_tokens(&[valid], &[])
        .expect("Duplicant needs a valid, exact linked-exile pair");

    let mismatched_producer = valid.replace(
        "producer_ability_id: \"imprint\"",
        "producer_ability_id: \"other\"",
    );
    let error = crate::CardRegistry::from_chunks_and_tokens(&[&mismatched_producer], &[])
        .expect_err("the static consumer must name the printed trigger producer");
    assert!(
        error
            .to_string()
            .contains("must match exactly one targeted self-entry exile producer"),
        "{error}"
    );

    let mismatched_link = valid.replace(
        "linked_exile_id: \"imprinted_creature\",\n        ))]",
        "linked_exile_id: \"other_creature\",\n        ))]",
    );
    let error = crate::CardRegistry::from_chunks_and_tokens(&[&mismatched_link], &[])
        .expect_err("the static consumer must name the exile effect's link id");
    assert!(
        error
            .to_string()
            .contains("must match exactly one targeted self-entry exile producer"),
        "{error}"
    );

    let cross_wired_pairs = r#"(
        id: "cross_wired_duplicant",
        name: "Cross-wired Duplicant",
        face_id: "cross_wired_duplicant",
        mana_cost: "{6}",
        types: ["Artifact", "Creature", "Shapeshifter"],
        power: 2,
        toughness: 4,
        triggered_abilities: [
            (
                ability_id: "imprint_a",
                presentation: OracleLines([1]),
                trigger: WhenSelfEntersBattlefield,
                may: true,
                targeting: Some((groups: [(min: 1, max: 1, prompt: "Choose target nontoken creature", effect_indices: [0])])),
                effect: [Exile(subject: Chosen((kind: Creature, token: Some(false))), linked_exile_id: Some("link_a"))],
            ),
            (
                ability_id: "imprint_b",
                presentation: OracleLines([1]),
                trigger: WhenSelfEntersBattlefield,
                may: true,
                targeting: Some((groups: [(min: 1, max: 1, prompt: "Choose target nontoken creature", effect_indices: [0])])),
                effect: [Exile(subject: Chosen((kind: Creature, token: Some(false))), linked_exile_id: Some("link_b"))],
            ),
        ],
        static_abilities: [
            (ability_id: "static_a", presentation: OracleLines([2]), definition: DuplicantImprint(producer_ability_id: "imprint_a", linked_exile_id: "link_b")),
            (ability_id: "static_b", presentation: OracleLines([2]), definition: DuplicantImprint(producer_ability_id: "imprint_b", linked_exile_id: "link_a")),
        ],
    )"#;
    let error = crate::CardRegistry::from_chunks_and_tokens(&[cross_wired_pairs], &[])
        .expect_err("each Duplicant static consumer must use its named producer's link id");
    assert!(
        error
            .to_string()
            .contains("must match exactly one targeted self-entry exile producer"),
        "{error}"
    );

    let missing_consumer = valid.replace(
        "        static_abilities: [(ability_id: \"imprint_static\", presentation: OracleLines([2]), definition: DuplicantImprint(\n            producer_ability_id: \"imprint\",\n            linked_exile_id: \"imprinted_creature\",\n        ))],\n",
        "",
    );
    let error = crate::CardRegistry::from_chunks_and_tokens(&[&missing_consumer], &[])
        .expect_err("a linked targeted exile producer without its static consumer is invalid");
    assert!(
        error
            .to_string()
            .contains("requires exactly one producer and one consumer"),
        "{error}"
    );
}

#[test]
fn issue_185_saga_vocabulary_and_validation_are_typed() {
    let card = r#"(
        id: "test_saga",
        name: "Test Saga",
        face_id: "test_saga",
        mana_cost: "{1}{R}",
        types: ["Enchantment", "Saga"],
        keywords: [ReadAhead],
        triggered_abilities: [(
            ability_id: "chapter_i_ii",
            presentation: Fallback,
            trigger: SagaChapter(chapters: [1, 2]),
            effect: [GainLife(amount: 1)],
        )],
    )"#;
    assert!(crate::CardRegistry::from_chunks_and_tokens(&[card], &[]).is_ok());

    for chapters in ["[]", "[0]", "[2, 1]", "[1, 1]"] {
        let trigger: super::TriggerCondition =
            ron::from_str(&format!("SagaChapter(chapters: {chapters})"))
                .expect("Saga chapter syntax must deserialize");
        assert!(trigger.validate().is_err(), "{chapters}");
    }

    let non_saga = card.replace("[\"Enchantment\", \"Saga\"]", "[\"Enchantment\"]");
    assert!(crate::CardRegistry::from_chunks_and_tokens(&[&non_saga], &[]).is_err());

    let no_chapters = r#"(
        id: "chapterless_saga",
        name: "Chapterless Saga",
        face_id: "chapterless_saga",
        types: ["Enchantment", "Saga"],
        keywords: [ReadAhead],
    )"#;
    assert!(crate::CardRegistry::from_chunks_and_tokens(&[no_chapters], &[]).is_err());

    let granted: super::SpellEffectKind = ron::from_str(
        r#"GrantTriggeredAbility(
            subject: Source,
            ability: (
                ability_id: "chapter_i",
                presentation: Fallback,
                trigger: SagaChapter(chapters: [1]),
                effect: [GainLife(amount: 1)],
            ),
        )"#,
    )
    .expect("nested chapter syntax deserializes before shape validation");
    assert!(granted.validate(super::EffectContext::Ability).is_err());
}

#[test]
fn issue_165_dynamic_consumers_reject_unavailable_result_contexts() {
    for template in [
        "Scry(count: AMOUNT)",
        "DamageAll(amount: AMOUNT)",
        "CounterTargetSpell(unless_controller_pays: Some(AMOUNT))",
    ] {
        let previous = template.replace("AMOUNT", "Count(CardsMatchingResult(filter: (source: PreviousEffect, action: Discard, players: Controller)))");
        let effect: super::SpellEffectKind = ron::from_str(&previous).unwrap();
        assert!(
            super::SpellEffectKind::validate_list(&[effect]).is_err(),
            "missing previous effect: {template}"
        );
        let payment = template.replace("AMOUNT", "Count(CardsMatchingResult(filter: (source: Payment, action: Discard, players: Controller)))");
        let card = format!(
            r#"(id: "test", name: "Test", face_id: "test", types: ["Instant"], spell_effect: [{payment}])"#
        );
        assert!(
            crate::CardRegistry::from_chunks_and_tokens(&[&card], &[]).is_err(),
            "missing payment: {template}"
        );
    }
}

#[test]
fn issue_165_quantities_reject_unsupported_shapes_and_source_contexts() {
    for source in [
        "Affine(terms: [])",
        "Affine(terms: [(coefficient: 0, quantity: SourcePower)])",
        "Affine(terms: [(coefficient: 1, quantity: Affine(terms: [(coefficient: 1, quantity: SourcePower)]))])",
        "Affine(terms: [(coefficient: 1, quantity: CardsMatchingResult(filter: (source: Payment, action: Discard, players: Controller)))])",
    ] {
        let expression: super::CountExpression = ron::from_str(source).unwrap();
        assert!(expression.validate().is_err(), "{source}");
    }
    let amount = super::Amount::Count(super::CountExpression::SourcePower);
    assert!(super::SpellEffectKind::GainLife {
        amount: amount.clone()
    }
    .validate(super::EffectContext::Spell)
    .is_err());
    assert!(super::SpellCostModifier::GenericReduction { amount }
        .validate()
        .is_err());
    for count in [
        "SourcePower",
        "BattlefieldMaximum(filter: (controllers: Controller), characteristic: Power)",
    ] {
        let expression: super::CountExpression = ron::from_str(count).unwrap();
        assert!(expression.validate_static_count().is_err());
    }
    let card = r#"(id: "test", name: "Test", face_id: "test", types: ["Creature"], power: 1, toughness: 2,
        static_abilities: [(ability_id: "static_01", presentation: Fallback, definition: EntersWithCounters(counter: PlusOnePlusOne, amount: Count(SourcePower)))])"#;
    assert!(
        crate::CardRegistry::from_chunks_and_tokens(&[card], &[]).is_err(),
        "entry replacement has no battlefield source power"
    );
}

#[test]
fn pentad_prism_cast_colors_are_intrinsic_entry_only_and_cannot_escape_through_wrappers() {
    for source in [
        "Count(ManaColorsSpentToCast)",
        "DivideRoundedDown(amount: Count(ManaColorsSpentToCast), divisor: 2)",
    ] {
        let amount: super::Amount = ron::from_str(source).unwrap();
        assert!(amount.validate_entry(true).is_ok());
        assert!(amount.validate_entry(false).is_err());
        assert!(amount.validate_live().is_err());
        assert!(amount.validate_cost(false).is_err());
        for context in [super::EffectContext::Spell, super::EffectContext::Ability] {
            assert!(super::SpellEffectKind::GainLife {
                amount: amount.clone()
            }
            .validate(context)
            .is_err());
        }
    }
    let count = super::CountExpression::ManaColorsSpentToCast;
    assert!(count.validate_static_count().is_err());
    let affine: super::CountExpression =
        ron::from_str("Affine(terms: [(coefficient: 1, quantity: ManaColorsSpentToCast)])")
            .unwrap();
    assert!(affine.validate().is_err());
    for affected in ["Self_", "Creatures((who: Controller))"] {
        let card = format!(
            r#"(id: "test", name: "Test", face_id: "test", types: ["Artifact"], static_abilities: [(ability_id: "static_01", presentation: Fallback, definition: EntersWithCounters(affected: {affected}, counter: Charge, amount: Count(ManaColorsSpentToCast)))])"#
        );
        let parsed = crate::CardRegistry::from_chunks_and_tokens(&[&card], &[]);
        assert_eq!(parsed.is_ok(), affected == "Self_", "{parsed:?}");
    }
}

#[test]
fn issue_165_dynamic_consumers_accept_quantities() {
    for effect in [
        "Scry(count: Count(SourcePower))",
        "DamageAll(amount: Count(GraveyardCards(owners: Controller)))",
        "CounterTargetSpell(unless_controller_pays: Some(Count(BattlefieldMaximum(filter: (controllers: Controller, card_type: Some(Creature)), characteristic: Power))))",
    ] {
        let effect: super::SpellEffectKind = ron::from_str(effect).expect("dynamic consumer");
        effect.validate(super::EffectContext::Ability).expect("valid dynamic consumer");
    }
}

#[test]
fn issue_229_mass_return_is_untargeted_and_validates_its_cohort() {
    let effect: super::SpellEffectKind = ron::from_str(
        "ReturnAllToOwnersHand(kind: (kind: Creature, excluded_subtypes: [\"Elemental\"]))",
    )
    .expect("mass return vocabulary");
    effect.validate(super::EffectContext::Ability).unwrap();
    assert!(effect.target_roles().is_empty());
    for filter in [
        "kind: AnyPlayer",
        "kind: Creature, controller: You",
        "kind: Creature, owner: You",
        "kind: Creature, required_subtypes: [\"Elemental\"], excluded_subtypes: [\"Elemental\"]",
    ] {
        let effect: super::SpellEffectKind =
            ron::from_str(&format!("ReturnAllToOwnersHand(kind: ({filter}))")).unwrap();
        assert!(
            effect.validate(super::EffectContext::Ability).is_err(),
            "{filter}"
        );
    }
}

#[test]
fn issue_229_cast_entry_condition_is_limited_to_self_entry_intervening_if() {
    let card = |body: &str| {
        format!(
            r#"(id: "test", name: "Test", face_id: "test", types: ["Creature"], power: 1, toughness: 1, {body})"#
        )
    };
    let ability = |trigger: &str| {
        format!(
            r#"triggered_abilities: [(ability_id: "entry", presentation: Fallback, trigger: {trigger}, intervening_if: Some(SelfWasCast), effect: [GainLife(amount: 1)])]"#
        )
    };
    assert!(crate::CardRegistry::from_chunks_and_tokens(
        &[&card(&ability("WhenSelfEntersBattlefield"))],
        &[]
    )
    .is_ok());
    for body in [
        ability("WhenSelfDies"),
        "cast_conditions: [SelfWasCast]".into(),
        "cost_modifiers: [ConditionalGenericReduction(amount: 1, condition: SelfWasCast)]".into(),
        "spell_effect: [Conditional(condition: SelfWasCast, effect: GainLife(amount: 1))]".into(),
        "spell_effect: [GainLife(amount: Conditional(condition: SelfWasCast, when_true: 2, otherwise: 1))]".into(),
    ] {
        assert!(crate::CardRegistry::from_chunks_and_tokens(&[&card(&body)], &[]).is_err(), "{body}");
    }
}
#[test]
fn issue_165_static_quantity_accepts_permanents_without_pt_recursion() {
    let card = r#"(id: "test", name: "Test", face_id: "test", types: ["Creature"], power: 1, toughness: 2,
        static_abilities: [(ability_id: "static_01", presentation: Fallback, definition: CountScaledSelfPt(count: BattlefieldPermanents(filter: (controllers: Controller, required_subtypes: ["Desert"])),
            power_per_match: 1, toughness_per_match: 1))])"#;
    assert!(crate::CardRegistry::from_chunks_and_tokens(&[card], &[]).is_ok());
}
#[test]
fn issue_165_public_quantities_roundtrip() {
    for source in [
        "Count(BattlefieldPermanents(filter: (controllers: Controller, required_subtypes: [\"Island\"])))",
        "Count(GraveyardCards(owners: Controller, filter: Some((required_subtypes: [\"Cave\"]))))",
        "Count(BattlefieldMaximum(filter: (controllers: Controller, card_type: Some(Creature)), characteristic: Toughness))",
        "Count(SourcePower)",
        "Count(DeclaredAttackers(players: All))",
        "Count(Affine(constant: 2, terms: [(coefficient: 2, quantity: SourcePower)]))",
    ] {
        let amount: super::Amount = ron::from_str(source)
            .unwrap_or_else(|error| panic!("quantity must deserialize: {source}: {error}"));
        amount.validate().expect("valid public quantity");
        let encoded = ron::to_string(&amount).unwrap();
        assert_eq!(ron::from_str::<super::Amount>(&encoded).unwrap(), amount);
    }
}

#[test]
fn issue_169_tap_trigger_cardinalities_are_typed() {
    for cardinality in ["EachObject", "OneOrMorePerAction"] {
        let definition = format!(
            "WheneverPlayerTapsCreature(player: Controller, controllers: Opponents, cardinality: {cardinality})"
        );
        assert!(
            ron::from_str::<super::TriggerCondition>(&definition).is_ok(),
            "actor-aware tap trigger must deserialize: {definition}"
        );
    }
}

#[test]
fn triggered_mana_output_detection_follows_supported_nested_effects() {
    let direct: super::SpellEffectKind =
        ron::from_str("AddMana(amount: (g: 1), retention: EndOfStep)").unwrap();
    assert!(direct.could_add_mana_to_pool());

    let conditional: super::SpellEffectKind = ron::from_str(
        "Conditional(condition: ActivePlayer(players: Controller), effect: AddMana(amount: (g: 1), retention: EndOfStep))",
    )
    .unwrap();
    assert!(conditional.could_add_mana_to_pool());

    let unrelated: super::SpellEffectKind = ron::from_str("GainLife(amount: 1)").unwrap();
    assert!(!unrelated.could_add_mana_to_pool());
}

#[test]
fn issue_169_grouped_taps_cannot_pretend_to_supply_one_observed_object() {
    for (cardinality, valid) in [("EachObject", true), ("OneOrMorePerAction", false)] {
        let definition = format!(
            r#"(
            ability_id: "triggered_01",
            presentation: Fallback,
            trigger: WheneverPlayerTapsCreature(player: Controller, controllers: Opponents, cardinality: {cardinality}),
            effect: [PutCounters(counter: Stun, count: 1, subject: TriggerObject)],
        )"#
        );
        let ability: super::TriggeredAbilityDef = ron::from_str(&definition).unwrap();
        assert_eq!(ability.validate_shape().is_ok(), valid);
    }
}

use super::*;

#[test]
fn attack_declaration_limit_authoring_preserves_zero_and_scope_and_rejects_ambiguous_input() {
    for (text, maximum, affected) in [
        (
            "LimitAttackers(maximum: 2, affected: AttackingController)",
            2,
            AttackLimitAffected::AttackingController,
        ),
        (
            "LimitAttackers(maximum: 0, affected: All)",
            0,
            AttackLimitAffected::All,
        ),
    ] {
        let ability: StaticAbilityDef = ron::from_str(text).unwrap();
        assert_eq!(
            ability,
            StaticAbilityDef::LimitAttackers { maximum, affected }
        );
        let encoded = ron::to_string(&ability).unwrap();
        assert_eq!(
            ron::from_str::<StaticAbilityDef>(&encoded).unwrap(),
            ability
        );
    }
    for invalid in [
        "LimitAttackers(maximum: -1, affected: All)",
        "LimitAttackers(maximum: 4294967296, affected: All)",
        "LimitAttackers(maximum: 2)",
        "LimitAttackers(maximum: 2, affected: DefendingPlayer)",
    ] {
        assert!(
            ron::from_str::<StaticAbilityDef>(invalid).is_err(),
            "{invalid}"
        );
    }
}
use crate::primitives::{AbilityCost, ActivatedAbilityDef};
use crate::{AbilityId, AbilityPresentation, ManaCost};

fn arena_opponent_chooser_ability() -> ActivatedAbilityDef {
    ron::from_str(
        r#"(
            ability_id: "activated_01",
            presentation: Fallback,
            costs: [Mana("{3}"), Tap],
            effect: [
                Tap(subject: Chosen((kind: Creature, controller: You))),
                Tap(subject: Chosen((kind: Creature, controller: Opponent))),
                Fight(first: Chosen((kind: Creature, controller: You)),
                      second: Chosen((kind: Creature, controller: Opponent))),
            ],
            targeting: Some((groups: [
                (min: 1, max: 1, prompt: "Choose your creature", effect_indices: [0, 2]),
                (min: 1, max: 1, prompt: "Choose your opponent's creature",
                 effect_indices: [1, 2], chooser: ChosenOpponent),
            ])),
        )"#,
    )
    .expect("Arena's authored activation shape parses")
}

#[test]
fn trigger_object_copy_is_trigger_context_scoped() {
    let effect = SpellEffectKind::CreateTokenCopies {
        count: Amount::Fixed(1),
        source: TokenCopySource::TriggerObject,
    };
    assert!(effect.uses_trigger_object_reference());
    assert!(effect
        .validate(EffectContext::Spell)
        .unwrap_err()
        .contains("trigger object"));

    let mut ability = TriggeredAbilityDef {
        ability_id: AbilityId::new("triggered_01").unwrap(),
        presentation: AbilityPresentation::Fallback,
        trigger: TriggerCondition::WhenSelfEntersBattlefield,
        effect: vec![effect.clone()],
        program: None,
        modal: None,
        targeting: None,
        may: false,
        intervening_if: None,
        triggers_only_once: false,
        max_triggers_per_turn: None,
    };
    assert!(ability
        .validate_shape()
        .unwrap_err()
        .contains("trigger-object effect requires a trigger that supplies an observed object"));

    let activated = ActivatedAbilityDef {
        ability_id: AbilityId::new("activated_01").unwrap(),
        presentation: AbilityPresentation::Fallback,
        intrinsic_land_mana: false,
        source_zone: Default::default(),
        costs: vec![AbilityCost::Tap],
        cost_modifiers: vec![],
        effect: vec![effect.clone()],
        program: None,
        targeting: None,
        timing: Default::default(),
        conditions: vec![],
        activation_limit: None,
    };
    assert!(activated
        .validate_shape()
        .unwrap_err()
        .contains("activated abilities cannot reference a trigger object"));

    ability.trigger = TriggerCondition::WheneverPermanentEntersBattlefield {
        controller: CastTriggerPlayer::Controller,
        filter: PermanentEventFilter {
            permanent_type: Some(PermanentTypeFilter::Artifact),
            exclude_source: true,
            token: Some(false),
            ..Default::default()
        },
        creature_filter: None,
    };
    ability
        .validate_shape()
        .expect("Mirrorworks' permanent-entry trigger supplies one observed object");

    let reflexive = ReflexiveTriggeredAbilityDef {
        ability_id: AbilityId::new("reflexive_01").unwrap(),
        presentation: AbilityPresentation::Fallback,
        effect: vec![effect],
        targeting: None,
        intervening_if: None,
    };
    assert!(reflexive
        .validate_shape()
        .unwrap_err()
        .contains("cannot reference a trigger object"));
}

#[test]
fn graveyard_trigger_object_mana_value_predicate_is_context_scoped() {
    let default_filter = ron::to_string(&super::GraveyardFilter::default()).unwrap();
    assert!(
        !default_filter.contains("mana_value_less_than_trigger_object"),
        "the default predicate must not change canonical RON for existing cards"
    );

    let effect: super::SpellEffectKind = ron::from_str(
        "MoveGraveyardCards(filter: (mana_value_less_than_trigger_object: true), destination: Hand)",
    )
    .unwrap();
    assert!(effect.uses_trigger_object_reference());
    assert!(effect.uses_trigger_object_mana_value_graveyard_filter());
    assert!(effect
        .validate(super::EffectContext::Spell)
        .unwrap_err()
        .contains("trigger object"));

    let filter =
        ron::from_str::<super::GraveyardFilter>("(mana_value_less_than_trigger_object: true)")
            .unwrap();
    assert!(super::TargetMatchFilter::Graveyard(filter)
        .validate()
        .unwrap_err()
        .contains("cost reductions"));

    let activated = r#"(
        id: "test", name: "Test", face_id: "test", mana_cost: "{3}",
        types: ["Artifact", "Creature"], power: 3, toughness: 2,
        activated_abilities: [(
            ability_id: "activated_01", presentation: Fallback,
            source_zone: Battlefield, costs: [Tap],
            effect: [MoveGraveyardCards(
                filter: (mana_value_less_than_trigger_object: true),
                destination: Hand)],
            targeting: Some((groups: [(min: 1, max: 1,
                prompt: "Choose target", effect_indices: [0])]))
        )]
    )"#;
    assert!(
        crate::CardRegistry::from_chunks_and_tokens(&[activated], &[]).is_err(),
        "an activated ability cannot use a trigger-object value"
    );

    let card = |trigger: &str| {
        format!(
            r#"(
                id: "test", name: "Test", face_id: "test", mana_cost: "{{3}}",
                types: ["Artifact", "Creature"], power: 3, toughness: 2,
                triggered_abilities: [(
                    ability_id: "triggered_01", presentation: Fallback,
                    trigger: {trigger},
                    effect: [MoveGraveyardCards(
                        filter: (card: Some((card_type: Some(Artifact))),
                                 mana_value_less_than_trigger_object: true),
                        destination: Hand)],
                    targeting: Some((groups: [(min: 1, max: 1,
                        prompt: "Choose target", effect_indices: [0])]))
                )]
            )"#
        )
    };
    for trigger in [
        "WhenSelfDies",
        "WheneverPermanentLeavesBattlefield(controller: Controller, filter: (permanent_type: Some(Artifact)), destination: OneOf([Graveyard]), cardinality: EachObject)",
    ] {
        let definition = card(trigger);
        crate::CardRegistry::from_chunks_and_tokens(&[&definition], &[])
            .unwrap_or_else(|error| panic!("supported event source {trigger}: {error}"));
    }
    for trigger in [
        "WhenSelfLeavesBattlefield",
        "WhenSelfEntersOrIsPutIntoGraveyardFromBattlefield",
        "WheneverPermanentLeavesBattlefield(controller: Controller, filter: (permanent_type: Some(Artifact)), destination: OneOf([Graveyard]), cardinality: OneOrMorePerAction)",
    ] {
        let definition = card(trigger);
        assert!(
            crate::CardRegistry::from_chunks_and_tokens(&[&definition], &[]).is_err(),
            "unsupported event source accepted: {trigger}"
        );
    }
}

#[test]
fn arena_opponent_chooser_survives_ron_roundtrip() {
    let ability = arena_opponent_chooser_ability();
    let encoded = ron::ser::to_string(&ability).unwrap();
    assert!(
        encoded.contains("ChosenOpponent"),
        "the opponent's target-choice authority must not be discarded"
    );
    assert_eq!(
        ron::from_str::<ActivatedAbilityDef>(&encoded).unwrap(),
        ability
    );
}

#[test]
fn arena_opponent_chooser_is_not_a_spell_or_trigger_target_contract() {
    let ability = arena_opponent_chooser_ability();
    let error = TargetingDef::validate_optional(ability.targeting.as_ref(), &ability.effect)
        .expect_err("ordinary spell/trigger target validation cannot assign an opponent's choice");
    assert!(error.contains("activated"), "{error}");
}

#[test]
fn arena_opponent_chooser_rejects_unsupported_activation_shapes() {
    let ability = arena_opponent_chooser_ability();
    ability.validate_shape().expect("bounded Arena activation");

    let mut positive_loyalty = ability.clone();
    positive_loyalty.costs.push(AbilityCost::Loyalty(1));
    positive_loyalty
        .validate_shape()
        .expect("a positive loyalty cost can be staged with mana and source tap");

    let mut sacrifice = ability.clone();
    sacrifice.costs.push(AbilityCost::SacrificeSelf);
    assert!(
        sacrifice.validate_shape().is_err(),
        "opponent-choice activation still rejects unsupported extra costs"
    );
    let mut loyalty_debit = ability.clone();
    loyalty_debit.costs.push(AbilityCost::Loyalty(-1));
    assert!(loyalty_debit.validate_shape().is_err());
    let mut optional = ability.clone();
    optional.targeting.as_mut().unwrap().groups[1].min = 0;
    assert!(optional.validate_shape().is_err());
    let mut several = ability.clone();
    several.targeting.as_mut().unwrap().groups[1].max = 2;
    assert!(several.validate_shape().is_err());
    let mut wrong_controller = ability;
    wrong_controller.effect[1] = SpellEffectKind::Tap {
        subject: EffectSubject::Chosen(Box::new(TargetFilter {
            kind: TargetKind::Creature,
            controller: TargetController::You,
            ..Default::default()
        })),
    };
    assert!(wrong_controller.validate_shape().is_err());
}

#[test]
fn issue_172_expend_threshold_is_positive_and_defaults_to_controller() {
    for amount in [0, 1, 4, u32::MAX] {
        let trigger: TriggerCondition =
            ron::from_str(&format!("WheneverPlayerExpendsMana(amount: {amount})")).unwrap();
        assert_eq!(
            trigger,
            TriggerCondition::WheneverPlayerExpendsMana {
                player: CastTriggerPlayer::Controller,
                amount,
            }
        );
        assert_eq!(trigger.validate().is_ok(), amount != 0);
    }
}

#[test]
fn special_action_mana_restrictions_validate_without_spell_or_ability_permissions() {
    for purpose in ["UnlockRoomDoor", "TurnFaceUp"] {
        let restriction: ManaSpendingRestriction = ron::from_str(&format!(
            "(restriction_id: \"restriction_01\", presentation: Fallback, special_actions: [{purpose}])"
        ))
        .expect("typed special-action restriction");
        assert!(restriction.validate().is_ok(), "{purpose}");
        assert!(restriction.cast_spell.is_empty());
        assert!(restriction.activate_ability.is_empty());
    }
}

#[test]
fn special_action_mana_restrictions_reject_unknown_purposes() {
    assert!(ron::from_str::<ManaSpendingRestriction>(
        "(restriction_id: \"restriction_01\", presentation: Fallback, special_actions: [CastAnything])"
    )
    .is_err());
}

#[test]
fn unrestricted_ability_and_all_nonspell_permissions_are_distinct_and_canonical() {
    let any_ability: ManaSpendingRestriction = ron::from_str(
        "(restriction_id: \"restriction_01\", presentation: Fallback, cast_spell: [(card_type: Some(Artifact))], activate_any_ability: true)",
    )
    .expect("unrestricted ability permission");
    assert!(any_ability.validate().is_ok());
    assert!(any_ability.activate_any_ability);
    assert!(!any_ability.all_nonspell_costs);

    let all_nonspell: ManaSpendingRestriction = ron::from_str(
        "(restriction_id: \"restriction_01\", presentation: Fallback, cast_spell: [(card_type: Some(Artifact))], all_nonspell_costs: true)",
    )
    .expect("all-nonspell permission");
    assert!(all_nonspell.validate().is_ok());
    assert!(!all_nonspell.activate_any_ability);
    assert!(all_nonspell.all_nonspell_costs);

    for redundant in [
        "(restriction_id: \"restriction_01\", presentation: Fallback, activate_ability: [(card_type: Some(Artifact))], activate_any_ability: true)",
        "(restriction_id: \"restriction_01\", presentation: Fallback, all_nonspell_costs: true, activate_any_ability: true)",
        "(restriction_id: \"restriction_01\", presentation: Fallback, all_nonspell_costs: true, special_actions: [TurnFaceUp])",
    ] {
        let restriction: ManaSpendingRestriction = ron::from_str(redundant).unwrap();
        assert!(restriction.validate().is_err(), "{redundant}");
    }
}

#[test]
fn unrestricted_mana_can_carry_a_validated_spell_keyword_effect() {
    let rule: ManaSpendingRestriction = ron::from_str(
        r#"(
            restriction_id: "restriction_01",
            presentation: Fallback,
            unrestricted: true,
            spending_effects: [GrantKeywordsToSpellUntilEndOfTurn(
                filter: (card_type: Some(Creature)),
                keywords: [Haste],
            )],
        )"#,
    )
    .expect("typed mana spending effect");
    assert!(rule.validate().is_ok());
    assert_eq!(
        rule.fallback_label(),
        "Spend this mana on any cost; a creature spell paid with it gains Haste until end of turn"
    );

    for invalid in [
        r#"(
            restriction_id: "restriction_01",
            presentation: Fallback,
            unrestricted: true,
            cast_spell: [(card_type: Some(Creature))],
        )"#,
        r#"(
            restriction_id: "restriction_01",
            presentation: Fallback,
            unrestricted: true,
            spending_effects: [GrantKeywordsToSpellUntilEndOfTurn(
                filter: (card_type: Some(Creature)),
                keywords: [],
            )],
        )"#,
        r#"(
            restriction_id: "restriction_01",
            presentation: Fallback,
            unrestricted: true,
            spending_effects: [GrantKeywordsToSpellUntilEndOfTurn(
                filter: (card_type: Some(Creature)),
                keywords: [Haste, Haste],
            )],
        )"#,
    ] {
        let rule: ManaSpendingRestriction = ron::from_str(invalid).unwrap();
        assert!(rule.validate().is_err(), "{invalid}");
    }
}

#[test]
fn firebending_uses_a_resolving_combat_retained_mana_effect() {
    let effect = SpellEffectKind::AddMana {
        amount: ManaAmount {
            r: 2,
            ..Default::default()
        },
        retention: ManaRetention::EndOfCombat,
    };

    assert!(effect.validate(EffectContext::Ability).is_ok());
    assert!(!matches!(effect, SpellEffectKind::ProduceMana { .. }));
}

#[test]
fn soft_counter_payment_must_be_nonzero() {
    let invalid = SpellEffectKind::CounterTargetSpell {
        spell_filter: StackSpellFilter::default(),
        unless_controller_pays: Some(Amount::Fixed(0)),
        unless_controller_pays_by_cast_cost: None,
    };
    assert!(invalid.validate(EffectContext::Spell).is_err());
    let mana_leak_shape = SpellEffectKind::CounterTargetSpell {
        spell_filter: StackSpellFilter::default(),
        unless_controller_pays: Some(Amount::Fixed(3)),
        unless_controller_pays_by_cast_cost: None,
    };
    assert!(mana_leak_shape.validate(EffectContext::Spell).is_ok());
}

#[test]
fn stack_spell_filter_rejects_inverted_mana_value_bounds() {
    let invalid = SpellEffectKind::CounterTargetSpell {
        spell_filter: StackSpellFilter {
            min_mana_value: Some(4),
            max_mana_value: Some(2),
            ..Default::default()
        },
        unless_controller_pays: None,
        unless_controller_pays_by_cast_cost: None,
    };
    assert!(invalid.validate(EffectContext::Spell).is_err());
}

#[test]
fn issue_231_stack_filter_disjunction_and_color_are_validated() {
    let filter: StackSpellFilter =
        ron::from_str("(any_of: Some([(card_type: Some(Artifact)), (is_color: Some(Red))]))")
            .expect("stack filters support type and color alternatives");
    assert!(filter.validate().is_ok());
    for text in [
        "(any_of: Some([]))",
        "(any_of: Some([(is_color: Some(Red))]))",
        "(any_of: Some([(is_color: Some(Red)), (is_color: Some(Red))]))",
        "(card_type: Some(Creature), any_of: Some([(is_color: Some(Red)), (is_color: Some(Green))]))",
        "(any_of: Some([(min_mana_value: Some(4), max_mana_value: Some(2)), (is_color: Some(Red))]))",
        "(any_of: Some([(is_color: Some(Red)), (any_of: Some([(is_color: Some(Green)), (is_color: Some(Red))]))]))",
    ] {
        let filter: StackSpellFilter = ron::from_str(text).unwrap();
        assert!(filter.validate().is_err(), "accepted {text}");
    }
}

#[test]
fn issue_231_owner_filter_requires_permanents_and_pure_or_nodes() {
    let filter: TargetFilter = ron::from_str("(kind: AnyPermanent, owner: You)").unwrap();
    assert!(filter.validate_target_constraints().is_ok());
    for text in [
        "(kind: AnyPlayer, owner: You)",
        "(kind: AnyTarget, owner: You)",
        "(owner: You, any_of: Some([(kind: Creature), (kind: AnyPermanent)]))",
    ] {
        let filter: TargetFilter = ron::from_str(text).unwrap();
        assert!(
            filter.validate_target_constraints().is_err(),
            "accepted {text}"
        );
        assert!(
            filter.validate_characteristic_constraints().is_err(),
            "accepted {text}"
        );
    }
}

#[test]
fn issue_231_context_free_filters_reject_relative_ownership() {
    for text in [
        "DestroyAll(kind: (kind: Creature, owner: You))",
        "TargetPlayerSacrifices(target: (kind: AnyPlayer), filter: (kind: Creature, owner: You))",
        "ApplyCombatRestriction(scope: Source, restriction: (cant_be_blocked_by: [(kind: Creature, owner: You)]))",
    ] {
        let effect: SpellEffectKind = ron::from_str(text).unwrap();
        assert!(effect.validate(EffectContext::Ability).is_err(), "accepted {text}");
    }
    let copy = r#"(id: "test", name: "Test", face_id: "test", types: ["Creature"], power: 1, toughness: 1,
        static_abilities: [(ability_id: "copy", presentation: Fallback, definition: EntersAsCopy(filter: (kind: Creature, owner: You)))])"#;
    assert!(crate::CardRegistry::from_chunks_and_tokens(&[copy], &[]).is_err());
}

#[test]
fn zone_card_filters_validate_leaf_and_recursive_or_shapes() {
    let living_phone = ZoneCardFilter {
        card_type: Some(CardTypeFilter::Creature),
        printed_power: Some(PowerComparison::AtMost(2)),
        ..Default::default()
    };
    assert!(living_phone.validate().is_ok());

    let say_its_name = ZoneCardFilter {
        any_of: Some(vec![
            ZoneCardFilter {
                card_type: Some(CardTypeFilter::Creature),
                ..Default::default()
            },
            ZoneCardFilter {
                card_type: Some(CardTypeFilter::Land),
                ..Default::default()
            },
        ]),
        ..Default::default()
    };
    assert!(say_its_name.validate().is_ok());

    let tempest_hawk = ZoneCardFilter {
        exact_name: Some("Tempest Hawk".into()),
        ..Default::default()
    };
    assert!(tempest_hawk.validate().is_ok());

    assert!(ZoneCardFilter::default().validate().is_err());
    assert!(ZoneCardFilter {
        exact_name: Some(" ".into()),
        ..Default::default()
    }
    .validate()
    .is_err());
    assert!(ZoneCardFilter {
        any_of: Some(vec![tempest_hawk]),
        ..Default::default()
    }
    .validate()
    .is_err());
    assert!(ZoneCardFilter {
        any_of: Some(vec![
            ZoneCardFilter {
                card_type: Some(CardTypeFilter::Creature),
                ..Default::default()
            },
            ZoneCardFilter {
                card_type: Some(CardTypeFilter::Land),
                ..Default::default()
            },
        ]),
        required_subtypes: vec!["Bird".into()],
        ..Default::default()
    }
    .validate()
    .is_err());
}

#[test]
fn zone_card_filter_required_supertypes_must_be_nonempty_and_unique() {
    let empty = ZoneCardFilter {
        card_type: Some(CardTypeFilter::Creature),
        required_supertypes: vec![String::new()],
        ..Default::default()
    };
    assert_eq!(
        empty.validate(),
        Err("zone card filter supertype names must not be empty".into())
    );

    let duplicate = ZoneCardFilter {
        card_type: Some(CardTypeFilter::Creature),
        required_supertypes: vec!["Legendary".into(), "Legendary".into()],
        ..Default::default()
    };
    assert_eq!(
        duplicate.validate(),
        Err("zone card filter cannot repeat a supertype predicate".into())
    );
}

#[test]
fn graveyard_card_cost_requires_a_positive_bounded_filtered_cohort() {
    let namesake = ZoneCardFilter {
        exact_name: Some("Say Its Name".into()),
        ..Default::default()
    };
    assert!(AbilityCost::ExileGraveyardCards {
        constraint: ObjectPaymentConstraint::ExactCount(2),
        filter: namesake.clone(),
        exclude_source: true,
    }
    .validate()
    .is_ok());
    assert!(AbilityCost::ExileGraveyardCards {
        constraint: ObjectPaymentConstraint::ExactCount(0),
        filter: namesake,
        exclude_source: true,
    }
    .validate()
    .is_err());
    assert!(AbilityCost::ExileGraveyardCards {
        constraint: ObjectPaymentConstraint::ExactCount(1),
        filter: ZoneCardFilter::default(),
        exclude_source: false,
    }
    .validate()
    .is_err());
}

#[test]
fn optional_discard_then_draw_is_an_authored_payment_result_branch() {
    let body: crate::primitives::AuthoredBlock = ron::from_str(
        r#"(steps: [
            (id: "discard", bind: ["discarded"], kind: Perform(Discard(
                who: Controller, quantity: OptionalExact(1)
            ))),
            (id: "draw_if_paid", kind: IfResult(
                condition: DiscardOutcome(result: "discarded", outcome: ChosenToPay),
                yes: (steps: [(id: "draw", kind: Perform(Draw(who: Controller, count: 1)))],
                    yields: []),
                no: (steps: [], yields: [])
            ))
        ], yields: [])"#,
    )
    .expect("optional discard program fixture parses");
    let program = crate::primitives::CardProgram {
        body,
        target_bindings: Vec::new(),
    };
    let owner = crate::ProgramOwnerId::ability(crate::AbilityDefinitionId {
        card_id: "optional_discard_program_fixture".into(),
        face_id: crate::CardFaceId::new("optional_discard_program_fixture").unwrap(),
        ability_path: vec![crate::AbilityId::new("triggered_01").unwrap()],
    });
    let compiled = crate::primitives::CompiledProgram::compile(
        owner,
        crate::primitives::EffectContext::Ability,
        &program,
    )
    .expect("optional discard and conditional draw compile as authored instructions");

    let crate::primitives::CompiledInstructionKind::Perform(discard) = &compiled.body.steps[0].kind
    else {
        panic!("optional discard remains the first authored operation");
    };
    assert!(matches!(
        discard.as_ref(),
        crate::primitives::SpellEffectKind::Discard {
            quantity: crate::primitives::DiscardQuantity::OptionalExact(1),
            ..
        }
    ));
    assert!(matches!(
        compiled
            .slot(compiled.body.steps[0].outputs[0])
            .unwrap()
            .result_type,
        crate::primitives::ResultType::DiscardResult {
            optional_payment: true,
            ..
        }
    ));
    let crate::primitives::CompiledInstructionKind::If { condition, yes, no } =
        &compiled.body.steps[1].kind
    else {
        panic!("the draw must remain guarded by the discard outcome");
    };
    assert!(matches!(
        condition.as_ref(),
        crate::primitives::CompiledProgramCondition::Result(
            crate::primitives::CompiledProgramResultCondition::DiscardOutcome {
                outcome: crate::primitives::DiscardResultOutcome::ChosenToPay,
                ..
            }
        )
    ));
    assert!(matches!(
        yes.steps[0].kind,
        crate::primitives::CompiledInstructionKind::Perform(ref effect)
            if matches!(effect.as_ref(), crate::primitives::SpellEffectKind::Draw { .. })
    ));
    assert!(no.steps.is_empty());
}

#[test]
fn issue_230_discard_alternatives_validate_both_real_card_filters() {
    for kind in [CardTypeFilter::Creature, CardTypeFilter::Artifact] {
        let effect = |count, filter| SpellEffectKind::Discard {
            who: PlayerRecipient::Controller,
            quantity: DiscardQuantity::UnlessOne { count, filter },
        };
        let filter = ZoneCardFilter {
            card_type: Some(kind),
            ..Default::default()
        };
        assert!(effect(2, filter.clone())
            .validate(EffectContext::Spell)
            .is_ok());
        assert!(effect(1, filter).validate(EffectContext::Spell).is_err());
        assert!(effect(2, ZoneCardFilter::default())
            .validate(EffectContext::Spell)
            .is_err());
    }
}

#[test]
fn amount_count_expression_reads_cards_drawn_this_turn() {
    let expression: CountExpression = ron::from_str("CardsDrawnThisTurn(players: Controller)")
        .expect("the controller's committed draw count should parse as an amount expression");
    let amount = Amount::Count(expression);
    assert!(amount.validate().is_ok());
    let encoded = ron::to_string(&amount).expect("draw-count amount should serialize");
    assert_eq!(ron::from_str::<Amount>(&encoded).unwrap(), amount);
}

#[test]
fn amount_serde_preserves_literals_x_and_named_conditionals() {
    assert_eq!(ron::from_str::<Amount>("4").unwrap(), Amount::Fixed(4));
    assert_eq!(ron::from_str::<Amount>(r#""X""#).unwrap(), Amount::X);

    let amount = Amount::Conditional {
        condition: GameCondition::CreatureDeathsThisTurn {
            min: Some(1),
            max: None,
        },
        when_true: 8,
        otherwise: 4,
    };
    let encoded = ron::to_string(&amount).unwrap();
    assert_eq!(ron::from_str::<Amount>(&encoded).unwrap(), amount);

    for amount in [
        Amount::Count(CountExpression::BattlefieldCreatures {
            filter: BattlefieldCreatureCountFilter {
                controllers: RelativePlayerSet::Controller,
                subtype: None,
                required_keywords: vec![],
                tapped: None,
                requires_any_counter: false,
                required_counter: None,
                exclude_source: false,
            },
        }),
        Amount::Count(CountExpression::GraveyardCardsNamed {
            owners: RelativePlayerSet::Controller,
            name: "Growth Cycle".into(),
        }),
        Amount::Count(CountExpression::CreatureDeathsThisTurn),
        Amount::Count(CountExpression::CardsMatchingResult {
            filter: CardResultFilter {
                source: CardResultSource::PreviousEffect,
                action: CardResultAction::Mill,
                players: RelativePlayerSet::Controller,
                card_type: Some(CardTypeFilter::Creature),
            },
        }),
    ] {
        let encoded = ron::to_string(&amount).unwrap();
        assert_eq!(ron::from_str::<Amount>(&encoded).unwrap(), amount);
    }
}

#[test]
fn divided_amount_roundtrips_rounds_down_and_rejects_zero_divisors() {
    let amount: Amount = ron::from_str(r#"DivideRoundedDown(amount: "X", divisor: 2)"#).unwrap();
    assert_eq!(
        amount,
        Amount::DivideRoundedDown {
            amount: Box::new(Amount::X),
            divisor: 2,
        }
    );
    assert_eq!(
        ron::from_str::<Amount>(&ron::to_string(&amount).unwrap()).unwrap(),
        amount
    );
    assert_eq!(
        [0, 1, 2, 5].map(|x| amount.resolve_unconditional(x).unwrap()),
        [0, 0, 1, 2]
    );
    assert!(amount.is_x());

    assert!(Amount::DivideRoundedDown {
        amount: Box::new(Amount::Fixed(4)),
        divisor: 0,
    }
    .validate()
    .is_err());
}

#[test]
fn counted_amounts_reject_empty_authored_filters() {
    assert!(Amount::Count(CountExpression::BattlefieldCreatures {
        filter: BattlefieldCreatureCountFilter {
            controllers: RelativePlayerSet::Controller,
            subtype: Some(" ".into()),
            required_keywords: vec![],
            tapped: None,
            requires_any_counter: false,
            required_counter: None,
            exclude_source: false,
        },
    })
    .validate()
    .is_err());
    assert!(Amount::Count(CountExpression::GraveyardCardsNamed {
        owners: RelativePlayerSet::Controller,
        name: String::new(),
    })
    .validate()
    .is_err());
}

#[test]
fn lose_life_serde_supports_explicit_and_default_player_recipients() {
    let explicit: SpellEffectKind = ron::from_str("LoseLife(amount: Fixed(2), who: EachOpponent)")
        .expect("each-opponent life loss should deserialize");
    assert!(matches!(
        explicit,
        SpellEffectKind::LoseLife {
            amount: LifeAmount::Fixed(2),
            ..
        }
    ));
    assert!(ron::to_string(&explicit)
        .expect("each-opponent life loss should serialize")
        .contains("who:EachOpponent"));

    let legacy: SpellEffectKind = ron::from_str("LoseLife(amount: Fixed(1))")
        .expect("legacy controller life loss should deserialize");
    assert!(ron::to_string(&legacy)
        .expect("legacy controller life loss should serialize")
        .contains("who:Controller"));
}

#[test]
fn turn_history_condition_requires_a_valid_bound() {
    assert!(GameCondition::CreatureDeathsThisTurn {
        min: None,
        max: None,
    }
    .validate()
    .is_err());
    assert!(GameCondition::CreatureDeathsThisTurn {
        min: Some(2),
        max: Some(1),
    }
    .validate()
    .is_err());
    assert!(GameCondition::CreatureDeathsThisTurn {
        min: Some(1),
        max: None,
    }
    .validate()
    .is_ok());
    assert!(GameCondition::SpellsCastThisTurn {
        players: RelativePlayerSet::Controller,
        filter: Default::default(),
        min: None,
        max: None,
    }
    .validate()
    .is_err());
    assert!(GameCondition::SpellsCastThisTurn {
        players: RelativePlayerSet::Controller,
        filter: Default::default(),
        min: Some(1),
        max: None,
    }
    .validate()
    .is_ok());
}

#[test]
fn game_condition_any_of_deserializes_and_round_trips() {
    let condition: GameCondition = ron::from_str(
        "AnyOf([ActivePlayer(players: Controller), CreatureDeathsThisTurn(min: Some(1))])",
    )
    .expect("condition disjunction should deserialize");
    condition.validate().expect("valid condition disjunction");

    let serialized = ron::to_string(&condition).expect("serialize condition disjunction");
    let decoded: GameCondition =
        ron::from_str(&serialized).expect("deserialize condition disjunction");
    assert_eq!(decoded, condition);
}

#[test]
fn game_condition_any_of_rejects_malformed_and_invalid_branches() {
    let active = GameCondition::ActivePlayer {
        players: RelativePlayerSet::Controller,
    };
    assert!(GameCondition::AnyOf(vec![active.clone()])
        .validate()
        .is_err());
    assert!(GameCondition::AnyOf(vec![active.clone(), active.clone()])
        .validate()
        .is_err());
    assert!(GameCondition::AnyOf(vec![
        active.clone(),
        GameCondition::CreatureDeathsThisTurn {
            min: None,
            max: None,
        },
    ])
    .validate()
    .is_err());

    let triggering_spell = GameCondition::AnyOf(vec![
        active.clone(),
        GameCondition::TriggeringSpellManaSpent {
            comparison: SpellManaSpentComparison::AtLeast(1),
        },
    ]);
    assert!(triggering_spell.validate_live().is_err());

    let cast_snapshot =
        GameCondition::AnyOf(vec![active, GameCondition::CastSnapshot { index: 1 }]);
    assert!(cast_snapshot.validate_cast_snapshot_reference(1).is_err());

    let trigger_with_cast_snapshot = GameCondition::AnyOf(vec![
        GameCondition::TriggeringSpellManaSpent {
            comparison: SpellManaSpentComparison::AtLeast(1),
        },
        GameCondition::CastSnapshot { index: 0 },
    ]);
    assert!(trigger_with_cast_snapshot
        .validate_trigger_condition()
        .is_err());
}

#[test]
fn resolution_cost_supports_exact_graveyard_exile_payment() {
    let _: ResolutionCost = ron::from_str(r#"ExileGraveyardCard(filter: None)"#)
        .expect("parse an unrestricted graveyard-exile resolution cost");
    let _: ResolutionCost =
        ron::from_str(r#"ExileGraveyardCard(filter: Some((card_type: Some(InstantOrSorcery))))"#)
            .expect("parse an exact graveyard-exile resolution cost");
}

#[test]
fn resolution_cost_supports_exact_hand_card_bottom_payment() {
    let cost: ResolutionCost = ron::from_str("PutHandCardOnLibraryBottom")
        .expect("parse an exact hand-card bottom resolution cost");
    assert_eq!(cost, ResolutionCost::PutHandCardOnLibraryBottom);
}

#[test]
fn issue_241_delayed_trigger_consumes_the_exact_prior_exiled_object() {
    let effects: Vec<SpellEffectKind> = ron::from_str(
        r#"[
          Exile(subject: Chosen((kind: Creature))),
          CreateDelayedTrigger(
            subject: Some(PreviousEffectObject),
            ability: (
              ability_id: "delayed_01",
              presentation: Fallback,
              trigger: AtBeginningOfNextEndStep,
            effect: [Draw(count: 1)],
            ),
          ),
        ]"#,
    )
    .expect("generic delayed pay-or-otherwise sequence parses");

    for effect in &effects {
        effect
            .validate(EffectContext::Ability)
            .expect("each issue 241 primitive is valid in an ability");
    }
    SpellEffectKind::validate_list(&effects)
        .expect("Exile publishes one exact object for the following delayed trigger");
}

#[test]
fn delayed_trigger_player_capture_requires_the_immediately_preceding_counter() {
    let counter: super::SpellEffectKind =
        ron::from_str("CounterTargetSpell(spell_filter: (), unless_controller_pays: None)")
            .expect("counter target spell effect");
    let delayed: super::SpellEffectKind = ron::from_str(
        r#"CreateDelayedTrigger(
          subject: None,
          affected_player: Some(PreviousTargetedSpellController),
          ability: (
            ability_id: "delayed_01",
            presentation: Fallback,
            trigger: AtBeginningOfNextTurnUpkeep,
            effect: [Draw(count: 1)],
          ),
        )"#,
    )
    .expect("delayed trigger captures the target spell controller");
    assert!(super::SpellEffectKind::validate_list(&[counter, delayed.clone()]).is_ok());

    let unrelated: super::SpellEffectKind = ron::from_str("Draw(count: 1)").unwrap();
    assert!(super::SpellEffectKind::validate_list(&[unrelated, delayed]).is_err());
}

#[test]
fn turn_history_trigger_ordinals_must_be_positive() {
    assert!(TriggerCondition::WheneverPlayerCastsSpell {
        caster: CastTriggerPlayer::Controller,
        filter: crate::SpellCastFilter {
            card_type: None,
            ..Default::default()
        },
        ordinal: Some(0),
        ordinal_scope: Default::default(),
    }
    .validate()
    .is_err());
    assert!(TriggerCondition::WheneverPlayerDrawsNthCard {
        drawer: CastTriggerPlayer::Controller,
        ordinal: 0,
    }
    .validate()
    .is_err());
    assert!(TriggerCondition::WheneverPlayerDrawsNthCard {
        drawer: CastTriggerPlayer::Controller,
        ordinal: 2,
    }
    .validate()
    .is_ok());
}

#[test]
fn graveyard_aggregate_condition_requires_valid_bounds_and_round_trips() {
    let condition = |min, max| GameCondition::GraveyardAggregate {
        owners: RelativePlayerSet::Controller,
        aggregate: GraveyardAggregate::DistinctCardTypes,
        filter: None,
        min,
        max,
    };
    assert!(condition(None, None).validate().is_err());
    assert!(condition(Some(4), Some(3)).validate().is_err());

    let delirium = condition(Some(4), None);
    delirium.validate().expect("valid delirium condition");
    let serialized = ron::to_string(&delirium).expect("serialize delirium condition");
    let decoded: GameCondition =
        ron::from_str(&serialized).expect("deserialize delirium condition");
    assert_eq!(decoded, delirium);
}

#[test]
fn graveyard_aggregate_condition_validates_its_optional_filter() {
    let malformed = GameCondition::GraveyardAggregate {
        owners: RelativePlayerSet::Controller,
        aggregate: GraveyardAggregate::CardCount,
        filter: Some(ZoneCardFilter {
            any_of: Some(Vec::new()),
            ..ZoneCardFilter::default()
        }),
        min: Some(1),
        max: None,
    };
    assert!(
        malformed.validate().is_err(),
        "a graveyard condition must validate its optional ZoneCardFilter"
    );
    GameCondition::GraveyardAggregate {
        owners: RelativePlayerSet::Controller,
        aggregate: GraveyardAggregate::CardCount,
        filter: None,
        min: Some(1),
        max: None,
    }
    .validate()
    .expect("an omitted graveyard filter means any card");
}

#[test]
fn activation_conditions_require_valid_count_bounds() {
    let condition = |min, max| GameCondition::BattlefieldCreatureCount {
        filter: BattlefieldCreatureCountFilter {
            controllers: RelativePlayerSet::Controller,
            subtype: None,
            required_keywords: vec![Keyword::Flying],
            tapped: None,
            requires_any_counter: false,
            required_counter: None,
            exclude_source: false,
        },
        min,
        max,
    };
    assert!(condition(None, None).validate().is_err());
    assert!(condition(Some(2), Some(1)).validate().is_err());
    assert!(condition(Some(1), None).validate().is_ok());
}

#[test]
fn spell_cost_conditions_require_valid_counts_and_nonzero_reductions() {
    let condition = |min, max| GameCondition::BattlefieldCreatureCount {
        filter: BattlefieldCreatureCountFilter {
            controllers: RelativePlayerSet::Controller,
            subtype: None,
            required_keywords: vec![Keyword::Flying],
            tapped: None,
            requires_any_counter: false,
            required_counter: None,
            exclude_source: false,
        },
        min,
        max,
    };
    assert!(condition(None, None).validate().is_err());
    assert!(condition(Some(2), Some(1)).validate().is_err());

    let zero = SpellCostModifier::ConditionalGenericReduction {
        amount: 0,
        condition: condition(Some(1), None),
    };
    assert!(zero.validate().is_err());
    let winged_words = SpellCostModifier::ConditionalGenericReduction {
        amount: 1,
        condition: condition(Some(1), None),
    };
    assert!(winged_words.validate().is_ok());
}

#[test]
fn explicit_and_intrinsic_sorcery_speed_share_one_query() {
    let explicit = ActivatedAbilityDef {
        intrinsic_land_mana: false,
        ability_id: crate::AbilityId::new("activated_01").unwrap(),
        presentation: crate::AbilityPresentation::Fallback,
        source_zone: AbilitySourceZone::Battlefield,
        costs: vec![],
        cost_modifiers: vec![],
        effect: vec![],
        program: None,
        targeting: None,
        timing: ActivationTiming::SorcerySpeed,
        conditions: vec![],
        activation_limit: None,
    };
    assert!(explicit.requires_sorcery_speed());

    let equip = ActivatedAbilityDef {
        intrinsic_land_mana: false,
        ability_id: crate::AbilityId::new("activated_01").unwrap(),
        presentation: crate::AbilityPresentation::Fallback,
        source_zone: AbilitySourceZone::Battlefield,
        costs: vec![],
        cost_modifiers: vec![],
        effect: vec![SpellEffectKind::Equip {
            target: TargetFilter::default(),
        }],
        program: None,
        targeting: None,
        timing: ActivationTiming::Normal,
        conditions: vec![],
        activation_limit: None,
    };
    assert!(equip.requires_sorcery_speed());
}

#[test]
fn activation_limit_rejects_zero_maximum_for_every_scope() {
    assert_eq!(
        ron::from_str::<ActivationLimit>("PerObject(max_activations: 1)")
            .expect("deserialize the authored Exhaust limit"),
        ActivationLimit::PerObject { max_activations: 1 }
    );
    let ability_with = |activation_limit| ActivatedAbilityDef {
        intrinsic_land_mana: false,
        ability_id: crate::AbilityId::new("activated_01").unwrap(),
        presentation: crate::AbilityPresentation::Fallback,
        source_zone: AbilitySourceZone::Battlefield,
        costs: vec![],
        cost_modifiers: vec![],
        effect: vec![SpellEffectKind::ProduceMana {
            commander_color_identity: false,
            options: vec![ManaAmount {
                g: 1,
                ..Default::default()
            }],
            restriction: None,
            conditional: None,
        }],
        program: None,
        targeting: None,
        timing: ActivationTiming::Normal,
        conditions: vec![],
        activation_limit: Some(activation_limit),
    };
    assert!(
        ability_with(ActivationLimit::PerTurn { max_activations: 0 })
            .validate_shape()
            .is_err()
    );
    assert!(
        ability_with(ActivationLimit::PerObject { max_activations: 0 })
            .validate_shape()
            .is_err()
    );
}

#[test]
fn commander_color_identity_mana_is_a_bounded_zero_or_more_choice_ability() {
    let ability = ActivatedAbilityDef {
        intrinsic_land_mana: false,
        ability_id: crate::AbilityId::new("activated_01").unwrap(),
        presentation: crate::AbilityPresentation::Fallback,
        source_zone: AbilitySourceZone::Battlefield,
        costs: vec![AbilityCost::Tap],
        cost_modifiers: vec![],
        effect: vec![SpellEffectKind::ProduceMana {
            options: vec![],
            commander_color_identity: true,
            restriction: None,
            conditional: None,
        }],
        program: None,
        targeting: None,
        timing: ActivationTiming::Normal,
        conditions: vec![],
        activation_limit: None,
    };
    ability
        .validate_shape()
        .expect("the two named Commander cards use this tapped battlefield shape");
    assert!(ability.is_mana_ability());
    assert!(ability.mana_options().unwrap().is_empty());
    assert!(ability.commander_color_identity_mana_output().is_some());

    let mut extra_cost = ability.clone();
    extra_cost
        .costs
        .push(AbilityCost::Mana(ManaCost::parse("{1}").unwrap()));
    assert!(extra_cost.validate_shape().is_err());

    let mut extra_effect = ability;
    extra_effect.effect.push(SpellEffectKind::GainLife {
        amount: Amount::Fixed(1),
    });
    assert!(extra_effect.validate_shape().is_err());
}

fn storage_counter_split_ability(mana_cost: &str) -> ActivatedAbilityDef {
    ActivatedAbilityDef {
        intrinsic_land_mana: false,
        ability_id: AbilityId::new("activated_03").unwrap(),
        presentation: AbilityPresentation::Fallback,
        source_zone: AbilitySourceZone::Battlefield,
        costs: vec![
            AbilityCost::Mana(ManaCost::parse(mana_cost).unwrap()),
            AbilityCost::RemoveXStorageCountersFromSource,
        ],
        cost_modifiers: vec![],
        effect: vec![
            SpellEffectKind::ProduceSplitManaFromRemovedStorageCounters {
                first_color: Color::Blue,
                second_color: Color::Black,
            },
        ],
        program: None,
        targeting: None,
        timing: ActivationTiming::Normal,
        conditions: vec![],
        activation_limit: None,
    }
}

#[test]
fn storage_counter_split_mana_is_only_the_bounded_storage_land_shape() {
    let ability = storage_counter_split_ability("{1}");
    ability
        .validate_shape()
        .expect("the reviewed storage-land cost and effect shape validates");
    assert_eq!(
        ability.storage_counter_split_mana(),
        Some((Color::Blue, Color::Black))
    );
    assert!(ability.is_mana_ability());
    assert!(ability.mana_options().is_none());

    for unsupported in ["{X}", "{2}", "{1}{U}"] {
        let ability = storage_counter_split_ability(unsupported);
        assert!(
            ability.validate_shape().is_err(),
            "unsupported cost {unsupported} cannot reuse the storage choice X"
        );
        assert!(ability.storage_counter_split_mana().is_none());
    }
}

#[test]
fn storage_counter_kind_has_a_distinct_typed_label() {
    assert_eq!(
        ron::from_str::<CounterKind>("Storage").expect("Storage counter parses"),
        CounterKind::Storage
    );
    assert_eq!(CounterKind::Storage.label(), "storage");
    assert_ne!(CounterKind::Storage, CounterKind::Charge);
}

fn mana_then_controller_damage_ability() -> super::ActivatedAbilityDef {
    super::ActivatedAbilityDef {
        intrinsic_land_mana: false,
        ability_id: crate::AbilityId::new("activated_01").unwrap(),
        presentation: crate::AbilityPresentation::Fallback,
        source_zone: super::AbilitySourceZone::Battlefield,
        costs: vec![super::AbilityCost::Tap],
        cost_modifiers: vec![],
        effect: vec![
            super::SpellEffectKind::ProduceMana {
                commander_color_identity: false,
                options: vec![
                    super::ManaAmount {
                        r: 1,
                        ..Default::default()
                    },
                    super::ManaAmount {
                        g: 1,
                        ..Default::default()
                    },
                ],
                restriction: None,
                conditional: None,
            },
            super::SpellEffectKind::DamagePlayer {
                amount: super::Amount::Fixed(1),
                who: super::PlayerRecipient::Controller,
            },
        ],
        program: None,
        targeting: None,
        timing: super::ActivationTiming::Normal,
        conditions: vec![],
        activation_limit: None,
    }
}

#[test]
fn damage_mana_ability_classifier_accepts_only_the_bounded_untargeted_shape() {
    let ability = mana_then_controller_damage_ability();
    assert_eq!(
        ability
            .mana_options()
            .expect("Talisman-shaped mana ability"),
        &vec![
            super::ManaAmount {
                r: 1,
                ..Default::default()
            },
            super::ManaAmount {
                g: 1,
                ..Default::default()
            },
        ]
    );
    assert!(ability.is_mana_ability());

    let mut targeted = ability.clone();
    targeted.targeting = Some(super::TargetingDef { groups: vec![] });
    assert!(
        targeted.mana_options().is_none(),
        "mana abilities cannot target"
    );

    let mut loyalty = ability.clone();
    loyalty.costs = vec![super::AbilityCost::Loyalty(1)];
    assert!(
        loyalty.mana_options().is_none(),
        "loyalty abilities are excluded"
    );

    let mut library_moving = ability.clone();
    library_moving.effect.push(super::SpellEffectKind::Draw {
        who: super::PlayerRecipient::Controller,
        count: super::Amount::Fixed(1),
    });
    assert!(
        library_moving.mana_options().is_none(),
        "unsupported trailing library movement remains outside the fast path"
    );

    let mut wrong_recipient = ability.clone();
    wrong_recipient.effect[1] = super::SpellEffectKind::DamagePlayer {
        amount: super::Amount::Fixed(1),
        who: super::PlayerRecipient::EachOpponent,
    };
    assert!(
        wrong_recipient.mana_options().is_none(),
        "the bounded pattern only supports damage to its controller"
    );

    let mut extra_effect = ability;
    extra_effect.effect.push(super::SpellEffectKind::GainLife {
        amount: super::Amount::Fixed(1),
    });
    assert!(
        extra_effect.mana_options().is_none(),
        "unsupported trailing effects remain outside the fast path"
    );
}

fn source_counter_scaled_mana_effect() -> SpellEffectKind {
    SpellEffectKind::ProduceManaPerSourceCounter {
        counter: CounterKind::Charge,
        options: vec![ManaAmount {
            g: 1,
            ..Default::default()
        }],
    }
}

fn opponent_land_mana_effect() -> SpellEffectKind {
    SpellEffectKind::ProduceManaFromOpponentLands { options: vec![] }
}

#[test]
fn opponent_land_mana_is_restricted_to_the_tap_activated_ability_shape() {
    let ability = ActivatedAbilityDef {
        intrinsic_land_mana: false,
        ability_id: crate::AbilityId::new("activated_01").unwrap(),
        presentation: crate::AbilityPresentation::Fallback,
        source_zone: AbilitySourceZone::Battlefield,
        costs: vec![AbilityCost::Tap],
        cost_modifiers: vec![],
        effect: vec![opponent_land_mana_effect()],
        program: None,
        targeting: None,
        timing: ActivationTiming::Normal,
        conditions: vec![],
        activation_limit: None,
    };
    ability
        .validate_shape()
        .expect("Orchard's dynamic output is a sole tap battlefield ability");
    assert!(ability.is_mana_ability());
    assert!(ability.opponent_land_mana_output());
    assert!(ability.mana_options().unwrap().is_empty());

    let mut extra_cost = ability.clone();
    extra_cost
        .costs
        .push(AbilityCost::Mana(ManaCost::parse("{1}").unwrap()));
    assert!(extra_cost.validate_shape().is_err());

    let mut nested = ability.clone();
    nested.effect = vec![SpellEffectKind::Conditional {
        condition: GameCondition::ActivePlayer {
            players: RelativePlayerSet::Controller,
        },
        effect: Box::new(opponent_land_mana_effect()),
    }];
    assert!(nested.validate_shape().is_err());

    let triggered = TriggeredAbilityDef {
        ability_id: crate::AbilityId::new("triggered_01").unwrap(),
        presentation: crate::AbilityPresentation::Fallback,
        trigger: TriggerCondition::WhenSelfEntersBattlefield,
        effect: vec![opponent_land_mana_effect()],
        program: None,
        modal: None,
        targeting: None,
        may: false,
        intervening_if: None,
        triggers_only_once: false,
        max_triggers_per_turn: None,
    };
    assert!(triggered.validate_shape().is_err());
}

#[test]
fn source_counter_scaled_mana_accepts_the_supported_activated_ability_shape() {
    let ability = ActivatedAbilityDef {
        intrinsic_land_mana: false,
        ability_id: crate::AbilityId::new("activated_01").unwrap(),
        presentation: crate::AbilityPresentation::Fallback,
        source_zone: AbilitySourceZone::Battlefield,
        costs: vec![AbilityCost::Tap],
        cost_modifiers: vec![],
        effect: vec![source_counter_scaled_mana_effect()],
        program: None,
        targeting: None,
        timing: ActivationTiming::Normal,
        conditions: vec![],
        activation_limit: None,
    };

    ability
        .validate_shape()
        .expect("a tapped battlefield source can produce one mana per counter");
    assert!(ability.mana_options().is_some());
    assert_eq!(ability.mana_source_counter(), Some(CounterKind::Charge));
}

#[test]
fn source_counter_scaled_mana_rejects_mixed_activated_effects() {
    let ability = ActivatedAbilityDef {
        intrinsic_land_mana: false,
        ability_id: crate::AbilityId::new("activated_01").unwrap(),
        presentation: crate::AbilityPresentation::Fallback,
        source_zone: AbilitySourceZone::Battlefield,
        costs: vec![AbilityCost::Tap],
        cost_modifiers: vec![],
        effect: vec![
            source_counter_scaled_mana_effect(),
            SpellEffectKind::GainLife {
                amount: Amount::Fixed(1),
            },
        ],
        program: None,
        targeting: None,
        timing: ActivationTiming::Normal,
        conditions: vec![],
        activation_limit: None,
    };

    assert!(ability
        .validate_shape()
        .expect_err("the special mana effect cannot be mixed with another effect")
        .contains("sole direct effect"));
}

#[test]
fn source_counter_scaled_mana_rejects_nested_activated_effects() {
    let ability = ActivatedAbilityDef {
        intrinsic_land_mana: false,
        ability_id: crate::AbilityId::new("activated_01").unwrap(),
        presentation: crate::AbilityPresentation::Fallback,
        source_zone: AbilitySourceZone::Battlefield,
        costs: vec![AbilityCost::Tap],
        cost_modifiers: vec![],
        effect: vec![SpellEffectKind::Conditional {
            condition: GameCondition::ActivePlayer {
                players: RelativePlayerSet::Controller,
            },
            effect: Box::new(source_counter_scaled_mana_effect()),
        }],
        program: None,
        targeting: None,
        timing: ActivationTiming::Normal,
        conditions: vec![],
        activation_limit: None,
    };

    assert!(ability
        .validate_shape()
        .expect_err("a nested special mana effect has no supported resolver")
        .contains("sole direct effect"));
}

#[test]
fn source_counter_scaled_mana_rejects_triggered_and_reflexive_appearances() {
    let triggered = TriggeredAbilityDef {
        ability_id: crate::AbilityId::new("triggered_01").unwrap(),
        presentation: crate::AbilityPresentation::Fallback,
        trigger: TriggerCondition::WhenSelfEntersBattlefield,
        effect: vec![source_counter_scaled_mana_effect()],
        program: None,
        modal: None,
        targeting: None,
        may: false,
        intervening_if: None,
        triggers_only_once: false,
        max_triggers_per_turn: None,
    };
    assert!(triggered
        .validate_shape()
        .expect_err("triggered abilities cannot use source-counter-scaled mana")
        .contains("source-counter-scaled mana"));

    let reflexive = ReflexiveTriggeredAbilityDef {
        ability_id: crate::AbilityId::new("reflexive_01").unwrap(),
        presentation: crate::AbilityPresentation::Fallback,
        effect: vec![source_counter_scaled_mana_effect()],
        targeting: None,
        intervening_if: None,
    };
    assert!(reflexive
        .validate_shape()
        .expect_err("reflexive abilities cannot use source-counter-scaled mana")
        .contains("source-counter-scaled mana"));
}

#[test]
fn damage_targets_rejects_conditional_amounts() {
    let effect = SpellEffectKind::DamageTargets {
        amount: Amount::Conditional {
            condition: GameCondition::CreatureDeathsThisTurn {
                min: Some(1),
                max: None,
            },
            when_true: 5,
            otherwise: 3,
        },
        target: TargetFilter::default(),
        division: DamageDivision::ChooseAtCast,
        extra_mana_per_target: 0,
    };
    assert!(effect.validate(EffectContext::Spell).is_err());
}

#[test]
fn previous_result_amount_requires_an_immediately_preceding_compatible_effect() {
    let counted_gain = SpellEffectKind::GainLife {
        amount: Amount::Count(CountExpression::CardsMatchingResult {
            filter: CardResultFilter {
                source: CardResultSource::PreviousEffect,
                action: CardResultAction::Mill,
                players: RelativePlayerSet::Controller,
                card_type: Some(CardTypeFilter::Creature),
            },
        }),
    };
    let mill = SpellEffectKind::Mill {
        count: Amount::Fixed(4),
        who: PlayerRecipient::Controller,
    };

    assert!(SpellEffectKind::validate_list(&[mill.clone(), counted_gain.clone()]).is_ok());
    assert!(SpellEffectKind::validate_list(std::slice::from_ref(&counted_gain)).is_err());
    assert!(SpellEffectKind::validate_list(&[
        mill,
        SpellEffectKind::Draw {
            who: PlayerRecipient::Controller,
            count: Amount::Fixed(1),
        },
        counted_gain,
    ])
    .is_err());
}

#[test]
fn player_effect_accepts_player_spec() {
    assert!(SpellEffectKind::TargetPlayerLosesLife {
        amount: 3,
        target: TargetFilter {
            kind: TargetKind::OpponentPlayer,
            ..Default::default()
        },
    }
    .validate(EffectContext::Spell)
    .is_ok());
}

#[test]
fn player_effect_rejects_nonplayer_spec() {
    assert!(SpellEffectKind::TargetPlayerGainsLife {
        amount: 3.into(),
        target: TargetFilter {
            kind: TargetKind::Creature,
            ..Default::default()
        },
    }
    .validate(EffectContext::Spell)
    .is_err());
}

#[test]
fn damage_accepts_any_kind() {
    for kind in [
        TargetKind::AnyTarget,
        TargetKind::Creature,
        TargetKind::AnyPlayer,
        TargetKind::OpponentPlayer,
    ] {
        assert!(SpellEffectKind::DamageTarget {
            amount: Amount::Fixed(3),
            target: TargetFilter {
                kind,
                ..Default::default()
            },
        }
        .validate(EffectContext::Spell)
        .is_ok());
    }
}

/// Issue #39: untargeted mass selection has no activating player, so a controller relation can
/// never be honored there — the effect's own `players` scope is the correct knob. Reject the
/// filter at load rather than silently ignoring the field.
#[test]
fn mass_effect_rejects_controller_relative_filter() {
    let scoped = TargetFilter {
        kind: TargetKind::Creature,
        controller: TargetController::You,
        ..Default::default()
    };
    assert!(SpellEffectKind::DestroyAll {
        kind: scoped.clone(),
        prevent_regeneration: false,
    }
    .validate(EffectContext::Spell)
    .is_err());
    assert!(SpellEffectKind::DamageAll {
        amount: Amount::Fixed(2),
        players: MassPlayerSet::All,
        kind: scoped,
    }
    .validate(EffectContext::Spell)
    .is_err());

    // The same effect without the controller restriction still loads.
    assert!(SpellEffectKind::DestroyAll {
        kind: TargetFilter {
            kind: TargetKind::Creature,
            ..Default::default()
        },
        prevent_regeneration: true,
    }
    .validate(EffectContext::Spell)
    .is_ok());
}

#[test]
fn source_subject_rejected_in_spell_context_allowed_in_ability() {
    let pump_self = SpellEffectKind::PumpTarget {
        power: 1,
        toughness: 1,
        scale: None,
        subject: EffectSubject::Source,
    };
    assert!(pump_self.validate(EffectContext::Spell).is_err());
    assert!(pump_self.validate(EffectContext::Ability).is_ok());

    let fight_from_source = SpellEffectKind::Fight {
        first: EffectSubject::Source,
        second: EffectSubject::Chosen(Box::new(TargetFilter {
            kind: TargetKind::Creature,
            controller: TargetController::NotYou,
            ..Default::default()
        })),
    };
    assert_eq!(fight_from_source.target_filters().len(), 1);
    assert!(fight_from_source.validate(EffectContext::Spell).is_err());
    assert!(fight_from_source.validate(EffectContext::Ability).is_ok());

    let invalid_fight = SpellEffectKind::Fight {
        first: EffectSubject::Source,
        second: EffectSubject::Chosen(Box::new(TargetFilter {
            kind: TargetKind::AnyPermanent,
            ..Default::default()
        })),
    };
    assert!(invalid_fight.validate(EffectContext::Ability).is_err());
}

#[test]
fn combat_restriction_scopes_validate_by_context_and_subject_kind() {
    let cant_block = CombatRestriction {
        cant_block: true,
        ..Default::default()
    };
    let source = SpellEffectKind::ApplyCombatRestriction {
        scope: CombatRestrictionScope::Source,
        restriction: cant_block.clone(),
    };
    assert!(source.validate(EffectContext::Spell).is_err());
    assert!(source.validate(EffectContext::Ability).is_ok());

    let chosen_player = SpellEffectKind::ApplyCombatRestriction {
        scope: CombatRestrictionScope::Chosen(TargetFilter {
            kind: TargetKind::OpponentPlayer,
            ..Default::default()
        }),
        restriction: cant_block.clone(),
    };
    assert!(chosen_player.validate(EffectContext::Spell).is_err());

    let matching_creatures = SpellEffectKind::ApplyCombatRestriction {
        scope: CombatRestrictionScope::Matching(TargetFilter {
            kind: TargetKind::Creature,
            excluded_keywords: vec![Keyword::Flying],
            ..Default::default()
        }),
        restriction: cant_block.clone(),
    };
    assert!(matching_creatures.validate(EffectContext::Spell).is_ok());

    let matching_permanents = SpellEffectKind::ApplyCombatRestriction {
        scope: CombatRestrictionScope::Matching(TargetFilter {
            kind: TargetKind::AnyPermanent,
            ..Default::default()
        }),
        restriction: cant_block.clone(),
    };
    assert!(matching_permanents.validate(EffectContext::Spell).is_err());

    let empty = SpellEffectKind::ApplyCombatRestriction {
        scope: CombatRestrictionScope::Chosen(TargetFilter::default_creature()),
        restriction: CombatRestriction::default(),
    };
    assert!(empty.validate(EffectContext::Spell).is_err());
}

#[test]
fn return_to_owners_hand_uses_a_validated_permanent_subject() {
    let opponent_creature = SpellEffectKind::ReturnToOwnersHand {
        subject: EffectSubject::Chosen(Box::new(TargetFilter {
            kind: TargetKind::Creature,
            controller: TargetController::Opponent,
            ..Default::default()
        })),
    };
    assert_eq!(opponent_creature.target_filters().len(), 1);
    assert!(opponent_creature.validate(EffectContext::Ability).is_ok());

    let player = SpellEffectKind::ReturnToOwnersHand {
        subject: EffectSubject::Chosen(Box::new(TargetFilter {
            kind: TargetKind::OpponentPlayer,
            ..Default::default()
        })),
    };
    assert!(player.validate(EffectContext::Ability).is_err());

    let source = SpellEffectKind::ReturnToOwnersHand {
        subject: EffectSubject::Source,
    };
    assert!(source.validate(EffectContext::Ability).is_ok());
    assert!(source.validate(EffectContext::Spell).is_err());
}

#[test]
fn multi_permanent_library_shuffle_has_general_but_bounded_subjects() {
    let valid = SpellEffectKind::ShufflePermanentsIntoOwnersLibraries {
        subjects: vec![
            EffectSubject::Source,
            EffectSubject::Chosen(Box::new(TargetFilter {
                kind: TargetKind::Creature,
                required_counter: Some(CounterKind::Stun),
                ..Default::default()
            })),
        ],
    };
    assert_eq!(valid.target_filters().len(), 1);
    assert!(valid.validate(EffectContext::Ability).is_ok());
    assert!(valid.validate(EffectContext::Spell).is_err());

    let too_few = SpellEffectKind::ShufflePermanentsIntoOwnersLibraries {
        subjects: vec![EffectSubject::Source],
    };
    assert!(too_few.validate(EffectContext::Ability).is_err());

    let player_subject = SpellEffectKind::ShufflePermanentsIntoOwnersLibraries {
        subjects: vec![
            EffectSubject::Source,
            EffectSubject::Chosen(Box::new(TargetFilter {
                kind: TargetKind::OpponentPlayer,
                ..Default::default()
            })),
        ],
    };
    assert!(player_subject.validate(EffectContext::Ability).is_err());

    let unsupported = SpellEffectKind::ShufflePermanentsIntoOwnersLibraries {
        subjects: vec![EffectSubject::Source, EffectSubject::AttachedObject],
    };
    assert!(unsupported.validate(EffectContext::Ability).is_err());
}

#[test]
fn chosen_subject_defaults_to_creature_target() {
    assert_eq!(
        EffectSubject::default(),
        EffectSubject::Chosen(Box::new(TargetFilter::default_creature()))
    );
}

#[test]
fn target_controller_relation_is_a_composable_filter_dimension() {
    let filter = TargetFilter {
        kind: TargetKind::Creature,
        controller: TargetController::Opponent,
        tapped: Some(true),
        excluded_permanent_types: vec![PermanentTypeFilter::Artifact],
        ..TargetFilter::default()
    };

    assert_eq!(filter.controller, TargetController::Opponent);
    assert_eq!(filter.tapped, Some(true));
    assert_eq!(
        filter.excluded_permanent_types,
        [PermanentTypeFilter::Artifact]
    );
    assert_eq!(TargetFilter::default().controller, TargetController::Any);
}

#[test]
fn controller_relative_target_filter_requires_a_permanent_kind() {
    for kind in [
        TargetKind::AnyTarget,
        TargetKind::AnyPlayer,
        TargetKind::OpponentPlayer,
    ] {
        let effect = SpellEffectKind::DamageTarget {
            amount: Amount::Fixed(1),
            target: TargetFilter {
                kind,
                controller: TargetController::Opponent,
                ..TargetFilter::default()
            },
        };
        assert!(effect.validate(EffectContext::Spell).is_err());
    }
}

#[test]
fn source_exclusion_requires_an_object_capable_target_kind() {
    for kind in [TargetKind::AnyPlayer, TargetKind::OpponentPlayer] {
        let effect = SpellEffectKind::TargetPlayerGainsLife {
            amount: 1.into(),
            target: TargetFilter {
                kind,
                excluded_objects: vec![crate::TargetObjectExclusion::Source],
                ..TargetFilter::default()
            },
        };
        assert!(effect.validate(EffectContext::Spell).is_err());
    }

    let object_capable = SpellEffectKind::DamageTarget {
        amount: Amount::Fixed(1),
        target: TargetFilter {
            kind: TargetKind::AnyTarget,
            excluded_objects: vec![crate::TargetObjectExclusion::Source],
            ..TargetFilter::default()
        },
    };
    assert!(object_capable.validate(EffectContext::Ability).is_ok());
}

#[test]
fn untargeted_filters_reject_source_exclusion() {
    let filter = TargetFilter {
        kind: TargetKind::Creature,
        excluded_objects: vec![crate::TargetObjectExclusion::Source],
        ..TargetFilter::default()
    };
    assert!(SpellEffectKind::DestroyAll {
        kind: filter.clone(),
        prevent_regeneration: false,
    }
    .validate(EffectContext::Spell)
    .is_err());
    assert!(SpellEffectKind::GrantKeywordsAllPermanents {
        filter,
        keywords: vec![Keyword::Flying],
    }
    .validate(EffectContext::Ability)
    .is_err());
}

#[test]
fn issue_327_put_counters_all_is_an_untargeted_mass_counter() {
    let valid = SpellEffectKind::PutCountersAll {
        counter: CounterKind::PlusOnePlusOne,
        count: Amount::Fixed(1),
        filter: CreatureScopeFilter {
            controller: Some(CreatureScopeController::YouControl),
            exclude_self: true,
            ..CreatureScopeFilter::default()
        },
    };
    assert!(
        valid.target_filters().is_empty(),
        "PutCountersAll is the untargeted mass sibling of PutCounters"
    );
    assert!(valid.validate(EffectContext::Spell).is_ok());
    assert!(valid.validate(EffectContext::Ability).is_ok());

    let conflicting_scope = SpellEffectKind::PutCountersAll {
        counter: CounterKind::PlusOnePlusOne,
        count: Amount::Fixed(1),
        filter: CreatureScopeFilter {
            required_counter: Some(CounterKind::Stun),
            requires_any_counter: true,
            ..CreatureScopeFilter::default()
        },
    };
    assert!(
        conflicting_scope.validate(EffectContext::Spell).is_err(),
        "any-counter and named-counter scope requirements cannot combine"
    );
}

#[test]
fn untargeted_filter_preserves_you_scope_but_defers_opponent_scope() {
    let keywords = vec![Keyword::Indestructible];
    let filter = |controller| TargetFilter {
        kind: TargetKind::AnyPermanent,
        controller,
        ..TargetFilter::default()
    };

    assert!(SpellEffectKind::GrantKeywordsAllPermanents {
        filter: filter(TargetController::You),
        keywords: keywords.clone(),
    }
    .validate(EffectContext::Spell)
    .is_ok());
    assert!(SpellEffectKind::GrantKeywordsAllPermanents {
        filter: filter(TargetController::Opponent),
        keywords,
    }
    .validate(EffectContext::Spell)
    .is_err());
    assert!(SpellEffectKind::TargetPlayerSacrifices {
        target: TargetFilter {
            kind: TargetKind::OpponentPlayer,
            ..TargetFilter::default()
        },
        filter: TargetFilter {
            kind: TargetKind::Creature,
            controller: TargetController::You,
            ..TargetFilter::default()
        },
    }
    .validate(EffectContext::Spell)
    .is_err());
}

#[test]
fn issue_206_explore_primitive_parses_targeted_and_source_forms() {
    for authored in [
        "Explore(subject: Chosen((kind: Creature, controller: You)))",
        "Explore(subject: Source)",
    ] {
        let parsed = ron::from_str::<SpellEffectKind>(authored);
        assert!(
            parsed.is_ok(),
            "Explore must be an authored rules primitive: {parsed:?}"
        );
    }
}

#[test]
fn issue_206_explore_rejects_unsupported_subjects_and_source_bound_spells() {
    for subject in [
        EffectSubject::AttachedObject,
        EffectSubject::TriggerObject,
        EffectSubject::PreviousEffectObject,
        EffectSubject::SearchedObject(crate::SearchResultId::new("search_01").unwrap()),
    ] {
        let error = SpellEffectKind::Explore { subject }
            .validate(EffectContext::Ability)
            .expect_err("unsupported Explore subject");
        assert!(error.contains("Source or Chosen"));
    }
    assert!(SpellEffectKind::Explore {
        subject: EffectSubject::Source,
    }
    .validate(EffectContext::Spell)
    .is_err());
}

#[test]
fn issue_479_opponent_land_comparison_condition_is_authored() {
    let condition = ron::from_str::<GameCondition>("OpponentHasMoreThanYou(metric: LandCount)");
    assert!(
        condition.is_ok(),
        "issue #479 needs a typed condition for an individual opponent's land count: {condition:?}"
    );
}

#[test]
fn issue_207_stack_ability_and_source_linked_ability_loss_primitives_parse() {
    for authored in [
        "CounterTargetAbility",
        "RemoveAllAbilities(subject: Chosen((kind: Creature)), duration: WhileSourceOnBattlefield)",
    ] {
        let parsed = ron::from_str::<SpellEffectKind>(authored);
        assert!(
            parsed.is_ok(),
            "issue #207 requires a reusable authored primitive: {authored}: {parsed:?}"
        );
    }

    let condition = ron::from_str::<GameCondition>(
        "ObjectMatches(object: PreviousEffectObject, filter: (kind: AnyPermanent, permanent_types: [Artifact, Creature, Planeswalker]))",
    );
    assert!(
        condition.is_ok(),
        "issue #207 requires generation-bound previous-object conditions: {condition:?}"
    );

    let tidebinder: Vec<SpellEffectKind> = ron::from_str(
        r#"[
            CounterTargetAbility,
            Conditional(
                condition: ObjectMatches(
                    object: PreviousEffectObject,
                    filter: (kind: AnyPermanent, permanent_types: [Artifact, Creature, Planeswalker]),
                ),
                effect: RemoveAllAbilities(
                    subject: PreviousEffectObject,
                    duration: WhileSourceOnBattlefield,
                ),
            ),
        ]"#,
    )
    .expect("Tidebinder effect sequence parses");
    SpellEffectKind::validate_list(&tidebinder)
        .expect("a successful ability counter is a compatible object producer");

    let orphan = tidebinder[1..].to_vec();
    assert!(SpellEffectKind::validate_list(&orphan).is_err());

    let remove = ron::from_str::<SpellEffectKind>(
        "RemoveAllAbilities(subject: Chosen((kind: Creature)), duration: WhileSourceOnBattlefield)",
    )
    .expect("source-linked removal parses");
    assert!(remove.validate(EffectContext::Spell).is_err());
    assert!(remove.validate(EffectContext::Ability).is_ok());
}
#[test]
fn coveted_jewel_control_change_requires_its_post_blocker_player_attack_group() {
    let ron = r#"(ability_id: "triggered_01", presentation: Fallback,
        trigger: WheneverOpponentAttackGroup(timing: AfterBlockersDeclared, destination: Controller),
        effect: [GiveControlOfSourceToAttackingPlayer])"#;
    let mut ability: super::TriggeredAbilityDef =
        ron::from_str(ron).expect("Coveted Jewel trigger parses");
    ability
        .validate_shape()
        .expect("the event-time attacker control effect has its required trigger context");
    assert!(super::SpellEffectKind::GiveControlOfSourceToAttackingPlayer
        .validate(super::EffectContext::Spell)
        .is_err());

    let activated: super::ActivatedAbilityDef = ron::from_str(
        r#"(ability_id: "activated_01", presentation: Fallback, costs: [Tap],
            effect: [GiveControlOfSourceToAttackingPlayer])"#,
    )
    .expect("activated ability with the trigger-only effect parses for validation");
    let activated_error = activated.validate_shape().expect_err(
        "source control by an attacking player must not be admitted on an activated ability",
    );
    assert!(
        activated_error.contains("post-blocker player attack-group trigger"),
        "unexpected activated source-control validation error: {activated_error}"
    );

    ability.trigger = super::TriggerCondition::WheneverSelfAttacks {
        minimum_other_attackers: 0,
    };
    assert!(ability
        .validate_shape()
        .expect_err("source control must not run without the opponent's unblocked attack event")
        .contains("post-blocker player attack-group trigger"));
}

#[test]
fn triggering_attacker_recipient_is_draw_only_and_requires_coveted_jewels_trigger() {
    let mut ability: super::TriggeredAbilityDef = ron::from_str(
        r#"(ability_id: "triggered_01", presentation: Fallback,
            trigger: WheneverOpponentAttackGroup(timing: AfterBlockersDeclared, destination: Controller),
            effect: [Draw(who: TriggeringAttackingPlayer, count: 3)])"#,
    )
    .expect("event-time attacker draw parses");
    ability
        .validate_shape()
        .expect("the post-blocker trigger supplies the event-time attacker");

    ability.trigger = super::TriggerCondition::WheneverAttachedPlayerIsAttacked;
    assert!(ability
        .validate_shape()
        .expect_err("an unrelated trigger cannot use Coveted Jewel's attacker recipient")
        .contains("post-blocker player attack-group trigger"));

    let activated: super::ActivatedAbilityDef = ron::from_str(
        r#"(ability_id: "activated_01", presentation: Fallback, costs: [Tap],
            effect: [Draw(who: TriggeringAttackingPlayer, count: 1)])"#,
    )
    .expect("activated ability with attacker recipient parses for validation");
    assert!(
        activated.validate_shape().is_err(),
        "an activated ability cannot use a recipient that requires a trigger's defender context"
    );

    let invalid_damage = super::SpellEffectKind::DamagePlayer {
        amount: super::Amount::Fixed(1),
        who: super::PlayerRecipient::TriggeringAttackingPlayer,
    };
    assert!(invalid_damage
        .validate(super::EffectContext::Ability)
        .expect_err("the event-time attacker recipient is implemented only for Draw")
        .contains("supported only by Draw"));
}

#[test]
fn simple_fallbacks_cover_synthetic_choices_and_keep_unknown_instructions_whole() {
    let branch: super::ResolutionBranchDef = ron::from_str(
        r#"(branch_id: "continue", presentation: Fallback, cost: None, effects: [])"#,
    )
    .unwrap();
    assert_eq!(branch.fallback_label(), "Continue");
    let ability: super::ActivatedAbilityDef = ron::from_str(
        r#"(ability_id: "activated_01", presentation: Fallback, costs: [Tap], effect: [ProduceMana(options: [(w: 1), (u: 1), (b: 1), (r: 1), (g: 1)])])"#,
    ).unwrap();
    assert_eq!(
        ability.fallback_text("Birds"),
        "{T}: Add one mana of any color."
    );
    let ability: super::TriggeredAbilityDef = ron::from_str(
        r#"(ability_id: "triggered_01", presentation: Fallback, trigger: WheneverSelfAttacks(minimum_other_attackers: 0), effect: [AddMana(amount: (r: 2), retention: EndOfCombat)])"#,
    ).unwrap();
    assert_eq!(
        ability.fallback_text("Firebender"),
        "Whenever Firebender attacks, add {R}{R}. This mana lasts until end of combat."
    );
    let unknown: super::ActivatedAbilityDef = ron::from_str(
        r#"(ability_id: "activated_01", presentation: Fallback, costs: [Tap], effect: [Draw(count: 1), ManifestDread])"#,
    ).unwrap();
    assert_eq!(
        unknown.fallback_text("Source"),
        "Source — activated ability (activated_01)"
    );
}

#[test]
fn issue_197_hand_choice_requires_an_explicit_typed_action() {
    for action in ["Discard", "Exile"] {
        let text =
            format!("ChooseHandCards(action: {action}, count: 1, target: (kind: AnyPlayer))");
        let effect = ron::from_str::<super::SpellEffectKind>(&text)
            .expect("shared hand choice action must deserialize");
        assert!(effect.validate(super::EffectContext::Spell).is_ok());
    }
    for text in [
        "ChooseHandCards(count: 1, target: (kind: AnyPlayer))",
        "DiscardCards(count: 1, target: (kind: AnyPlayer))",
        "ExileCardsFromHand(count: 1, target: (kind: AnyPlayer))",
    ] {
        assert!(ron::from_str::<super::SpellEffectKind>(text).is_err());
    }
}

#[test]
fn exile_graveyards_is_untargeted_and_validates_printed_card_filters() {
    let card = r#"(id: "test", name: "Test", face_id: "test", mana_cost: "{1}", types: ["Sorcery"], spell_effect: [ExileGraveyards(players: Opponents)])"#;
    let registry = crate::CardRegistry::from_chunks_and_tokens(&[card], &[])
        .expect("untargeted graveyard cohort is valid");
    let effect = &registry.get("test").unwrap().primary_face().spell_effect[0];
    assert!(effect.target_roles().is_empty());
    let invalid = card.replace(
        "players: Opponents",
        "players: All, filter: Some((min_mana_value: Some(4), max_mana_value: Some(2)))",
    );
    assert!(crate::CardRegistry::from_chunks_and_tokens(&[&invalid], &[]).is_err());
}
#[test]
fn issue_227_discard_quantity_accepts_whole_hand() {
    let effect = ron::from_str::<super::SpellEffectKind>("Discard(who: Controller, quantity: All)");
    assert!(
        effect.is_ok(),
        "whole-hand discard must be expressible: {effect:?}"
    );
    let definition = |quantity| {
        format!(
            r#"(id: "test", name: "Test", face_id: "test", mana_cost: "{{R}}", types: ["Sorcery"], spell_effect: [Discard(quantity: {quantity})])"#,
        )
    };
    for quantity in ["All", "Exact(1)", "Exact(300)"] {
        let card = definition(quantity);
        let registry = crate::CardRegistry::from_chunks_and_tokens(&[&card], &[]).unwrap();
        assert!(registry.get("test").unwrap().primary_face().spell_effect[0]
            .target_roles()
            .is_empty());
    }
    assert!(crate::CardRegistry::from_chunks_and_tokens(&[&definition("Exact(0)")], &[]).is_err());
    assert!(ron::from_str::<super::SpellEffectKind>("Discard(count: 1)").is_err());
}

#[test]
fn boseiju_counted_activation_reduction_validates_cost_time_predicates() {
    let modifier = |amount: &str| {
        ron::from_str::<super::ActivatedCostModifier>(&format!(
            "GenericReduction(amount: {amount})"
        ))
        .expect("counted activation reduction must be expressible")
    };
    modifier("Count(BattlefieldPermanents(filter: (controllers: Controller, card_type: Some(Creature), required_supertypes: [\"Legendary\"])))")
        .validate()
        .expect("Boseiju and Otawara count derived legendary creatures");
    for invalid in [
        "0",
        "\"X\"",
        "EventCount",
        "Count(SourcePower)",
        "Count(CardsMatchingResult(filter: (source: PreviousEffect, action: Discard, players: Controller)))",
        "Count(BattlefieldPermanents(filter: (controllers: Controller, required_supertypes: [\"\"])))",
        "Count(BattlefieldPermanents(filter: (controllers: Controller, required_supertypes: [\"Legendary\", \"Legendary\"])))",
    ] {
        assert!(modifier(invalid).validate().is_err(), "accepted {invalid}");
    }
}

#[test]
fn windfall_maximum_card_result_roundtrip_and_context_boundaries() {
    let source = "MaximumCardsMatchingResult(filter: (source: PreviousEffect, action: Discard, players: All, card_type: None))";
    let expression: super::CountExpression = ron::from_str(source).unwrap();
    assert!(expression.validate().is_ok());
    let encoded = ron::ser::to_string(&expression).unwrap();
    assert_eq!(
        ron::from_str::<super::CountExpression>(&encoded).unwrap(),
        expression
    );
    assert!(expression.validate_static_count().is_err());
    assert!(super::Amount::Count(expression)
        .validate_cost(true)
        .is_err());
    let affine: super::CountExpression = ron::from_str(&format!(
        "Affine(terms: [(coefficient: 1, quantity: {source})])"
    ))
    .unwrap();
    assert!(affine.validate().is_err());
    for preceding in ["", "Draw(count: 1),"] {
        let card = format!(
            r#"(id: "bad_wheel", name: "Bad Wheel", face_id: "bad_wheel", types: ["Sorcery"], spell_effect: [{preceding} Draw(who: EachPlayer, count: Count({source}))])"#
        );
        let error = crate::CardRegistry::from_chunks_and_tokens(&[&card], &[]).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("immediately preceding compatible"),
            "{error}"
        );
    }
    let entry = format!(
        r#"(id: "bad_entry", name: "Bad Entry", face_id: "bad_entry", types: ["Artifact"], static_abilities: [(ability_id: "entry", presentation: Fallback, definition: EntersWithCounters(counter: Charge, amount: Count({source})))])"#
    );
    let error = crate::CardRegistry::from_chunks_and_tokens(&[&entry], &[]).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("card result counts are valid only in a resolving effect list"),
        "{error}"
    );
}

#[test]
fn portal_to_phyrexia_effects_are_representable() {
    let portal = r#"(
        id: "portal_to_phyrexia",
        name: "Portal to Phyrexia",
        face_id: "portal_to_phyrexia",
        mana_cost: "{9}",
        types: ["Artifact"],
        triggered_abilities: [(
            ability_id: "etb",
            presentation: OracleLines([1]),
            trigger: WhenSelfEntersBattlefield,
            effect: [ChooseAndSacrifice(
                players: EachOpponent,
                count: 3,
                filter: (kind: AnyPermanent, permanent_types: [Creature]),
            )],
        ), (
            ability_id: "upkeep",
            presentation: OracleLines([2]),
            trigger: AtBeginningOfUpkeep(player: Controller),
            effect: [MoveGraveyardCards(
                filter: (owner: AnyPlayer, card: Some((card_type: Some(Creature)))),
                destination: Battlefield(tapped: false),
                entry_modifiers: [AddTypes((creature_types: ["Phyrexian"]))],
            )],
            targeting: Some((groups: [(
                min: 1,
                max: 1,
                prompt: "Choose target creature card from a graveyard",
                effect_indices: [0],
            )])),
        )],
    )"#;

    crate::CardRegistry::from_chunks_and_tokens(&[portal], &[])
        .expect("Portal's simultaneous counted sacrifice and modified return must be typed");
}

#[test]
fn chimil_discover_and_controller_spell_protection_are_typed() {
    assert!(
        ron::from_str::<super::StaticAbilityDef>(
            "ControllerSpellsCannotBeCountered(spell_filter:())"
        )
        .is_ok(),
        "Chimil and Rhythm of the Wild need controller-scoped counter protection"
    );
    assert!(
        ron::from_str::<super::SpellEffectKind>("Discover(value:5)").is_ok(),
        "Chimil and Geological Appraiser need the Discover effect"
    );
}

fn program_fixture(body: &str) -> String {
    format!(
        r#"(id: "program_fixture", name: "Program Fixture", face_id: "program_fixture",
            mana_cost: "{{0}}", types: ["Instant"],
            spell_program: Some((body: {body})))"#
    )
}

#[test]
fn legacy_flat_resolution_branch_ron_is_rejected() {
    let legacy = r#"ChooseResolutionBranch(
        chooser: Controller,
        optional: false,
        selection: FirstApplicable,
        branches: [(
            branch_id: "branch_01",
            presentation: Fallback,
            cost: None,
            requirement: Always,
            effects: [GainLife(amount: 1)],
        )],
        otherwise: [],
    )"#;

    assert!(
        ron::from_str::<super::SpellEffectKind>(legacy).is_err(),
        "the retired flat branch syntax must no longer deserialize"
    );
}

#[test]
fn typed_program_choose_remains_admitted_after_flat_branch_retirement() {
    let definition = program_fixture(
        r#"(steps: [
            (id: "choose_draw", kind: Choose(
                choice: "choose_draw", chooser: Controller, optional: false,
                branches: [(
                    branch_id: "draw", presentation: Fallback,
                    cost: None, requirement: Always,
                    body: (steps: [
                        (id: "draw", kind: Perform(Draw(who: Controller, count: 1))),
                    ], yields: []),
                )],
                otherwise: (steps: [], yields: []),
            )),
        ], yields: [])"#,
    );
    let registry = crate::CardRegistry::from_chunks_and_tokens(&[&definition], &[])
        .expect("the typed Program Choose boundary remains supported");
    let owner = crate::ProgramOwnerId::face(
        "program_fixture",
        crate::CardFaceId::new("program_fixture").unwrap(),
    );
    assert!(
        registry.compiled_program_for_owner(&owner).is_some(),
        "the exact Program Choose owner compiles"
    );
}

fn ability_program_fixture(
    card_id: &str,
    body: &str,
) -> Result<crate::primitives::CompiledProgram, String> {
    let definition = format!(
        r#"(id: "{card_id}", name: "Ability Program Fixture", face_id: "{card_id}",
            types: ["Creature"], power: 1, toughness: 1,
            triggered_abilities: [(
                ability_id: "triggered_01", presentation: Fallback,
                trigger: WhenSelfEntersBattlefield, effect: [],
                program: Some((body: {body})), modal: None, targeting: None,
                may: false, intervening_if: None, triggers_only_once: false,
                max_triggers_per_turn: None,
            )])"#
    );
    let registry = crate::CardRegistry::from_chunks_and_tokens(&[&definition], &[])
        .map_err(|error| error.to_string())?;
    let owner = crate::ProgramOwnerId::ability(crate::AbilityDefinitionId {
        card_id: card_id.into(),
        face_id: crate::CardFaceId::new(card_id).unwrap(),
        ability_path: vec![crate::AbilityId::new("triggered_01").unwrap()],
    });
    registry
        .compiled_program_for_owner(&owner)
        .cloned()
        .ok_or_else(|| format!("the exact triggered ability owner {owner:?} did not compile"))
}

fn resultless_direct_hand_search_program(filter: &str) -> String {
    program_fixture(&format!(
        r#"(steps: [
            (id: "search_choice", bind: [], input: None, kind: Choose(
                choice: "search", chooser: Controller, optional: true,
                branches: [(branch_id: "search_for_card", presentation: Fallback,
                    cost: None, requirement: Always, body: (steps: [
                        (id: "search_hand", bind: [], input: None, kind: Perform(SearchLibrary(
                            who: Controller, optional: false, count: 1,
                            count_by_cast_cost: None,
                            filter: Some({filter}),
                            selection_constraint: None, slots: [],
                            zones: Fixed([Library]), destination: Hand,
                            conditional_destination: None, shuffle: true, reveal: true,
                            result_id: None, selection_bounds: Some((min: 0, max: 1)),
                        )))
                    ], yields: []))], otherwise: (steps: [], yields: []))),
        ], yields: [])"#
    ))
}

#[test]
fn authored_choose_accepts_resultless_direct_hand_search_filters() {
    for (label, filter) in [
        ("BasicLand", "(card_type: Some(BasicLand))"),
        ("exact name", "(exact_name: Some(\"Legal Test Commander\"))"),
    ] {
        let definition = resultless_direct_hand_search_program(filter);
        let registry = crate::CardRegistry::from_chunks_and_tokens(&[&definition], &[])
            .unwrap_or_else(|error| {
                panic!(
                    "the resultless direct Hand search with {label} filter must compile: {error}"
                )
            });
        let owner = crate::ProgramOwnerId::face(
            "program_fixture",
            crate::CardFaceId::new("program_fixture").unwrap(),
        );
        let program = registry
            .compiled_program_for_owner(&owner)
            .expect("the fixture face owns its compiled Choose program");
        let [choice] = program.body.steps.as_slice() else {
            panic!("expected one outer optional Choose instruction");
        };
        let crate::primitives::CompiledInstructionKind::Choose { branches, .. } = &choice.kind
        else {
            panic!("the optionality must remain on the outer Choose");
        };
        assert!(choice.outputs.is_empty());
        let [branch] = branches.as_slice() else {
            panic!("expected one direct Hand search branch");
        };
        let [search] = branch.body.steps.as_slice() else {
            panic!("expected one direct Hand search operation");
        };
        assert!(search.outputs.is_empty(), "the direct search is resultless");
        let crate::primitives::CompiledInstructionKind::Perform(effect) = &search.kind else {
            panic!("the selected branch must execute the direct search leaf");
        };
        assert!(matches!(
            effect.as_ref(),
            crate::primitives::SpellEffectKind::SearchLibrary {
                who: crate::primitives::PlayerRecipient::Controller,
                optional: false,
                count: 1,
                filter: Some(actual_filter),
                selection_constraint: None,
                slots,
                zones: crate::primitives::SearchZoneSelection::Fixed(zones),
                destination: crate::primitives::SearchDestination::Hand,
                conditional_destination: None,
                shuffle: true,
                reveal: true,
                result_id: None,
                selection_bounds: Some(crate::primitives::SearchSelectionBounds { min: 0, max: 1 }),
                ..
            } if zones == &[crate::primitives::CardSearchZone::Library]
                && slots.is_empty()
                && actual_filter.validate().is_ok()
        ));
        assert!(program.slots.is_empty());
    }

    for (label, filter) in [
        ("an empty filter", "()"),
        ("an empty name", "(exact_name: Some(\"\"))"),
    ] {
        let definition = resultless_direct_hand_search_program(filter);
        let error = crate::CardRegistry::from_chunks_and_tokens(&[&definition], &[])
            .expect_err("direct Hand search must reject invalid ordinary filters")
            .to_string();
        assert!(
            error.contains("zone card filter"),
            "{label}: unexpected diagnostic: {error}"
        );
    }
}

#[test]
fn authored_choose_accepts_resultless_direct_basic_land_search_to_library_top() {
    let definition = program_fixture(
        r#"(steps: [
            (id: "search_choice", bind: [], input: None, kind: Choose(
                choice: "search", chooser: Controller, optional: true,
                branches: [(branch_id: "search_for_a_basic_land", presentation: Fallback,
                    cost: None, requirement: Always, body: (steps: [
                        (id: "search_top", bind: [], input: None, kind: Perform(SearchLibrary(
                            who: Controller, optional: false, count: 1,
                            count_by_cast_cost: None,
                            filter: Some((card_type: Some(BasicLand))),
                            selection_constraint: None, slots: [],
                            zones: Fixed([Library]), destination: TopOfLibrary,
                            conditional_destination: None, shuffle: true, reveal: true,
                            result_id: None, selection_bounds: Some((min: 0, max: 1)),
                        )))
                    ], yields: []))], otherwise: (steps: [], yields: []))),
        ], yields: [])"#,
    );
    let registry = crate::CardRegistry::from_chunks_and_tokens(&[&definition], &[])
        .expect("the exact resultless direct top-search shape compiles");
    let owner = crate::ProgramOwnerId::face(
        "program_fixture",
        crate::CardFaceId::new("program_fixture").unwrap(),
    );
    let program = registry
        .compiled_program_for_owner(&owner)
        .expect("the exact face owns its compiled Choose program");
    let [choice] = program.body.steps.as_slice() else {
        panic!("expected one outer optional Choose instruction");
    };
    let crate::primitives::CompiledInstructionKind::Choose {
        choice: choice_id,
        chooser,
        optional,
        branches,
        otherwise,
        ..
    } = &choice.kind
    else {
        panic!("the search decision must remain a structural Choose");
    };
    assert_eq!(choice_id.as_str(), "search");
    assert_eq!(*chooser, crate::primitives::PlayerRecipient::Controller);
    assert!(*optional);
    assert!(choice.outputs.is_empty());
    assert!(otherwise.steps.is_empty());
    let [branch] = branches.as_slice() else {
        panic!("expected one search branch");
    };
    assert_eq!(branch.branch_id.as_str(), "search_for_a_basic_land");
    assert_eq!(branch.cost, crate::primitives::ResolutionCost::None);
    assert_eq!(
        branch.requirement,
        crate::primitives::CompiledChooseRequirement::Always
    );
    let [search] = branch.body.steps.as_slice() else {
        panic!("expected one direct search operation");
    };
    assert!(search.outputs.is_empty(), "the direct search is resultless");
    let crate::primitives::CompiledInstructionKind::Perform(effect) = &search.kind else {
        panic!("the selected branch must execute the direct search leaf");
    };
    let crate::primitives::SpellEffectKind::SearchLibrary {
        who,
        optional,
        count,
        count_by_cast_cost,
        filter,
        selection_constraint,
        slots,
        zones,
        destination,
        conditional_destination,
        shuffle,
        reveal,
        result_id,
        selection_bounds,
    } = effect.as_ref()
    else {
        panic!("the selected leaf must remain SearchLibrary");
    };
    assert_eq!(*who, crate::primitives::PlayerRecipient::Controller);
    assert!(!optional);
    assert_eq!(*count, 1);
    assert!(count_by_cast_cost.is_none());
    assert_eq!(
        filter,
        &Some(crate::primitives::ZoneCardFilter {
            card_type: Some(crate::primitives::CardTypeFilter::BasicLand),
            ..Default::default()
        })
    );
    assert!(selection_constraint.is_none());
    assert!(slots.is_empty());
    assert_eq!(
        zones,
        &crate::primitives::SearchZoneSelection::Fixed(vec![
            crate::primitives::CardSearchZone::Library
        ])
    );
    assert_eq!(
        *destination,
        crate::primitives::SearchDestination::TopOfLibrary
    );
    assert!(conditional_destination.is_none());
    assert!(*shuffle);
    assert!(*reveal);
    assert!(result_id.is_none());
    assert_eq!(
        *selection_bounds,
        Some(crate::primitives::SearchSelectionBounds { min: 0, max: 1 })
    );
    assert!(program.slots.is_empty());

    for (label, malformed) in [
        (
            "inner search may not be optional",
            definition.replace("optional: false", "optional: true"),
        ),
        ("count must stay fixed at one", definition.replace("count: 1,", "count: 2,")),
        (
            "filters still pass ordinary filter validation",
            definition.replace("card_type: Some(BasicLand)", ""),
        ),
        (
            "selection constraints are excluded",
            definition.replace("selection_constraint: None", "selection_constraint: Some(DifferentNames)"),
        ),
        (
            "selection slots are excluded",
            definition.replace(
                "slots: [],",
                "slots: [(slot_id: \"basic\", presentation: Fallback, filter: (card_type: Some(BasicLand)))],",
            ),
        ),
        (
            "searches only the controller's Library",
            definition.replace("Fixed([Library])", "Fixed([Library, Hand])"),
        ),
        (
            "Battlefield destination remains outside the direct Top search subset",
            definition.replace(
                "destination: TopOfLibrary",
                "destination: Battlefield(tapped: true)",
            ),
        ),
        (
            "conditional destinations are excluded",
            definition.replace(
                "conditional_destination: None",
                "conditional_destination: Some((condition: ControllerLibraryEmpty, destination: Hand))",
            ),
        ),
        (
            "the operation remains resultless",
            definition.replace("result_id: None", "result_id: Some(\"found\")"),
        ),
        (
            "bounds stay 0..1",
            definition.replace("min: 0, max: 1", "min: 1, max: 1"),
        ),
        (
            "shuffle remains enabled",
            definition.replace("shuffle: true", "shuffle: false"),
        ),
    ] {
        let error = crate::CardRegistry::from_chunks_and_tokens(&[&malformed], &[])
            .expect_err("the direct Top search extension must fail closed")
            .to_string();
        assert!(
            error.contains("program direct TopOfLibrary SearchLibrary")
                || error.contains("zone card filter")
                || error.contains("program conditional TopOfLibrary SearchLibrary supports only")
                || error.contains("program direct Battlefield SearchLibrary supports only")
                || error.contains("retained SearchLibrary supports only"),
            "{label}: unexpected diagnostic: {error}"
        );
    }
}

fn resultless_direct_battlefield_search_program(count: u32, max: u32) -> String {
    let definition = program_fixture(
        r#"(steps: [
            (id: "search_choice", bind: [], input: None, kind: Choose(
                choice: "search", chooser: Controller, optional: true,
                branches: [(branch_id: "search_for_a_basic_land", presentation: Fallback,
                    cost: None, requirement: Always, body: (steps: [
                        (id: "search_battlefield", bind: [], input: None, kind: Perform(SearchLibrary(
                            who: Controller, optional: false, count: 1,
                            count_by_cast_cost: None,
                            filter: Some((card_type: Some(BasicLand))),
                            selection_constraint: None, slots: [],
                            zones: Fixed([Library]),
                            destination: Battlefield(tapped: true),
                            conditional_destination: None, shuffle: true, reveal: false,
                            result_id: None, selection_bounds: Some((min: 0, max: 1)),
                        )))
                    ], yields: []))], otherwise: (steps: [], yields: [])))
        ], yields: [])"#,
    );
    definition
        .replace("count: 1,", &format!("count: {count},"))
        .replace("min: 0, max: 1", &format!("min: 0, max: {max}"))
}

fn resultless_conditional_top_search_program() -> String {
    program_fixture(
        r#"(steps: [
            (id: "search_choice", bind: [], input: None, kind: Choose(
                choice: "search", chooser: Controller, optional: true,
                branches: [(branch_id: "search_for_a_basic_land", presentation: Fallback,
                    cost: None, requirement: Always, body: (steps: [
                        (id: "search_top", bind: [], input: None, kind: Perform(SearchLibrary(
                            who: Controller, optional: false, count: 1,
                            count_by_cast_cost: None,
                            filter: Some((card_type: Some(BasicLand))),
                            selection_constraint: None, slots: [],
                            zones: Fixed([Library]), destination: TopOfLibrary,
                            conditional_destination: Some((condition: BattlefieldAggregate(
                                filter: (controllers: Controller, card_type: None,
                                    required_subtypes: ["Dragon"], exclude_source: false),
                                aggregate: Count, min: Some(1), max: None,
                                exclude_observed_object: false,
                            ), destination: Battlefield(tapped: true))),
                            shuffle: true, reveal: true, result_id: None,
                            selection_bounds: Some((min: 0, max: 1)),
                        )))
                    ], yields: []))], otherwise: (steps: [], yields: [])))
        ], yields: [])"#,
    )
}

#[test]
fn program_scry_fixed_counts_are_resultless_for_spell_and_ability_owners() {
    let scry_body = |count: u32| {
        format!(
            r#"(steps: [
                (id: "scry", bind: [], input: None,
                    kind: Perform(Scry(count: {count})))
            ], yields: [])"#
        )
    };
    let assert_scry = |label: &str, program: &crate::primitives::CompiledProgram, expected| {
        let [step] = program.body.steps.as_slice() else {
            panic!("{label} must compile as one Scry step");
        };
        assert!(step.outputs.is_empty(), "{label} Scry remains resultless");
        let crate::primitives::CompiledInstructionKind::Perform(effect) = &step.kind else {
            panic!("{label} must compile as an effect leaf");
        };
        assert!(
            matches!(
                effect.as_ref(),
                crate::primitives::SpellEffectKind::Scry {
                    count: crate::primitives::Amount::Fixed(actual),
                } if *actual == expected
            ),
            "{label} must retain the fixed Scry count"
        );
    };

    for count in [1, 2, 3] {
        let body = scry_body(count);

        let spell_definition = program_fixture(&body);
        let spell_registry = crate::CardRegistry::from_chunks_and_tokens(&[&spell_definition], &[])
            .unwrap_or_else(|error| {
                panic!("Spell Scry {count} must compile through the registry: {error}")
            });
        let spell_owner = crate::ProgramOwnerId::face(
            "program_fixture",
            crate::CardFaceId::new("program_fixture").unwrap(),
        );
        let spell_program = spell_registry
            .compiled_program_for_owner(&spell_owner)
            .expect("the Spell fixture face owns its compiled Program");
        assert_scry(&format!("Spell Scry {count}"), spell_program, count);

        let ability_program = ability_program_fixture("program_scry_ability", &body)
            .unwrap_or_else(|error| {
                panic!("Ability Scry {count} must compile through the registry: {error}")
            });
        assert_scry(&format!("Ability Scry {count}"), &ability_program, count);
    }
}

#[test]
fn program_scry_allows_fixed_zero_without_widening_legacy_scry_validation() {
    let fixed_zero = r#"(steps: [
        (id: "scry", bind: [], input: None,
            kind: Perform(Scry(count: 0)))
    ], yields: [])"#;

    let spell_definition = program_fixture(fixed_zero);
    let spell_registry = crate::CardRegistry::from_chunks_and_tokens(&[&spell_definition], &[])
        .unwrap_or_else(|error| {
            panic!("fixed-zero Program Scry must compile through the registry: {error}")
        });
    let spell_owner = crate::ProgramOwnerId::face(
        "program_fixture",
        crate::CardFaceId::new("program_fixture").unwrap(),
    );
    let spell_program = spell_registry
        .compiled_program_for_owner(&spell_owner)
        .expect("the zero-count Spell fixture owns its compiled Program");
    assert!(spell_program.body.steps[0].outputs.is_empty());
    assert!(matches!(
        &spell_program.body.steps[0].kind,
        crate::primitives::CompiledInstructionKind::Perform(effect)
            if matches!(effect.as_ref(), crate::primitives::SpellEffectKind::Scry {
                count: crate::primitives::Amount::Fixed(0),
            })
    ));

    let ability_program = ability_program_fixture("program_scry_zero_ability", fixed_zero)
        .unwrap_or_else(|error| {
            panic!("fixed-zero Ability Program Scry must compile through the registry: {error}")
        });
    assert!(ability_program.body.steps[0].outputs.is_empty());
    assert!(matches!(
        &ability_program.body.steps[0].kind,
        crate::primitives::CompiledInstructionKind::Perform(effect)
            if matches!(effect.as_ref(), crate::primitives::SpellEffectKind::Scry {
                count: crate::primitives::Amount::Fixed(0),
            })
    ));

    let legacy_zero = r#"(id: "legacy_scry_zero", name: "Legacy Scry Zero", face_id: "legacy_scry_zero",
        mana_cost: "{0}", types: ["Instant"], spell_effect: [Scry(count: 0)])"#;
    let Err(legacy_error) = crate::CardRegistry::from_chunks_and_tokens(&[legacy_zero], &[]) else {
        panic!(
            "the fixed-zero exception belongs to Programs; a legacy Scry list still rejects zero"
        );
    };
    assert!(
        legacy_error
            .to_string()
            .contains("Scry requires a count of at least 1"),
        "legacy Scry must retain its positive-count validation: {legacy_error}"
    );
}

#[test]
fn program_scry_rejects_dynamic_counts_captured_inputs_and_output_bindings() {
    let definition = |body: &str| program_fixture(body);
    let dynamic_count = definition(
        r#"(steps: [(id: "scry", bind: [], input: None, kind: Perform(Scry(count: Count(
            BattlefieldPermanents(filter: (controllers: Controller, required_subtypes: ["Island"]))
        ))))], yields: [])"#,
    );
    let captured_input = definition(
        r#"(steps: [
            (id: "draw", bind: ["drawn"], input: None, kind: Perform(Draw(count: 1))),
            (id: "scry", bind: [], input: Some("drawn"), kind: Perform(Scry(count: 1)))
        ], yields: [])"#,
    );
    let nonempty_binding = definition(
        r#"(steps: [(id: "scry", bind: ["scry_result"], input: None,
            kind: Perform(Scry(count: 1)))], yields: [])"#,
    );

    let Err(dynamic_error) = crate::CardRegistry::from_chunks_and_tokens(&[&dynamic_count], &[])
    else {
        panic!("a dynamic Scry count must be rejected");
    };
    assert!(
        dynamic_error
            .to_string()
            .contains("Scry count supports only a fixed literal in spell_program"),
        "dynamic Scry must fail the fixed-count contract: {dynamic_error}"
    );

    let Err(captured_error) = crate::CardRegistry::from_chunks_and_tokens(&[&captured_input], &[])
    else {
        panic!("Scry must not consume even a valid captured Draw receipt");
    };
    assert!(
        captured_error
            .to_string()
            .contains("instruction does not accept an input result"),
        "the bound Draw receipt reaches the Scry input check: {captured_error}"
    );

    let Err(binding_error) = crate::CardRegistry::from_chunks_and_tokens(&[&nonempty_binding], &[])
    else {
        panic!("resultless Scry must reject a nonempty output binding");
    };
    assert!(
        binding_error
            .to_string()
            .contains("binds 1 names for 0 results"),
        "Scry must remain resultless: {binding_error}"
    );
}

#[test]
fn program_reusable_operation_contract() {
    let search_program =
        |filter: &str, destination: &str, conditional_destination: &str, reveal: bool, min: u32| {
            program_fixture(&format!(
                r#"(steps: [
                (id: "search_choice", bind: [], input: None, kind: Choose(
                    choice: "search", chooser: Controller, optional: true,
                    branches: [(branch_id: "search_for_card", presentation: Fallback,
                        cost: None, requirement: Always, body: (steps: [
                            (id: "search", bind: [], input: None, kind: Perform(SearchLibrary(
                                who: Controller, optional: false, count: 1,
                                count_by_cast_cost: None,
                                filter: {filter}, selection_constraint: None, slots: [],
                                zones: Fixed([Library]), destination: {destination},
                                conditional_destination: {conditional_destination},
                                shuffle: true, reveal: {reveal}, result_id: None,
                                selection_bounds: Some((min: {min}, max: 1)),
                            )))
                        ], yields: []))], otherwise: (steps: [], yields: []))),
            ], yields: [])"#
            ))
        };
    let reflexive_program = |when: &str, child_metadata: &str, damage: &str| {
        let effect = format!(
            r#"CreateReflexiveTrigger({when}ability: (
                ability_id: "reflexive_01", presentation: Fallback,
                {child_metadata}effect: [{damage}],
            ))"#
        );
        let body = format!(
            r#"(steps: [(id: "reflexive", bind: [], input: None,
                kind: Perform({effect}))], yields: [])"#
        );
        ability_program_fixture("program_reusable_fixture", &body)
    };

    let valid_searches = [
        (
            "qualified Creature/minimum-mana-value Hand search",
            r#"Some((card_type: Some(Creature), min_mana_value: Some(6)))"#,
            "Hand",
            "None",
            true,
            0,
        ),
        (
            "validated OR/name filter to Hand",
            r#"Some((any_of: [(card_type: Some(Creature)), (exact_name: Some("Legal Test Commander"))]))"#,
            "Hand",
            "None",
            true,
            0,
        ),
        (
            "unrestricted private Hand search",
            "None",
            "Hand",
            "None",
            false,
            1,
        ),
        (
            "validated OR filter to TopOfLibrary",
            r#"Some((any_of: [(card_type: Some(Creature)), (required_subtypes: ["Elf"])]))"#,
            "TopOfLibrary",
            "None",
            true,
            0,
        ),
        (
            "unrestricted TopOfLibrary search with no reveal",
            "None",
            "TopOfLibrary",
            "None",
            false,
            1,
        ),
        (
            "recursive live condition on the existing conditional destination",
            r#"Some((card_type: Some(BasicLand)))"#,
            "TopOfLibrary",
            r#"Some((condition: AllOf([
                ControllerLibraryEmpty,
                AnyOf([
                    CardsInHand(players: Relative(Controller), min: Some(1)),
                    BattlefieldAggregate(
                        filter: (controllers: Controller, card_type: Some(Creature)),
                        aggregate: Count, min: Some(1), max: None,
                        exclude_observed_object: false,
                    ),
                ]),
            ]), destination: Battlefield(tapped: true)))"#,
            true,
            0,
        ),
    ];

    for (label, filter, destination, conditional_destination, reveal, min) in valid_searches {
        let definition = search_program(filter, destination, conditional_destination, reveal, min);
        crate::CardRegistry::from_chunks_and_tokens(&[&definition], &[])
            .unwrap_or_else(|error| panic!("{label} must compile: {error}"));
    }

    for (label, filter, destination, conditional_destination, reveal, min, expected_error) in [
        (
            "empty filter cannot be treated as a qualified search",
            "Some(())",
            "Hand",
            "None",
            false,
            0,
            "zone card filter requires at least one predicate",
        ),
        (
            "duplicate filter leaves remain invalid",
            r#"Some((any_of: [(card_type: Some(Creature)), (card_type: Some(Creature))]))"#,
            "Hand",
            "None",
            true,
            0,
            "zone card filter cannot contain duplicate terminal predicates",
        ),
        (
            "unrestricted search cannot fail to find while a card exists",
            "None",
            "Hand",
            "None",
            false,
            0,
            "SearchLibrary",
        ),
        (
            "qualified search preserves its fail-to-find minimum",
            r#"Some((card_type: Some(Creature)))"#,
            "Hand",
            "None",
            true,
            1,
            "SearchLibrary",
        ),
        (
            "source-counter condition stays outside the live If domain",
            r#"Some((card_type: Some(BasicLand)))"#,
            "TopOfLibrary",
            r#"Some((condition: SourceCounterCount(counter: Quest, min: Some(1)), destination: Battlefield(tapped: true)))"#,
            true,
            0,
            "If condition uses context",
        ),
        (
            "cast-snapshot condition stays outside the live If domain",
            r#"Some((card_type: Some(BasicLand)))"#,
            "TopOfLibrary",
            r#"Some((condition: CastOrigin(origin: Hand), destination: Battlefield(tapped: true)))"#,
            true,
            0,
            "If condition uses context",
        ),
    ] {
        let definition = search_program(filter, destination, conditional_destination, reveal, min);
        let error = invalid_program_reason(&definition);
        assert!(
            error.contains(expected_error),
            "{label}: unexpected semantic validation error: {error}"
        );
    }

    for (label, damage) in [
        (
            "existing Sparktongue-style AnyTarget parameter",
            "DamageTarget(amount: 3, target: (kind: AnyTarget))",
        ),
        (
            "fixed Creature-filtered damage parameter",
            "DamageTarget(amount: 4, target: (kind: Creature))",
        ),
    ] {
        reflexive_program("", "", damage)
            .unwrap_or_else(|error| panic!("{label} ability Program must compile: {error}"));
    }

    let unsupported_reflexive_shapes = [
        (
            "receipt-bound reflexive child",
            "when: Some(CountersPlaced(counter: Quest, object: Source)), ",
            "",
            "DamageTarget(amount: 3, target: (kind: AnyTarget))",
            "CreateReflexiveTrigger supports only the approved resultless target shapes",
        ),
        (
            "intervening-if metadata",
            "",
            "intervening_if: Some(ControllerLibraryEmpty), ",
            "DamageTarget(amount: 3, target: (kind: AnyTarget))",
            "CreateReflexiveTrigger supports only the approved resultless target shapes",
        ),
        (
            "explicit child target metadata",
            "",
            "targeting: Some((groups: [(min: 1, max: 1, prompt: \"Choose target creature\", effect_indices: [0])])), ",
            "DamageTarget(amount: 3, target: (kind: AnyTarget))",
            "CreateReflexiveTrigger supports only the approved resultless target shapes",
        ),
        (
            "dynamic reflexive damage amount",
            "",
            "",
            "DamageTarget(amount: Conditional(condition: ControllerLibraryEmpty, when_true: 3, otherwise: 4), target: (kind: Creature))",
            "CreateReflexiveTrigger DamageTarget supports only a fixed amount",
        ),
        (
            "defending-player target context",
            "",
            "",
            "DamageTarget(amount: 3, target: (kind: Creature, controller: DefendingPlayer))",
            "cannot depend on a defending player outside its typed program inputs",
        ),
        (
            "nested attached-object target context",
            "",
            "",
            "DamageTarget(amount: 3, target: (any_of: [(kind: Creature, excluded_objects: [AttachedObject]), (kind: Creature)]))",
            "cannot depend on an attached object outside its typed program inputs",
        ),
    ];
    for (label, when, child_metadata, damage, expected_error) in unsupported_reflexive_shapes {
        let error = reflexive_program(when, child_metadata, damage)
            .expect_err("unsupported ability-owned reflexive shape must fail closed");
        assert!(
            error.contains(expected_error),
            "{label}: unexpected semantic validation error: {error}"
        );
    }
}

#[test]
fn authored_choose_accepts_the_closed_resultless_conditional_top_search() {
    let definition = resultless_conditional_top_search_program();
    let registry = crate::CardRegistry::from_chunks_and_tokens(&[&definition], &[])
        .expect("Embermouth's resultless conditional search shape compiles");
    let owner = crate::ProgramOwnerId::face(
        "program_fixture",
        crate::CardFaceId::new("program_fixture").unwrap(),
    );
    let program = registry
        .compiled_program_for_owner(&owner)
        .expect("the fixture face owns its compiled Choose program");
    let [choice] = program.body.steps.as_slice() else {
        panic!("expected one outer optional Choose instruction");
    };
    let crate::primitives::CompiledInstructionKind::Choose {
        chooser,
        optional,
        branches,
        otherwise,
        ..
    } = &choice.kind
    else {
        panic!("the outer optionality remains structural Choose control flow");
    };
    assert_eq!(*chooser, crate::primitives::PlayerRecipient::Controller);
    assert!(*optional);
    assert!(otherwise.steps.is_empty() && otherwise.yields.is_empty());
    assert!(choice.outputs.is_empty());
    let [branch] = branches.as_slice() else {
        panic!("expected one no-cost search branch");
    };
    assert_eq!(branch.branch_id.as_str(), "search_for_a_basic_land");
    assert_eq!(branch.cost, crate::primitives::ResolutionCost::None);
    let [search] = branch.body.steps.as_slice() else {
        panic!("expected one conditional search operation");
    };
    assert!(search.outputs.is_empty(), "the search stays resultless");
    let crate::primitives::CompiledInstructionKind::Perform(effect) = &search.kind else {
        panic!("the selected branch contains a normal SearchLibrary leaf");
    };
    assert!(matches!(
        effect.as_ref(),
        crate::primitives::SpellEffectKind::SearchLibrary {
            who: crate::primitives::PlayerRecipient::Controller,
            optional: false,
            count: 1,
            count_by_cast_cost: None,
            filter: Some(filter),
            selection_constraint: None,
            slots,
            zones: crate::primitives::SearchZoneSelection::Fixed(zones),
            destination: crate::primitives::SearchDestination::TopOfLibrary,
            conditional_destination: Some(conditional),
            shuffle: true,
            reveal: true,
            result_id: None,
            selection_bounds: Some(crate::primitives::SearchSelectionBounds { min: 0, max: 1 }),
        } if filter == &crate::primitives::ZoneCardFilter {
                card_type: Some(crate::primitives::CardTypeFilter::BasicLand),
                ..Default::default()
            }
            && zones == &[crate::primitives::CardSearchZone::Library]
            && slots.is_empty()
            && filter.validate().is_ok()
            && conditional.condition == crate::primitives::GameCondition::BattlefieldAggregate {
                filter: crate::primitives::BattlefieldPermanentFilter {
                    required_supertypes: Vec::new(),
                    token: None,
                    any_of: None,
                    controllers: crate::primitives::RelativePlayerSet::Controller,
                    card_type: None,
                    color: None,
                    name: None,
                    required_subtypes: vec!["Dragon".into()],
                    exclude_source: false,
                },
                aggregate: crate::primitives::BattlefieldAggregate::Count,
                min: Some(1),
                max: None,
                exclude_observed_object: false,
            }
            && conditional.destination == crate::primitives::SearchDestination::Battlefield { tapped: true }
    ));
    assert!(program.slots.is_empty());

    for (label, malformed) in [
        (
            "the optionality belongs to Choose",
            definition.replace("optional: false", "optional: true"),
        ),
        (
            "the count is fixed at one",
            definition.replace("count: 1,", "count: 2,"),
        ),
        (
            "the filter is exactly BasicLand",
            definition.replace("card_type: Some(BasicLand)", "card_type: Some(Land)"),
        ),
        (
            "the base destination is TopOfLibrary",
            definition.replace("destination: TopOfLibrary", "destination: Hand"),
        ),
        (
            "the conditional destination is tapped Battlefield",
            definition.replace("Battlefield(tapped: true)", "Battlefield(tapped: false)"),
        ),
        (
            "the predicate remains in the live controller domain",
            definition.replace(
                "controllers: Controller, card_type: None",
                "controllers: Opponents, card_type: None",
            ),
        ),
        (
            "the predicate is a Count aggregate",
            definition.replace("aggregate: Count", "aggregate: DistinctNames"),
        ),
        (
            "the search does not reveal",
            definition.replace("reveal: true", "reveal: false"),
        ),
        (
            "the search shuffles",
            definition.replace("shuffle: true", "shuffle: false"),
        ),
        (
            "the leaf remains resultless",
            definition.replace("result_id: None", "result_id: Some(\"found\")"),
        ),
        (
            "the bounds are 0..1",
            definition.replace("min: 0, max: 1", "min: 1, max: 1"),
        ),
    ] {
        let error = crate::CardRegistry::from_chunks_and_tokens(&[&malformed], &[])
            .expect_err("the conditional search admission must remain closed")
            .to_string();
        assert!(
            error.contains("conditional")
                || error.contains("SearchLibrary")
                || error.contains("If condition")
                || error.contains("ProgramIf"),
            "{label}: unexpected diagnostic: {error}"
        );
    }
}

#[test]
fn conditional_top_search_rejects_void_as_its_predicate() {
    let definition = resultless_conditional_top_search_program();
    let void_condition = definition.replace(
        "condition: BattlefieldAggregate(\n                                filter: (controllers: Controller, card_type: None,\n                                    required_subtypes: [\"Dragon\"], exclude_source: false),\n                                aggregate: Count, min: Some(1), max: None,\n                                exclude_observed_object: false,\n                            ), destination:",
        "condition: Void, destination:",
    );
    assert_ne!(
        void_condition, definition,
        "the conditional search fixture's existing predicate was replaced"
    );
    let error = invalid_program_reason(&void_condition);
    assert!(
        error.contains(
            "If condition uses context outside controller hand, library, CardCount graveyard, or Controller battlefield Count state"
        ),
        "Void remains outside the conditional TopOfLibrary search predicate domain: {error}"
    );
}

#[test]
fn authored_choose_accepts_resultless_direct_basic_land_search_to_battlefield_tapped() {
    let definition = resultless_direct_battlefield_search_program(1, 1);
    let registry = crate::CardRegistry::from_chunks_and_tokens(&[&definition], &[])
        .expect("the exact resultless tapped Battlefield BasicLand search shape compiles");
    let owner = crate::ProgramOwnerId::face(
        "program_fixture",
        crate::CardFaceId::new("program_fixture").unwrap(),
    );
    let program = registry
        .compiled_program_for_owner(&owner)
        .expect("the fixture face owns its compiled Choose program");
    let [choice] = program.body.steps.as_slice() else {
        panic!("expected one outer optional Choose instruction");
    };
    let crate::primitives::CompiledInstructionKind::Choose {
        chooser,
        optional,
        branches,
        otherwise,
        ..
    } = &choice.kind
    else {
        panic!("the optionality must remain on the outer Choose");
    };
    assert_eq!(*chooser, crate::primitives::PlayerRecipient::Controller);
    assert!(*optional);
    assert!(otherwise.steps.is_empty());
    assert!(choice.outputs.is_empty());
    let [branch] = branches.as_slice() else {
        panic!("expected one BasicLand search branch");
    };
    let [search] = branch.body.steps.as_slice() else {
        panic!("expected one Battlefield search operation");
    };
    assert!(search.outputs.is_empty(), "the direct search is resultless");
    let crate::primitives::CompiledInstructionKind::Perform(effect) = &search.kind else {
        panic!("the selected branch must execute the direct search leaf");
    };
    assert!(matches!(
        effect.as_ref(),
        crate::primitives::SpellEffectKind::SearchLibrary {
            who: crate::primitives::PlayerRecipient::Controller,
            optional: false,
            count: 1,
            count_by_cast_cost: None,
            filter: Some(filter),
            selection_constraint: None,
            slots,
            zones: crate::primitives::SearchZoneSelection::Fixed(zones),
            destination: crate::primitives::SearchDestination::Battlefield { tapped: true },
            conditional_destination: None,
            shuffle: true,
            reveal: false,
            result_id: None,
            selection_bounds: Some(crate::primitives::SearchSelectionBounds { min: 0, max: 1 }),
        } if filter == &crate::primitives::ZoneCardFilter {
            card_type: Some(crate::primitives::CardTypeFilter::BasicLand),
            ..Default::default()
        } && zones == &[crate::primitives::CardSearchZone::Library]
            && slots.is_empty()
            && filter.validate().is_ok()
    ));
    assert!(program.slots.is_empty());

    for (label, malformed) in [
        (
            "the search remains mandatory once selected",
            definition.replace("optional: false", "optional: true"),
        ),
        (
            "the search count does not exceed two",
            definition.replace("count: 1,", "count: 3,"),
        ),
        (
            "only BasicLand is supported",
            definition.replace("card_type: Some(BasicLand)", "card_type: Some(Land)"),
        ),
        (
            "the search remains limited to the controller's Library",
            definition.replace("Fixed([Library])", "Fixed([Library, Hand])"),
        ),
        (
            "the entrant remains tapped",
            definition.replace(
                "destination: Battlefield(tapped: true)",
                "destination: Battlefield(tapped: false)",
            ),
        ),
        (
            "Battlefield entry search does not reveal",
            definition.replace("reveal: false", "reveal: true"),
        ),
        (
            "the search has no selection constraint",
            definition.replace("selection_constraint: None", "selection_constraint: Some(DifferentNames)"),
        ),
        (
            "the search has no slot definitions",
            definition.replace(
                "slots: [],",
                "slots: [(slot_id: \"basic\", presentation: Fallback, filter: (card_type: Some(BasicLand)))],",
            ),
        ),
        (
            "the destination is not conditional",
            definition.replace(
                "conditional_destination: None",
                "conditional_destination: Some((condition: ControllerLibraryEmpty, destination: Hand))",
            ),
        ),
        (
            "the operation has no SearchResultId",
            definition.replace("result_id: None", "result_id: Some(\"found\")"),
        ),
        (
            "the operation is shuffled",
            definition.replace("shuffle: true", "shuffle: false"),
        ),
        (
            "the selection bounds remain 0..1",
            definition.replace("min: 0, max: 1", "min: 1, max: 1"),
        ),
        (
            "the resultless instruction cannot bind a cohort",
            definition.replace(
                "id: \"search_battlefield\", bind: [],",
                "id: \"search_battlefield\", bind: [\"found\"],",
            ),
        ),
    ] {
        assert!(
            crate::CardRegistry::from_chunks_and_tokens(&[&malformed], &[]).is_err(),
            "{label} must be rejected"
        );
    }
}

#[test]
fn authored_choose_accepts_resultless_direct_two_basic_land_search_to_battlefield_tapped() {
    let definition = resultless_direct_battlefield_search_program(2, 2);
    let registry = crate::CardRegistry::from_chunks_and_tokens(&[&definition], &[])
        .expect("the exact resultless tapped two-BasicLand search shape compiles");
    let owner = crate::ProgramOwnerId::face(
        "program_fixture",
        crate::CardFaceId::new("program_fixture").unwrap(),
    );
    let program = registry
        .compiled_program_for_owner(&owner)
        .expect("the fixture face owns its compiled Choose program");
    let [choice] = program.body.steps.as_slice() else {
        panic!("expected one outer optional Choose instruction");
    };
    let crate::primitives::CompiledInstructionKind::Choose {
        chooser,
        optional,
        branches,
        otherwise,
        ..
    } = &choice.kind
    else {
        panic!("the optionality must remain on the outer Choose");
    };
    assert_eq!(*chooser, crate::primitives::PlayerRecipient::Controller);
    assert!(*optional);
    assert!(otherwise.steps.is_empty());
    let [branch] = branches.as_slice() else {
        panic!("expected one BasicLand search branch");
    };
    let [search] = branch.body.steps.as_slice() else {
        panic!("expected one Battlefield search operation");
    };
    assert!(search.outputs.is_empty(), "the direct search is resultless");
    let crate::primitives::CompiledInstructionKind::Perform(effect) = &search.kind else {
        panic!("the selected branch must execute the direct search leaf");
    };
    assert!(matches!(
        effect.as_ref(),
        crate::primitives::SpellEffectKind::SearchLibrary {
            who: crate::primitives::PlayerRecipient::Controller,
            optional: false,
            count: 2,
            count_by_cast_cost: None,
            filter: Some(filter),
            selection_constraint: None,
            slots,
            zones: crate::primitives::SearchZoneSelection::Fixed(zones),
            destination: crate::primitives::SearchDestination::Battlefield { tapped: true },
            conditional_destination: None,
            shuffle: true,
            reveal: false,
            result_id: None,
            selection_bounds: Some(crate::primitives::SearchSelectionBounds {
                min: 0,
                max: 2,
            }),
        } if filter == &crate::primitives::ZoneCardFilter {
            card_type: Some(crate::primitives::CardTypeFilter::BasicLand),
            ..Default::default()
        } && zones == &[crate::primitives::CardSearchZone::Library]
            && slots.is_empty()
            && filter.validate().is_ok()
    ));
    assert!(program.slots.is_empty());

    for (label, malformed) in [
        (
            "the search count does not exceed two",
            definition.replace("count: 2,", "count: 3,"),
        ),
        (
            "the selection bounds match the two-card count",
            definition.replace("min: 0, max: 2", "min: 0, max: 1"),
        ),
        (
            "the selection bounds do not exceed the two-card count",
            definition.replace("min: 0, max: 2", "min: 0, max: 3"),
        ),
        (
            "only BasicLand is supported",
            definition.replace("card_type: Some(BasicLand)", "card_type: Some(Land)"),
        ),
        (
            "the destination remains the tapped Battlefield",
            definition.replace(
                "destination: Battlefield(tapped: true)",
                "destination: Hand",
            ),
        ),
        (
            "the destination is not TopOfLibrary",
            definition.replace(
                "destination: Battlefield(tapped: true)",
                "destination: TopOfLibrary",
            ),
        ),
        (
            "the entrant remains tapped",
            definition.replace(
                "destination: Battlefield(tapped: true)",
                "destination: Battlefield(tapped: false)",
            ),
        ),
    ] {
        assert!(
            crate::CardRegistry::from_chunks_and_tokens(&[&malformed], &[]).is_err(),
            "{label} must be rejected"
        );
    }
}

fn choose_cast_cost_program(group_id: &str, option_id: &str) -> String {
    format!(
        r#"Some((body: (steps: [
            (id: "cast_cost_choice", kind: Choose(
                choice: "cast_cost_choice", chooser: Controller, optional: false,
                branches: [(branch_id: "selected", presentation: Fallback, cost: None,
                    requirement: CastCostReceipt((group_id: "{group_id}",
                        option_id: "{option_id}", expected_selected: true)),
                    body: (steps: [], yields: []))],
                otherwise: (steps: [], yields: [])
            ))
        ], yields: [])))"#
    )
}

#[test]
fn legacy_custom_effect_is_rejected_before_registry_admission() {
    let legacy = r#"(id: "legacy_custom_fixture", name: "Legacy Custom Fixture",
        face_id: "legacy_custom_fixture", mana_cost: "{U}", types: ["Instant"],
        custom_effect: "retired_algorithm")"#;
    let error = crate::CardRegistry::from_chunks_and_tokens(&[legacy], &[])
        .expect_err("retired custom effects must not enter an admitted registry")
        .to_string();

    assert!(
        error.contains("custom_effect `retired_algorithm` is unsupported"),
        "registry rejection must identify the unsupported legacy key: {error}"
    );
}

fn choose_lands_instruction(id: &str, bind: &str) -> String {
    format!(
        r#"(id: "{id}", bind: ["{bind}"], kind: Perform(ChoosePermanents(
            chooser: Controller,
            filter: (kind: AnyPermanent, permanent_types: [Land]),
            min: 0, max: 3)))"#
    )
}

#[test]
fn program_conditions_roundtrip_legacy_game_and_typed_discard_result_ron() {
    let game_ron = r#"(steps: [(id: "if", kind: If(
        condition: AllOf([
            ControllerLibraryEmpty,
            CardsInHand(players: Relative(Controller), min: Some(1)),
        ]),
        yes: (steps: [], yields: []),
        no: (steps: [], yields: []),
    ))], yields: [])"#;
    let game: super::AuthoredBlock =
        ron::from_str(game_ron).expect("legacy game-condition program RON remains accepted");
    let game_roundtrip: super::AuthoredBlock =
        ron::from_str(&ron::ser::to_string(&game).expect("legacy program serializes"))
            .expect("serialized legacy program parses back");
    assert_eq!(game_roundtrip, game);

    let result_ron = r#"(steps: [(id: "if_result", kind: IfResult(
        condition: DiscardCount(
            result: "discarded",
            players: Controller,
            card_type: Some(Nonland),
            min: Some(1),
            max: None,
        ),
        yes: (steps: [], yields: []),
        no: (steps: [], yields: []),
    ))], yields: [])"#;
    let result: super::AuthoredBlock =
        ron::from_str(result_ron).expect("typed result program condition parses");
    let result_roundtrip: super::AuthoredBlock =
        ron::from_str(&ron::ser::to_string(&result).expect("typed result program serializes"))
            .expect("serialized typed result program parses back");
    assert_eq!(result_roundtrip, result);
}

#[test]
fn program_choose_raw_ron_compiles_nested_paid_and_otherwise_blocks() {
    let definition = program_fixture(
        r#"(steps: [
            (id: "waterbend_or_discard", bind: [], input: None,
             kind: Choose(
                choice: "waterbend_or_discard", chooser: Controller, optional: true,
                branches: [(
                    branch_id: "waterbend_2", presentation: Fallback,
                    cost: Waterbend("{2}"), requirement: Always,
                    body: (steps: [], yields: []),
                )],
                otherwise: (steps: [
                    (id: "discard_one", bind: [], input: None,
                     kind: Perform(Discard(who: Controller, quantity: Exact(1)))),
                ], yields: []),
             )),
        ], yields: [])"#,
    );
    let registry = crate::CardRegistry::from_chunks_and_tokens(&[&definition], &[])
        .expect("bounded optional Choose with nested paid and otherwise blocks compiles");
    let owner = crate::ProgramOwnerId::face(
        "program_fixture",
        crate::CardFaceId::new("program_fixture").unwrap(),
    );
    let compiled = registry
        .compiled_program_for_owner(&owner)
        .expect("the exact fixture owner retains its compiled program");
    let [instruction] = compiled.body.steps.as_slice() else {
        panic!("Choose remains one typed control-flow instruction")
    };
    let crate::primitives::CompiledInstructionKind::Choose {
        choice,
        chooser,
        optional,
        branches,
        otherwise,
        ..
    } = &instruction.kind
    else {
        panic!("authored Choose compiles to the typed Choose instruction")
    };
    assert_eq!(instruction.id.as_str(), "waterbend_or_discard");
    assert_eq!(choice.as_str(), "waterbend_or_discard");
    assert_eq!(*chooser, crate::primitives::PlayerRecipient::Controller);
    assert!(*optional);
    assert_eq!(branches.len(), 1);
    assert_eq!(branches[0].branch_id.as_str(), "waterbend_2");
    assert_eq!(
        branches[0].cost,
        crate::primitives::ResolutionCost::Waterbend(
            ron::from_str("\"{2}\"").expect("fixed waterbend mana parses")
        )
    );
    assert!(branches[0].body.steps.is_empty());
    assert!(otherwise.steps.iter().any(|step| step.id == "discard_one"));
}

#[test]
fn program_if_accepts_controller_card_count_graveyard_condition() {
    let definition = program_fixture(
        r#"(steps: [
            (id: "mill_then_check", kind: If(
                condition: GraveyardAggregate(
                    owners: Controller, aggregate: CardCount,
                    filter: Some((required_subtypes: ["Elf"])),
                    min: Some(1), max: None,
                ),
                yes: (steps: [(id: "gain", kind: Perform(GainLife(amount: 2)))], yields: []),
                no: (steps: [], yields: []),
            )),
        ], yields: [])"#,
    );
    let registry = crate::CardRegistry::from_chunks_and_tokens(&[&definition], &[])
        .expect("controller CardCount graveyard conditions compile in ProgramIf");
    let owner = crate::ProgramOwnerId::face(
        "program_fixture",
        crate::CardFaceId::new("program_fixture").unwrap(),
    );
    let compiled = registry
        .compiled_program_for_owner(&owner)
        .expect("the exact fixture owner retains its compiled program");
    let [instruction] = compiled.body.steps.as_slice() else {
        panic!("the graveyard condition remains one typed If instruction")
    };
    let crate::primitives::CompiledInstructionKind::If { condition, yes, no } = &instruction.kind
    else {
        panic!("graveyard condition compiles to ProgramIf")
    };
    let crate::primitives::CompiledProgramCondition::Game(GameCondition::GraveyardAggregate {
        owners,
        aggregate,
        filter,
        min,
        max,
    }) = condition.as_ref()
    else {
        panic!("ProgramIf preserves the exact game condition")
    };
    assert_eq!(*owners, RelativePlayerSet::Controller);
    assert_eq!(*aggregate, GraveyardAggregate::CardCount);
    assert_eq!(*min, Some(1));
    assert_eq!(*max, None);
    assert_eq!(
        filter.as_ref().unwrap().required_subtypes,
        ["Elf"],
        "Elf card filtering has no creature restriction"
    );
    assert_eq!(yes.steps.len(), 1);
    assert!(no.steps.is_empty());
}

#[test]
fn spell_program_if_accepts_direct_cast_snapshot_condition() {
    use crate::primitives::{
        CompiledInstructionKind, CompiledProgram, CompiledProgramCondition, EffectContext,
    };

    let program: crate::CardProgram = ron::from_str(
        r#"(body: (steps: [(id: "cast_origin", kind: If(
            condition: CastSnapshot(index: 0),
            yes: (steps: [], yields: []), no: (steps: [], yields: []),
        ))], yields: []))"#,
    )
    .expect("the direct cast-snapshot Program parses");
    let owner = crate::ProgramOwnerId::face(
        "cast_snapshot_if",
        crate::CardFaceId::new("cast_snapshot_if").unwrap(),
    );
    let compiled = CompiledProgram::compile(owner, EffectContext::Spell, &program)
        .expect("a direct spell ProgramIf may read its owning face's cast condition");
    let [instruction] = compiled.body.steps.as_slice() else {
        panic!("the cast-origin check remains one authored If instruction")
    };
    assert!(matches!(
        &instruction.kind,
        CompiledInstructionKind::If { condition, .. }
            if matches!(
                condition.as_ref(),
                CompiledProgramCondition::Game(crate::primitives::GameCondition::CastSnapshot {
                    index: 0
                })
            )
    ));
}

#[test]
fn cast_snapshot_program_if_stays_spell_face_scoped_and_checks_its_index() {
    use crate::primitives::{CompiledProgram, EffectContext};

    let body = r#"(steps: [(id: "cast_origin", kind: If(
        condition: CastSnapshot(index: 0),
        yes: (steps: [], yields: []), no: (steps: [], yields: []),
    ))], yields: [])"#;
    let spell_program: crate::CardProgram =
        ron::from_str(&format!("(body: {body})")).expect("spell Program parses");
    let ability_owner = crate::ProgramOwnerId::ability(crate::AbilityDefinitionId {
        card_id: "cast_snapshot_ability".into(),
        face_id: crate::CardFaceId::new("cast_snapshot_ability").unwrap(),
        ability_path: vec![crate::AbilityId::new("triggered_01").unwrap()],
    });
    assert!(
        CompiledProgram::compile(ability_owner, EffectContext::Ability, &spell_program).is_err(),
        "CastSnapshot remains unavailable to ability ProgramIf"
    );

    let face = |id: &str, cast_conditions: &str, index: u32| {
        format!(
            r#"(id: "{id}", name: "Cast Snapshot Fixture", face_id: "{id}",
                mana_cost: "{{0}}", types: ["Instant"],
                cast_conditions: [{cast_conditions}],
                spell_program: Some((body: (steps: [(id: "cast_origin", kind: If(
                    condition: CastSnapshot(index: {index}),
                    yes: (steps: [], yields: []), no: (steps: [], yields: []),
                ))], yields: []))))"#
        )
    };
    let valid = face(
        "cast_snapshot_index_valid",
        "CastOrigin(origin: Graveyard)",
        0,
    );
    crate::CardRegistry::from_chunks_and_tokens(&[&valid], &[])
        .expect("the direct spell ProgramIf references its face's cast condition");

    for (id, conditions, index) in [
        ("cast_snapshot_index_missing", "", 0),
        (
            "cast_snapshot_index_out_of_range",
            "CastOrigin(origin: Graveyard)",
            1,
        ),
    ] {
        let invalid = face(id, conditions, index);
        let error = crate::CardRegistry::from_chunks_and_tokens(&[&invalid], &[])
            .expect_err("a Program CastSnapshot must reference an owning face condition");
        assert!(
            format!("{error}").contains(
                "CastSnapshot requires an existing condition on the resolving spell face"
            ),
            "unexpected {id} diagnostic: {error}"
        );
    }

    let choose = r#"(id: "cast_snapshot_choose_stays_unsupported",
        name: "Cast Snapshot Choose Fixture", face_id: "cast_snapshot_choose_stays_unsupported",
        mana_cost: "{0}", types: ["Instant"],
        cast_conditions: [CastOrigin(origin: Graveyard)],
        spell_program: Some((body: (steps: [(id: "choose", kind: Choose(
            choice: "choose", chooser: Controller, optional: false,
            branches: [(branch_id: "cast", presentation: Fallback, cost: None,
                requirement: GameCondition(CastSnapshot(index: 0)),
                body: (steps: [], yields: []))],
            otherwise: (steps: [], yields: []),
        ))], yields: []))))"#;
    assert!(
        crate::CardRegistry::from_chunks_and_tokens(&[choose], &[]).is_err(),
        "the shared validator must not admit CastSnapshot in a Choose requirement"
    );
}

#[test]
fn registry_checks_cast_snapshot_references_inside_nested_if_result_blocks() {
    let nested = r#"(id: "cast_snapshot_nested_if_result_fixture",
        name: "Cast Snapshot Nested IfResult Fixture",
        face_id: "cast_snapshot_nested_if_result_fixture", mana_cost: "{0}",
        types: ["Instant"], cast_conditions: [CastOrigin(origin: Graveyard)],
        spell_program: Some((body: (steps: [
            (id: "discard", bind: ["discarded"], kind: Perform(Discard(
                who: Controller, quantity: OptionalExact(1)
            ))),
            (id: "paid", kind: IfResult(
                condition: DiscardOutcome(result: "discarded", outcome: ChosenToPay),
                yes: (steps: [(id: "nested_snapshot", kind: If(
                    condition: CastSnapshot(index: 1),
                    yes: (steps: [], yields: []), no: (steps: [], yields: []),
                ))], yields: []),
                no: (steps: [], yields: []),
            )),
        ], yields: []), target_bindings: [])))"#;
    let error = crate::CardRegistry::from_chunks_and_tokens(&[nested], &[])
        .expect_err("CastSnapshot references must be checked in IfResult branches");
    assert!(
        format!("{error}")
            .contains("CastSnapshot requires an existing condition on the resolving spell face"),
        "unexpected nested snapshot diagnostic: {error}"
    );
}

#[test]
fn spell_program_create_tokens_accepts_typed_graveyard_count() {
    use crate::primitives::{
        Amount, CompiledInstructionKind, CompiledProgram, CountExpression, EffectContext,
        PlayerRecipient, RelativePlayerSet, SpellEffectKind,
    };

    let compile =
        |card_id: &str, count: &str, token: &str, who: &str, tapped: bool, sacrifice: &str| {
            let program: crate::CardProgram = ron::from_str(&format!(
                r#"(body: (steps: [(id: "create_tokens", kind: Perform(CreateTokens(
                token: "{token}", count: {count}, who: {who}, tapped: {tapped},
                sacrifice_timing: {sacrifice},
            )))], yields: []))"#
            ))
            .expect("the CreateTokens Program parses");
            let owner =
                crate::ProgramOwnerId::face(card_id, crate::CardFaceId::new(card_id).unwrap());
            CompiledProgram::compile(owner, EffectContext::Spell, &program)
        };
    let count =
        "Count(GraveyardCards(owners: Controller, filter: Some((card_type: Some(Creature)))))";
    let compiled = compile(
        "graveyard_count_tokens",
        count,
        "horror_b_2_2",
        "Controller",
        true,
        "None",
    )
    .expect("spell Programs may count the controller's filtered graveyard for token creation");
    let [instruction] = compiled.body.steps.as_slice() else {
        panic!("the compiled token operation remains one instruction")
    };
    assert!(matches!(
        &instruction.kind,
        CompiledInstructionKind::Perform(effect)
            if matches!(
                effect.as_ref(),
                SpellEffectKind::CreateTokens {
                    token,
                    count: Amount::Count(CountExpression::GraveyardCards {
                        owners: RelativePlayerSet::Controller,
                        filter: Some(filter),
                    }),
                    who: PlayerRecipient::Controller,
                    tapped: true,
                    sacrifice_timing: None,
                } if token == "horror_b_2_2"
                    && filter.card_type == Some(crate::primitives::CardTypeFilter::Creature)
            )
    ));

    compile(
        "generic_count_token",
        count,
        "generic_token",
        "Controller",
        false,
        "None",
    )
    .expect("the generic count profile accepts other valid token identities and tapped states");

    for (label, unsupported_count) in [
        (
            "other_dynamic_source",
            "Count(GraveyardCardsNamed(owners: Controller, name: \"Other Card\"))",
        ),
        (
            "opponent_graveyard",
            "Count(GraveyardCards(owners: Opponents, filter: Some((card_type: Some(Creature)))))",
        ),
        (
            "unfiltered_graveyard",
            "Count(GraveyardCards(owners: Controller, filter: None))",
        ),
        (
            "implicit_card_result_count",
            "Count(CardsMatchingResult(filter: (source: PreviousEffect, action: Discard, players: Controller)))",
        ),
    ] {
        let id = format!("unsupported_{}", label.replace(' ', "_"));
        let error = compile(
            &id,
            unsupported_count,
            "generic_token",
            "Controller",
            false,
            "None",
        )
        .expect_err("only the explicit Controller graveyard-card profile is admitted");
        assert!(
            error.contains("CreateTokens count")
                || error.contains("program amounts cannot read an implicit card result"),
            "unexpected {label} diagnostic: {error}"
        );
    }

    for (label, who, sacrifice) in [
        ("spell source controller", "SourceController", "None"),
        ("delayed sacrifice", "Controller", "Some(NextEndStep)"),
    ] {
        let id = format!("unsupported_{}", label.replace(' ', "_"));
        assert!(
            compile(&id, count, "generic_token", who, false, sacrifice,).is_err(),
            "the spell token profile rejects {label}"
        );
    }
}

#[test]
fn ability_program_source_controller_rejects_dynamic_graveyard_count() {
    use crate::primitives::{CompiledProgram, EffectContext};

    let program: crate::CardProgram = ron::from_str(
        r#"(body: (steps: [(id: "create_tokens", kind: Perform(CreateTokens(
            token: "generic_token",
            count: Count(GraveyardCards(owners: Controller,
                filter: Some((card_type: Some(Creature))))),
            who: SourceController, tapped: false, sacrifice_timing: None,
        )))], yields: []))"#,
    )
    .expect("the ability token Program parses");
    let owner = crate::ProgramOwnerId::ability(crate::AbilityDefinitionId {
        card_id: "ability_source_controller_dynamic_count".into(),
        face_id: crate::CardFaceId::new("ability_source_controller_dynamic_count").unwrap(),
        ability_path: vec![crate::AbilityId::new("triggered_01").unwrap()],
    });
    assert!(
        CompiledProgram::compile(owner, EffectContext::Ability, &program).is_err(),
        "dynamic graveyard counts require ordinary Controller token creation"
    );

    let fixed_program: crate::CardProgram = ron::from_str(
        r#"(body: (steps: [(id: "create_tokens", kind: Perform(CreateTokens(
            token: "generic_token", count: 1, who: SourceController,
            tapped: false, sacrifice_timing: None,
        )))], yields: []))"#,
    )
    .expect("the fixed-count ability Program parses");
    let fixed_owner = crate::ProgramOwnerId::ability(crate::AbilityDefinitionId {
        card_id: "ability_source_controller_fixed_count".into(),
        face_id: crate::CardFaceId::new("ability_source_controller_fixed_count").unwrap(),
        ability_path: vec![crate::AbilityId::new("triggered_01").unwrap()],
    });
    CompiledProgram::compile(fixed_owner, EffectContext::Ability, &fixed_program)
        .expect("fixed SourceController token creation remains supported for ability Programs");
}

#[test]
fn program_if_accepts_controller_distinct_card_types_graveyard_condition() {
    let definition = program_fixture(
        r#"(steps: [
            (id: "delirium", kind: If(
                condition: GraveyardAggregate(
                    owners: Controller, aggregate: DistinctCardTypes,
                    filter: None, min: Some(4), max: None,
                ),
                yes: (steps: [], yields: []),
                no: (steps: [], yields: []),
            )),
        ], yields: [])"#,
    );
    crate::CardRegistry::from_chunks_and_tokens(&[&definition], &[])
        .expect("the exact Controller four-type graveyard ProgramIf is admitted");
}

#[test]
fn program_if_rejects_other_distinct_card_types_graveyard_profiles() {
    let exact = "GraveyardAggregate(owners: Controller, aggregate: DistinctCardTypes, filter: None, min: Some(4), max: None)";
    let conditions = [
        (
            "other owner",
            "GraveyardAggregate(owners: Opponents, aggregate: DistinctCardTypes, filter: None, min: Some(4), max: None)".to_string(),
        ),
        (
            "other threshold",
            "GraveyardAggregate(owners: Controller, aggregate: DistinctCardTypes, filter: None, min: Some(3), max: None)".to_string(),
        ),
        (
            "upper bound",
            "GraveyardAggregate(owners: Controller, aggregate: DistinctCardTypes, filter: None, min: Some(4), max: Some(5))".to_string(),
        ),
        (
            "extra filter",
            "GraveyardAggregate(owners: Controller, aggregate: DistinctCardTypes, filter: Some((required_subtypes: [\"Elf\"])), min: Some(4), max: None)".to_string(),
        ),
        (
            "composite wrapper",
            format!("AllOf([{exact}, ControllerLibraryEmpty])"),
        ),
    ];

    for (label, condition) in conditions {
        let body = format!(
            r#"(steps: [(id: "delirium", kind: If(
                condition: {condition}, yes: (steps: [], yields: []),
                no: (steps: [], yields: []),
            ))], yields: [])"#
        );
        let definition = program_fixture(&body);
        assert!(
            crate::CardRegistry::from_chunks_and_tokens(&[&definition], &[]).is_err(),
            "unsupported Delirium condition profile was admitted: {label}"
        );
    }

    let ability_definition = ability_program_fixture(
        "distinct_card_types_ability",
        &format!(
            r#"(steps: [(id: "delirium", kind: If(
                condition: {exact}, yes: (steps: [], yields: []),
                no: (steps: [], yields: []),
            ))], yields: [])"#
        ),
    );
    assert!(
        ability_definition.is_err(),
        "the Delirium condition profile remains spell-only"
    );
}

#[test]
fn spell_program_accepts_resultless_controller_exile_permission_leaf() {
    let definition = program_fixture(
        r#"(steps: [
            (id: "exile_top", kind: Perform(ExileTopWithPlayPermission(
                player: Controller, count: 1, count_by_cast_cost: None,
            ))),
        ], yields: [])"#,
    );
    crate::CardRegistry::from_chunks_and_tokens(&[&definition], &[])
        .expect("the fixed one-card Controller permission leaf is admitted in a spell Program");
}

#[test]
fn spell_program_rejects_other_exile_permission_leaf_profiles() {
    let effects = [
        (
            "other player",
            "ExileTopWithPlayPermission(player: EachOpponent, count: 1, count_by_cast_cost: None)",
        ),
        (
            "other count",
            "ExileTopWithPlayPermission(player: Controller, count: 2, count_by_cast_cost: None)",
        ),
    ];
    for (label, effect) in effects {
        let body = format!(r#"(steps: [(id: "exile_top", kind: Perform({effect}))], yields: [])"#);
        let definition = program_fixture(&body);
        assert!(
            crate::CardRegistry::from_chunks_and_tokens(&[&definition], &[]).is_err(),
            "unsupported exile permission leaf profile was admitted: {label}"
        );
    }

    let conditional_body = r#"(steps: [
        (id: "exile_top", kind: Perform(ExileTopWithPlayPermission(
            player: Controller, count: 1,
            count_by_cast_cost: Some((condition: (group_id: "cast_cost_01",
                option_id: "option_01", expected_selected: true),
                if_selected: 2, otherwise: 1)),
        ))),
    ], yields: [])"#;
    let conditional_definition = program_fixture(conditional_body).replace(
        "types: [\"Instant\"],",
        "types: [\"Instant\"], cast_cost_groups: [(group_id: \"cast_cost_01\", presentation: Fallback, options: [Mana(option_id: \"option_01\", presentation: Fallback, kind: AdditionalPayment, cost: \"{1}\")])],",
    );
    assert!(
        crate::CardRegistry::from_chunks_and_tokens(&[&conditional_definition], &[]).is_err(),
        "the Program leaf does not admit cast-cost-dependent counts"
    );

    let ability_definition = ability_program_fixture(
        "exile_permission_ability",
        r#"(steps: [(id: "exile_top", kind: Perform(ExileTopWithPlayPermission(
            player: Controller, count: 1, count_by_cast_cost: None,
        )))], yields: [])"#,
    );
    assert!(
        ability_definition.is_err(),
        "the exile play permission leaf remains spell-only in Programs"
    );
}

#[test]
fn program_if_accepts_void_and_rejects_unrelated_turn_history() {
    let void_definition = program_fixture(
        r#"(steps: [
            (id: "void_branch", kind: If(
                condition: Void,
                yes: (steps: [], yields: []),
                no: (steps: [], yields: []),
            )),
        ], yields: [])"#,
    );
    let registry = crate::CardRegistry::from_chunks_and_tokens(&[&void_definition], &[])
        .expect("Void is an admitted live ProgramIf condition");
    let owner = crate::ProgramOwnerId::face(
        "program_fixture",
        crate::CardFaceId::new("program_fixture").unwrap(),
    );
    let compiled = registry
        .compiled_program_for_owner(&owner)
        .expect("the exact fixture owner retains its compiled program");
    let [instruction] = compiled.body.steps.as_slice() else {
        panic!("Void remains one typed If instruction")
    };
    let crate::primitives::CompiledInstructionKind::If { condition, yes, no } = &instruction.kind
    else {
        panic!("Void compiles to ProgramIf")
    };
    assert!(matches!(
        condition.as_ref(),
        crate::primitives::CompiledProgramCondition::Game(GameCondition::Void)
    ));
    assert!(yes.steps.is_empty() && yes.yields.is_empty());
    assert!(no.steps.is_empty() && no.yields.is_empty());

    let unrelated_turn_history = program_fixture(
        r#"(steps: [
            (id: "if", kind: If(
                condition: SpellsCastThisTurn(players: Controller, min: Some(1)),
                yes: (steps: [], yields: []),
                no: (steps: [], yields: []),
            )),
        ], yields: [])"#,
    );
    let error = invalid_program_reason(&unrelated_turn_history);
    assert!(
        error.contains(
            "If condition uses context outside controller hand, library, CardCount graveyard, or Controller battlefield Count state"
        ),
        "an unrelated turn-history condition remains rejected: {error}"
    );

    let composite_void = program_fixture(
        r#"(steps: [
            (id: "if", kind: If(
                condition: AllOf([Void]),
                yes: (steps: [], yields: []),
                no: (steps: [], yields: []),
            )),
        ], yields: [])"#,
    );
    let error = invalid_program_reason(&composite_void);
    assert!(
        error.contains(
            "If condition uses context outside controller hand, library, CardCount graveyard, or Controller battlefield Count state"
        ),
        "Void remains unsupported inside composite ProgramIf conditions: {error}"
    );
}

#[test]
fn program_if_keeps_graveyard_condition_admission_narrow() {
    for (label, condition, expected_error) in [
        (
            "opponent-owned graveyards",
            "GraveyardAggregate(owners: Opponents, aggregate: CardCount, min: Some(1))",
            "If condition uses context outside controller hand, library, CardCount graveyard, or Controller battlefield Count state",
        ),
        (
            "distinct card-type aggregates",
            "GraveyardAggregate(owners: Controller, aggregate: DistinctCardTypes, min: Some(1))",
            "If condition uses context outside controller hand, library, CardCount graveyard, or Controller battlefield Count state",
        ),
        (
            "nested unsupported owners",
            "AllOf([ControllerLibraryEmpty, GraveyardAggregate(owners: Opponents, aggregate: CardCount, min: Some(1))])",
            "If condition uses context outside controller hand, library, CardCount graveyard, or Controller battlefield Count state",
        ),
        (
            "invalid bounds",
            "GraveyardAggregate(owners: Controller, aggregate: CardCount, min: Some(3), max: Some(2))",
            "GraveyardAggregate min cannot exceed max",
        ),
        (
            "malformed optional filter",
            "GraveyardAggregate(owners: Controller, aggregate: CardCount, filter: Some((any_of: Some([]))), min: Some(1))",
            "zone card filter any_of requires at least two branches",
        ),
    ] {
        let body = format!(
            "(steps: [(id: \"if\", kind: If(condition: {condition}, yes: (steps: [], yields: []), no: (steps: [], yields: [])))], yields: [])"
        );
        let error = invalid_program_reason(&program_fixture(&body));
        assert!(
            error.contains(expected_error),
            "{label}: unexpected diagnostic: {error}"
        );
    }
}

#[test]
fn program_if_accepts_controller_battlefield_count_conditions() {
    let fixture = |condition: &str| {
        program_fixture(&format!(
            r#"(steps: [(id: "if", kind: If(
                condition: {condition},
                yes: (steps: [], yields: []),
                no: (steps: [], yields: []),
            ))], yields: [])"#
        ))
    };
    let conditions = [
        (
            "another Hero",
            r#"BattlefieldAggregate(
                filter: (controllers: Controller, required_subtypes: ["Hero"]),
                aggregate: Count, min: Some(1), max: None,
                exclude_observed_object: false,
            )"#,
        ),
        (
            "Land-or-Treefolk union",
            r#"BattlefieldAggregate(
                filter: (controllers: Controller, any_of: Some([
                    (controllers: Controller, card_type: Some(Land)),
                    (controllers: Controller, required_subtypes: ["Treefolk"]),
                ])),
                aggregate: Count, min: Some(7), max: None,
                exclude_observed_object: false,
            )"#,
        ),
        (
            "recursive condition composition",
            r#"AllOf([
                ControllerLibraryEmpty,
                BattlefieldAggregate(
                    filter: (controllers: Controller, any_of: Some([
                        (controllers: Controller, card_type: Some(Land)),
                        (controllers: Controller, required_subtypes: ["Treefolk"]),
                    ])),
                    aggregate: Count, min: Some(7), max: None,
                    exclude_observed_object: false,
                ),
            ])"#,
        ),
    ];

    for (label, condition) in conditions {
        let definition = fixture(condition);
        let registry = crate::CardRegistry::from_chunks_and_tokens(&[&definition], &[])
            .unwrap_or_else(|error| {
                panic!(
                    "a spell ProgramIf with a Controller battlefield Count for {label} must compile: {error}"
                )
            });
        let owner = crate::ProgramOwnerId::face(
            "program_fixture",
            crate::CardFaceId::new("program_fixture").unwrap(),
        );
        let compiled = registry
            .compiled_program_for_owner(&owner)
            .expect("the exact fixture face retains its compiled spell program");
        assert_eq!(
            compiled.body.steps.len(),
            1,
            "{label} remains one ProgramIf"
        );
        assert!(matches!(
            compiled.body.steps[0].kind,
            crate::primitives::CompiledInstructionKind::If { .. }
        ));
    }
}

#[test]
fn program_if_keeps_battlefield_count_admission_narrow_and_live_validated() {
    let fixture = |condition: &str| {
        program_fixture(&format!(
            r#"(steps: [(id: "if", kind: If(
                condition: {condition},
                yes: (steps: [], yields: []),
                no: (steps: [], yields: []),
            ))], yields: [])"#
        ))
    };
    let unsupported =
        "If condition uses context outside controller hand, library, CardCount graveyard, or Controller battlefield Count state";
    let cases = [
        (
            "DistinctNames",
            r#"BattlefieldAggregate(filter: (controllers: Controller), aggregate: DistinctNames, min: Some(1))"#,
            unsupported,
        ),
        (
            "TotalPower",
            r#"BattlefieldAggregate(filter: (controllers: Controller), aggregate: TotalPower, min: Some(1))"#,
            unsupported,
        ),
        (
            "legacy BattlefieldCreatureCount",
            r#"BattlefieldCreatureCount(filter: (controllers: Controller, subtype: Some("Hero")), min: Some(1))"#,
            unsupported,
        ),
        (
            "SourceCounterCount",
            r#"SourceCounterCount(counter: Quest, min: Some(1))"#,
            unsupported,
        ),
        (
            "TargetedPlayer at the root",
            r#"BattlefieldAggregate(
                filter: (controllers: TargetedPlayer(group_index: 0, kind: AnyPlayer), required_subtypes: ["Hero"]),
                aggregate: Count, min: Some(1), exclude_observed_object: false,
            )"#,
            "ProgramIf battlefield aggregate filters require Controller at every any_of node",
        ),
        (
            "TargetedPlayer inside any_of",
            r#"BattlefieldAggregate(
                filter: (controllers: Controller, any_of: Some([
                    (controllers: Controller, card_type: Some(Land)),
                    (controllers: TargetedPlayer(group_index: 0, kind: AnyPlayer), required_subtypes: ["Treefolk"]),
                ])),
                aggregate: Count, min: Some(1), exclude_observed_object: false,
            )"#,
            "ProgramIf battlefield aggregate filters require Controller at every any_of node",
        ),
        (
            "observed object at the root",
            r#"BattlefieldAggregate(
                filter: (controllers: Controller), aggregate: Count,
                min: Some(1), exclude_observed_object: true,
            )"#,
            "event-observed condition requires a supplying trigger",
        ),
        (
            "observed object inside condition composition",
            r#"AnyOf([
                ControllerLibraryEmpty,
                BattlefieldAggregate(
                    filter: (controllers: Controller), aggregate: Count,
                    min: Some(1), exclude_observed_object: true,
                ),
            ])"#,
            "event-observed condition requires a supplying trigger",
        ),
        (
            "observed object inside AllOf",
            r#"AllOf([
                ControllerLibraryEmpty,
                BattlefieldAggregate(
                    filter: (controllers: Controller), aggregate: Count,
                    min: Some(1), exclude_observed_object: true,
                ),
            ])"#,
            "event-observed condition requires a supplying trigger",
        ),
        (
            "invalid bounds",
            r#"BattlefieldAggregate(
                filter: (controllers: Controller), aggregate: Count,
                min: Some(3), max: Some(2), exclude_observed_object: false,
            )"#,
            "BattlefieldAggregate min cannot exceed max",
        ),
        (
            "malformed filter union",
            r#"BattlefieldAggregate(
                filter: (controllers: Controller, any_of: Some([
                    (controllers: Controller, card_type: Some(Land)),
                ])),
                aggregate: Count, min: Some(1), exclude_observed_object: false,
            )"#,
            "battlefield permanent filter any_of requires at least two branches",
        ),
    ];

    for (label, condition, expected_error) in cases {
        let definition = fixture(condition);
        let error = invalid_program_reason(&definition);
        assert!(
            error.contains(expected_error),
            "{label}: expected {expected_error:?}, got {error:?}"
        );
    }
}

#[test]
fn program_if_accepts_controller_creature_maximum_power_conditions() {
    let body = r#"(steps: [(id: "qualifying_creature", kind: If(
        condition: BattlefieldAggregate(
            filter: (controllers: Controller, card_type: Some(Creature)),
            aggregate: MaximumPower, min: Some(4), max: None,
            exclude_observed_object: false,
        ),
        yes: (steps: [], yields: []),
        no: (steps: [], yields: []),
    ))], yields: [])"#;
    let program = ability_program_fixture("maximum_power_program_fixture", body)
        .expect("a Controller Creature MaximumPower ProgramIf compiles");
    let [instruction] = program.body.steps.as_slice() else {
        panic!("the fixture retains one ProgramIf instruction");
    };
    let crate::primitives::CompiledInstructionKind::If { condition, .. } = &instruction.kind else {
        panic!("the maximum-power predicate remains a typed ProgramIf");
    };

    let expected = crate::primitives::CompiledProgramCondition::Game(
        crate::primitives::GameCondition::BattlefieldAggregate {
            filter: crate::primitives::BattlefieldPermanentFilter {
                required_supertypes: Vec::new(),
                token: None,
                any_of: None,
                controllers: crate::primitives::RelativePlayerSet::Controller,
                card_type: Some(crate::primitives::CardTypeFilter::Creature),
                color: None,
                name: None,
                required_subtypes: Vec::new(),
                exclude_source: false,
            },
            aggregate: crate::primitives::BattlefieldAggregate::MaximumPower,
            min: Some(4),
            max: None,
            exclude_observed_object: false,
        },
    );
    assert_eq!(condition.as_ref(), &expected);
}

#[test]
fn program_choose_accepts_exact_controller_creature_total_power_ten_condition() {
    use crate::primitives::{
        ChooseDeparturePolicy, CompiledChooseRequirement, CompiledInstructionKind,
        RelativePlayerSet,
    };

    let condition = r#"BattlefieldAggregate(
        filter: (controllers: Controller, card_type: Some(Creature)),
        aggregate: TotalPower, min: Some(10), max: None,
        exclude_observed_object: false,
    )"#;
    let body = mosswort_total_power_choose_body(condition);
    let program = ability_program_fixture("mosswort_condition_compiler_fixture", &body)
        .expect("the exact Mosswort TotalPower condition compiles");
    let [instruction] = program.body.steps.as_slice() else {
        panic!("the condition fixture retains one Choose instruction");
    };
    let CompiledInstructionKind::Choose {
        departure_policy,
        branches,
        ..
    } = &instruction.kind
    else {
        panic!("the condition fixture retains a compiled Choose");
    };
    assert_eq!(
        *departure_policy,
        ChooseDeparturePolicy::DelegateNonCostChoice,
        "Mosswort's optional effect branch is a non-cost choice"
    );
    let [branch] = branches.as_slice() else {
        panic!("the condition fixture has one branch");
    };
    assert!(matches!(
        &branch.requirement,
        CompiledChooseRequirement::GameCondition(crate::primitives::GameCondition::BattlefieldAggregate {
            filter,
            aggregate: crate::primitives::BattlefieldAggregate::TotalPower,
            min: Some(10),
            max: None,
            exclude_observed_object: false,
        }) if filter.controllers == RelativePlayerSet::Controller
            && filter.card_type == Some(crate::primitives::CardTypeFilter::Creature)
            && filter.required_supertypes.is_empty()
            && filter.token.is_none()
            && filter.any_of.is_none()
            && filter.color.is_none()
            && filter.name.is_none()
            && filter.required_subtypes.is_empty()
            && !filter.exclude_source
    ));
}

fn mosswort_total_power_choose_body(condition: &str) -> String {
    format!(
        r#"(steps: [(id: "choose_linked_play", kind: Choose(
            choice: "choose_linked_play", chooser: Controller, optional: true,
            branches: [(branch_id: "play_linked_card", presentation: Fallback, cost: None,
                requirement: GameCondition({condition}),
                body: (steps: [], yields: []))],
            otherwise: (steps: [], yields: []),
        ))], yields: [])"#
    )
}

#[test]
fn program_choose_accepts_trade_route_envoy_live_countered_creature_branch() {
    use crate::primitives::{
        CompiledChooseRequirement, CompiledInstructionKind, GameCondition, PlayerRecipient,
        RelativePlayerSet,
    };

    let condition = r#"BattlefieldCreatureCount(
        filter: (controllers: Controller, requires_any_counter: true, exclude_source: false),
        min: Some(1), max: None,
    )"#;
    let body = trade_route_envoy_counter_choose_body(condition);
    let program = ability_program_fixture("trade_route_envoy_compiler_fixture", &body)
        .expect("the complete Trade Route Envoy Choose Program compiles");

    let [instruction] = program.body.steps.as_slice() else {
        panic!("Trade Route Envoy retains one root Choose instruction");
    };
    let CompiledInstructionKind::Choose {
        chooser,
        optional,
        branches,
        otherwise,
        ..
    } = &instruction.kind
    else {
        panic!("Trade Route Envoy retains one root Choose instruction");
    };
    assert_eq!(*chooser, PlayerRecipient::Controller);
    assert!(!optional, "the entry branch is mandatory");
    let [branch] = branches.as_slice() else {
        panic!("the entry Program has exactly one draw branch");
    };
    assert!(matches!(
        &branch.requirement,
        CompiledChooseRequirement::GameCondition(
            GameCondition::BattlefieldCreatureCount {
                filter,
                min: Some(1),
                max: None,
            }
        ) if filter.controllers == RelativePlayerSet::Controller
            && filter.subtype.is_none()
            && filter.required_keywords.is_empty()
            && filter.tapped.is_none()
            && filter.requires_any_counter
            && filter.required_counter.is_none()
            && !filter.exclude_source
    ));
    assert_eq!(branch.body.steps.len(), 1, "the branch draws one card");
    assert_eq!(
        otherwise.steps.len(),
        1,
        "Otherwise adds one source counter"
    );
}

fn trade_route_envoy_counter_choose_body(condition: &str) -> String {
    format!(
        r#"(steps: [(id: "draw_or_counter", kind: Choose(
            choice: "draw_if_countered", chooser: Controller, optional: false,
            branches: [(branch_id: "draw_a_card", presentation: Fallback, cost: None,
                requirement: GameCondition({condition}),
                body: (steps: [(id: "draw", kind: Perform(Draw(who: Controller, count: 1)))], yields: []))],
            otherwise: (steps: [(id: "put_counter", kind: Perform(PutCounters(
                counter: PlusOnePlusOne, count: 1, subject: Source,
            )))], yields: []),
        ))], yields: [])"#
    )
}

#[test]
fn program_choose_rejects_nonexact_trade_route_envoy_counter_conditions() {
    let base = r#"BattlefieldCreatureCount(
        filter: (controllers: Controller, requires_any_counter: true, exclude_source: false),
        min: Some(1), max: None,
    )"#;
    let cases = [
        (
            "opponent cohort",
            base.replace("controllers: Controller", "controllers: Opponents"),
        ),
        (
            "source exclusion",
            base.replace("exclude_source: false", "exclude_source: true"),
        ),
        (
            "subtype filter",
            base.replace(
                "controllers: Controller",
                "controllers: Controller, subtype: Some(\"Elf\")",
            ),
        ),
        (
            "keyword filter",
            base.replace(
                "controllers: Controller",
                "controllers: Controller, required_keywords: [Flying]",
            ),
        ),
        (
            "tapped filter",
            base.replace(
                "controllers: Controller",
                "controllers: Controller, tapped: Some(true)",
            ),
        ),
        (
            "specific counter instead of any counter",
            base.replace(
                "requires_any_counter: true",
                "requires_any_counter: false, required_counter: Some(PlusOnePlusOne)",
            ),
        ),
        (
            "wrong minimum",
            base.replace("min: Some(1)", "min: Some(2)"),
        ),
        ("missing minimum", base.replace("min: Some(1)", "min: None")),
        ("maximum bound", base.replace("max: None", "max: Some(2)")),
    ];

    for (label, condition) in cases {
        let body = trade_route_envoy_counter_choose_body(&condition);
        assert!(
            ability_program_fixture("trade_route_envoy_compiler_fixture", &body).is_err(),
            "the Program compiler must reject the {label} counter-count shape"
        );
    }
}

#[test]
fn program_if_still_rejects_trade_route_envoy_counter_condition() {
    let condition = r#"BattlefieldCreatureCount(
        filter: (controllers: Controller, requires_any_counter: true, exclude_source: false),
        min: Some(1), max: None,
    )"#;
    let body = format!(
        r#"(steps: [(id: "counter_check", kind: If(
            condition: {condition},
            yes: (steps: [], yields: []),
            no: (steps: [], yields: []),
        ))], yields: [])"#
    );
    let error = ability_program_fixture("trade_route_envoy_compiler_fixture", &body)
        .expect_err("the Choose-only profile must not widen generic Program If");
    assert!(
        error.contains("If condition uses context outside"),
        "unexpected Program If diagnostic: {error}"
    );
}

#[test]
fn program_choose_rejects_nonexact_controller_creature_total_power_conditions() {
    let base = r#"BattlefieldAggregate(
        filter: (controllers: Controller, card_type: Some(Creature)),
        aggregate: TotalPower, min: Some(10), max: None,
        exclude_observed_object: false,
    )"#;
    let cases = [
        (
            "opponent cohort",
            base.replace("controllers: Controller", "controllers: Opponents"),
        ),
        (
            "noncreature filter",
            base.replace("Some(Creature)", "Some(Land)"),
        ),
        ("wrong threshold", base.replace("Some(10)", "Some(9)")),
        ("maximum bound", base.replace("max: None", "max: Some(20)")),
        (
            "supertype filter",
            base.replace(
                "card_type: Some(Creature)",
                "card_type: Some(Creature), required_supertypes: [\"Legendary\"]",
            ),
        ),
        (
            "token filter",
            base.replace(
                "card_type: Some(Creature)",
                "card_type: Some(Creature), token: Some(true)",
            ),
        ),
        (
            "name filter",
            base.replace(
                "card_type: Some(Creature)",
                "card_type: Some(Creature), name: Some(\"Elf\")",
            ),
        ),
        (
            "subtype filter",
            base.replace(
                "card_type: Some(Creature)",
                "card_type: Some(Creature), required_subtypes: [\"Elf\"]",
            ),
        ),
        (
            "source exclusion",
            base.replace(
                "card_type: Some(Creature)",
                "card_type: Some(Creature), exclude_source: true",
            ),
        ),
        (
            "observed-object exclusion",
            base.replace(
                "exclude_observed_object: false",
                "exclude_observed_object: true",
            ),
        ),
    ];

    for (label, condition) in cases {
        let body = mosswort_total_power_choose_body(&condition);
        assert!(
            ability_program_fixture("mosswort_condition_compiler_fixture", &body).is_err(),
            "the Program compiler must reject the {label} TotalPower shape"
        );
    }
}

const MOSSWORT_LINKED_PLAY_BODY: &str = r#"(steps: [(id: "choose_linked_play", kind: Choose(
    choice: "choose_linked_play", chooser: Controller, optional: true,
    branches: [(branch_id: "play_linked_card", presentation: Fallback, cost: None,
        requirement: GameCondition(BattlefieldAggregate(
            filter: (controllers: Controller, card_type: Some(Creature)),
            aggregate: TotalPower, min: Some(10), max: None,
            exclude_observed_object: false,
        )),
        body: (steps: [(id: "play_selected_linked_card", kind: Perform(
            PlayLinkedExiledCard(
                producer_ability_id: "hideaway",
                linked_exile_id: "exiled_cards",
                filter: None,
                play_kind: Play,
                return_source_to_owner_hand_if_cast: false,
            ),
        ))], yields: []))],
    otherwise: (steps: [], yields: []),
))], yields: [])"#;

fn mosswort_terminal_program_owner() -> crate::ProgramOwnerId {
    crate::ProgramOwnerId::ability(crate::AbilityDefinitionId {
        card_id: "mosswort_bridge".into(),
        face_id: crate::CardFaceId::new("mosswort_bridge").unwrap(),
        ability_path: vec![crate::AbilityId::new("activated_02").unwrap()],
    })
}

fn parse_mosswort_terminal_program(text: &str) -> Result<crate::CardProgram, String> {
    ron::from_str(text).map_err(|error| error.to_string())
}

fn compile_mosswort_terminal_program(
    body: &str,
    owner: crate::ProgramOwnerId,
    context: crate::primitives::EffectContext,
) -> Result<crate::primitives::CompiledProgram, String> {
    let program = parse_mosswort_terminal_program(&format!("(body: {body})"))?;
    crate::primitives::CompiledProgram::compile(owner, context, &program)
}

fn mosswort_terminal_play_step(body: &str) -> &str {
    let start = body
        .find("body: (steps: [")
        .expect("fixture has a branch body")
        + "body: (steps: [".len();
    let end = body[start..]
        .find("], yields: [])")
        .expect("fixture has a branch body yield list")
        + start;
    &body[start..end]
}

fn replace_mosswort_fixture_last(text: &str, needle: &str, replacement: &str) -> String {
    let index = text
        .rfind(needle)
        .expect("fixture contains the requested final mutation point");
    format!(
        "{}{}{}",
        &text[..index],
        replacement,
        &text[index + needle.len()..]
    )
}

#[test]
fn mosswort_activated_program_accepts_exact_terminal_linked_play() {
    use crate::primitives::{ChooseDeparturePolicy, CompiledInstructionKind, EffectContext};

    let compiled = compile_mosswort_terminal_program(
        MOSSWORT_LINKED_PLAY_BODY,
        mosswort_terminal_program_owner(),
        EffectContext::Ability,
    )
    .unwrap_or_else(|error| panic!("Mosswort's exact terminal Program should compile: {error}"));

    let [choose] = compiled.body.steps.as_slice() else {
        panic!("Mosswort's Program contains one root Choose instruction");
    };
    let CompiledInstructionKind::Choose {
        departure_policy,
        branches,
        ..
    } = &choose.kind
    else {
        panic!("Mosswort's root instruction is Choose");
    };
    assert_eq!(
        *departure_policy,
        ChooseDeparturePolicy::DelegateNonCostChoice
    );
    let [branch] = branches.as_slice() else {
        panic!("Mosswort's Choose has exactly one branch");
    };
    assert!(matches!(
        branch.body.steps.as_slice(),
        [instruction]
            if instruction.input.is_none()
                && instruction.outputs.is_empty()
                && matches!(
                    &instruction.kind,
                    CompiledInstructionKind::Perform(effect)
                        if matches!(
                            effect.as_ref(),
                            crate::primitives::SpellEffectKind::PlayLinkedExiledCard {
                                producer_ability_id,
                                linked_exile_id,
                                filter: None,
                                play_kind: crate::primitives::LinkedExiledCardPlayKind::Play,
                                return_source_to_owner_hand_if_cast: false,
                            } if producer_ability_id.as_str() == "hideaway"
                                && linked_exile_id.as_str() == "exiled_cards"
                        )
                )
    ));
}

#[test]
fn mosswort_terminal_linked_play_profile_rejects_near_misses() {
    use crate::primitives::EffectContext;

    let branch_start = MOSSWORT_LINKED_PLAY_BODY
        .find("branches: [")
        .expect("fixture has one branch list")
        + "branches: [".len();
    let branch_end = MOSSWORT_LINKED_PLAY_BODY[branch_start..]
        .find("],\n    otherwise:")
        .expect("fixture has one Otherwise block")
        + branch_start;
    let original_branch = &MOSSWORT_LINKED_PLAY_BODY[branch_start..branch_end];
    let second_branch = original_branch.replace("play_linked_card", "second_branch");
    let multiple_branches = format!(
        "{} , {}{}",
        &MOSSWORT_LINKED_PLAY_BODY[..branch_end],
        second_branch,
        &MOSSWORT_LINKED_PLAY_BODY[branch_end..],
    );
    let terminal_play = mosswort_terminal_play_step(MOSSWORT_LINKED_PLAY_BODY);
    let nested_if = MOSSWORT_LINKED_PLAY_BODY.replace(
        &format!("body: (steps: [{terminal_play}], yields: [])"),
        &format!(
            "body: (steps: [(id: \"nested_if\", kind: If(condition: Void, yes: (steps: [{terminal_play}], yields: []), no: (steps: [], yields: [])))], yields: [])"
        ),
    );
    let nested_choose = MOSSWORT_LINKED_PLAY_BODY.replace(
        &format!("body: (steps: [{terminal_play}], yields: [])"),
        &format!(
            "body: (steps: [(id: \"nested_choose\", kind: Choose(choice: \"nested_choice\", chooser: Controller, optional: true, branches: [(branch_id: \"nested_branch\", presentation: Fallback, cost: None, body: (steps: [{terminal_play}], yields: []))], otherwise: (steps: [], yields: [])))], yields: [])"
        ),
    );
    let branch_yield =
        MOSSWORT_LINKED_PLAY_BODY.replace("))], yields: []))],", "))], yields: [\"missing\"]))],");
    let root_yield = replace_mosswort_fixture_last(
        MOSSWORT_LINKED_PLAY_BODY,
        "))], yields: [])",
        "))], yields: [\"missing\"])",
    );
    let filtered_linked_play = MOSSWORT_LINKED_PLAY_BODY
        .replace("filter: None,", "filter: Some((card_type: Some(Land))),");

    let cases = [
        (
            "wrong aggregate",
            MOSSWORT_LINKED_PLAY_BODY.replace("aggregate: TotalPower", "aggregate: MaximumPower"),
        ),
        (
            "wrong threshold",
            MOSSWORT_LINKED_PLAY_BODY.replace("min: Some(10)", "min: Some(9)"),
        ),
        (
            "wrong cohort",
            MOSSWORT_LINKED_PLAY_BODY.replace(
                "controllers: Controller",
                "controllers: Opponents",
            ),
        ),
        (
            "noncreature filter",
            MOSSWORT_LINKED_PLAY_BODY.replace("Some(Creature)", "Some(Land)"),
        ),
        (
            "bounded aggregate",
            MOSSWORT_LINKED_PLAY_BODY.replace("max: None", "max: Some(20)"),
        ),
        (
            "nonoptional choice",
            MOSSWORT_LINKED_PLAY_BODY.replace("optional: true", "optional: false"),
        ),
        (
            "paid branch",
            MOSSWORT_LINKED_PLAY_BODY.replace("cost: None", "cost: DiscardCard(filter: None)"),
        ),
        (
            "wrong producer ability",
            MOSSWORT_LINKED_PLAY_BODY.replace("producer_ability_id: \"hideaway\"", "producer_ability_id: \"other\""),
        ),
        (
            "wrong linked-exile id",
            MOSSWORT_LINKED_PLAY_BODY.replace("linked_exile_id: \"exiled_cards\"", "linked_exile_id: \"other\""),
        ),
        (
            "cast instead of Play",
            MOSSWORT_LINKED_PLAY_BODY.replace("play_kind: Play", "play_kind: Cast"),
        ),
        (
            "source return on cast",
            MOSSWORT_LINKED_PLAY_BODY.replace(
                "return_source_to_owner_hand_if_cast: false",
                "return_source_to_owner_hand_if_cast: true",
            ),
        ),
        (
            "nonempty Otherwise",
            MOSSWORT_LINKED_PLAY_BODY.replace(
                "otherwise: (steps: [], yields: [])",
                "otherwise: (steps: [(id: \"otherwise_tail\", kind: Perform(GainLife(amount: 1)))], yields: [])",
            ),
        ),
        (
            "branch tail",
            MOSSWORT_LINKED_PLAY_BODY.replace(
                "))], yields: []))],",
                ")), (id: \"branch_tail\", kind: Perform(GainLife(amount: 1)))], yields: []))],",
            ),
        ),
        (
            "root tail",
            replace_mosswort_fixture_last(
                MOSSWORT_LINKED_PLAY_BODY,
                "))], yields: [])",
                ")), (id: \"root_tail\", kind: Perform(GainLife(amount: 1)))], yields: [])",
            ),
        ),
        ("nested If", nested_if),
        ("nested Choose", nested_choose),
        ("branch yield", branch_yield),
        ("root yield", root_yield),
        ("filtered linked play", filtered_linked_play),
        ("multiple branches", multiple_branches),
        (
            "result binding",
            MOSSWORT_LINKED_PLAY_BODY.replace(
                "id: \"choose_linked_play\", kind:",
                "id: \"choose_linked_play\", bind: [\"unused\"], kind:",
            ),
        ),
        (
            "result input",
            MOSSWORT_LINKED_PLAY_BODY.replace(
                "id: \"choose_linked_play\", kind:",
                "id: \"choose_linked_play\", input: Some(\"missing\"), kind:",
            ),
        ),
    ];

    for (label, body) in cases {
        assert_ne!(
            body, MOSSWORT_LINKED_PLAY_BODY,
            "fixture mutation for {label}"
        );
        parse_mosswort_terminal_program(&format!("(body: {body})")).unwrap_or_else(|error| {
            panic!("the {label} negative fixture must be valid RON: {error}")
        });
        let error = compile_mosswort_terminal_program(
            &body,
            mosswort_terminal_program_owner(),
            EffectContext::Ability,
        )
        .expect_err("near-miss linked-Play profile must be rejected");
        if label == "root tail" || label == "branch tail" {
            assert!(
                error.contains("exact terminal Mosswort Bridge activated_02 Choose profile"),
                "{label} case should fail at the terminal-profile guard, got: {error}"
            );
        }
    }

    let target_bound = format!(
        "(body: {MOSSWORT_LINKED_PLAY_BODY}, target_bindings: [(instruction_id: \"choose_linked_play\", role: Target, target_group_index: 0)])"
    );
    let program = parse_mosswort_terminal_program(&target_bound)
        .expect("the target-bound negative fixture must be valid RON");
    assert!(
        crate::primitives::CompiledProgram::compile(
            mosswort_terminal_program_owner(),
            EffectContext::Ability,
            &program,
        )
        .is_err(),
        "nonempty target bindings are outside the terminal profile"
    );

    let wrong_owner = crate::ProgramOwnerId::ability(crate::AbilityDefinitionId {
        card_id: "other_card".into(),
        face_id: crate::CardFaceId::new("other_card").unwrap(),
        ability_path: vec![crate::AbilityId::new("activated_02").unwrap()],
    });
    assert!(compile_mosswort_terminal_program(
        MOSSWORT_LINKED_PLAY_BODY,
        wrong_owner,
        EffectContext::Ability,
    )
    .is_err());
    assert!(compile_mosswort_terminal_program(
        MOSSWORT_LINKED_PLAY_BODY,
        mosswort_terminal_program_owner(),
        EffectContext::Spell,
    )
    .is_err());
}

#[test]
fn program_if_rejects_unsupported_maximum_power_shapes() {
    let cases = [
        (
            "Controller cohort without an explicit Creature filter",
            r#"BattlefieldAggregate(
                filter: (controllers: Controller), aggregate: MaximumPower,
                min: Some(4), max: None, exclude_observed_object: false,
            )"#,
        ),
        (
            "opponent cohort",
            r#"BattlefieldAggregate(
                filter: (controllers: Opponents, card_type: Some(Creature)),
                aggregate: MaximumPower, min: Some(4), max: None,
                exclude_observed_object: false,
            )"#,
        ),
        (
            "noncreature filter",
            r#"BattlefieldAggregate(
                filter: (controllers: Controller, card_type: Some(Land)),
                aggregate: MaximumPower, min: Some(4), max: None,
                exclude_observed_object: false,
            )"#,
        ),
        (
            "required supertype filter",
            r#"BattlefieldAggregate(
                filter: (controllers: Controller, card_type: Some(Creature),
                    required_supertypes: ["Legendary"]),
                aggregate: MaximumPower, min: Some(4), max: None,
                exclude_observed_object: false,
            )"#,
        ),
        (
            "token filter",
            r#"BattlefieldAggregate(
                filter: (controllers: Controller, card_type: Some(Creature), token: Some(true)),
                aggregate: MaximumPower, min: Some(4), max: None,
                exclude_observed_object: false,
            )"#,
        ),
        (
            "disjunctive filter",
            r#"BattlefieldAggregate(
                filter: (controllers: Controller, card_type: Some(Creature), any_of: Some([
                    (controllers: Controller, card_type: Some(Creature)),
                    (controllers: Controller, required_subtypes: ["Dinosaur"]),
                ])),
                aggregate: MaximumPower, min: Some(4), max: None,
                exclude_observed_object: false,
            )"#,
        ),
        (
            "color filter",
            r#"BattlefieldAggregate(
                filter: (controllers: Controller, card_type: Some(Creature), color: Some(Green)),
                aggregate: MaximumPower, min: Some(4), max: None,
                exclude_observed_object: false,
            )"#,
        ),
        (
            "name filter",
            r#"BattlefieldAggregate(
                filter: (controllers: Controller, card_type: Some(Creature), name: Some("Grizzly Bears")),
                aggregate: MaximumPower, min: Some(4), max: None,
                exclude_observed_object: false,
            )"#,
        ),
        (
            "subtype filter",
            r#"BattlefieldAggregate(
                filter: (controllers: Controller, card_type: Some(Creature),
                    required_subtypes: ["Dinosaur"]),
                aggregate: MaximumPower, min: Some(4), max: None,
                exclude_observed_object: false,
            )"#,
        ),
        (
            "source exclusion",
            r#"BattlefieldAggregate(
                filter: (controllers: Controller, card_type: Some(Creature), exclude_source: true),
                aggregate: MaximumPower, min: Some(4), max: None,
                exclude_observed_object: false,
            )"#,
        ),
        (
            "observed-object exclusion",
            r#"BattlefieldAggregate(
                filter: (controllers: Controller, card_type: Some(Creature)),
                aggregate: MaximumPower, min: Some(4), max: None,
                exclude_observed_object: true,
            )"#,
        ),
        (
            "upper bound",
            r#"BattlefieldAggregate(
                filter: (controllers: Controller, card_type: Some(Creature)),
                aggregate: MaximumPower, min: Some(4), max: Some(5),
                exclude_observed_object: false,
            )"#,
        ),
        (
            "zero threshold",
            r#"BattlefieldAggregate(
                filter: (controllers: Controller, card_type: Some(Creature)),
                aggregate: MaximumPower, min: Some(0), max: None,
                exclude_observed_object: false,
            )"#,
        ),
        (
            "missing threshold",
            r#"BattlefieldAggregate(
                filter: (controllers: Controller, card_type: Some(Creature)),
                aggregate: MaximumPower, min: None, max: None,
                exclude_observed_object: false,
            )"#,
        ),
        (
            "unsupported TotalPower aggregate",
            r#"BattlefieldAggregate(
                filter: (controllers: Controller, card_type: Some(Creature)),
                aggregate: TotalPower, min: Some(4), max: None,
                exclude_observed_object: false,
            )"#,
        ),
    ];

    for (index, (label, condition)) in cases.into_iter().enumerate() {
        let body = format!(
            r#"(steps: [(id: "if", kind: If(
                condition: {condition},
                yes: (steps: [], yields: []),
                no: (steps: [], yields: []),
            ))], yields: [])"#
        );
        let card_id = format!("maximum_power_invalid_{index}");
        let result = ability_program_fixture(&card_id, &body);
        assert!(result.is_err(), "unsupported {label} was admitted");
    }
}

#[test]
fn program_accepts_resultless_each_opponent_fixed_life_loss() {
    let definition = program_fixture(
        r#"(steps: [
            (id: "lose_life", kind: Perform(LoseLife(
                amount: Fixed(3), who: EachOpponent,
            ))),
        ], yields: [])"#,
    );
    let registry = crate::CardRegistry::from_chunks_and_tokens(&[&definition], &[])
        .expect("fixed EachOpponent life loss is an admitted resultless program leaf");
    let owner = crate::ProgramOwnerId::face(
        "program_fixture",
        crate::CardFaceId::new("program_fixture").unwrap(),
    );
    let compiled = registry
        .compiled_program_for_owner(&owner)
        .expect("the exact fixture owner retains its compiled program");
    let [instruction] = compiled.body.steps.as_slice() else {
        panic!("the fixed loss remains one typed instruction")
    };
    assert!(matches!(
        &instruction.kind,
        crate::primitives::CompiledInstructionKind::Perform(effect)
            if matches!(effect.as_ref(), SpellEffectKind::LoseLife {
                amount: LifeAmount::Fixed(3),
                who: PlayerRecipient::EachOpponent,
            })
    ));
    assert!(
        instruction.outputs.is_empty(),
        "life loss has no typed result"
    );
}

#[test]
fn spell_and_ability_programs_admit_only_fixed_controller_life_loss() {
    use crate::primitives::{CompiledProgram, EffectContext};

    let compile = |card_id: &str, context, amount: &str, recipient: &str| {
        let program: crate::CardProgram = ron::from_str(&format!(
            r#"(body: (steps: [(id: "lose_life", bind: [], kind: Perform(LoseLife(
                amount: {amount}, who: {recipient},
            )))], yields: []))"#
        ))
        .expect("program fixture parses");
        let owner = if context == EffectContext::Spell {
            crate::ProgramOwnerId::face(card_id, crate::CardFaceId::new(card_id).unwrap())
        } else {
            crate::ProgramOwnerId::ability(crate::AbilityDefinitionId {
                card_id: card_id.into(),
                face_id: crate::CardFaceId::new(card_id).unwrap(),
                ability_path: vec![crate::AbilityId::new("triggered_01").unwrap()],
            })
        };
        CompiledProgram::compile(owner, context, &program)
    };

    let compiled = compile(
        "controller_life_loss_fixture",
        EffectContext::Ability,
        "Fixed(1)",
        "Controller",
    )
    .expect("the Blighted Blackthorn tail can lose one life from its ability controller");
    assert!(matches!(
        &compiled.body.steps[0].kind,
        crate::primitives::CompiledInstructionKind::Perform(effect)
            if matches!(effect.as_ref(), SpellEffectKind::LoseLife {
                amount: LifeAmount::Fixed(1),
                who: PlayerRecipient::Controller,
            })
    ));
    assert!(compiled.body.steps[0].outputs.is_empty());
    let spell = compile(
        "spell_controller_life_loss_fixture",
        EffectContext::Spell,
        "Fixed(2)",
        "Controller",
    )
    .expect("spell programs admit a fixed controller life-loss tail");
    assert_eq!(
        spell.owner,
        crate::ProgramOwnerId::face(
            "spell_controller_life_loss_fixture",
            crate::CardFaceId::new("spell_controller_life_loss_fixture").unwrap(),
        )
    );
    assert!(matches!(
        &spell.body.steps[0].kind,
        crate::primitives::CompiledInstructionKind::Perform(effect)
            if matches!(effect.as_ref(), SpellEffectKind::LoseLife {
                amount: LifeAmount::Fixed(2),
                who: PlayerRecipient::Controller,
            })
    ));
    assert!(spell.body.steps[0].outputs.is_empty());
    assert!(
        compile(
            "variable_controller_life_loss_fixture",
            EffectContext::Ability,
            "TargetManaValue",
            "Controller",
        )
        .is_err(),
        "target-dependent life loss remains outside this fixed tail"
    );
    assert!(
        compile(
            "variable_spell_controller_life_loss_fixture",
            EffectContext::Spell,
            "TargetManaValue",
            "Controller",
        )
        .is_err(),
        "spell controller life loss remains fixed-only"
    );
    assert!(
        compile(
            "unsupported_spell_recipient_fixture",
            EffectContext::Spell,
            "Fixed(2)",
            "AffectedPlayer",
        )
        .is_err(),
        "spell life loss does not admit unrelated recipients"
    );
}

#[test]
fn program_each_opponent_life_loss_rejects_unsupported_shapes_and_results() {
    let valid = program_fixture(
        r#"(steps: [
            (id: "lose_life", kind: Perform(LoseLife(
                amount: Fixed(3), who: EachOpponent,
            ))),
        ], yields: [])"#,
    );
    for (label, malformed) in [
        (
            "affected-player loss",
            valid.replace("who: EachOpponent", "who: AffectedPlayer"),
        ),
        (
            "target-dependent amount",
            valid.replace("amount: Fixed(3)", "amount: TargetManaValue"),
        ),
        (
            "attempted result binding",
            valid.replace(
                "id: \"lose_life\", kind:",
                "id: \"lose_life\", bind: [\"life_loss\"], kind:",
            ),
        ),
    ] {
        let error = invalid_program_reason(&malformed);
        assert!(
            error.contains("LoseLife") || error.contains("binds 1 names for 0 results"),
            "{label}: unexpected diagnostic: {error}"
        );
    }
}

#[test]
fn program_accepts_resultless_proliferate_leaf() {
    let definition = program_fixture(
        r#"(steps: [
            (id: "first_proliferate", bind: [], input: None,
             kind: Perform(Proliferate)),
            (id: "second_proliferate", bind: [], input: None,
             kind: Perform(Proliferate)),
        ], yields: [])"#,
    );
    let registry = crate::CardRegistry::from_chunks_and_tokens(&[&definition], &[])
        .expect("Proliferate is a resultless effect leaf in a typed resolution program");
    let owner = crate::ProgramOwnerId::face(
        "program_fixture",
        crate::CardFaceId::new("program_fixture").unwrap(),
    );
    let compiled = registry
        .compiled_program_for_owner(&owner)
        .expect("the exact fixture owner keeps its compiled program");
    assert_eq!(compiled.body.steps.len(), 2);
    assert!(compiled.body.steps.iter().all(|step| {
        matches!(
            &step.kind,
            crate::primitives::CompiledInstructionKind::Perform(effect)
                if matches!(effect.as_ref(), crate::primitives::SpellEffectKind::Proliferate)
        )
    }));
}

#[test]
fn program_ability_admits_two_source_effect_leaves_and_types_source_sacrifice() {
    let body = |leaf: &str, bind: &str| {
        format!(
            r#"(steps: [(id: "effect", bind: {bind}, input: None,
                kind: Perform({leaf}))], yields: [])"#
        )
    };
    let source_sacrifice = crate::primitives::SpellEffectKind::Sacrifice {
        subject: crate::primitives::EffectSubject::Source,
    };
    let source_cannot_be_blocked = crate::primitives::SpellEffectKind::ApplyCombatRestriction {
        scope: crate::primitives::CombatRestrictionScope::Source,
        restriction: crate::primitives::CombatRestriction {
            cant_be_blocked: true,
            ..Default::default()
        },
    };

    for (card_id, leaf, expected) in [
        (
            "program_source_sacrifice_fixture",
            "Sacrifice(subject: Source)",
            source_sacrifice,
        ),
        (
            "program_source_unblockable_fixture",
            "ApplyCombatRestriction(scope: Source, restriction: (cant_be_blocked: true))",
            source_cannot_be_blocked,
        ),
    ] {
        let compiled = ability_program_fixture(card_id, &body(leaf, "[]"))
            .unwrap_or_else(|error| panic!("the exact Ability source leaf is admitted: {error}"));
        assert_eq!(compiled.context, crate::primitives::EffectContext::Ability);
        assert!(compiled.required_inputs.is_empty());
        assert!(compiled.target_bindings.is_empty());
        assert!(compiled.slots.is_empty());
        assert!(compiled.body.yields.is_empty());
        let [step] = compiled.body.steps.as_slice() else {
            panic!("the resultless source effect remains one direct instruction")
        };
        assert!(step.input.is_none());
        assert!(step.outputs.is_empty());
        let crate::primitives::CompiledInstructionKind::Perform(effect) = &step.kind else {
            panic!("the source effect remains a Perform instruction")
        };
        assert_eq!(**effect, expected, "{card_id}");
    }

    for (label, leaf) in [
        ("source sacrifice", "Sacrifice(subject: Source)"),
        (
            "source combat restriction",
            "ApplyCombatRestriction(scope: Source, restriction: (cant_be_blocked: true))",
        ),
    ] {
        let spell = program_fixture(&body(leaf, "[]"));
        assert!(
            crate::CardRegistry::from_chunks_and_tokens(&[&spell], &[]).is_err(),
            "the Ability-only {label} is rejected for a spell owner"
        );
    }

    for (label, leaf) in [
        (
            "non-source sacrifice",
            "Sacrifice(subject: AttachedObject)",
        ),
        (
            "other source combat restriction",
            "ApplyCombatRestriction(scope: Source, restriction: (cant_attack: true))",
        ),
        (
            "combined source combat restriction flags",
            "ApplyCombatRestriction(scope: Source, restriction: (cant_be_blocked: true, cant_attack: true))",
        ),
        (
            "valid non-source combat restriction scope",
            "ApplyCombatRestriction(scope: Matching((kind: Creature)), restriction: (cant_be_blocked: true))",
        ),
    ] {
        let card_id = format!("program_reject_{}_fixture", label.replace(' ', "_"));
        assert!(
            ability_program_fixture(&card_id, &body(leaf, "[]")).is_err(),
            "only the fixed Source effect shape is admitted: {label}"
        );
    }

    let bound_sacrifice = ability_program_fixture(
        "program_bound_source_sacrifice_fixture",
        &body("Sacrifice(subject: Source)", "[\"sacrificed\"]"),
    )
    .expect("a source sacrifice can bind its typed committed-movement cohort");
    let [sacrifice] = bound_sacrifice.body.steps.as_slice() else {
        panic!("the bound source sacrifice remains one direct instruction")
    };
    assert_eq!(sacrifice.outputs.len(), 1);
    assert_eq!(
        bound_sacrifice
            .slot(sacrifice.outputs[0])
            .unwrap()
            .result_type,
        crate::primitives::ResultType::CardResultCohort {
            action: crate::primitives::CardResultAction::Sacrifice,
            who: crate::primitives::PlayerRecipient::Controller,
            cardinality: crate::primitives::Cardinality {
                min: 0,
                max: Some(1),
            },
        }
    );

    assert!(
        ability_program_fixture(
            "program_bound_source_restriction_fixture",
            &body(
                "ApplyCombatRestriction(scope: Source, restriction: (cant_be_blocked: true))",
                "[\"effect_result\"]",
            ),
        )
        .is_err(),
        "the resultless combat restriction still cannot bind a result"
    );
}

#[test]
fn program_admits_only_exact_private_chosen_library_look() {
    let valid = program_fixture(
        r#"(steps: [
            (id: "look", bind: [], input: None,
             kind: Perform(LookChooseToHand(
                count: 3, filter: None, min: 2, max: 2,
                reveal: false, bottom_order: Chosen,
             ))),
        ], yields: [])"#,
    );
    let registry = crate::CardRegistry::from_chunks_and_tokens(&[&valid], &[])
        .expect("the fixed private chosen library look is a resultless spell-program leaf");
    let owner = crate::ProgramOwnerId::face(
        "program_fixture",
        crate::CardFaceId::new("program_fixture").unwrap(),
    );
    let program = registry
        .compiled_program_for_owner(&owner)
        .expect("the exact spell face retains its compiled program");
    assert_eq!(program.context, crate::EffectContext::Spell);
    assert!(program.slots.is_empty());
    let [step] = program.body.steps.as_slice() else {
        panic!("the program has one library-look instruction");
    };
    assert!(step.input.is_none());
    assert!(step.outputs.is_empty());
    assert!(matches!(
        &step.kind,
        crate::primitives::CompiledInstructionKind::Perform(effect)
            if matches!(effect.as_ref(), crate::primitives::SpellEffectKind::LookChooseToHand {
                count: 3,
                filter: None,
                min: 2,
                max: 2,
                reveal: false,
                bottom_order: crate::primitives::LibraryBottomOrder::Chosen,
            })
    ));

    for (label, effect) in [
        (
            "filtered",
            "count: 3, filter: Some((card_type: Some(Instant))), min: 2, max: 2, reveal: false, bottom_order: Chosen",
        ),
        (
            "revealed",
            "count: 3, filter: None, min: 2, max: 2, reveal: true, bottom_order: Chosen",
        ),
        (
            "random-bottom",
            "count: 3, filter: None, min: 2, max: 2, reveal: false, bottom_order: Random",
        ),
        (
            "optional",
            "count: 3, filter: None, min: 0, max: 1, reveal: false, bottom_order: Chosen",
        ),
        (
            "non-exact",
            "count: 3, filter: None, min: 1, max: 2, reveal: false, bottom_order: Chosen",
        ),
    ] {
        let definition = program_fixture(&format!(
            r#"(steps: [(id: "look", bind: [], input: None,
                kind: Perform(LookChooseToHand({effect})) )], yields: [])"#
        ));
        assert!(
            crate::CardRegistry::from_chunks_and_tokens(&[&definition], &[]).is_err(),
            "{label} library-look program shape must remain outside the admitted subset"
        );
    }

    let result_binding = program_fixture(
        r#"(steps: [(id: "look", bind: ["looked"], input: None,
            kind: Perform(LookChooseToHand(
                count: 3, filter: None, min: 2, max: 2,
                reveal: false, bottom_order: Chosen,
            )))], yields: [])"#,
    );
    assert!(
        crate::CardRegistry::from_chunks_and_tokens(&[&result_binding], &[]).is_err(),
        "the resultless library-look leaf cannot publish a result binding"
    );

    let ability = ability_program_fixture(
        "look_choose_ability_fixture",
        r#"(steps: [(id: "look", bind: [], input: None,
            kind: Perform(LookChooseToHand(
                count: 3, filter: None, min: 2, max: 2,
                reveal: false, bottom_order: Chosen,
            )))], yields: [])"#,
    );
    assert!(
        ability.is_err(),
        "the private library-look program leaf is spell-only"
    );
}

#[test]
fn program_accepts_only_supported_resultless_reflexive_trigger_shapes() {
    let body = |effect: &str| {
        format!(
            r#"(steps: [
                (id: "create_reflexive", bind: [], input: None,
                 kind: Perform({effect})),
            ], yields: [])"#
        )
    };
    let sentry = body(
        r#"CreateReflexiveTrigger(ability: (
            ability_id: "reflexive_01", presentation: Fallback,
            effect: [Tap(subject: Chosen((kind: Creature, controller: Opponent)))],
        ))"#,
    );
    let sparktongue = body(
        r#"CreateReflexiveTrigger(ability: (
            ability_id: "reflexive_01", presentation: Fallback,
            effect: [DamageTarget(amount: 3, target: (kind: AnyTarget))],
        ))"#,
    );
    let creature_damage = body(
        r#"CreateReflexiveTrigger(ability: (
            ability_id: "reflexive_01", presentation: Fallback,
            effect: [DamageTarget(amount: 3, target: (kind: Creature))],
        ))"#,
    );
    for card in [&sentry, &sparktongue, &creature_damage] {
        let compiled = ability_program_fixture("program_reflexive_fixture", card)
            .expect("the ability-owned resultless reflexive target shape compiles");
        assert!(matches!(
            &compiled.body.steps[0].kind,
            crate::primitives::CompiledInstructionKind::Perform(effect)
                if matches!(effect.as_ref(), crate::primitives::SpellEffectKind::CreateReflexiveTrigger { .. })
        ));
    }

    for unsupported in [
        r#"CreateReflexiveTrigger(ability: (
            ability_id: "reflexive_01", presentation: Fallback,
            effect: [Draw(count: 1)],
        ))"#,
        r#"CreateReflexiveTrigger(ability: (
            ability_id: "reflexive_01", presentation: Fallback,
            effect: [Tap(subject: Chosen((kind: Creature, controller: You)))],
        ))"#,
    ] {
        let invalid = body(unsupported);
        let error = ability_program_fixture("program_reflexive_fixture", &invalid)
            .expect_err("unsupported ability-owned reflexive shape must fail closed");
        assert!(
            error.contains(
                "CreateReflexiveTrigger supports only the approved resultless target shapes"
            ),
            "unexpected reflexive admission error: {error}"
        );
    }
}

#[test]
fn program_reflexive_trigger_rejects_spell_without_source_capture() {
    let definition = program_fixture(
        r#"(steps: [(id: "create_reflexive", bind: [], input: None,
            kind: Perform(CreateReflexiveTrigger(ability: (
                ability_id: "reflexive_01", presentation: Fallback,
                effect: [DamageTarget(amount: 3, target: (kind: AnyTarget))],
            ))))], yields: [])"#,
    );
    assert!(
        crate::CardRegistry::from_chunks_and_tokens(&[&definition], &[]).is_err(),
        "a spell Program cannot stage a reflexive child without a captured permanent source"
    );
}

#[test]
fn choose_cast_cost_receipts_compile_for_exact_face_and_selected_mode_owners() {
    let face_program = choose_cast_cost_program("kicker_group", "kicker_option");
    let face_definition = format!(
        r#"(id: "face_choose_cost_receipt", name: "Face Choose Cost Receipt",
            face_id: "face_choose_cost_receipt", mana_cost: "{{0}}", types: ["Instant"],
            cast_cost_groups: [(group_id: "kicker_group", presentation: Fallback,
                min: 0, max: 1, options: [Mana(option_id: "kicker_option",
                    presentation: Fallback, kind: Kicker, cost: "{{1}}")])],
            spell_program: {face_program})"#
    );
    let mode_program = choose_cast_cost_program("kicker_group", "kicker_option");
    let mode_definition = format!(
        r#"(id: "mode_choose_cost_receipt", name: "Mode Choose Cost Receipt",
            face_id: "mode_choose_cost_receipt", mana_cost: "{{0}}", types: ["Instant"],
            cast_cost_groups: [(group_id: "kicker_group", presentation: Fallback,
                min: 0, max: 1, options: [Mana(option_id: "kicker_option",
                    presentation: Fallback, kind: Kicker, cost: "{{1}}")])],
            modal_spell: Some((min_modes: 1, max_modes: 1, modes: [
                (mode_id: "receipt_mode", presentation: Fallback, linked_cast_cost: None,
                    effects: [], program: {mode_program}, targeting: None)
            ])))"#
    );
    let registry =
        crate::CardRegistry::from_chunks_and_tokens(&[&face_definition, &mode_definition], &[])
            .expect("each exact spell owner can bind its face-local cast-cost receipt");
    for owner in [
        crate::ProgramOwnerId::face(
            "face_choose_cost_receipt",
            crate::CardFaceId::new("face_choose_cost_receipt").unwrap(),
        ),
        crate::ProgramOwnerId::face_mode(
            "mode_choose_cost_receipt",
            crate::CardFaceId::new("mode_choose_cost_receipt").unwrap(),
            crate::ModeId::new("receipt_mode").unwrap(),
        ),
    ] {
        let compiled = registry
            .compiled_program_for_owner(&owner)
            .unwrap_or_else(|| panic!("missing exact owner {owner:?}"));
        let crate::primitives::CompiledInstructionKind::Choose { branches, .. } =
            &compiled.body.steps[0].kind
        else {
            panic!("receipt-bearing branch remains a Choose instruction")
        };
        assert!(matches!(
            &branches[0].requirement,
            crate::primitives::CompiledChooseRequirement::CastCostReceipt(condition)
                if condition.group_id.as_str() == "kicker_group"
                    && condition.option_id.as_str() == "kicker_option"
                    && condition.expected_selected
        ));
    }
}

#[test]
fn choose_cast_cost_receipts_reject_unknown_group_option_and_sibling_face() {
    let definition = |card_id: &str, program_group: &str, program_option: &str| {
        let program = choose_cast_cost_program(program_group, program_option);
        format!(
            r#"(id: "{card_id}", name: "Receipt Negative",
                face_id: "{card_id}", mana_cost: "{{0}}", types: ["Instant"],
                cast_cost_groups: [(group_id: "kicker_group", presentation: Fallback,
                    min: 0, max: 1, options: [Mana(option_id: "kicker_option",
                        presentation: Fallback, kind: Kicker, cost: "{{1}}")])],
                spell_program: {program})"#
        )
    };
    let unknown_group = definition("choose_unknown_group", "sibling_group", "kicker_option");
    let error = crate::CardRegistry::from_chunks_and_tokens(&[&unknown_group], &[])
        .expect_err("a group name is bound only in the exact spell face owner")
        .to_string();
    assert!(error.contains("unknown owning cast-cost group"), "{error}");

    let unknown_option = definition("choose_unknown_option", "kicker_group", "sibling_option");
    let error = crate::CardRegistry::from_chunks_and_tokens(&[&unknown_option], &[])
        .expect_err("an option name is bound only in the exact owning group")
        .to_string();
    assert!(
        error.contains("unknown option in its owning cast-cost group"),
        "{error}"
    );

    let program = choose_cast_cost_program("kicker_group", "kicker_option");
    let sibling_face = format!(
        r#"(id: "choose_sibling_face", name: "Choose Sibling Face", layout: Split,
            faces: [
                (name: "Cost Face", face_id: "cost_face", mana_cost: "{{0}}",
                    types: ["Instant"], cast_cost_groups: [(group_id: "kicker_group",
                        presentation: Fallback, min: 0, max: 1,
                        options: [Mana(option_id: "kicker_option", presentation: Fallback,
                            kind: Kicker, cost: "{{1}}")])]),
                (name: "Program Face", face_id: "program_face", mana_cost: "{{0}}",
                    types: ["Instant"], spell_program: {program})
            ])"#
    );
    let error = crate::CardRegistry::from_chunks_and_tokens(&[&sibling_face], &[])
        .expect_err("a sibling face cannot lend its cast-cost group to another owner")
        .to_string();
    assert!(error.contains("unknown owning cast-cost group"), "{error}");
}

#[test]
fn registry_compiles_nested_delayed_program_under_its_exact_child_owner() {
    let fixture = r#"(id: "delayed_program_owner_fixture", name: "Delayed Program Owner Fixture",
        face_id: "delayed_program_owner_fixture", mana_cost: "{2}{W}",
        types: ["Legendary", "Creature"], power: 2, toughness: 1,
        triggered_abilities: [(
            ability_id: "triggered_01", presentation: Fallback,
            trigger: WhenSelfEntersBattlefield,
            targeting: Some((groups: [(
                min: 0, max: 1, prompt: "Choose up to one target creature",
                effect_indices: [0],
            )])),
            effect: [
                Exile(subject: Chosen((kind: Creature, excluded_objects: [Source]))),
                CreateDelayedTrigger(subject: Some(PreviousEffectObject), ability: (
                    ability_id: "delayed_01", presentation: Fallback,
                    trigger: AtBeginningOfNextEndStep,
                    program: Some((body: (steps: [
                        (id: "choose_return", kind: Choose(
                            choice: "pay_or_return", chooser: Controller, optional: true,
                            branches: [(branch_id: "pay", presentation: Fallback,
                                cost: Mana("{3}{B}"), requirement: Always,
                                body: (steps: [], yields: []))],
                            otherwise: (steps: [
                                (id: "return", kind: Perform(ReturnTriggeredCard(
                                    reference: ExactTriggerObject, from: [Exile],
                                    destination: Battlefield, controller: Owner,
                                )))
                            ], yields: []),
                        )),
                        (id: "gain_life", kind: Perform(GainLife(amount: 1)))
                    ], yields: []))),
                )),
            ],
        )])"#
        .to_owned();

    let registry = crate::CardRegistry::from_chunks_and_tokens(&[&fixture], &[])
        .expect("a delayed ability with an immutable nested definition ID compiles");
    let owner = crate::ProgramOwnerId::ability(crate::AbilityDefinitionId {
        card_id: "delayed_program_owner_fixture".into(),
        face_id: crate::CardFaceId::new("delayed_program_owner_fixture").unwrap(),
        ability_path: vec![
            crate::AbilityId::new("triggered_01").unwrap(),
            crate::AbilityId::new("delayed_01").unwrap(),
        ],
    });
    let compiled = registry
        .compiled_program_for_owner(&owner)
        .expect("the child definition owns the nested compiled program");
    assert_eq!(compiled.owner, owner);
    assert_eq!(compiled.body.steps.len(), 2);
    assert_eq!(
        compiled.required_inputs,
        vec![crate::primitives::ProgramRequiredInput::CapturedTriggerObjectRef]
    );
    assert!(matches!(
        &compiled.body.steps[0].kind,
        crate::primitives::CompiledInstructionKind::Choose { otherwise, .. }
            if matches!(
                otherwise.steps[0].kind,
                crate::primitives::CompiledInstructionKind::Perform(ref effect)
                    if matches!(effect.as_ref(), crate::primitives::SpellEffectKind::ReturnTriggeredCard {
                        reference: crate::primitives::TriggeredCardReference::ExactTriggerObject,
                        ..
                    })
            )
    ));

    for (label, invalid) in [
        (
            "another delayed-only timing",
            fixture.replace(
                "trigger: AtBeginningOfNextEndStep,\n                    program",
                "trigger: AtBeginningOfNextTurnUpkeep,\n                    program",
            ),
        ),
        (
            "a maximum-two parent target group",
            fixture.replace(
                "min: 0, max: 1, prompt: \"Choose up to one target creature\"",
                "min: 0, max: 2, prompt: \"Choose up to two target creatures\"",
            ),
        ),
        (
            "a may delayed child",
            fixture.replace(
                "trigger: AtBeginningOfNextEndStep,\n                    program",
                "trigger: AtBeginningOfNextEndStep, may: true,\n                    program",
            ),
        ),
        (
            "a once-only delayed child",
            fixture.replace(
                "trigger: AtBeginningOfNextEndStep,\n                    program",
                "trigger: AtBeginningOfNextEndStep, triggers_only_once: true,\n                    program",
            ),
        ),
        (
            "a capped delayed child",
            fixture.replace(
                "trigger: AtBeginningOfNextEndStep,\n                    program",
                "trigger: AtBeginningOfNextEndStep, max_triggers_per_turn: Some(1),\n                    program",
            ),
        ),
        (
            "an intervening-if delayed child",
            fixture.replace(
                "trigger: AtBeginningOfNextEndStep,\n                    program",
                "trigger: AtBeginningOfNextEndStep, intervening_if: Some(ControllerLibraryEmpty),\n                    program",
            ),
        ),
        (
            "a targeted delayed child",
            fixture.replace(
                "trigger: AtBeginningOfNextEndStep,\n                    program",
                "trigger: AtBeginningOfNextEndStep, targeting: Some((groups: [])),\n                    program",
            ),
        ),
        (
            "a non-adjacent Exile producer",
            fixture.replace(
                "Exile(subject: Chosen((kind: Creature, excluded_objects: [Source]))),\n                CreateDelayedTrigger",
                "Exile(subject: Chosen((kind: Creature, excluded_objects: [Source]))),\n                Draw(count: 1),\n                CreateDelayedTrigger",
            ),
        ),
        (
            "a plural previous-effect subject",
            fixture.replace(
                "subject: Some(PreviousEffectObject), ability:",
                "subject: Some(PreviousEffectObjects), ability:",
            ),
        ),
    ] {
        let error = crate::CardRegistry::from_chunks_and_tokens(&[&invalid], &[])
            .expect_err(&format!("{label} is outside this exact delayed-owner contract"))
            .to_string();
        let expected = match label {
            "a non-adjacent Exile producer" => {
                "PreviousEffectObject requires an immediately preceding compatible single-object producer"
            }
            "a plural previous-effect subject" => {
                "PreviousEffectObjects delayed triggers require one next-end-step ExileObservedObjects effect"
            }
            _ => "authored delayed child programs require",
        };
        assert!(error.contains(expected), "{label}: {error}");
    }

    let return_leaf = "reference: ExactTriggerObject, from: [Exile],\n                                    destination: Battlefield, controller: Owner,";
    for (label, replacement) in [
        ("wrong reference", "reference: TriggerObject, from: [Exile],\n                                    destination: Battlefield, controller: Owner,"),
        ("wrong source zone", "reference: ExactTriggerObject, from: [Graveyard],\n                                    destination: Battlefield, controller: Owner,"),
        ("wrong destination", "reference: ExactTriggerObject, from: [Exile],\n                                    destination: Hand, controller: Owner,"),
        ("wrong controller", "reference: ExactTriggerObject, from: [Exile],\n                                    destination: Battlefield, controller: AbilityController,"),
        ("tapped entry", "reference: ExactTriggerObject, from: [Exile],\n                                    destination: Battlefield, controller: Owner, tapped: true,"),
        (
            "entry counters",
            "reference: ExactTriggerObject, from: [Exile],\n                                    destination: Battlefield, controller: Owner, entry_counters: [(counter: PlusOnePlusOne, count: 1)],",
        ),
        (
            "type-setting replacement",
            "reference: ExactTriggerObject, from: [Exile],\n                                    destination: Battlefield, controller: Owner, set_types: Some((card_types: [Creature])),",
        ),
    ] {
        let invalid = fixture.replace(return_leaf, replacement);
        assert_ne!(invalid, fixture, "{label} mutation reaches the return leaf");
        let error = crate::CardRegistry::from_chunks_and_tokens(&[&invalid], &[])
            .expect_err(&format!("{label} is outside the exact ReturnTriggeredCard leaf"))
            .to_string();
        assert!(
            error.contains("effect is not in the resolution-program operation whitelist"),
            "{label}: {error}"
        );
    }
}

#[test]
fn program_compiler_resolves_only_dominating_discard_result_conditions() {
    let definition = program_fixture(
        r#"(steps: [
            (id: "discard", bind: ["discarded"], kind: Perform(Discard(
                who: Controller, quantity: OptionalExact(2)
            ))),
            (id: "paid", kind: IfResult(
                condition: DiscardOutcome(result: "discarded", outcome: ChosenToPay),
                yes: (steps: [], yields: []),
                no: (steps: [], yields: []),
            )),
        ], yields: [])"#,
    );
    let registry = crate::CardRegistry::from_chunks_and_tokens(&[&definition], &[])
        .expect("a typed optional discard result can control a later program branch");
    let owner = crate::ProgramOwnerId::face(
        "program_fixture",
        crate::CardFaceId::new("program_fixture").unwrap(),
    );
    assert!(registry.compiled_program_for_owner(&owner).is_some());

    for (result, expected) in [
        ("later", "not in scope"),
        ("drawn", "not a compatible discard result"),
    ] {
        let body = format!(
            r#"(steps: [
                (id: "draw", bind: ["drawn"], kind: Perform(Draw(count: 1))),
                (id: "paid", kind: IfResult(
                    condition: DiscardOutcome(result: "{result}", outcome: ChosenToPay),
                    yes: (steps: [], yields: []), no: (steps: [], yields: []),
                )),
            ], yields: [])"#
        );
        let invalid = program_fixture(&body);
        let error = invalid_program_reason(&invalid);
        assert!(error.contains(expected), "{error}");
    }
}

#[test]
fn program_conditions_accept_typed_counter_payment_result_ron() {
    for outcome in ["Paid", "Declined", "NotOffered", "ForcedUnpaid"] {
        let condition = ron::from_str::<crate::primitives::ProgramResultCondition>(&format!(
            r#"CounterPayment(result: "payment", outcome: {outcome})"#,
        ));
        assert!(
            condition.is_ok(),
            "a fixed-cost counter result should expose {outcome} as a typed payment status"
        );
    }
}

#[test]
fn program_compiler_types_fixed_cost_counter_payment_results() {
    let body: crate::primitives::AuthoredBlock = ron::from_str(
        r#"(steps: [
            (id: "counter", bind: ["payment"], kind: Perform(CounterTargetSpell(
                spell_filter: (), unless_controller_pays: Some(2)
            ))),
            (id: "paid", kind: IfResult(
                condition: CounterPayment(result: "payment", outcome: Paid),
                yes: (steps: [], yields: []),
                no: (steps: [], yields: []),
            )),
        ], yields: [])"#,
    )
    .expect("typed counter-payment result fixture parses");
    let program = crate::primitives::CardProgram {
        body,
        target_bindings: vec![crate::primitives::ProgramTargetReference {
            instruction_id: "counter".into(),
            role: crate::primitives::ProgramTargetRole::Spell,
            target_group_index: 0,
        }],
    };
    let targeting = crate::primitives::TargetingDef {
        groups: vec![crate::primitives::TargetGroupDef {
            chooser: crate::primitives::TargetChooser::Controller,
            min: 1,
            max: 1,
            prompt: "Choose a target".into(),
            effect_indices: Vec::new(),
            distinct_from: Vec::new(),
            same_graveyard: false,
            cast_cost_expansion: None,
        }],
    };
    let owner = crate::ProgramOwnerId::face(
        "counter_payment_program_fixture",
        crate::CardFaceId::new("counter_payment_program_fixture").unwrap(),
    );
    let compiled = crate::primitives::CompiledProgram::compile_with_targeting(
        owner,
        crate::primitives::EffectContext::Spell,
        &program,
        Some(&targeting),
    )
    .expect("fixed-cost soft-counter results may feed a typed paid branch");

    assert_eq!(
        compiled
            .slot(compiled.body.steps[0].outputs[0])
            .unwrap()
            .result_type,
        crate::primitives::ResultType::CounterPaymentOutcome,
    );
    let crate::primitives::CompiledInstructionKind::If { condition, .. } =
        &compiled.body.steps[1].kind
    else {
        panic!("payment status remains a typed result condition");
    };
    assert!(matches!(
        condition.as_ref(),
        crate::primitives::CompiledProgramCondition::Result(
            crate::primitives::CompiledProgramResultCondition::CounterPayment {
                result,
                outcome: crate::primitives::CounterPaymentOutcome::Paid,
            }
        ) if *result == compiled.body.steps[0].outputs[0]
    ));
}

#[test]
fn counter_payment_outputs_remain_optional_and_conditions_are_typed() {
    let compile = |steps: &str| {
        let body: crate::primitives::AuthoredBlock =
            ron::from_str(&format!("(steps: [{steps}], yields: [])"))
                .expect("counter-payment program fixture parses");
        let program = crate::primitives::CardProgram {
            body,
            target_bindings: vec![crate::primitives::ProgramTargetReference {
                instruction_id: "counter".into(),
                role: crate::primitives::ProgramTargetRole::Spell,
                target_group_index: 0,
            }],
        };
        let owner = crate::ProgramOwnerId::face(
            "counter_payment_optional_fixture",
            crate::CardFaceId::new("counter_payment_optional_fixture").unwrap(),
        );
        let targeting = crate::primitives::TargetingDef {
            groups: vec![crate::primitives::TargetGroupDef {
                chooser: crate::primitives::TargetChooser::Controller,
                min: 1,
                max: 1,
                prompt: "Choose a target".into(),
                effect_indices: Vec::new(),
                distinct_from: Vec::new(),
                same_graveyard: false,
                cast_cost_expansion: None,
            }],
        };
        crate::primitives::CompiledProgram::compile_with_targeting(
            owner,
            crate::primitives::EffectContext::Spell,
            &program,
            Some(&targeting),
        )
    };

    let unbound = compile(
        r#"(id: "counter", bind: [], kind: Perform(CounterTargetSpell(
            spell_filter: (), unless_controller_pays: Some(2)
        )))"#,
    )
    .expect("fixed-cost counter result binding remains optional");
    assert!(unbound.body.steps[0].outputs.is_empty());

    let plain_counter = compile(
        r#"(id: "counter", bind: [], kind: Perform(CounterTargetSpell(
            spell_filter: (), unless_controller_pays: None
        )))"#,
    )
    .expect("plain counters remain valid with no payment output");
    assert!(plain_counter.body.steps[0].outputs.is_empty());

    let out_of_scope = compile(
        r#"(id: "counter", bind: [], kind: Perform(CounterTargetSpell(
            spell_filter: (), unless_controller_pays: Some(2)
        ))), (id: "paid", kind: IfResult(
            condition: CounterPayment(result: "later", outcome: Paid),
            yes: (steps: [], yields: []), no: (steps: [], yields: [])
        ))"#,
    )
    .expect_err("a payment condition cannot read a future result");
    assert!(out_of_scope.contains("not in scope"), "{out_of_scope}");

    let wrong_type = compile(
        r#"(id: "draw", bind: ["drawn"], kind: Perform(Draw(
            who: Controller, count: 1
        ))), (id: "counter", bind: [], kind: Perform(CounterTargetSpell(
            spell_filter: (), unless_controller_pays: Some(2)
        ))), (id: "paid", kind: IfResult(
            condition: CounterPayment(result: "drawn", outcome: Paid),
            yes: (steps: [], yields: []), no: (steps: [], yields: [])
        ))"#,
    )
    .expect_err("a payment condition cannot read a draw result");
    assert!(
        wrong_type.contains("not a compatible counter-payment result"),
        "{wrong_type}"
    );

    let cast_cost_payment = compile(
        r#"(id: "counter", bind: [], kind: Perform(CounterTargetSpell(
            spell_filter: (), unless_controller_pays_by_cast_cost: Some((
                condition: (group_id: "cast_cost_01", option_id: "option_01", expected_selected: true),
                if_selected: 4, otherwise: 2
            ))
        )))"#,
    )
    .expect_err("by-cast-cost counter programs remain outside this result contract");
    assert!(
        cast_cost_payment.contains("cast-cost payment is outside the typed program input contract"),
        "{cast_cost_payment}"
    );
}

#[test]
fn spell_program_typechecks_a_generation_bound_permanent_result_consumer() {
    let program = format!(
        "(steps: [{}, (id: \"untap_lands\", input: Some(\"lands\"), kind: Perform(UntapChosenPermanents))], yields: [])",
        choose_lands_instruction("choose_lands", "lands"),
    );
    let definition = program_fixture(&program);
    let registry = crate::CardRegistry::from_chunks_and_tokens(&[&definition], &[])
        .expect("a ChoosePermanents result may feed an explicitly bound untap instruction");
    let face = registry
        .get("program_fixture")
        .expect("fixture registered")
        .primary_face();
    assert!(
        face.spell_effect.is_empty(),
        "program is the spell resolution owner"
    );
    assert!(face.custom_effect.is_none());
    assert!(
        face.spell_program.is_some(),
        "program remains part of normalized face data"
    );
    let face_owner = crate::ProgramOwnerId::face(
        "program_fixture",
        crate::CardFaceId::new("program_fixture").unwrap(),
    );
    let compiled = registry
        .compiled_program_for_owner(&face_owner)
        .expect("registry keeps the immutable compiled form separate from face data");
    assert!(
        std::ptr::eq(
            compiled,
            registry
                .compiled_program("program_fixture", "program_fixture")
                .expect("compatibility accessor resolves the exact face owner")
        ),
        "the existing face lookup delegates to the typed owner key"
    );
    assert_eq!(compiled.owner, face_owner);
    assert_eq!(compiled.context, super::EffectContext::Spell);
    assert_eq!(
        compiled.target_projection,
        super::ProgramTargetProjection::OwnerLocal,
        "the program resolves targets through its exact owner's local projection"
    );
    assert!(
        registry
            .compiled_program_for_owner(&crate::ProgramOwnerId::face_mode(
                "program_fixture",
                crate::CardFaceId::new("program_fixture").unwrap(),
                crate::ModeId::new("same_local_name").unwrap(),
            ))
            .is_none(),
        "a mode owner never falls back to its containing face program"
    );
    let normalized = ron::ser::to_string(face).expect("normalized face serializes");
    assert!(
        normalized.contains("spell_program"),
        "the validated program must remain part of the normalized face"
    );
}

#[test]
fn program_compiler_preserves_owner_context_and_local_target_projection() {
    let definition = program_fixture(
        r#"(steps: [(id: "draw", bind: ["drawn"], kind: Perform(Draw(count: 1)))], yields: [])"#,
    );
    let registry = crate::CardRegistry::from_chunks_and_tokens(&[&definition], &[])
        .expect("face program fixture registers");
    let face = registry
        .get("program_fixture")
        .expect("fixture registered")
        .primary_face();
    let program = face.spell_program.as_ref().expect("authored face program");
    let mode_id = crate::ModeId::new("choose_mode").unwrap();
    let mode_owner = crate::ProgramOwnerId::face_mode(
        "program_fixture",
        crate::CardFaceId::new("program_fixture").unwrap(),
        mode_id.clone(),
    );

    let compiled_mode =
        super::CompiledProgram::compile(mode_owner.clone(), super::EffectContext::Spell, program)
            .expect(
                "compiler accepts the explicit spell context for metadata-only mode compilation",
            );
    assert_eq!(compiled_mode.owner, mode_owner);
    assert_eq!(compiled_mode.context, super::EffectContext::Spell);
    assert_eq!(
        compiled_mode.target_projection,
        super::ProgramTargetProjection::ModeLocal { mode_id },
        "a mode's target projection is addressed by its stable local ModeId"
    );

    let ability_owner = crate::ProgramOwnerId::ability(crate::AbilityDefinitionId {
        card_id: "program_fixture".into(),
        face_id: crate::CardFaceId::new("program_fixture").unwrap(),
        ability_path: vec![crate::AbilityId::new("activated_01").unwrap()],
    });
    let compiled_ability = super::CompiledProgram::compile(
        ability_owner.clone(),
        super::EffectContext::Ability,
        program,
    )
    .expect("compiler carries the actual ability effect context");
    assert_eq!(compiled_ability.owner, ability_owner);
    assert_eq!(compiled_ability.context, super::EffectContext::Ability);
    assert_eq!(
        compiled_ability.target_projection,
        super::ProgramTargetProjection::OwnerLocal
    );
}

#[test]
fn public_program_compilers_reject_target_leaves_without_a_complete_schema() {
    let body: super::AuthoredBlock = ron::from_str(
        r#"(steps: [(id: "damage", kind: Perform(
            DamageTarget(amount: 3, target: (kind: Creature))
        ))], yields: [])"#,
    )
    .expect("target leaf fixture parses");
    let owner = crate::ProgramOwnerId::face(
        "public_target_compile_fixture",
        crate::CardFaceId::new("public_target_compile_fixture").unwrap(),
    );
    let bound_program = super::CardProgram {
        body: body.clone(),
        target_bindings: vec![super::ProgramTargetReference {
            instruction_id: "damage".into(),
            role: super::ProgramTargetRole::Target,
            target_group_index: 0,
        }],
    };
    let unbound_program = super::CardProgram {
        body,
        target_bindings: Vec::new(),
    };

    let plain_compile =
        super::CompiledProgram::compile(owner.clone(), super::EffectContext::Spell, &bound_program);
    let missing_schema_compile = super::CompiledProgram::compile_with_targeting(
        owner,
        super::EffectContext::Spell,
        &unbound_program,
        None,
    );

    assert!(
        plain_compile.is_err(),
        "plain public compilation must not return a targetful body without target-group metadata"
    );
    assert!(
        missing_schema_compile.is_err(),
        "targeting-aware compilation must validate target roles even when targeting is absent"
    );
}

#[test]
fn program_compiler_rejects_untyped_dynamic_and_event_bound_leaf_parameters() {
    let compile_error = |effect: &str, context: super::EffectContext| {
        let body: super::AuthoredBlock = ron::from_str(&format!(
            r#"(steps: [(id: "effect", bind: [], input: None,
                kind: Perform({effect}))], yields: [])"#
        ))
        .expect("leaf fixture parses");
        let program = super::CardProgram {
            body,
            target_bindings: Vec::new(),
        };
        let owner = match context {
            super::EffectContext::Spell => crate::ProgramOwnerId::face(
                "leaf_contract_fixture",
                crate::CardFaceId::new("leaf_contract_fixture").unwrap(),
            ),
            super::EffectContext::Ability => {
                crate::ProgramOwnerId::ability(crate::AbilityDefinitionId {
                    card_id: "leaf_contract_fixture".into(),
                    face_id: crate::CardFaceId::new("leaf_contract_fixture").unwrap(),
                    ability_path: vec![crate::AbilityId::new("triggered_01").unwrap()],
                })
            }
        };
        super::CompiledProgram::compile(owner, context, &program)
            .expect_err("unsupported dynamic or event-bound parameter must fail closed")
    };

    for (effect, context, expected) in [
        (
            "DamageTarget(amount: Conditional(condition: ControllerLibraryEmpty, when_true: 1, otherwise: 2), target: (kind: Creature))",
            super::EffectContext::Spell,
            "DamageTarget amount must be fixed, or X",
        ),
        (
            "CounterTargetSpell(unless_controller_pays: Some(EventCount))",
            super::EffectContext::Spell,
            "spells cannot reference a trigger event's count",
        ),
        (
            "ReturnToOwnersHand(subject: PreviousEffectObject)",
            super::EffectContext::Spell,
            "owner-local chosen target",
        ),
        (
            "CreateTokenCopies(count: 1, source: Source)",
            super::EffectContext::Ability,
            "CreateTokenCopies source is not supported in a resolution program",
        ),
        (
            "DamageTarget(amount: 1, target: (kind: Creature, controller: DefendingPlayer))",
            super::EffectContext::Spell,
            "defending player outside its typed program inputs",
        ),
    ] {
        let error = compile_error(effect, context);
        assert!(error.contains(expected), "{effect}: {error}");
    }
}

#[test]
fn program_compiler_accepts_a_bound_mill_cohort_for_choose_card() {
    let body: super::AuthoredBlock = ron::from_str(
        r#"(steps: [
            (id: "mill", bind: ["milled"], input: None,
                kind: Perform(Mill(count: 4, who: Controller))),
            (id: "recover", bind: [], input: Some("milled"),
                kind: Perform(ChooseCard(
                    source: CardResultCohort(players: Controller),
                    filter: (any_of: Some([
                        (card_type: Some(Land)),
                        (card_type: Some(NonlandPermanent)),
                    ])),
                    destination: Hand,
                    optional: true,
                )))
        ], yields: [])"#,
    )
    .expect("the bounded Mill-to-ChooseCard authoring form should parse");
    let program = super::CardProgram {
        body,
        target_bindings: Vec::new(),
    };
    let owner = crate::ProgramOwnerId::face(
        "mill_choose_card_fixture",
        crate::CardFaceId::new("mill_choose_card_fixture").unwrap(),
    );
    let compiled = super::CompiledProgram::compile(owner, super::EffectContext::Spell, &program)
        .expect("ChooseCard should consume a bound Mill result cohort");
    assert_eq!(compiled.body.steps[1].input, Some(super::ResultSlotId(0)));
    assert!(matches!(
        compiled.slots[0].result_type,
        super::ResultType::CardResultCohort { .. }
    ));
}

#[test]
fn gifts_typed_program_shape_compiles_with_an_explicit_target_binding() {
    let definition = r#"(id: "gifts_program_fixture", name: "Gifts Program Fixture",
        face_id: "gifts_program_fixture", mana_cost: "{3}{U}", types: ["Instant"],
        spell_program: Some((
            body: (steps: [
                (id: "search", bind: ["found"], input: None,
                    kind: Perform(SearchLibrary(
                        who: Controller, optional: false, count: 4,
                        count_by_cast_cost: None, filter: None,
                        selection_constraint: Some(DifferentNames), slots: [],
                        zones: Fixed([Library]), destination: KeepInLibrary,
                        conditional_destination: None, shuffle: false, reveal: false,
                        result_id: None, selection_bounds: Some((min: 0, max: 4)),
                    ))),
                (id: "reveal", bind: ["revealed"], input: Some("found"),
                    kind: Perform(RevealCohort)),
                (id: "choose_split", bind: ["partition"], input: Some("revealed"),
                    kind: Perform(ChooseCohortPartition(
                        chooser: TargetedPlayer(group_index: 0, kind: OpponentPlayer),
                        selection: ExactlyUpTo(2),
                    ))),
                (id: "move_split", bind: [], input: Some("partition"),
                    kind: Perform(MoveCohortPartition(
                        selected_to: Graveyard, remainder_to: Hand,
                    ))),
                (id: "shuffle_library", bind: [], input: None,
                    kind: Perform(ShuffleLibrary(who: Controller))),
            ], yields: []),
            target_bindings: [
                (instruction_id: "choose_split", role: Target, target_group_index: 0),
            ],
        )),
        targeting: Some((groups: [(
            min: 1, max: 1, prompt: "Choose target opponent", effect_indices: [],
        )])))"#;

    let registry = crate::CardRegistry::from_chunks_and_tokens(&[definition], &[])
        .expect("the complete typed Gifts authoring shape compiles");
    let owner = crate::ProgramOwnerId::face(
        "gifts_program_fixture",
        crate::CardFaceId::new("gifts_program_fixture").unwrap(),
    );
    let compiled = registry
        .compiled_program_for_owner(&owner)
        .expect("the Gifts program is retained by the exact face owner");
    assert_eq!(compiled.target_bindings.len(), 1);
}

#[test]
fn gifts_partition_rejects_nonopponent_and_missing_target_bindings() {
    let definition = r#"(id: "gifts_program_fixture", name: "Gifts Program Fixture",
        face_id: "gifts_program_fixture", mana_cost: "{3}{U}", types: ["Instant"],
        spell_program: Some((
            body: (steps: [
                (id: "search", bind: ["found"], input: None,
                    kind: Perform(SearchLibrary(
                        who: Controller, optional: false, count: 4,
                        count_by_cast_cost: None, filter: None,
                        selection_constraint: Some(DifferentNames), slots: [],
                        zones: Fixed([Library]), destination: KeepInLibrary,
                        conditional_destination: None, shuffle: false, reveal: false,
                        result_id: None, selection_bounds: Some((min: 0, max: 4)),
                    ))),
                (id: "reveal", bind: ["revealed"], input: Some("found"),
                    kind: Perform(RevealCohort)),
                (id: "choose_split", bind: ["partition"], input: Some("revealed"),
                    kind: Perform(ChooseCohortPartition(
                        chooser: TargetedPlayer(group_index: 0, kind: AnyPlayer),
                        selection: ExactlyUpTo(2),
                    ))),
                (id: "move_split", bind: [], input: Some("partition"),
                    kind: Perform(MoveCohortPartition(
                        selected_to: Graveyard, remainder_to: Hand,
                    ))),
                (id: "shuffle_library", bind: [], input: None,
                    kind: Perform(ShuffleLibrary(who: Controller))),
            ], yields: []),
            target_bindings: [
                (instruction_id: "choose_split", role: Target, target_group_index: 0),
            ],
        )),
        targeting: Some((groups: [(
            min: 1, max: 1, prompt: "Choose target opponent", effect_indices: [],
        )])))"#;

    let error = crate::CardRegistry::from_chunks_and_tokens(&[definition], &[])
        .expect_err("cohort partition only admits the intended opponent chooser");
    assert!(format!("{error:#}").contains("must target an opponent"));

    let missing_binding = definition
        .replace("kind: AnyPlayer", "kind: OpponentPlayer")
        .replace(
            "target_bindings: [\n                (instruction_id: \"choose_split\", role: Target, target_group_index: 0),\n            ],",
            "target_bindings: [],",
        );
    let error = crate::CardRegistry::from_chunks_and_tokens(&[&missing_binding], &[])
        .expect_err("a targeted cohort chooser must bind to the authored opponent group")
        .to_string();
    assert!(
        error.contains("target group must reference at least one instruction role"),
        "missing target binding should be an admission error: {error}"
    );
}

#[test]
fn intuition_retained_search_without_different_names_compiles_with_fixed_three_card_bounds() {
    let intuition = r#"(id: "intuition_retained_fixture", name: "Intuition Retained Fixture",
        face_id: "intuition_retained_fixture", mana_cost: "{1}{U}{U}", types: ["Instant"],
        spell_program: Some((body: (steps: [
            (id: "search", bind: ["found"], input: None,
                kind: Perform(SearchLibrary(
                    who: Controller, optional: false, count: 3,
                    count_by_cast_cost: None, filter: None,
                    selection_constraint: None, slots: [],
                    zones: Fixed([Library]), destination: KeepInLibrary,
                    conditional_destination: None, shuffle: false, reveal: false,
                    result_id: None, selection_bounds: Some((min: 3, max: 3)),
                ))),
            (id: "reveal", bind: ["revealed"], input: Some("found"),
                kind: Perform(RevealCohort)),
            (id: "choose_split", bind: ["partition"], input: Some("revealed"),
                kind: Perform(ChooseCohortPartition(
                    chooser: TargetedPlayer(group_index: 0, kind: OpponentPlayer),
                    selection: ExactlyUpTo(1),
                ))),
            (id: "move_split", bind: [], input: Some("partition"),
                kind: Perform(MoveCohortPartition(
                    selected_to: Hand, remainder_to: Graveyard,
                ))),
            (id: "shuffle", bind: [], input: None,
                kind: Perform(ShuffleLibrary(who: Controller))),
        ], yields: []), target_bindings: [
            (instruction_id: "choose_split", role: Target, target_group_index: 0),
        ])),
        targeting: Some((groups: [(
            min: 1, max: 1, prompt: "Choose target opponent", effect_indices: [],
        )])))"#;
    let registry = crate::CardRegistry::from_chunks_and_tokens(&[intuition], &[])
        .expect("retained Intuition search permits a typed fixed count without DifferentNames");
    let owner = crate::ProgramOwnerId::face(
        "intuition_retained_fixture",
        crate::CardFaceId::new("intuition_retained_fixture").unwrap(),
    );
    let compiled = registry
        .compiled_program_for_owner(&owner)
        .expect("Intuition fixture receives a compiled program");
    let super::CompiledInstructionKind::Perform(effect) = &compiled.body.steps[0].kind else {
        panic!("the first instruction is the retained search");
    };
    assert!(matches!(
        effect.as_ref(),
        super::SpellEffectKind::SearchLibrary {
            count: 3,
            selection_constraint: None,
            selection_bounds: Some(super::SearchSelectionBounds { min: 3, max: 3 }),
            ..
        }
    ));
}

#[test]
fn retained_cohort_compiler_rejects_broken_input_bindings_and_target_contracts() {
    let definition = r#"(id: "cohort_validation_fixture", name: "Cohort Validation Fixture",
        face_id: "cohort_validation_fixture", mana_cost: "{3}{U}", types: ["Instant"],
        spell_program: Some((body: (steps: [
            (id: "search", bind: ["found"], input: None,
                kind: Perform(SearchLibrary(
                    who: Controller, optional: false, count: 4,
                    count_by_cast_cost: None, filter: None,
                    selection_constraint: Some(DifferentNames), slots: [],
                    zones: Fixed([Library]), destination: KeepInLibrary,
                    conditional_destination: None, shuffle: false, reveal: false,
                    result_id: None, selection_bounds: Some((min: 0, max: 4)),
                ))),
            (id: "reveal", bind: ["revealed"], input: Some("found"),
                kind: Perform(RevealCohort)),
            (id: "choose", bind: ["partition"], input: Some("revealed"),
                kind: Perform(ChooseCohortPartition(
                    chooser: TargetedPlayer(group_index: 0, kind: OpponentPlayer),
                    selection: ExactlyUpTo(2),
                ))),
            (id: "move", bind: [], input: Some("partition"),
                kind: Perform(MoveCohortPartition(
                    selected_to: Graveyard, remainder_to: Hand,
                ))),
            (id: "shuffle", bind: [], input: None,
                kind: Perform(ShuffleLibrary(who: Controller))),
        ], yields: []), target_bindings: [
            (instruction_id: "choose", role: Target, target_group_index: 0),
        ])),
        targeting: Some((groups: [(
            min: 1, max: 1, prompt: "Choose target opponent", effect_indices: [],
        )])))"#;

    let rejected = [
        (
            definition.replace(
                "(id: \"choose\", bind: [\"partition\"], input: Some(\"revealed\")",
                "(id: \"choose\", bind: [\"partition\"], input: Some(\"found\")",
            ),
            "wrong type for this instruction",
        ),
        (
            definition.replace(
                "(id: \"reveal\", bind: [\"revealed\"],",
                "(id: \"reveal\", bind: [\"revealed\", \"extra\"],",
            ),
            "must bind its typed cohort result",
        ),
        (
            definition.replace("chooser: TargetedPlayer(group_index: 0", "chooser: TargetedPlayer(group_index: 1"),
            "requires its bound caster-chosen singleton player target group",
        ),
        (
            definition.replace(
                "target_bindings: [",
                "target_bindings: [(instruction_id: \"choose\", role: Target, target_group_index: 0),",
            ),
            "ChooseCohortPartition requires its bound caster-chosen singleton player target group",
        ),
        (
            definition.replace("role: Target, target_group_index: 0", "role: Source, target_group_index: 0"),
            "does not expose target role Source",
        ),
        (
            definition.replace("min: 1, max: 1, prompt: \"Choose target opponent\"", "min: 1, max: 2, prompt: \"Choose target opponent\""),
            "requires its bound caster-chosen singleton player target group",
        ),
    ];
    for (malformed, expected) in rejected {
        let error = crate::CardRegistry::from_chunks_and_tokens(&[&malformed], &[])
            .expect_err("malformed typed cohort contract must fail at admission")
            .to_string();
        assert!(
            error.contains(expected),
            "expected {expected:?}, got {error}"
        );
    }
}

#[test]
fn typed_retained_search_fields_are_rejected_on_direct_effects_and_shared_land_search_remains_valid(
) {
    let direct = |search: &str| {
        format!(
            r#"(id: "direct_search_fixture", name: "Direct Search Fixture",
                face_id: "direct_search_fixture", mana_cost: "{{1}}{{U}}", types: ["Instant"],
                spell_effect: [{search}])"#
        )
    };
    let ordinary = r#"SearchLibrary(
        who: Controller, optional: false, count: 4, count_by_cast_cost: None,
        filter: None, selection_constraint: None, slots: [],
        zones: Fixed([Library]), destination: Hand,
        conditional_destination: None, shuffle: false, reveal: false,
        result_id: None, selection_bounds: None,
    )"#;
    for (unsupported, expected_field) in [
        (
            ordinary.replace(
                "selection_bounds: None",
                "selection_bounds: Some((min: 0, max: 4))",
            ),
            "selection_bounds",
        ),
        (
            ordinary.replace(
                "selection_constraint: None",
                "selection_constraint: Some(DifferentNames)",
            ),
            "DifferentNames",
        ),
        (
            ordinary.replace("destination: Hand", "destination: KeepInLibrary"),
            "KeepInLibrary",
        ),
    ] {
        let definition = direct(&unsupported);
        let error = crate::CardRegistry::from_chunks_and_tokens(&[&definition], &[])
            .expect_err("typed retained-search fields cannot be smuggled into direct effects")
            .to_string();
        assert!(
            error.contains(
                "typed retained search/cohort operations are valid only inside a spell_program"
            ),
            "{expected_field} should be rejected on a direct effect, got {error}"
        );
    }

    let shared_land = direct(
        r#"SearchLibrary(
            who: Controller, optional: false, count: 2, count_by_cast_cost: None,
            filter: Some((card_type: Some(BasicLand))),
            selection_constraint: Some(SharedLandType), slots: [],
            zones: Fixed([Library]), destination: Battlefield(tapped: true),
            conditional_destination: None, shuffle: true, reveal: false,
            result_id: None, selection_bounds: None,
        )"#,
    );
    crate::CardRegistry::from_chunks_and_tokens(&[&shared_land], &[])
        .expect("the unrelated direct SharedLandType operation remains supported");
}

#[test]
fn program_compiler_rejects_unbound_incompatible_and_forward_choose_card_inputs() {
    let owner = crate::ProgramOwnerId::face(
        "mill_choose_card_invalid_fixture",
        crate::CardFaceId::new("mill_choose_card_invalid_fixture").unwrap(),
    );
    let compile = |body: &str| {
        let body: super::AuthoredBlock = ron::from_str(body).expect("invalid-input fixture parses");
        let program = super::CardProgram {
            body,
            target_bindings: Vec::new(),
        };
        super::CompiledProgram::compile(owner.clone(), super::EffectContext::Spell, &program)
    };
    let choose = r#"Perform(ChooseCard(
        source: CardResultCohort(players: Controller),
        filter: (card_type: Some(Land)), destination: Hand, optional: true))"#;

    let missing = format!(
        r#"(steps: [
            (id: "mill", bind: ["milled"], input: None,
                kind: Perform(Mill(count: 4, who: Controller))),
            (id: "recover", bind: [], input: None, kind: {choose}),
        ], yields: [])"#
    );
    assert!(compile(&missing)
        .unwrap_err()
        .contains("requires an explicitly bound input result"));

    let incompatible = format!(
        r#"(steps: [
            (id: "discard", bind: ["discarded"], input: None,
                kind: Perform(Discard(who: Controller, quantity: Exact(1)))),
            (id: "recover", bind: [], input: Some("discarded"), kind: {choose}),
        ], yields: [])"#
    );
    assert!(compile(&incompatible)
        .unwrap_err()
        .contains("has the wrong type for this instruction"));

    let forward = format!(
        r#"(steps: [
            (id: "recover", bind: [], input: Some("milled"), kind: {choose}),
            (id: "mill", bind: ["milled"], input: None,
                kind: Perform(Mill(count: 4, who: Controller))),
        ], yields: [])"#
    );
    assert!(compile(&forward)
        .unwrap_err()
        .contains("input result 'milled' is not in scope"));
}

#[test]
fn authored_program_rejects_targeted_leaf_without_compiled_binding() {
    let definition = program_fixture(
        r#"(steps: [(id: "damage", bind: [], kind: Perform(
            DamageTarget(amount: 3, target: (kind: Creature))
        ))], yields: [])"#,
    );
    let error = crate::CardRegistry::from_chunks_and_tokens(&[&definition], &[])
        .expect_err("authored target-bearing leaf has no owner-local target schema");
    assert!(
        error
            .to_string()
            .contains("targeted program instruction has no authored target group"),
        "{error}"
    );
}

#[test]
fn authored_program_accepts_targets_bound_by_program_instructions() {
    let definition = r#"(id: "target_program_fixture", name: "Target Program Fixture",
        face_id: "target_program_fixture", mana_cost: "{R}", types: ["Instant"],
        spell_program: Some((body: (steps: [
            (id: "damage", kind: Perform(DamageTarget(amount: 3, target: (kind: Creature))))
        ], yields: []), target_bindings: [
            (instruction_id: "damage", role: Target, target_group_index: 0),
        ])),
        targeting: Some((groups: [(
            min: 1, max: 1, prompt: "Choose target creature", effect_indices: [],
        )])))"#;

    let registry = crate::CardRegistry::from_chunks_and_tokens(&[definition], &[])
        .expect("an authored program target group binds its instruction directly");
    let owner = crate::ProgramOwnerId::face(
        "target_program_fixture",
        crate::CardFaceId::new("target_program_fixture").unwrap(),
    );
    let compiled = registry
        .compiled_program_for_owner(&owner)
        .expect("targetful program owner is compiled once at registry load");
    assert_eq!(
        compiled.target_bindings,
        [crate::primitives::ProgramTargetBinding {
            instruction_address: 0,
            target_group_index: 0,
            role_index: 0,
        }]
    );
    let face = registry
        .get("target_program_fixture")
        .unwrap()
        .primary_face();
    let schema =
        crate::primitives::TargetSchema::compile_program(compiled, face.targeting.as_ref())
            .expect("compiled program and authored group share the target contract");
    assert_eq!(schema.groups[0].min, 1);
    assert_eq!(schema.groups[0].max, 1);
    assert_eq!(schema.groups[0].bindings.len(), 1);
    assert!(matches!(
        schema.groups[0].bindings[0].role,
        crate::primitives::TargetRole::Filtered(_)
    ));

    for (malformed, expected) in [
        (
            definition.replace("role: Target", "role: Source"),
            "does not expose target role",
        ),
        (
            definition.replace("min: 1, max: 1", "min: 2, max: 1"),
            "min <= max",
        ),
        (
            definition.replace("target_group_index: 0", "target_group_index: 1"),
            "unknown target group",
        ),
        (
            definition.replace("effect_indices: []", "effect_indices: [0]"),
            "cannot reference legacy effect indices",
        ),
    ] {
        let error = crate::CardRegistry::from_chunks_and_tokens(&[&malformed], &[])
            .expect_err("malformed program-target schemas fail closed")
            .to_string();
        assert!(
            error.contains(expected),
            "expected {expected:?}, got {error}"
        );
    }
}

#[test]
fn authored_copy_program_requires_exactly_one_source_target() {
    let definition = r#"(id: "copy_target_program_fixture", name: "Copy Target Program Fixture",
        face_id: "copy_target_program_fixture", mana_cost: "{U}", types: ["Instant"],
        spell_program: Some((body: (steps: [
            (id: "copy", kind: Perform(CreateTokenCopies(count: 1,
                source: Chosen((kind: Creature)))))
        ], yields: []), target_bindings: [
            (instruction_id: "copy", role: Source, target_group_index: 0),
        ])),
        targeting: Some((groups: [(
            min: 1, max: 1, prompt: "Choose target creature", effect_indices: [],
        )])))"#;

    crate::CardRegistry::from_chunks_and_tokens(&[definition], &[])
        .expect("copy-source target uses one required target");
    let optional_source = definition.replace("min: 1, max: 1", "min: 0, max: 1");
    let error = crate::CardRegistry::from_chunks_and_tokens(&[&optional_source], &[])
        .expect_err("copy source cannot use an optional target group")
        .to_string();
    assert!(
        error.contains("CreateTokenCopies requires exactly one source target"),
        "{error}"
    );
    let multiple_sources = definition.replace("min: 1, max: 1", "min: 1, max: 2");
    let error = crate::CardRegistry::from_chunks_and_tokens(&[&multiple_sources], &[])
        .expect_err("copy source cannot use a multi-target group")
        .to_string();
    assert!(
        error.contains("CreateTokenCopies requires exactly one source target"),
        "{error}"
    );
}

#[test]
fn authored_programs_compile_for_exact_mode_activated_and_triggered_owners() {
    let definition = r#"(id: "program_owner_fixture", name: "Program Owner Fixture",
        face_id: "program_owner_fixture", mana_cost: "{1}{U}", types: ["Creature"],
        power: Some(2), toughness: Some(2),
        activated_abilities: [(
            ability_id: "activated_02", presentation: Fallback,
            source_zone: Battlefield, costs: [], effect: [],
            program: Some((body: (steps: [
                (id: "activation_draw", bind: ["drawn"], kind: Perform(Draw(count: 1)))
            ], yields: []))),
            targeting: None, timing: Normal, conditions: [], activation_limit: None,
        )],
        triggered_abilities: [(
            ability_id: "triggered_01", presentation: Fallback,
            trigger: WhenSelfEntersBattlefield, effect: [], modal: None,
            program: Some((body: (steps: [
                (id: "trigger_draw", bind: ["drawn"], kind: Perform(Draw(count: 1)))
            ], yields: []))),
            targeting: None, may: false, intervening_if: None,
            triggers_only_once: false, max_triggers_per_turn: None,
        ), (
            ability_id: "triggered_02", presentation: Fallback,
            trigger: WhenSelfEntersBattlefield, effect: [], program: None,
            modal: Some((min_modes: 1, max_modes: 2, modes: [
                (mode_id: "mode_01", presentation: Fallback, linked_cast_cost: None,
                    effects: [], program: Some((body: (steps: [
                        (id: "mode_draw", bind: ["drawn"], kind: Perform(Draw(count: 1)))
                    ], yields: []))), targeting: None),
                (mode_id: "mode_02", presentation: Fallback, linked_cast_cost: None,
                    effects: [], program: Some((body: (steps: [
                        (id: "mode_discard", bind: ["discarded"], kind: Perform(Discard(
                            who: Controller, quantity: Exact(1)
                        )))
                    ], yields: []))), targeting: None),
            ])), targeting: None, may: false, intervening_if: None,
            triggers_only_once: false, max_triggers_per_turn: None,
        )])"#;

    let registry = crate::CardRegistry::from_chunks_and_tokens(&[definition], &[])
        .expect("each exact owner compiles through the common program admission path");
    let face_id = crate::CardFaceId::new("program_owner_fixture").unwrap();
    let ability_owner = |ability_id: &str| {
        crate::ProgramOwnerId::ability(crate::AbilityDefinitionId {
            card_id: "program_owner_fixture".into(),
            face_id: face_id.clone(),
            ability_path: vec![crate::AbilityId::new(ability_id).unwrap()],
        })
    };
    let mode_owners = ["mode_01", "mode_02"].map(|mode_id| {
        crate::ProgramOwnerId::ability_mode(
            crate::AbilityDefinitionId {
                card_id: "program_owner_fixture".into(),
                face_id: face_id.clone(),
                ability_path: vec![crate::AbilityId::new("triggered_02").unwrap()],
            },
            crate::ModeId::new(mode_id).unwrap(),
        )
    });
    for owner in [
        mode_owners[0].clone(),
        mode_owners[1].clone(),
        ability_owner("activated_02"),
        ability_owner("triggered_01"),
    ] {
        let compiled = registry
            .compiled_program_for_owner(&owner)
            .unwrap_or_else(|| panic!("missing exact program owner {owner:?}"));
        assert!(compiled.target_bindings.is_empty());
    }
}

#[test]
fn chosen_artifact_destroy_ability_mode_requires_exact_subject_binding() {
    const EXACT_DESTROY: &str = r#"(id: "destroy_artifact", bind: [], input: None,
        kind: Perform(Destroy(subject: Chosen((kind: AnyPermanent,
            permanent_types: [Artifact])))))"#;
    const TARGETING: &str = r#"Some((groups: [(min: 0, max: 1,
        prompt: "Choose up to one target artifact", effect_indices: [])]))"#;

    fn ability_mode_card(step: &str, bindings: &str, targeting: &str) -> String {
        format!(
            r#"(id: "gear_destroy_mode_fixture", name: "Gear Destroy Mode Fixture",
                face_id: "gear_destroy_mode_fixture", types: ["Creature"], power: 2, toughness: 2,
                triggered_abilities: [(ability_id: "triggered_01", presentation: Fallback,
                    trigger: WhenSelfEntersBattlefield, effect: [], program: None,
                    modal: Some((min_modes: 1, max_modes: 1, modes: [(
                        mode_id: "mode_01", presentation: Fallback, linked_cast_cost: None,
                        effects: [], program: Some((body: (steps: [{step}], yields: []),
                            target_bindings: [{bindings}])), targeting: {targeting}
                    )])), targeting: None, may: false, intervening_if: None,
                    triggers_only_once: false, max_triggers_per_turn: None)])"#
        )
    }

    let exact = ability_mode_card(
        EXACT_DESTROY,
        r#"(instruction_id: "destroy_artifact", role: Subject, target_group_index: 0)"#,
        TARGETING,
    );
    let registry = crate::CardRegistry::from_chunks_and_tokens(&[&exact], &[])
        .expect("the exact resultless chosen-Artifact Destroy mode is admitted");
    let owner = crate::ProgramOwnerId::ability_mode(
        crate::AbilityDefinitionId {
            card_id: "gear_destroy_mode_fixture".into(),
            face_id: crate::CardFaceId::new("gear_destroy_mode_fixture").unwrap(),
            ability_path: vec![crate::AbilityId::new("triggered_01").unwrap()],
        },
        crate::ModeId::new("mode_01").unwrap(),
    );
    let compiled = registry
        .compiled_program_for_owner(&owner)
        .expect("the mode keeps its exact ability-mode owner");
    assert_eq!(compiled.owner, owner);
    assert_eq!(compiled.context, crate::primitives::EffectContext::Ability);
    assert_eq!(
        compiled.target_projection,
        crate::primitives::ProgramTargetProjection::ModeLocal {
            mode_id: crate::ModeId::new("mode_01").unwrap(),
        }
    );
    assert!(compiled.body.yields.is_empty());
    let [destroy] = compiled.body.steps.as_slice() else {
        panic!("the mode has exactly one Destroy instruction");
    };
    assert!(destroy.outputs.is_empty(), "Destroy is resultless here");
    let expected_filter = crate::primitives::TargetFilter {
        kind: crate::primitives::TargetKind::AnyPermanent,
        permanent_types: vec![crate::primitives::PermanentTypeFilter::Artifact],
        ..crate::primitives::TargetFilter::default()
    };
    assert!(matches!(
        &destroy.kind,
        crate::primitives::CompiledInstructionKind::Perform(effect)
            if matches!(effect.as_ref(), crate::primitives::SpellEffectKind::Destroy {
                subject: crate::primitives::EffectSubject::Chosen(filter)
            } if filter.as_ref() == &expected_filter)
    ));
    assert_eq!(compiled.target_bindings.len(), 1);
    assert_eq!(
        compiled.target_bindings[0].instruction_address,
        destroy.instruction_address
    );
    assert_eq!(compiled.target_bindings[0].target_group_index, 0);
    assert_eq!(compiled.target_bindings[0].role_index, 0);

    for (label, step, bindings) in [
        (
            "broad permanent filter",
            r#"(id: "destroy_artifact", bind: [], input: None,
                kind: Perform(Destroy(subject: Chosen((kind: AnyPermanent)))))"#,
            r#"(instruction_id: "destroy_artifact", role: Subject, target_group_index: 0)"#,
        ),
        (
            "non-Artifact filter",
            r#"(id: "destroy_artifact", bind: [], input: None,
                kind: Perform(Destroy(subject: Chosen((kind: AnyPermanent,
                    permanent_types: [Creature])))))"#,
            r#"(instruction_id: "destroy_artifact", role: Subject, target_group_index: 0)"#,
        ),
        (
            "non-Chosen subject",
            r#"(id: "destroy_artifact", bind: [], input: None,
                kind: Perform(Destroy(subject: Source)))"#,
            r#"(instruction_id: "destroy_artifact", role: Subject, target_group_index: 0)"#,
        ),
        (
            "result binding",
            r#"(id: "destroy_artifact", bind: ["destroyed"], input: None,
                kind: Perform(Destroy(subject: Chosen((kind: AnyPermanent,
                    permanent_types: [Artifact])))))"#,
            r#"(instruction_id: "destroy_artifact", role: Subject, target_group_index: 0)"#,
        ),
        ("missing binding", EXACT_DESTROY, ""),
        (
            "wrong role",
            EXACT_DESTROY,
            r#"(instruction_id: "destroy_artifact", role: Target, target_group_index: 0)"#,
        ),
        (
            "wrong target group",
            EXACT_DESTROY,
            r#"(instruction_id: "destroy_artifact", role: Subject, target_group_index: 1)"#,
        ),
    ] {
        let rejected = ability_mode_card(step, bindings, TARGETING);
        assert!(
            crate::CardRegistry::from_chunks_and_tokens(&[&rejected], &[]).is_err(),
            "the authored Destroy mode must reject {label}"
        );
    }

    let spell = format!(
        r#"(id: "gear_destroy_mode_fixture", name: "Gear Destroy Mode Fixture",
            face_id: "gear_destroy_mode_fixture", mana_cost: "{{0}}", types: ["Instant"],
            spell_effect: [], modal_spell: Some((min_modes: 1, max_modes: 1, modes: [(
                mode_id: "mode_01", presentation: Fallback, linked_cast_cost: None,
                effects: [], program: Some((body: (steps: [{EXACT_DESTROY}], yields: []),
                    target_bindings: [(instruction_id: "destroy_artifact", role: Subject,
                        target_group_index: 0)])), targeting: {TARGETING}
            )])))"#
    );
    assert!(
        crate::CardRegistry::from_chunks_and_tokens(&[&spell], &[]).is_err(),
        "the narrow chosen-Artifact Destroy leaf is ability-only"
    );
}

#[test]
fn spell_chosen_destroy_program_captures_a_single_subject_result_for_card_type_count() {
    let shredder = r#"(id: "shredder_destroy_result_fixture", name: "Shredder Destroy Result Fixture",
        face_id: "shredder_destroy_result_fixture", mana_cost: "{2}{B}", types: ["Sorcery"],
        spell_program: Some((body: (steps: [
            (id: "destroy", bind: ["destroyed"], input: None,
                kind: Perform(Destroy(subject: Chosen((any_of: Some([
                    (kind: Creature),
                    (kind: AnyPermanent, permanent_types: [Enchantment]),
                ])))))),
            (id: "enchantment_count", bind: [], input: None,
                kind: IfResult(condition: CardResultCount(result: "destroyed",
                    card_type: Some(Enchantment), min: Some(1), max: Some(1)),
                    yes: (steps: [], yields: []), no: (steps: [], yields: [])))
        ], yields: []), target_bindings: [
            (instruction_id: "destroy", role: Subject, target_group_index: 0),
        ])),
        targeting: Some((groups: [(
            min: 1, max: 1, prompt: "Choose target creature or enchantment", effect_indices: [],
        )])))"#;
    let registry = crate::CardRegistry::from_chunks_and_tokens(&[shredder], &[])
        .expect("spell-owned chosen Destroy binds a typed result for its Enchantment count");
    let owner = crate::ProgramOwnerId::face(
        "shredder_destroy_result_fixture",
        crate::CardFaceId::new("shredder_destroy_result_fixture").unwrap(),
    );
    let compiled = registry
        .compiled_program_for_owner(&owner)
        .expect("the exact Shredder spell face owns its compiled Program");
    assert_eq!(compiled.target_bindings.len(), 1);
    assert_eq!(compiled.target_bindings[0].target_group_index, 0);
    let destroy = compiled
        .slots
        .iter()
        .find(|slot| slot.name == "destroyed")
        .expect("the Destroy instruction exposes its named result");
    let crate::primitives::ResultType::TargetCardResultCohort {
        action,
        cardinality,
        target_group_index,
    } = &destroy.result_type
    else {
        panic!("chosen Destroy binds a target-origin result cohort");
    };
    assert_eq!(*action, crate::primitives::CardResultAction::Destroy);
    assert_eq!(
        *cardinality,
        crate::primitives::Cardinality {
            min: 0,
            max: Some(1),
        }
    );
    assert_eq!(*target_group_index, 0);
}

#[test]
fn spell_targeted_hand_discard_program_captures_target_group_result_for_card_type_count() {
    let verdict = r#"(id: "gerrards_verdict_result_fixture", name: "Gerrard's Verdict Result Fixture",
        face_id: "gerrards_verdict_result_fixture", mana_cost: "{W}{B}", types: ["Sorcery"],
        spell_effect: [],
        spell_program: Some((body: (steps: [
            (id: "discard", bind: ["discarded"], input: None,
                kind: Perform(ChooseHandCards(action: Discard, count: 2,
                    target: (kind: AnyPlayer), chooser: AffectedPlayer,
                    card_filter: None, optional: false))),
            (id: "gain_six", bind: [], input: None,
                kind: IfResult(condition: CardResultCount(result: "discarded",
                    card_type: Some(Land), min: Some(2), max: Some(2)),
                    yes: (steps: [(id: "six_life", bind: [], input: None,
                        kind: Perform(GainLife(amount: 6)))], yields: []),
                    no: (steps: [(id: "gain_three", bind: [], input: None,
                        kind: IfResult(condition: CardResultCount(result: "discarded",
                            card_type: Some(Land), min: Some(1), max: Some(1)),
                            yes: (steps: [(id: "three_life", bind: [], input: None,
                                kind: Perform(GainLife(amount: 3)))], yields: []),
                            no: (steps: [], yields: [])))], yields: [])))
        ], yields: []), target_bindings: [
            (instruction_id: "discard", role: Target, target_group_index: 0),
        ])),
        targeting: Some((groups: [(
            min: 1, max: 1, prompt: "Choose target player", effect_indices: [],
        )])))"#;
    let registry = crate::CardRegistry::from_chunks_and_tokens(&[verdict], &[])
        .expect("targeted affected-player discard binds a bounded typed result");
    let owner = crate::ProgramOwnerId::face(
        "gerrards_verdict_result_fixture",
        crate::CardFaceId::new("gerrards_verdict_result_fixture").unwrap(),
    );
    let compiled = registry
        .compiled_program_for_owner(&owner)
        .expect("the fixture sorcery owns the typed resolution Program");
    assert_eq!(compiled.target_bindings.len(), 1);
    assert_eq!(compiled.target_bindings[0].target_group_index, 0);
    let discard = compiled
        .slots
        .iter()
        .find(|slot| slot.name == "discarded")
        .expect("discard instruction exposes its named result");
    let crate::primitives::ResultType::TargetCardResultCohort {
        action,
        cardinality,
        target_group_index,
    } = &discard.result_type
    else {
        panic!("targeted discard binds a target-origin result cohort");
    };
    assert_eq!(*action, crate::primitives::CardResultAction::Discard);
    assert_eq!(
        *cardinality,
        crate::primitives::Cardinality {
            min: 0,
            max: Some(2),
        }
    );
    assert_eq!(*target_group_index, 0);

    let no_target_bindings = verdict.replace(
        "target_bindings: [\n            (instruction_id: \"discard\", role: Target, target_group_index: 0),\n        ]",
        "target_bindings: []",
    );
    let negative_profiles = [
        ("missing Target role binding", no_target_bindings),
        (
            "wrong target role",
            verdict.replace("role: Target", "role: Subject"),
        ),
        (
            "non-singleton target group",
            verdict.replace("min: 1, max: 1, prompt", "min: 0, max: 1, prompt"),
        ),
        (
            "non-player target",
            verdict.replace("target: (kind: AnyPlayer)", "target: (kind: Creature)"),
        ),
        (
            "controller-selected hand cards",
            verdict.replace("chooser: AffectedPlayer", "chooser: Controller"),
        ),
        (
            "exile instead of discard",
            verdict.replace("action: Discard", "action: Exile"),
        ),
        (
            "filtered hand selection",
            verdict.replace("card_filter: None", "card_filter: Some(Land)"),
        ),
        (
            "optional hand selection",
            verdict.replace("optional: false", "optional: true"),
        ),
        (
            "publicly revealed hand",
            verdict.replace(
                "optional: false))",
                "optional: false, visibility: PublicReveal))",
            ),
        ),
    ];
    for (case, source) in negative_profiles {
        assert!(
            crate::CardRegistry::from_chunks_and_tokens(&[source.as_str()], &[]).is_err(),
            "unsupported targeted hand-discard profile must fail closed: {case}"
        );
    }
}

#[test]
fn chosen_destroy_program_requires_one_singleton_subject_binding_and_joins_only_same_origin() {
    let fixture = |id: &str, body: &str, bindings: &str, groups: &str| {
        format!(
            r#"(id: "{id}", name: "Chosen Destroy Result Fixture", face_id: "{id}",
                mana_cost: "{{2}}", types: ["Sorcery"],
                spell_program: Some((body: {body}, target_bindings: {bindings})),
                targeting: Some((groups: [{groups}])))"#
        )
    };
    let destroy = |instruction_id: &str| {
        format!(
            r#"(id: "{instruction_id}", bind: ["destroyed"], input: None,
                kind: Perform(Destroy(subject: Chosen((kind: Creature)))))"#
        )
    };
    let body =
        |instruction_id: &str| format!(r#"(steps: [{}], yields: [])"#, destroy(instruction_id));
    let group = |min: u32, max: u32| {
        format!(r#"(min: {min}, max: {max}, prompt: "Choose a target", effect_indices: [])"#)
    };
    let subject0 = r#"[(instruction_id: "destroy", role: Subject, target_group_index: 0)]"#;

    for (id, bindings, groups) in [
        ("destroy_without_subject", "[]", group(1, 1)),
        (
            "destroy_wrong_subject_role",
            r#"[(instruction_id: "destroy", role: Target, target_group_index: 0)]"#,
            group(1, 1),
        ),
        (
            "destroy_mismatched_subject_instruction",
            r#"[(instruction_id: "other", role: Subject, target_group_index: 0)]"#,
            group(1, 1),
        ),
        (
            "destroy_repeated_subject_binding",
            r#"[(instruction_id: "destroy", role: Subject, target_group_index: 0),
                (instruction_id: "destroy", role: Subject, target_group_index: 0)]"#,
            group(1, 1),
        ),
        ("destroy_nontarget_group_0_1", subject0, group(0, 1)),
        ("destroy_nontarget_group_1_2", subject0, group(1, 2)),
        ("destroy_nontarget_group_2_2", subject0, group(2, 2)),
    ] {
        let spell = fixture(id, &body("destroy"), bindings, &groups);
        assert!(
            crate::CardRegistry::from_chunks_and_tokens(&[&spell], &[]).is_err(),
            "{id} must fail closed"
        );
    }

    let same_group = r#"(steps: [(id: "branch", bind: ["joined"], input: None,
        kind: If(condition: Void,
            yes: (steps: [(id: "left_destroy", bind: ["left"], input: None,
                kind: Perform(Destroy(subject: Chosen((kind: Creature)))))], yields: ["left"]),
            no: (steps: [(id: "right_destroy", bind: ["right"], input: None,
                kind: Perform(Destroy(subject: Chosen((kind: Creature)))))], yields: ["right"])))],
        yields: [])"#;
    let same_bindings = r#"[(instruction_id: "left_destroy", role: Subject, target_group_index: 0),
        (instruction_id: "right_destroy", role: Subject, target_group_index: 0)]"#;
    let same = fixture(
        "destroy_same_group_join",
        same_group,
        same_bindings,
        &group(1, 1),
    );
    crate::CardRegistry::from_chunks_and_tokens(&[&same], &[])
        .expect("branch Destroy result cohorts with the same action and target origin join");

    let different_bindings = r#"[(instruction_id: "left_destroy", role: Subject, target_group_index: 0),
        (instruction_id: "right_destroy", role: Subject, target_group_index: 1)]"#;
    let different = fixture(
        "destroy_different_group_join",
        same_group,
        different_bindings,
        &format!("{}, {}", group(1, 1), group(1, 1)),
    );
    assert!(
        crate::CardRegistry::from_chunks_and_tokens(&[&different], &[]).is_err(),
        "branch cohorts from distinct target groups cannot join"
    );
}

#[test]
fn ward_program_admission_is_ability_owned_and_requires_its_exact_trigger_producer() {
    fn triggered_definition(trigger: &str, cost: &str) -> String {
        format!(
            r#"(id: "ward_program_admission_fixture", name: "Ward Program Admission Fixture",
                face_id: "ward_program_admission_fixture", types: ["Creature"], power: 2, toughness: 2,
                triggered_abilities: [(ability_id: "triggered_01", presentation: Fallback,
                    trigger: {trigger}, effect: [], program: Some((body: (steps: [
                        (id: "ward", kind: Perform(CounterTriggeringStackObjectUnlessPays(cost: {cost})))
                    ], yields: []))), modal: None, targeting: None, may: false,
                    intervening_if: None, triggers_only_once: false, max_triggers_per_turn: None
                )])"#
        )
    }

    let targeting_trigger =
        "WheneverSelfBecomesTarget(source: SpellOrAbility, source_controller: Opponent)";
    for cost in ["Mana(\"{2}\")", "DiscardCard(filter: None)"] {
        let definition = triggered_definition(targeting_trigger, cost);
        let registry = crate::CardRegistry::from_chunks_and_tokens(&[&definition], &[])
            .unwrap_or_else(|error| panic!("supported Ward Program cost {cost} rejected: {error}"));
        let face_id = crate::CardFaceId::new("ward_program_admission_fixture").unwrap();
        let owner = crate::ProgramOwnerId::ability(crate::AbilityDefinitionId {
            card_id: "ward_program_admission_fixture".into(),
            face_id,
            ability_path: vec![crate::AbilityId::new("triggered_01").unwrap()],
        });
        let compiled = registry
            .compiled_program_for_owner(&owner)
            .expect("the exact Ward ability owner has its program");
        assert_eq!(
            compiled.required_inputs,
            vec![
                crate::ProgramRequiredInput::TriggerAffectedPlayer,
                crate::ProgramRequiredInput::TriggerTargetingStackObject,
            ],
            "Ward must capture its payer and exact targeting stack object"
        );
    }

    for cost in [
        "Mana(\"{X}\")",
        "Mana(\"{0}\")",
        "Waterbend(\"{2}\")",
        "DiscardCard(filter: Some(Creature))",
    ] {
        let _: crate::primitives::ResolutionCost = ron::from_str(cost).unwrap_or_else(|error| {
            panic!("unsupported Ward cost fixture {cost} is malformed: {error}")
        });
        let definition = triggered_definition(targeting_trigger, cost);
        assert!(
            crate::CardRegistry::from_chunks_and_tokens(&[&definition], &[]).is_err(),
            "unsupported Ward Program cost {cost} must fail closed"
        );
    }

    let affected_player_only_trigger =
        triggered_definition("AtBeginningOfDrawStep(player: AnyPlayer)", "Mana(\"{2}\")");
    let error = crate::CardRegistry::from_chunks_and_tokens(&[&affected_player_only_trigger], &[])
        .expect_err("a draw-step trigger cannot supply Ward's exact targeting object");
    assert!(
        error.to_string().contains("TriggerTargetingStackObject"),
        "the Ward input rejection should name its unsupplied captured reference: {error}"
    );

    let spell_program = r#"(id: "ward_spell_program_fixture", name: "Ward Spell Program Fixture",
        face_id: "ward_spell_program_fixture", types: ["Instant"],
        spell_program: Some((body: (steps: [
            (id: "ward", kind: Perform(CounterTriggeringStackObjectUnlessPays(cost: Mana("{2}"))))
        ], yields: [])))"#;
    assert!(
        crate::CardRegistry::from_chunks_and_tokens(&[spell_program], &[]).is_err(),
        "Ward is an ability-owned leaf, not a spell operation"
    );

    let activated_program = r#"(id: "ward_activated_program_fixture", name: "Ward Activated Program Fixture",
        face_id: "ward_activated_program_fixture", types: ["Creature"], power: 2, toughness: 2,
        activated_abilities: [(ability_id: "activated_01", presentation: Fallback,
            source_zone: Battlefield, costs: [], effect: [],
            program: Some((body: (steps: [
                (id: "ward", kind: Perform(CounterTriggeringStackObjectUnlessPays(cost: Mana("{2}"))))
            ], yields: []))), targeting: None, timing: Normal, conditions: [], activation_limit: None
        )])"#;
    assert!(
        crate::CardRegistry::from_chunks_and_tokens(&[activated_program], &[]).is_err(),
        "an activated ability cannot claim a targeting trigger's captured inputs"
    );
}

#[test]
fn legacy_flat_ward_trigger_requires_an_authored_program() {
    let definition = r#"(id: "legacy_flat_ward_fixture", name: "Legacy Flat Ward Fixture",
        face_id: "legacy_flat_ward_fixture", types: ["Creature"], power: 2, toughness: 2,
        triggered_abilities: [(ability_id: "triggered_01", presentation: Fallback,
            trigger: WheneverSelfBecomesTarget(source: SpellOrAbility, source_controller: Opponent),
            effect: [CounterTriggeringStackObjectUnlessPays(cost: Mana("{2}"))],
            program: None, modal: None, targeting: None, may: false,
            intervening_if: None, triggers_only_once: false, max_triggers_per_turn: None
        )])"#;
    let error = crate::CardRegistry::from_chunks_and_tokens(&[definition], &[])
        .expect_err("a supported legacy Ward effect list must be retired");
    assert!(
        error
            .to_string()
            .to_lowercase()
            .contains("authored program"),
        "legacy Ward rejection should identify the missing authored Program, got {error}"
    );
}

#[test]
fn legacy_flat_spell_ward_requires_an_authored_program() {
    let definition = r#"(id: "legacy_flat_spell_ward_fixture", name: "Legacy Flat Spell Ward Fixture",
        face_id: "legacy_flat_spell_ward_fixture", mana_cost: "{1}{U}", types: ["Instant"],
        spell_effect: [CounterTriggeringStackObjectUnlessPays(cost: Mana("{2}"))])"#;
    let error = crate::CardRegistry::from_chunks_and_tokens(&[definition], &[])
        .expect_err("a legacy spell effect list cannot own Ward");
    assert!(
        error
            .to_string()
            .to_lowercase()
            .contains("authored program"),
        "legacy spell Ward rejection should name the Program requirement, got {error}"
    );
}

#[test]
fn legacy_flat_activated_ward_requires_an_authored_program() {
    let definition = r#"(id: "legacy_flat_activated_ward_fixture", name: "Legacy Flat Activated Ward Fixture",
        face_id: "legacy_flat_activated_ward_fixture", mana_cost: "{2}",
        types: ["Artifact", "Creature"], power: 2, toughness: 2,
        activated_abilities: [(ability_id: "activated_01", presentation: Fallback,
            source_zone: Battlefield, costs: [Tap],
            effect: [CounterTriggeringStackObjectUnlessPays(cost: Mana("{2}"))],
            targeting: None, timing: Normal, conditions: [], activation_limit: None
        )])"#;
    let error = crate::CardRegistry::from_chunks_and_tokens(&[definition], &[])
        .expect_err("a legacy activated effect list cannot own Ward");
    assert!(
        error
            .to_string()
            .to_lowercase()
            .contains("authored program"),
        "legacy activated Ward rejection should name the Program requirement, got {error}"
    );
}

#[test]
fn legacy_ward_modal_and_grant_owners_require_programs() {
    const WARD: &str = "CounterTriggeringStackObjectUnlessPays(cost: Mana(\"{2}\"))";

    fn triggered_card(resolution: &str) -> String {
        format!(
            r#"(id: "nested_legacy_ward_fixture", name: "Nested Legacy Ward Fixture",
                face_id: "nested_legacy_ward_fixture", types: ["Creature"], power: 2, toughness: 2,
                triggered_abilities: [(ability_id: "triggered_01", presentation: Fallback,
                    trigger: WheneverSelfBecomesTarget(source: SpellOrAbility, source_controller: Opponent),
                    {resolution}
                )])"#
        )
    }

    fn assert_retired(definition: &str, owner_shape: &str) {
        let error = crate::CardRegistry::from_chunks_and_tokens(&[definition], &[]).unwrap_err();
        assert!(
            error
                .to_string()
                .to_lowercase()
                .contains("authored program"),
            "{owner_shape} must reach the retired-Ward admission boundary, got {error}"
        );
    }

    let modal = triggered_card(&format!(
        r#"effect: [], program: None,
        modal: Some((min_modes: 1, max_modes: 1, modes: [(
            mode_id: "ward",
            presentation: Fallback,
            linked_cast_cost: None,
            effects: [{WARD}],
            program: None,
            targeting: None,
        )])), targeting: None, may: false, intervening_if: None,
        triggers_only_once: false, max_triggers_per_turn: None"#
    ));
    assert_retired(&modal, "modal mode");

    let static_grant = format!(
        r#"(id: "static_legacy_ward_fixture", name: "Static Legacy Ward Fixture",
            face_id: "static_legacy_ward_fixture", types: ["Creature"], power: 2, toughness: 2,
            static_abilities: [(ability_id: "static_02", presentation: Fallback,
                definition: GrantTriggeredAbilityToPermanents(
                    filter: (kind: Creature, controller: You),
                    condition: Some(HasEnduringStory(players: Controller)),
                    triggered_abilities: [(
                        ability_id: "triggered_01", presentation: Fallback,
                        trigger: WheneverSelfBecomesTarget(source: SpellOrAbility, source_controller: Opponent),
                        effect: [{WARD}], program: None, modal: None, targeting: None,
                    )],
                ))])"#
    );
    assert_retired(&static_grant, "conditional static grant");

    let dynamic_grant = format!(
        r#"(id: "dynamic_legacy_ward_fixture", name: "Dynamic Legacy Ward Fixture",
            face_id: "dynamic_legacy_ward_fixture", mana_cost: "{{0}}", types: ["Instant"],
            spell_effect: [GrantTriggeredAbility(
                subject: Chosen((kind: Creature)),
                ability: (
                    ability_id: "granted", presentation: Fallback,
                    trigger: WheneverSelfBecomesTarget(source: SpellOrAbility, source_controller: Opponent),
                    effect: [{WARD}], program: None, modal: None, targeting: None,
                ),
            )],
            targeting: Some((groups: [(min: 1, max: 1, prompt: "Choose target creature",
                effect_indices: [0])])) )"#
    );
    assert_retired(&dynamic_grant, "resolving-created trigger grant");
}

#[test]
fn all_legacy_modal_spell_owners_stay_on_legacy_dispatch() {
    let definition = r#"(id: "all_legacy_modal_fixture", name: "All Legacy Modal Fixture",
        face_id: "all_legacy_modal_fixture", mana_cost: "{0}", types: ["Instant"],
        modal_spell: Some((min_modes: 1, max_modes: 1, modes: [
            (mode_id: "draw", presentation: Fallback, linked_cast_cost: None,
                effects: [Draw(count: 1)], program: None, targeting: None),
            (mode_id: "life", presentation: Fallback, linked_cast_cost: None,
                effects: [GainLife(amount: 3)], program: None, targeting: None),
        ])))"#;
    let registry = crate::CardRegistry::from_chunks_and_tokens(&[definition], &[])
        .expect("all-legacy modal owners remain valid effect-list owners");
    let face_id = crate::CardFaceId::new("all_legacy_modal_fixture").unwrap();
    for mode_id in ["draw", "life"] {
        let owner = crate::ProgramOwnerId::face_mode(
            "all_legacy_modal_fixture",
            face_id.clone(),
            crate::ModeId::new(mode_id).unwrap(),
        );
        assert!(
            registry.compiled_program_for_owner(&owner).is_none(),
            "all-legacy mode {mode_id} remains on legacy effect dispatch"
        );
    }
}

#[test]
fn registry_rejects_mixed_authored_and_legacy_modal_spell_siblings() {
    let definition = r#"(id: "mixed_modal_spell_fixture", name: "Mixed Modal Spell Fixture",
        face_id: "mixed_modal_spell_fixture", mana_cost: "{0}", types: ["Instant"],
        modal_spell: Some((min_modes: 1, max_modes: 1, modes: [
            (mode_id: "authored", presentation: Fallback, linked_cast_cost: None,
                effects: [], program: Some((body: (steps: [
                    (id: "authored_draw", bind: ["drawn"], kind: Perform(Draw(count: 1)))
                ], yields: []))), targeting: None),
            (mode_id: "legacy_draw", presentation: Fallback, linked_cast_cost: None,
                effects: [Draw(count: 1)], program: None, targeting: None),
        ])))"#;
    assert!(
        crate::CardRegistry::from_chunks_and_tokens(&[definition], &[]).is_err(),
        "a programmed modal spell cannot retain a legacy-only sibling"
    );
}

#[test]
fn registry_rejects_mixed_authored_and_legacy_triggered_modal_siblings() {
    let definition = r#"(id: "mixed_triggered_modal_fixture", name: "Mixed Triggered Modal Fixture",
        face_id: "mixed_triggered_modal_fixture", types: ["Creature"], power: 2, toughness: 2,
        triggered_abilities: [(ability_id: "triggered_01", presentation: Fallback,
            trigger: WhenSelfEntersBattlefield, effect: [], program: None,
            modal: Some((min_modes: 1, max_modes: 1, modes: [
                (mode_id: "authored", presentation: Fallback, linked_cast_cost: None,
                    effects: [], program: Some((body: (steps: [
                        (id: "authored_draw", bind: ["drawn"], kind: Perform(Draw(count: 1)))
                    ], yields: []))), targeting: None),
                (mode_id: "legacy_draw", presentation: Fallback, linked_cast_cost: None,
                    effects: [Draw(count: 1)], program: None, targeting: None),
            ])), targeting: None, may: false, intervening_if: None,
            triggers_only_once: false, max_triggers_per_turn: None
        )])"#;
    assert!(
        crate::CardRegistry::from_chunks_and_tokens(&[definition], &[]).is_err(),
        "a programmed triggered-modal ability cannot retain a legacy-only sibling"
    );
}

#[test]
fn resolving_created_ability_owners_reject_programs_including_nested_modal_modes() {
    let program = r#"Some((body: (steps: [
        (id: "draw", bind: ["drawn"], kind: Perform(Draw(count: 1)))
    ], yields: [])))"#;
    let direct_program = format!(
        r#"(id: "unowned_program_grant", name: "Unowned Program Grant",
            face_id: "unowned_program_grant", mana_cost: "{{0}}", types: ["Instant"],
            spell_effect: [GrantTriggeredAbility(
                subject: Chosen((kind: Creature)),
                ability: (ability_id: "granted", presentation: Fallback,
                    trigger: WhenSelfDies, effect: [], program: {program}, modal: None,
                    targeting: None))],
            targeting: Some((groups: [(min: 1, max: 1, prompt: "Choose target creature",
                effect_indices: [0])])) )"#
    );
    let nested_modal_program = r#"(id: "unowned_modal_grant", name: "Unowned Modal Grant",
        face_id: "unowned_modal_grant", mana_cost: "{0}", types: ["Instant"],
        spell_effect: [CreateDelayedTrigger(subject: None, affected_player: None,
            ability: (ability_id: "delayed", presentation: Fallback,
                trigger: AtBeginningOfNextTurnUpkeep, effect: [], program: None,
                modal: Some((min_modes: 1, max_modes: 1, modes: [
                    (mode_id: "mode_01", presentation: Fallback, linked_cast_cost: None,
                        effects: [], program: Some((body: (steps: [
                            (id: "draw", bind: ["drawn"], kind: Perform(Draw(count: 1)))
                        ], yields: []))), targeting: None),
                ])), targeting: None))])"#;
    let dynamic_activated_program = format!(
        r#"(id: "unowned_activated_grant", name: "Unowned Activated Grant",
            face_id: "unowned_activated_grant", mana_cost: "{{0}}", types: ["Instant"],
            spell_effect: [ApplyPermanentModifier(
                subject: Chosen((kind: Creature)),
                modifier: GrantActivatedAbility((ability_id: "granted", presentation: Fallback,
                    source_zone: Battlefield, costs: [], effect: [], program: {program},
                    targeting: None, timing: Normal, conditions: [], activation_limit: None)),
                duration: UntilEndOfTurn)],
            targeting: Some((groups: [(min: 1, max: 1, prompt: "Choose target creature",
                effect_indices: [0])])) )"#
    );
    let nested_reflexive_program = format!(
        r#"(id: "unowned_reflexive_grant", name: "Unowned Reflexive Grant",
            face_id: "unowned_reflexive_grant", mana_cost: "{{0}}", types: ["Instant"],
            spell_effect: [CreateReflexiveTrigger(ability: (
                ability_id: "reflexive", presentation: Fallback,
                effect: [GrantTriggeredAbility(subject: Source,
                    ability: (ability_id: "granted", presentation: Fallback,
                        trigger: WhenSelfDies, effect: [], program: {program},
                        modal: None, targeting: None))], targeting: None))])"#
    );

    for definition in [
        direct_program,
        nested_modal_program.to_owned(),
        dynamic_activated_program,
        nested_reflexive_program,
    ] {
        let error = crate::CardRegistry::from_chunks_and_tokens(&[&definition], &[])
            .expect_err("a resolving-created owner has no stable compiled program identity")
            .to_string();
        assert!(
            error.contains("resolving-created abilities cannot use authored programs"),
            "expected the unowned-program boundary, got {error}"
        );
    }
}

#[test]
fn nonactivated_programs_reject_chosen_opponent_target_groups() {
    let definitions = [
        r#"(id: "spell_chosen_opponent_fixture", name: "Spell Chosen Opponent Fixture",
            face_id: "spell_chosen_opponent_fixture", mana_cost: "{1}{R}", types: ["Instant"],
            spell_program: Some((body: (steps: [
                (id: "damage", kind: Perform(DamageTarget(amount: 1,
                    target: (kind: Creature))))
            ], yields: []), target_bindings: [
                (instruction_id: "damage", role: Target, target_group_index: 0),
            ])),
            targeting: Some((groups: [(
                min: 1, max: 1, prompt: "Choose a target", chooser: ChosenOpponent,
                effect_indices: [],
            )])))"#,
        r#"(id: "trigger_chosen_opponent_fixture", name: "Trigger Chosen Opponent Fixture",
            face_id: "trigger_chosen_opponent_fixture", mana_cost: "{1}{R}", types: ["Creature"],
            power: Some(2), toughness: Some(2), triggered_abilities: [(
                ability_id: "triggered_01", presentation: Fallback,
                trigger: WhenSelfEntersBattlefield, effect: [], modal: None,
                program: Some((body: (steps: [
                    (id: "damage", kind: Perform(DamageTarget(amount: 1,
                        target: (kind: Creature))))
                ], yields: []), target_bindings: [
                    (instruction_id: "damage", role: Target, target_group_index: 0),
                ])),
                targeting: Some((groups: [(
                    min: 1, max: 1, prompt: "Choose a target", chooser: ChosenOpponent,
                    effect_indices: [],
                )])), may: false, intervening_if: None,
                triggers_only_once: false, max_triggers_per_turn: None,
            )])"#,
    ];

    for definition in definitions {
        let error = crate::CardRegistry::from_chunks_and_tokens(&[definition], &[])
            .expect_err("opponent-selected targets are restricted to supported activations")
            .to_string();
        assert!(
            error.contains("opponent target choices require a supported activated ability"),
            "{error}"
        );
    }
}

#[test]
fn authored_nonbattlefield_activated_programs_reject_chosen_opponent_groups() {
    for source_zone in ["Hand", "Graveyard"] {
        let ability: ActivatedAbilityDef = ron::from_str(&format!(
            r#"(
                ability_id: "activated_01", presentation: Fallback,
                source_zone: {source_zone}, costs: [], effect: [],
                program: Some((body: (steps: [
                    (id: "damage", bind: [], kind: Perform(DamageTarget(
                        amount: 1, target: (kind: Creature))))
                ], yields: []), target_bindings: [
                    (instruction_id: "damage", role: Target, target_group_index: 0),
                ])),
                targeting: Some((groups: [(
                    min: 1, max: 1, prompt: "Choose an opponent's creature",
                    chooser: ChosenOpponent, effect_indices: [],
                )])),
                timing: Normal, conditions: [], activation_limit: None,
            )"#
        ))
        .expect("nonbattlefield program fixture parses");

        let error = ability
            .validate_shape()
            .expect_err("an opponent-selected authored target cannot be activated off battlefield");
        assert!(
            error.contains("authored opponent-choice groups require a battlefield source"),
            "expected the precise authored source-zone boundary, got {error}"
        );
    }

    let ordinary_hand_target: ActivatedAbilityDef = ron::from_str(
        r#"(
            ability_id: "activated_01", presentation: Fallback,
            source_zone: Hand, costs: [], effect: [],
            program: Some((body: (steps: [
                (id: "damage", bind: [], kind: Perform(DamageTarget(
                    amount: 1, target: (kind: Creature))))
            ], yields: []), target_bindings: [
                (instruction_id: "damage", role: Target, target_group_index: 0),
            ])),
            targeting: Some((groups: [(
                min: 1, max: 1, prompt: "Choose a creature", effect_indices: [],
            )])),
            timing: Normal, conditions: [], activation_limit: None,
        )"#,
    )
    .expect("ordinary hand target fixture parses");
    ordinary_hand_target
        .validate_shape()
        .expect("ordinary controller-chosen hand targets remain accepted");
}

#[test]
fn authored_opponent_program_registry_source_zone_admission_is_narrow() {
    let card = |name: &str, source_zone: &str, costs: &str, program: &str, targeting: &str| {
        format!(
            r#"(
                id: "{name}", name: "{name}", face_id: "{name}",
                mana_cost: "{{0}}", types: ["Creature"], power: Some(2), toughness: Some(2),
                activated_abilities: [(
                    ability_id: "activated_01", presentation: Fallback,
                    source_zone: {source_zone}, costs: {costs}, effect: [],
                    program: {program}, targeting: Some({targeting}),
                    timing: Normal, conditions: [], activation_limit: None,
                )],
            )"#
        )
    };
    let single_opponent_program = r#"Some((body: (steps: [
        (id: "damage", kind: Perform(DamageTarget(amount: 1,
            target: (kind: Creature, controller: Opponent))))
    ], yields: []), target_bindings: [
        (instruction_id: "damage", role: Target, target_group_index: 0),
    ]))"#;
    let chosen_opponent_targeting = r#"(groups: [(
        min: 1, max: 1, prompt: "Choose an opponent's creature",
        chooser: ChosenOpponent, effect_indices: [],
    )])"#;

    for source_zone in ["Hand", "Graveyard"] {
        let definition = card(
            &format!(
                "authored_{}_opponent_target_fixture",
                source_zone.to_lowercase()
            ),
            source_zone,
            "[]",
            single_opponent_program,
            chosen_opponent_targeting,
        );
        let error = crate::CardRegistry::from_chunks_and_tokens(&[&definition], &[])
            .expect_err("nonbattlefield chosen-opponent programs are not admitted")
            .to_string();
        assert!(
            error.contains("authored opponent-choice groups require a battlefield source"),
            "expected the scoped source-zone admission reason, got {error}"
        );
    }

    let ordinary_hand = card(
        "authored_hand_controller_target_fixture",
        "Hand",
        "[]",
        r#"Some((body: (steps: [
            (id: "damage", kind: Perform(DamageTarget(amount: 1,
                target: (kind: Creature))))
        ], yields: []), target_bindings: [
            (instruction_id: "damage", role: Target, target_group_index: 0),
        ]))"#,
        r#"(groups: [(min: 1, max: 1, prompt: "Choose a creature", effect_indices: [])])"#,
    );
    crate::CardRegistry::from_chunks_and_tokens(&[&ordinary_hand], &[])
        .expect("direct ordinary hand target groups remain supported");

    let battlefield_opponent_program = card(
        "authored_battlefield_opponent_target_fixture",
        "Battlefield",
        r#"[Mana("{3}"), Tap]"#,
        r#"Some((body: (steps: [
            (id: "damage_you", kind: Perform(DamageTarget(amount: 1,
                target: (kind: Creature, controller: You)))),
            (id: "damage_opponent", kind: Perform(DamageTarget(amount: 1,
                target: (kind: Creature, controller: Opponent))))
        ], yields: []), target_bindings: [
            (instruction_id: "damage_you", role: Target, target_group_index: 0),
            (instruction_id: "damage_opponent", role: Target, target_group_index: 1),
        ]))"#,
        r#"(groups: [
            (min: 1, max: 1, prompt: "Choose your creature", effect_indices: []),
            (min: 1, max: 1, prompt: "Choose an opponent's creature",
                chooser: ChosenOpponent, effect_indices: []),
        ])"#,
    );
    crate::CardRegistry::from_chunks_and_tokens(&[&battlefield_opponent_program], &[])
        .expect("the supported battlefield opponent-choice program remains admitted");
}

#[test]
fn authored_program_rejects_unknown_token_references() {
    let definition = program_fixture(
        r#"(steps: [(id: "tokens", bind: [], kind: Perform(
            CreateTokens(token: "missing_token", count: 1, who: Controller,
                tapped: false, sacrifice_timing: None)
        ))], yields: [])"#,
    );
    let error = crate::CardRegistry::from_chunks_and_tokens(&[&definition], &[])
        .expect_err("CreateTokens must reference a token in the loaded token namespace");
    assert!(
        error
            .to_string()
            .contains("references unknown token 'missing_token'"),
        "{error}"
    );
}

#[test]
fn spell_program_rejects_wrong_type_and_non_dominating_result_bindings() {
    let wrong_type = program_fixture(
        r#"(steps: [
            (id: "draw", bind: ["drawn"], kind: Perform(Draw(count: 1))),
            (id: "untap", input: Some("drawn"), kind: Perform(UntapChosenPermanents))
          ], yields: [])"#,
    );
    let wrong_type_error = invalid_program_reason(&wrong_type);
    assert!(
        wrong_type_error.contains("wrong type"),
        "{wrong_type_error}"
    );

    let out_of_scope = program_fixture(
        r#"(steps: [(id: "if", bind: [], kind: If(condition: ControllerLibraryEmpty,
            yes: (steps: [(id: "choose", bind: ["lands"], kind: Perform(ChoosePermanents(
                chooser: Controller, filter: (kind: AnyPermanent, permanent_types: [Land]),
                min: 0, max: 3)))], yields: []),
            no: (steps: [], yields: []))),
          (id: "untap", input: Some("lands"), kind: Perform(UntapChosenPermanents))], yields: [])"#,
    );
    let scope_error = invalid_program_reason(&out_of_scope);
    assert!(scope_error.contains("not in scope"), "{scope_error}");
}

#[test]
fn spell_program_requires_matching_explicit_if_result_joins() {
    let joined = program_fixture(
        r#"(steps: [(id: "if", bind: ["lands"], kind: If(condition: ControllerLibraryEmpty,
            yes: (steps: [(id: "yes_choose", bind: ["yes_lands"], kind: Perform(ChoosePermanents(
                chooser: Controller, filter: (kind: AnyPermanent, permanent_types: [Land]),
                min: 0, max: 3)))], yields: ["yes_lands"]),
            no: (steps: [(id: "no_choose", bind: ["no_lands"], kind: Perform(ChoosePermanents(
                chooser: Controller, filter: (kind: AnyPermanent, permanent_types: [Land]),
                min: 0, max: 3)))], yields: ["no_lands"]))),
          (id: "untap", input: Some("lands"), kind: Perform(UntapChosenPermanents))], yields: [])"#,
    );
    crate::CardRegistry::from_chunks_and_tokens(&[&joined], &[])
        .expect("an explicit join of the same result type on each arm is well typed");

    let mismatched = program_fixture(
        r#"(steps: [(id: "if", bind: ["result"], kind: If(condition: ControllerLibraryEmpty,
            yes: (steps: [(id: "yes_choose", bind: ["yes_lands"], kind: Perform(ChoosePermanents(
                chooser: Controller, filter: (kind: AnyPermanent, permanent_types: [Land]),
                min: 0, max: 3)))], yields: ["yes_lands"]),
            no: (steps: [(id: "no_draw", bind: ["drawn"], kind: Perform(Draw(count: 1)))],
                 yields: ["drawn"]))),
          ], yields: [])"#,
    );
    let join_error = invalid_program_reason(&mismatched);
    assert!(
        join_error.contains("mismatched explicit yield types"),
        "{join_error}"
    );
}

fn invalid_program_reason(definition: &str) -> String {
    match crate::CardRegistry::from_chunks_and_tokens(&[definition], &[]) {
        Err(crate::registry::RegistryError::InvalidCard { reason, .. }) => reason,
        Err(other) => panic!("program fixture failed before semantic validation: {other}"),
        Ok(_) => panic!("invalid spell program was accepted"),
    }
}

#[test]
fn spell_program_rejects_implicit_previous_result_counts() {
    for effect in [
        r#"Draw(count: Count(CardsMatchingResult(filter: (source: PreviousEffect, action: Discard, players: All))))"#,
        r#"SelectHandCards(count: Count(CardsMatchingResult(filter: (source: PreviousEffect, action: Discard, players: All))), ordered: true, prompt: "Choose")"#,
    ] {
        let definition = program_fixture(&format!(
            "(steps: [(id: \"read\", kind: Perform({effect}))], yields: [])"
        ));
        let reason = invalid_program_reason(&definition);
        assert!(reason.contains("result"), "{reason}");
    }
}

#[test]
fn spell_program_rejects_non_controller_context_and_cross_object_choices() {
    let invalid_effects = [
        (
            "draw affected player",
            r#"Draw(who: AffectedPlayer, count: 1)"#,
            "Draw recipient supports only the resolving spell controller",
        ),
        (
            "draw defending player",
            r#"Draw(who: DefendingPlayer, count: 1)"#,
            "Draw recipient supports only the resolving spell controller",
        ),
        (
            "draw prior target controller",
            r#"Draw(who: PreviousTargetedSpellController, count: 1)"#,
            "Draw recipient supports only the resolving spell controller",
        ),
        (
            "draw target-group controller",
            r#"Draw(who: ControllerOfTargetGroup(group_index: 0), count: 1)"#,
            "Draw recipient supports only the resolving spell controller",
        ),
        (
            "discard event player",
            r#"Discard(who: DefendingPlayer, quantity: Exact(1))"#,
            "Discard recipient supports only the resolving spell controller",
        ),
        (
            "choose for event player",
            r#"ChoosePermanents(chooser: DefendingPlayer,
                filter: (kind: AnyPermanent, permanent_types: [Land]), min: 0, max: 1)"#,
            "ChoosePermanents chooser supports only the resolving spell controller",
        ),
        (
            "choose with event-bound filter",
            r#"ChoosePermanents(chooser: Controller,
                filter: (kind: AnyPermanent, controller: DefendingPlayer,
                    permanent_types: [Land]), min: 0, max: 1)"#,
            "ChoosePermanents filter cannot use DefendingPlayer",
        ),
        (
            "choose with untyped cross-object constraint",
            r#"ChoosePermanents(chooser: Controller,
                filter: (kind: AnyPermanent, permanent_types: [Artifact],
                    required_subtypes: ["Equipment"]), min: 0, max: 1,
                constraints: [EquipmentAttachableTo(recipient: PreviousEffectObject)])"#,
            "ChoosePermanents constraints are not supported in spell_program",
        ),
        (
            "choose with event-bound attached-object exclusion",
            r#"ChoosePermanents(chooser: Controller,
                filter: (kind: AnyPermanent, excluded_objects: [AttachedObject]),
                min: 0, max: 1)"#,
            "ChoosePermanents filter cannot use AttachedObject",
        ),
    ];

    for (context, effect, expected_reason) in invalid_effects {
        let definition = program_fixture(&format!(
            "(steps: [(id: \"operation\", kind: Perform({effect}))], yields: [])"
        ));
        let reason = invalid_program_reason(&definition);
        assert!(reason.contains(expected_reason), "{context}: {reason}");
    }
}

#[test]
fn spell_program_rejects_cast_cost_and_trigger_event_amounts() {
    let cast_cost = r#"CastCost(cast_cost: (group_id: "cast_cost_01", option_id: "option_01"), when_true: 2, otherwise: 1)"#;
    for (label, amount) in [
        ("cast cost", cast_cost),
        ("trigger event count", "EventCount"),
    ] {
        for (effect, expected_reason) in [
            (
                format!("Draw(count: {amount})"),
                if label == "trigger event count" {
                    "trigger event's count"
                } else {
                    "cast-cost amount"
                },
            ),
            (
                format!("SelectHandCards(count: {amount}, ordered: true, prompt: \"Choose\")"),
                if label == "trigger event count" {
                    "trigger event's count"
                } else {
                    "cast-cost amount"
                },
            ),
        ] {
            let definition = program_fixture(&format!(
                "(steps: [(id: \"operation\", kind: Perform({effect}))], yields: [])"
            ));
            let reason = invalid_program_reason(&definition);
            assert!(
                reason.contains(expected_reason),
                "{label}, {effect}: {reason}"
            );
        }
    }
}

#[test]
fn program_grant_keywords_all_admits_untargeted_spell_and_ability_scopes() {
    let spell = program_fixture(
        r#"(steps: [(id: "grant_opponents", bind: [], input: None,
            kind: Perform(GrantKeywordsAll(
                filter: (controller: Some(Opponents), subtype: Some("Elf"), attacking: true),
                keywords: [Flying, Trample],
            )))], yields: [])"#,
    );
    let registry = crate::CardRegistry::from_chunks_and_tokens(&[&spell], &[])
        .unwrap_or_else(|error| panic!("an untargeted spell keyword grant compiles: {error}"));
    let owner = crate::ProgramOwnerId::face(
        "program_fixture",
        crate::CardFaceId::new("program_fixture").unwrap(),
    );
    let compiled = registry
        .compiled_program_for_owner(&owner)
        .expect("the spell face owns its resultless keyword-grant program");
    assert_eq!(compiled.owner, owner);
    assert_eq!(compiled.context, crate::primitives::EffectContext::Spell);
    assert!(compiled.required_inputs.is_empty());
    assert!(compiled.target_bindings.is_empty());
    assert!(compiled.body.yields.is_empty());
    let [step] = compiled.body.steps.as_slice() else {
        panic!("the spell keyword grant is one direct instruction");
    };
    assert!(step.outputs.is_empty());
    let crate::primitives::CompiledInstructionKind::Perform(effect) = &step.kind else {
        panic!("the spell keyword grant remains a Perform instruction");
    };
    assert_eq!(
        **effect,
        crate::primitives::SpellEffectKind::GrantKeywordsAll {
            filter: crate::primitives::CreatureScopeFilter {
                controller: Some(crate::primitives::CreatureScopeController::Opponents),
                subtype: Some("Elf".into()),
                attacking: true,
                ..Default::default()
            },
            keywords: vec![crate::Keyword::Flying, crate::Keyword::Trample],
        }
    );

    let ability = ability_program_fixture(
        "program_ability_keyword_scope_fixture",
        r#"(steps: [(id: "grant_friendly", bind: [], input: None,
            kind: Perform(GrantKeywordsAll(
                filter: (controller: Some(YouControl), color: Some(Blue), exclude_self: true),
                keywords: [Haste],
            )))], yields: [])"#,
    )
    .unwrap_or_else(|error| panic!("an ability keyword grant compiles: {error}"));
    assert_eq!(ability.context, crate::primitives::EffectContext::Ability);
    assert!(ability.required_inputs.is_empty());
    assert!(ability.target_bindings.is_empty());
    assert!(ability.body.yields.is_empty());
    let [step] = ability.body.steps.as_slice() else {
        panic!("the ability keyword grant is one direct instruction");
    };
    assert!(step.outputs.is_empty());
    let crate::primitives::CompiledInstructionKind::Perform(effect) = &step.kind else {
        panic!("the ability keyword grant remains a Perform instruction");
    };
    assert_eq!(
        **effect,
        crate::primitives::SpellEffectKind::GrantKeywordsAll {
            filter: crate::primitives::CreatureScopeFilter {
                controller: Some(crate::primitives::CreatureScopeController::YouControl),
                color: Some(crate::primitives::Color::Blue),
                exclude_self: true,
                ..Default::default()
            },
            keywords: vec![crate::Keyword::Haste],
        }
    );
}

#[test]
fn program_grant_keywords_all_rejects_targeted_player_and_invalid_scopes() {
    let targeted = program_fixture(
        r#"(steps: [(id: "grant_targeted", bind: [], input: None,
            kind: Perform(GrantKeywordsAll(
                filter: (controller: Some(TargetedPlayer(group_index: 0, kind: AnyPlayer))),
                keywords: [Menace],
            )))], yields: [])"#,
    );
    let reason = invalid_program_reason(&targeted);
    assert!(
        reason.contains("TargetedPlayer"),
        "a targeted player scope has no GrantKeywordsAll Program target-role binding: {reason}"
    );

    let invalid_scope = program_fixture(
        r#"(steps: [(id: "grant_invalid_scope", bind: [], input: None,
            kind: Perform(GrantKeywordsAll(
                filter: (requires_any_counter: true, required_counter: Some(PlusOnePlusOne)),
                keywords: [Vigilance],
            )))], yields: [])"#,
    );
    let reason = invalid_program_reason(&invalid_scope);
    assert!(
        reason.contains("cannot combine any-counter and specific-counter requirements"),
        "GrantKeywordsAll still runs ordinary CreatureScopeFilter validation: {reason}"
    );
}

#[test]
fn spell_program_rejects_dynamic_amount_selectors_and_if_contexts() {
    for (label, amount) in [
        (
            "target-selected player count",
            "Count(CardsInHand(players: ChosenTarget(group_index: 0, target_index: 0)))",
        ),
        (
            "event-selected player count",
            "Count(CardsInHand(players: AffectedPlayer))",
        ),
    ] {
        for effect in [
            format!("Draw(count: {amount})"),
            format!("SelectHandCards(count: {amount}, ordered: true, prompt: \"Choose\")"),
        ] {
            let definition = program_fixture(&format!(
                "(steps: [(id: \"operation\", kind: Perform({effect}))], yields: [])"
            ));
            let reason = invalid_program_reason(&definition);
            assert!(
                reason.contains("supports only a fixed literal"),
                "{label}, {effect}: {reason}"
            );
        }
    }

    for (label, condition) in [
        (
            "target-selected player condition",
            "CardsInHand(players: ChosenTarget(group_index: 0, target_index: 0), min: 1)",
        ),
        (
            "event-selected player condition",
            "CardsInHand(players: AffectedPlayer, min: 1)",
        ),
        (
            "previous-object condition",
            "ObjectMatches(object: PreviousEffectObject, filter: (kind: Creature))",
        ),
    ] {
        let body = format!(
            "(steps: [(id: \"if\", kind: If(condition: {condition}, yes: (steps: [], yields: []), no: (steps: [], yields: [])))], yields: [])"
        );
        let reason = invalid_program_reason(&program_fixture(&body));
        assert!(
            reason.contains("If condition uses context outside controller hand, library, CardCount graveyard, or Controller battlefield Count state"),
            "{label}: {reason}"
        );
    }

    let recursive_controller_condition = program_fixture(
        r#"(steps: [(id: "if", kind: If(
            condition: AllOf([
                ControllerLibraryEmpty,
                CardsInHand(players: Relative(Controller), min: 1),
            ]),
            yes: (steps: [], yields: []),
            no: (steps: [], yields: [])))], yields: [])"#,
    );
    crate::CardRegistry::from_chunks_and_tokens(&[&recursive_controller_condition], &[])
        .expect("recursive controller-only hand/library predicates are in the pilot subset");
}

#[test]
fn program_only_hand_operations_are_rejected_from_ordinary_effect_lists() {
    let legacy = r#"(id: "legacy_program_leaf", name: "Legacy Program Leaf", face_id: "legacy_program_leaf",
        types: ["Sorcery"], spell_effect: [SelectHandCards(count: 2, ordered: true, prompt: "Choose")])"#;
    let reason = match crate::CardRegistry::from_chunks_and_tokens(&[legacy], &[]) {
        Err(crate::registry::RegistryError::InvalidCard { reason, .. }) => reason,
        Err(other) => panic!("legacy fixture failed before semantic validation: {other}"),
        Ok(_) => panic!("program-only selection was accepted in spell_effect"),
    };
    assert!(reason.contains("only inside a spell_program"), "{reason}");
}

#[test]
fn program_choose_rejects_unsupported_admission_shapes() {
    let definition = |chooser: &str, cost: &str, requirement: &str, input: &str| {
        let body = format!(
            r#"(steps: [
                (id: "before", bind: ["prior"], kind: Perform(Draw(count: 1))),
                (id: "choose", bind: [], input: {input}, kind: Choose(
                    choice: "choice_01", chooser: {chooser}, optional: false,
                    branches: [(
                        branch_id: "branch_01", presentation: Fallback,
                        cost: {cost}, requirement: {requirement},
                        body: (steps: [], yields: []),
                    )],
                    otherwise: (steps: [], yields: []),
                )),
            ], yields: [])"#
        );
        program_fixture(&body)
    };

    for (label, chooser, cost, requirement, input, expected) in [
        (
            "event-bound chooser",
            "AffectedPlayer",
            "None",
            "Always",
            "None",
            "supports Controller, SourceController/AffectedPlayer on an ability program, or an optional OpponentPlayer target on a spell or ability program, as chooser",
        ),
        (
            "SourceController chooser on Spell",
            "SourceController",
            "None",
            "Always",
            "None",
            "supports Controller, SourceController/AffectedPlayer on an ability program, or an optional OpponentPlayer target on a spell or ability program, as chooser",
        ),
        (
            "object-payment cost",
            "Controller",
            "DiscardCard(filter: None)",
            "Always",
            "None",
            "does not support resolution cost",
        ),
        (
            "flat-list applicability requirement",
            "Controller",
            "None",
            "EffectsApplicable",
            "None",
            "supports EffectsApplicable only for an ability branch containing one resultless fixed positive Source +1/+1 PutCounters instruction",
        ),
        (
            "Void game-condition requirement remains unsupported for Choose",
            "Controller",
            "None",
            "GameCondition(Void)",
            "None",
            "If condition uses context outside controller hand, library, CardCount graveyard, or Controller battlefield Count state",
        ),
        (
            "implicit prior-result input",
            "Controller",
            "None",
            "Always",
            r#"Some("prior")"#,
            "cannot consume an input result",
        ),
    ] {
        let error = invalid_program_reason(&definition(chooser, cost, requirement, input));
        assert!(error.contains(expected), "{label}: {error}");
    }
}

#[test]
fn program_choose_admits_controller_spell_hand_bottom_cost() {
    let body = r#"(steps: [
            (id: "choose", bind: [], input: None, kind: Choose(
                choice: "choice_01", chooser: Controller, optional: true,
                branches: [(
                    branch_id: "bottom_and_draw", presentation: Fallback,
                    cost: PutHandCardOnLibraryBottom, requirement: Always,
                    body: (steps: [
                        (id: "draw", bind: [], input: None,
                            kind: Perform(Draw(who: Controller, count: 1)))
                    ], yields: []),
                )],
                otherwise: (steps: [], yields: []),
            )),
        ], yields: [])"#;
    let definition = program_fixture(body);
    let registry = crate::CardRegistry::from_chunks_and_tokens(&[&definition], &[])
        .expect("the fixed controller spell cost is admitted");
    let owner = crate::ProgramOwnerId::face(
        "program_fixture",
        crate::CardFaceId::new("program_fixture").unwrap(),
    );
    let program = registry
        .compiled_program_for_owner(&owner)
        .expect("the exact spell face owns its compiled Choose program");
    let [choose] = program.body.steps.as_slice() else {
        panic!("expected one structural Choose instruction");
    };
    let crate::primitives::CompiledInstructionKind::Choose {
        chooser,
        optional,
        branches,
        otherwise,
        ..
    } = &choose.kind
    else {
        panic!("the hand-bottom cost remains a Choose branch cost");
    };
    assert_eq!(*chooser, crate::primitives::PlayerRecipient::Controller);
    assert!(*optional);
    assert!(otherwise.steps.is_empty());
    let [branch] = branches.as_slice() else {
        panic!("expected one paid branch");
    };
    assert_eq!(
        branch.cost,
        crate::primitives::ResolutionCost::PutHandCardOnLibraryBottom
    );
    let [draw] = branch.body.steps.as_slice() else {
        panic!("the paid child draws exactly once");
    };
    assert!(matches!(
        &draw.kind,
        crate::primitives::CompiledInstructionKind::Perform(effect)
            if matches!(effect.as_ref(), crate::primitives::SpellEffectKind::Draw {
                who: crate::primitives::PlayerRecipient::Controller,
                count: crate::primitives::Amount::Fixed(1),
            })
    ));

    let source_controller_definition =
        program_fixture(&body.replace("chooser: Controller", "chooser: SourceController"));
    assert!(
        crate::CardRegistry::from_chunks_and_tokens(&[&source_controller_definition], &[]).is_err(),
        "the hand-bottom cost does not admit SourceController as a spell chooser"
    );

    let ability_program: crate::CardProgram =
        ron::from_str(&format!("(body: {body})")).expect("ability fixture parses");
    let ability_owner = crate::ProgramOwnerId::ability(crate::AbilityDefinitionId {
        card_id: "program_fixture".into(),
        face_id: crate::CardFaceId::new("program_fixture").unwrap(),
        ability_path: vec![crate::AbilityId::new("triggered_01").unwrap()],
    });
    let ability_error = crate::primitives::CompiledProgram::compile(
        ability_owner,
        crate::primitives::EffectContext::Ability,
        &ability_program,
    )
    .expect_err("the fixed hand-bottom spell cost is unavailable to abilities");
    assert!(
        ability_error.contains("does not support resolution cost"),
        "the failure is from the hand-bottom cost admission rule: {ability_error}"
    );
}

#[test]
fn program_choose_admits_controller_ability_sacrifice_permanent_cost() {
    let body = |chooser: &str, cost: &str| {
        format!(
            r#"(steps: [(id: "choose", bind: [], input: None, kind: Choose(
                choice: "sacrifice", chooser: {chooser}, optional: true,
                branches: [(
                    branch_id: "pay_sacrifice", presentation: Fallback,
                    cost: {cost}, requirement: Always,
                    body: (steps: [], yields: []),
                )],
                otherwise: (steps: [], yields: []),
            ))], yields: [])"#
        )
    };
    let servant_cost =
        "SacrificePermanent(filter: (kind: AnyPermanent, controller: You), source_only: true)";
    let west_wind_cost = "SacrificePermanent(filter: (any_of: Some([
        (kind: AnyPermanent, controller: You, token: Some(true)),
        (kind: AnyPermanent, controller: You, permanent_types: [Land]),
    ])))";

    for (card_id, cost, expected_cost) in [
        ("servant_sacrifice_fixture", servant_cost, servant_cost),
        (
            "west_wind_sacrifice_fixture",
            west_wind_cost,
            west_wind_cost,
        ),
    ] {
        let compiled =
            ability_program_fixture(card_id, &body("Controller", cost)).unwrap_or_else(|error| {
                panic!("the fixed Controller ability cost is admitted: {error}")
            });
        let [choose] = compiled.body.steps.as_slice() else {
            panic!("the sacrifice cost remains inside one typed Choose instruction")
        };
        let crate::primitives::CompiledInstructionKind::Choose {
            chooser, branches, ..
        } = &choose.kind
        else {
            panic!("the sacrifice payment remains a Choose branch cost")
        };
        assert_eq!(*chooser, crate::primitives::PlayerRecipient::Controller);
        let [branch] = branches.as_slice() else {
            panic!("the authored sacrifice branch remains unique")
        };
        let expected: crate::primitives::ResolutionCost =
            ron::from_str(expected_cost).expect("the exact owner cost is valid RON");
        assert_eq!(branch.cost, expected, "{card_id}");
    }

    let source_controller = ability_program_fixture(
        "source_controller_sacrifice_fixture",
        &body("SourceController", servant_cost),
    )
    .expect_err("SourceController is outside the fixed ability cost contract");
    assert!(
        source_controller.contains("does not support resolution cost"),
        "the failure comes from the narrow cost admission: {source_controller}"
    );

    let opponent_filter = ability_program_fixture(
        "opponent_sacrifice_filter_fixture",
        &body(
            "Controller",
            "SacrificePermanent(filter: (kind: AnyPermanent, controller: Opponent), source_only: false)",
        ),
    )
    .expect_err("a resolution sacrifice cannot select an opponent's permanent");
    assert!(
        opponent_filter.contains("resolution sacrifice cost must select a permanent you control"),
        "the shared sacrifice filter validation rejects opponent-controlled permanents: {opponent_filter}"
    );

    let source_only_exclusion = ability_program_fixture(
        "source_only_sacrifice_exclusion_fixture",
        &body(
            "Controller",
            "SacrificePermanent(filter: (kind: AnyPermanent, controller: You, excluded_objects: [Source]), source_only: true)",
        ),
    )
    .expect_err("a source-only sacrifice cannot exclude its source");
    assert!(
        source_only_exclusion.contains("source-only sacrifice cannot exclude its source"),
        "the shared sacrifice validation preserves the source-only exclusion rule: {source_only_exclusion}"
    );

    let spell = program_fixture(&body("Controller", servant_cost));
    let spell_error = match crate::CardRegistry::from_chunks_and_tokens(&[&spell], &[]) {
        Err(crate::registry::RegistryError::InvalidCard { reason, .. }) => reason,
        Err(other) => panic!("spell fixture failed before semantic validation: {other}"),
        Ok(_) => panic!("the ability-only sacrifice cost was admitted in a spell program"),
    };
    assert!(
        spell_error.contains("does not support resolution cost"),
        "the failure comes from the narrow ability-only cost admission: {spell_error}"
    );
}

#[test]
fn program_choose_admits_fixed_controller_ability_tap_permanent_cost() {
    let body = |chooser: &str, optional: bool, cost: &str, bind: &str, input: &str| {
        format!(
            r#"(steps: [(id: "choose", bind: {bind}, input: {input}, kind: Choose(
                choice: "tap", chooser: {chooser}, optional: {optional},
                branches: [(
                    branch_id: "pay_tap", presentation: Fallback,
                    cost: {cost}, requirement: Always,
                    body: (steps: [], yields: []),
                )],
                otherwise: (steps: [], yields: []),
            ))], yields: [])"#
        )
    };
    let any_permanent_cost =
        "TapPermanents(count: 1, filter: (kind: AnyPermanent, controller: You), exclude_source: false)";
    let creature_cost =
        "TapPermanents(count: 1, filter: (kind: Creature, controller: You), exclude_source: true)";

    for (card_id, optional, cost) in [
        ("command_bridge_tap_cost_fixture", true, any_permanent_cost),
        ("gravelgill_tap_cost_fixture", true, creature_cost),
        ("required_tap_cost_fixture", false, creature_cost),
    ] {
        let compiled =
            ability_program_fixture(card_id, &body("Controller", optional, cost, "[]", "None"))
                .unwrap_or_else(|error| {
                    panic!("the fixed Controller ability tap cost is admitted: {error}")
                });
        let [choose] = compiled.body.steps.as_slice() else {
            panic!("the tap payment remains inside one typed Choose instruction")
        };
        let crate::primitives::CompiledInstructionKind::Choose {
            chooser,
            optional: compiled_optional,
            branches,
            ..
        } = &choose.kind
        else {
            panic!("the tap payment remains a Choose branch cost")
        };
        assert_eq!(*chooser, crate::primitives::PlayerRecipient::Controller);
        assert_eq!(*compiled_optional, optional);
        let [branch] = branches.as_slice() else {
            panic!("the authored tap branch remains unique")
        };
        let expected: crate::primitives::ResolutionCost =
            ron::from_str(cost).expect("the exact tap cost is valid RON");
        assert_eq!(branch.cost, expected, "{card_id}");
    }

    for (label, chooser, cost) in [
        (
            "SourceController chooser",
            "SourceController",
            creature_cost,
        ),
        (
            "zero count",
            "Controller",
            "TapPermanents(count: 0, filter: (kind: Creature, controller: You), exclude_source: true)",
        ),
        (
            "count above one",
            "Controller",
            "TapPermanents(count: 2, filter: (kind: Creature, controller: You), exclude_source: true)",
        ),
        (
            "opponent-controlled filter",
            "Controller",
            "TapPermanents(count: 1, filter: (kind: Creature, controller: Opponent), exclude_source: true)",
        ),
        (
            "unsupported permanent kind",
            "Controller",
            "TapPermanents(count: 1, filter: (kind: AnyTarget, controller: You), exclude_source: true)",
        ),
        (
            "any_of filter",
            "Controller",
            "TapPermanents(count: 1, filter: (any_of: Some([(kind: Creature, controller: You), (kind: AnyPermanent, controller: You)])), exclude_source: true)",
        ),
        (
            "filter exclusions",
            "Controller",
            "TapPermanents(count: 1, filter: (kind: Creature, controller: You, excluded_objects: [Source]), exclude_source: true)",
        ),
    ] {
        assert!(
            ability_program_fixture(
                &format!("program_reject_tap_{}_fixture", label.replace(' ', "_")),
                &body(chooser, true, cost, "[]", "None"),
            )
            .is_err(),
            "the TapPermanents admission rejects {label}"
        );
    }

    let spell = program_fixture(&body("Controller", true, creature_cost, "[]", "None"));
    assert!(
        crate::CardRegistry::from_chunks_and_tokens(&[&spell], &[]).is_err(),
        "TapPermanents resolution costs are ability-only"
    );

    for (label, bind, input) in [
        ("result binding", "[\"tap_result\"]", "None"),
        ("result input", "[]", "Some(\"prior_result\")"),
    ] {
        assert!(
            ability_program_fixture(
                &format!("program_reject_tap_{}_fixture", label.replace(' ', "_")),
                &body("Controller", true, creature_cost, bind, input),
            )
            .is_err(),
            "the tap payment does not expose or consume results: {label}"
        );
    }

    let mixed = r#"(steps: [(id: "choose", bind: [], input: None, kind: Choose(
        choice: "tap", chooser: Controller, optional: false,
        branches: [
            (branch_id: "free", presentation: Fallback, cost: None,
                requirement: Always, body: (steps: [], yields: [])),
            (branch_id: "paid", presentation: Fallback,
                cost: TapPermanents(count: 1,
                    filter: (kind: Creature, controller: You), exclude_source: true),
                requirement: Always, body: (steps: [], yields: [])),
        ],
        otherwise: (steps: [], yields: []),
    ))], yields: [])"#;
    let mixed_error = ability_program_fixture("mixed_tap_cost_fixture", mixed)
        .expect_err("TapPermanents cannot introduce mixed free and paid sibling branches");
    assert!(
        mixed_error.contains("cannot mix free and paid sibling branches"),
        "{mixed_error}"
    );
}

#[test]
fn program_choose_admits_optional_controller_spell_graveyard_exile_cost() {
    let body = |cost: &str| {
        format!(
            r#"(steps: [(id: "choose", bind: [], input: None, kind: Choose(
                choice: "exile_graveyard_card", chooser: Controller, optional: true,
                branches: [(
                    branch_id: "exile_and_draw", presentation: Fallback,
                    cost: ExileGraveyardCard(filter: {cost}), requirement: Always,
                    body: (steps: [(id: "draw", bind: [], input: None,
                        kind: Perform(Draw(who: Controller, count: 1)))], yields: []),
                )],
                otherwise: (steps: [], yields: []),
            ))], yields: [])"#
        )
    };

    for (filter, expected_filter) in [
        ("None", None),
        (
            "Some((card_type: Some(InstantOrSorcery)))",
            Some(crate::primitives::ZoneCardFilter {
                card_type: Some(crate::primitives::CardTypeFilter::InstantOrSorcery),
                ..Default::default()
            }),
        ),
    ] {
        let definition = program_fixture(&body(filter));
        let registry = crate::CardRegistry::from_chunks_and_tokens(&[&definition], &[])
            .unwrap_or_else(|error| {
                panic!("the fixed controller spell graveyard-exile cost is admitted: {error}")
            });
        let owner = crate::ProgramOwnerId::face(
            "program_fixture",
            crate::CardFaceId::new("program_fixture").unwrap(),
        );
        let program = registry
            .compiled_program_for_owner(&owner)
            .expect("the exact spell face owns its compiled Choose program");
        let [choose] = program.body.steps.as_slice() else {
            panic!("graveyard exile remains one structural Choose instruction");
        };
        let crate::primitives::CompiledInstructionKind::Choose {
            chooser,
            optional,
            branches,
            otherwise,
            ..
        } = &choose.kind
        else {
            panic!("the graveyard exile remains a Choose branch cost");
        };
        assert_eq!(*chooser, crate::primitives::PlayerRecipient::Controller);
        assert!(*optional);
        assert!(otherwise.steps.is_empty() && otherwise.yields.is_empty());
        let [branch] = branches.as_slice() else {
            panic!("the spell has one graveyard-exile payment branch");
        };
        assert_eq!(
            branch.cost,
            crate::primitives::ResolutionCost::ExileGraveyardCard {
                filter: expected_filter,
            }
        );
        assert_eq!(branch.fallback_label(), "Exile a card from your graveyard");
        let [draw] = branch.body.steps.as_slice() else {
            panic!("the paid branch draws exactly one card");
        };
        assert!(matches!(
            &draw.kind,
            crate::primitives::CompiledInstructionKind::Perform(effect)
                if matches!(effect.as_ref(), crate::primitives::SpellEffectKind::Draw {
                    who: crate::primitives::PlayerRecipient::Controller,
                    count: crate::primitives::Amount::Fixed(1),
                })
        ));
    }

    let mandatory = body("None").replace("optional: true", "optional: false");
    let mandatory_error = invalid_program_reason(&program_fixture(&mandatory));
    assert!(
        mandatory_error.contains("requires an optional Choose"),
        "graveyard exile is admitted only as an optional payment: {mandatory_error}"
    );
}

#[test]
fn program_spell_graveyard_exile_cost_rejects_bad_context_filters_and_mixed_branches() {
    let body = |chooser: &str, filter: &str| {
        format!(
            r#"(steps: [(id: "choose", bind: [], input: None, kind: Choose(
                choice: "exile_graveyard_card", chooser: {chooser}, optional: true,
                branches: [(
                    branch_id: "exile_and_draw", presentation: Fallback,
                    cost: ExileGraveyardCard(filter: {filter}), requirement: Always,
                    body: (steps: [(id: "draw", bind: [], input: None,
                        kind: Perform(Draw(who: Controller, count: 1)))], yields: []),
                )],
                otherwise: (steps: [], yields: []),
            ))], yields: [])"#
        )
    };
    let valid_filter = "Some((card_type: Some(InstantOrSorcery)))";

    let wrong_chooser =
        invalid_program_reason(&program_fixture(&body("SourceController", valid_filter)));
    assert!(
        wrong_chooser.contains("supports Controller"),
        "the spell cost requires its Controller to choose: {wrong_chooser}"
    );

    let ability_error = ability_program_fixture(
        "spell_graveyard_exile_ability_fixture",
        &body("Controller", "None"),
    )
    .expect_err("graveyard-exile resolution payment remains spell-only");
    assert!(
        ability_error.contains("does not support resolution cost"),
        "the ability rejection comes from the cost context gate: {ability_error}"
    );

    for (label, filter, expected) in [
        (
            "one-branch any_of",
            "Some((any_of: Some([(card_type: Some(Instant))])))",
            "zone card filter any_of requires at least two branches",
        ),
        (
            "empty exact name",
            "Some((exact_name: Some(\" \")))",
            "zone card filter exact name cannot be empty",
        ),
    ] {
        let error = invalid_program_reason(&program_fixture(&body("Controller", filter)));
        assert!(error.contains(expected), "{label}: {error}");
    }

    let mixed = r#"(steps: [(id: "choose", bind: [], input: None, kind: Choose(
        choice: "exile_graveyard_card", chooser: Controller, optional: true,
        branches: [
            (branch_id: "free", presentation: Fallback, cost: None,
                requirement: Always,
                body: (steps: [(id: "free_draw", kind: Perform(Draw(who: Controller, count: 1)))], yields: [])),
            (branch_id: "paid", presentation: Fallback,
                cost: ExileGraveyardCard(filter: None), requirement: Always,
                body: (steps: [(id: "paid_draw", kind: Perform(Draw(who: Controller, count: 1)))], yields: [])),
        ],
        otherwise: (steps: [], yields: []),
    ))], yields: [])"#;
    let mixed_error = invalid_program_reason(&program_fixture(mixed));
    assert!(
        mixed_error.contains("cannot mix free and paid sibling branches"),
        "the spell Choose keeps its homogeneous cost contract: {mixed_error}"
    );
}

#[test]
fn spell_program_accepts_fixed_controller_of_target_group_damage_riders() {
    let compile = |card_id: &str,
                   context: crate::primitives::EffectContext,
                   body: &str,
                   target_bindings: Vec<crate::primitives::ProgramTargetReference>,
                   targeting: &crate::primitives::TargetingDef| {
        let program = crate::primitives::CardProgram {
            body: ron::from_str(body).expect("damage rider program fixture parses"),
            target_bindings,
        };
        let owner = match context {
            crate::primitives::EffectContext::Spell => {
                crate::ProgramOwnerId::face(card_id, crate::CardFaceId::new(card_id).unwrap())
            }
            crate::primitives::EffectContext::Ability => {
                crate::ProgramOwnerId::ability(crate::AbilityDefinitionId {
                    card_id: card_id.into(),
                    face_id: crate::CardFaceId::new(card_id).unwrap(),
                    ability_path: vec![crate::AbilityId::new("triggered_01").unwrap()],
                })
            }
        };
        crate::primitives::CompiledProgram::compile_with_targeting(
            owner,
            context,
            &program,
            Some(targeting),
        )
    };
    let targeting = |min, max| crate::primitives::TargetingDef {
        groups: vec![crate::primitives::TargetGroupDef {
            chooser: crate::primitives::TargetChooser::Controller,
            min,
            max,
            prompt: "Choose target creature".into(),
            effect_indices: Vec::new(),
            distinct_from: Vec::new(),
            same_graveyard: false,
            cast_cost_expansion: None,
        }],
    };
    let binding = || {
        vec![crate::primitives::ProgramTargetReference {
            instruction_id: "damage_target".into(),
            role: crate::primitives::ProgramTargetRole::Target,
            target_group_index: 0,
        }]
    };
    let direct_body =
        |target_amount: u32, player_amount: &str, recipient: &str, target_kind: &str| {
            format!(
                r#"(steps: [
                (id: "damage_target", kind: Perform(DamageTarget(
                    amount: {target_amount}, target: (kind: {target_kind}),
                ))),
                (id: "damage_player", kind: Perform(DamagePlayer(
                    amount: {player_amount}, who: {recipient},
                ))),
            ], yields: [])"#
            )
        };
    let creature_targeting = targeting(1, 1);

    for (card_id, target_amount, player_amount) in [
        ("chandras_outrage_spell_shape", 4, 2),
        ("synthetic_fixed_three_damage_rider", 4, 3),
    ] {
        let body = direct_body(
            target_amount,
            &player_amount.to_string(),
            "ControllerOfTargetGroup(group_index: 0)",
            "Creature",
        );
        let compiled = compile(
            card_id,
            crate::primitives::EffectContext::Spell,
            &body,
            binding(),
            &creature_targeting,
        )
        .unwrap_or_else(|error| {
            panic!("the fixed {target_amount}+{player_amount} spell shape is admitted: {error}")
        });
        assert_eq!(compiled.target_bindings.len(), 1);
        assert_eq!(compiled.target_bindings[0].instruction_address, 0);
        assert_eq!(compiled.target_bindings[0].target_group_index, 0);
        assert_eq!(compiled.target_bindings[0].role_index, 0);
        assert_eq!(compiled.group_inputs.len(), 1);
        assert_eq!(
            compiled.group_inputs[0],
            crate::primitives::ProgramGroupInput {
                instruction_address: 1,
                group_index: 0,
            }
        );
        let schema =
            crate::primitives::TargetSchema::compile_program(&compiled, Some(&creature_targeting))
                .expect("only DamageTarget contributes the public target role");
        assert_eq!(schema.groups.len(), 1);
        assert_eq!(schema.groups[0].bindings.len(), 1);
        assert!(schema.effect_is_targeted(0));
        assert!(!schema.effect_is_targeted(1));
        let [_, rider] = compiled.body.steps.as_slice() else {
            panic!("the direct spell shape has target damage followed by one player rider");
        };
        assert!(matches!(
            &rider.kind,
            crate::primitives::CompiledInstructionKind::Perform(effect)
                if matches!(effect.as_ref(), crate::primitives::SpellEffectKind::DamagePlayer {
                    amount: crate::primitives::Amount::Fixed(amount),
                    who: crate::primitives::PlayerRecipient::ControllerOfTargetGroup { group_index: 0 },
                } if *amount == player_amount)
        ));
    }

    let heated_body = r#"(steps: [
        (id: "damage_target", kind: Perform(DamageTarget(
            amount: 6, target: (kind: Creature),
        ))),
        (id: "exile_choice", kind: Choose(
            choice: "exile_graveyard_card", chooser: Controller, optional: true,
            branches: [(branch_id: "pay_exile", presentation: Fallback,
                cost: ExileGraveyardCard(filter: None), requirement: Always,
                body: (steps: [(id: "damage_controller", kind: Perform(DamagePlayer(
                    amount: 2, who: ControllerOfTargetGroup(group_index: 0),
                )))], yields: []))],
            otherwise: (steps: [], yields: []),
        )),
    ], yields: [])"#;
    let heated = compile(
        "heated_argument_program_shape",
        crate::primitives::EffectContext::Spell,
        heated_body,
        binding(),
        &creature_targeting,
    )
    .expect("Heated Argument's 6+2 optional-payment program shape is admitted");
    assert_eq!(heated.target_bindings.len(), 1);
    assert_eq!(heated.group_inputs.len(), 1);
    assert_eq!(
        heated.group_inputs[0],
        crate::primitives::ProgramGroupInput {
            instruction_address: 2,
            group_index: 0,
        }
    );
    let heated_schema =
        crate::primitives::TargetSchema::compile_program(&heated, Some(&creature_targeting))
            .expect("Heated Argument keeps one DamageTarget role and no player target role");
    assert_eq!(heated_schema.groups.len(), 1);
    assert_eq!(heated_schema.groups[0].bindings.len(), 1);
    assert!(heated_schema.effect_is_targeted(0));
    assert!(!heated_schema.effect_is_targeted(2));
    let [_, choice] = heated.body.steps.as_slice() else {
        panic!("Heated Argument damages its creature before the optional cost choice");
    };
    let crate::primitives::CompiledInstructionKind::Choose {
        chooser,
        optional,
        branches,
        otherwise,
        ..
    } = &choice.kind
    else {
        panic!("the graveyard exile remains an optional Program Choose");
    };
    assert_eq!(*chooser, crate::primitives::PlayerRecipient::Controller);
    assert!(*optional);
    assert!(otherwise.steps.is_empty() && otherwise.yields.is_empty());
    let [branch] = branches.as_slice() else {
        panic!("Heated Argument has one optional graveyard-exile branch");
    };
    assert_eq!(
        branch.cost,
        crate::primitives::ResolutionCost::ExileGraveyardCard { filter: None }
    );
    let [player_damage] = branch.body.steps.as_slice() else {
        panic!("the paid branch damages the target creature's controller exactly once");
    };
    assert!(matches!(
        &player_damage.kind,
        crate::primitives::CompiledInstructionKind::Perform(effect)
            if matches!(effect.as_ref(), crate::primitives::SpellEffectKind::DamagePlayer {
                amount: crate::primitives::Amount::Fixed(2),
                who: crate::primitives::PlayerRecipient::ControllerOfTargetGroup { group_index: 0 },
            })
    ));

    let public_effects = [
        crate::primitives::SpellEffectKind::DamageTarget {
            amount: crate::primitives::Amount::Fixed(4),
            target: crate::primitives::TargetFilter {
                kind: crate::primitives::TargetKind::Creature,
                ..Default::default()
            },
        },
        crate::primitives::SpellEffectKind::DamagePlayer {
            amount: crate::primitives::Amount::Fixed(3),
            who: crate::primitives::PlayerRecipient::ControllerOfTargetGroup { group_index: 0 },
        },
    ];
    let public_schema = crate::primitives::TargetSchema::compile(&public_effects, None)
        .expect("public target schema exposes only the DamageTarget role");
    assert_eq!(public_schema.groups.len(), 1);
    assert_eq!(public_schema.groups[0].bindings.len(), 1);
    assert!(public_schema.effect_is_targeted(0));
    assert!(!public_schema.effect_is_targeted(1));

    let singleton = targeting(1, 1).groups.remove(0);
    let two_group_targeting = crate::primitives::TargetingDef {
        groups: vec![singleton.clone(), singleton],
    };
    let nonzero_group = compile(
        "nonzero_owner_local_group_input",
        crate::primitives::EffectContext::Spell,
        r#"(steps: [
            (id: "damage_first", kind: Perform(DamageTarget(
                amount: 1, target: (kind: Creature),
            ))),
            (id: "damage_second", kind: Perform(DamageTarget(
                amount: 1, target: (kind: Creature),
            ))),
            (id: "damage_player", kind: Perform(DamagePlayer(
                amount: 3, who: ControllerOfTargetGroup(group_index: 1),
            ))),
        ], yields: [])"#,
        vec![
            crate::primitives::ProgramTargetReference {
                instruction_id: "damage_first".into(),
                role: crate::primitives::ProgramTargetRole::Target,
                target_group_index: 0,
            },
            crate::primitives::ProgramTargetReference {
                instruction_id: "damage_second".into(),
                role: crate::primitives::ProgramTargetRole::Target,
                target_group_index: 1,
            },
        ],
        &two_group_targeting,
    )
    .expect("the non-targeting group input uses the owner-local second target group");
    assert_eq!(nonzero_group.group_inputs.len(), 1);
    assert_eq!(
        nonzero_group.group_inputs[0],
        crate::primitives::ProgramGroupInput {
            instruction_address: 2,
            group_index: 1,
        }
    );
}

#[test]
fn spell_program_admits_positive_fixed_damage_to_each_opponent() {
    let compile = |card_id: &str, context: crate::primitives::EffectContext, amount: &str| {
        let body = format!(
            r#"(steps: [(id: "damage_each_opponent", bind: [], input: None,
                kind: Perform(DamagePlayer(amount: {amount}, who: EachOpponent)))], yields: [])"#
        );
        let program = crate::primitives::CardProgram {
            body: ron::from_str(&body).expect("EachOpponent damage program parses"),
            target_bindings: Vec::new(),
        };
        let owner = match context {
            crate::primitives::EffectContext::Spell => {
                crate::ProgramOwnerId::face(card_id, crate::CardFaceId::new(card_id).unwrap())
            }
            crate::primitives::EffectContext::Ability => {
                crate::ProgramOwnerId::ability(crate::AbilityDefinitionId {
                    card_id: card_id.into(),
                    face_id: crate::CardFaceId::new(card_id).unwrap(),
                    ability_path: vec![crate::AbilityId::new("triggered_01").unwrap()],
                })
            }
        };
        crate::primitives::CompiledProgram::compile(owner, context, &program)
    };

    let compiled = compile(
        "positive_each_opponent_damage",
        crate::primitives::EffectContext::Spell,
        "2",
    )
    .expect("a positive fixed spell amount can damage each opponent");
    assert!(matches!(
        &compiled.body.steps[0].kind,
        crate::primitives::CompiledInstructionKind::Perform(effect)
            if matches!(effect.as_ref(), crate::primitives::SpellEffectKind::DamagePlayer {
                amount: crate::primitives::Amount::Fixed(2),
                who: crate::primitives::PlayerRecipient::EachOpponent,
            })
    ));
    for (amount, id_suffix) in [("0", "zero"), ("\"X\"", "x")] {
        let error = compile(
            &format!("invalid_each_opponent_damage_{id_suffix}"),
            crate::primitives::EffectContext::Spell,
            amount,
        )
        .expect_err("zero and variable all-opponent damage are outside this admission");
        assert!(
            error.contains("requires a positive fixed amount"),
            "{amount}: {error}"
        );
    }
    let ability_error = compile(
        "each_opponent_damage_ability",
        crate::primitives::EffectContext::Ability,
        "2",
    )
    .expect_err("this reusable admission stays spell-owned");
    assert!(ability_error.contains("is spell-only"), "{ability_error}");
}

#[test]
fn spell_program_target_group_damage_rider_rejects_unsupported_contracts() {
    let compile = |card_id: &str,
                   context: crate::primitives::EffectContext,
                   body: &str,
                   target_bindings: Vec<crate::primitives::ProgramTargetReference>,
                   targeting: &crate::primitives::TargetingDef| {
        let program = crate::primitives::CardProgram {
            body: ron::from_str(body).expect("damage rider program fixture parses"),
            target_bindings,
        };
        let owner = match context {
            crate::primitives::EffectContext::Spell => {
                crate::ProgramOwnerId::face(card_id, crate::CardFaceId::new(card_id).unwrap())
            }
            crate::primitives::EffectContext::Ability => {
                crate::ProgramOwnerId::ability(crate::AbilityDefinitionId {
                    card_id: card_id.into(),
                    face_id: crate::CardFaceId::new(card_id).unwrap(),
                    ability_path: vec![crate::AbilityId::new("triggered_01").unwrap()],
                })
            }
        };
        crate::primitives::CompiledProgram::compile_with_targeting(
            owner,
            context,
            &program,
            Some(targeting),
        )
    };
    let targeting = |min, max| crate::primitives::TargetingDef {
        groups: vec![crate::primitives::TargetGroupDef {
            chooser: crate::primitives::TargetChooser::Controller,
            min,
            max,
            prompt: "Choose target creature".into(),
            effect_indices: Vec::new(),
            distinct_from: Vec::new(),
            same_graveyard: false,
            cast_cost_expansion: None,
        }],
    };
    let body = |amount: &str, recipient: &str, target_kind: &str| {
        format!(
            r#"(steps: [
                (id: "damage_target", kind: Perform(DamageTarget(
                    amount: 4, target: (kind: {target_kind}),
                ))),
                (id: "damage_player", kind: Perform(DamagePlayer(
                    amount: {amount}, who: {recipient},
                ))),
            ], yields: [])"#
        )
    };
    let binding = || {
        vec![crate::primitives::ProgramTargetReference {
            instruction_id: "damage_target".into(),
            role: crate::primitives::ProgramTargetRole::Target,
            target_group_index: 0,
        }]
    };
    let singleton = targeting(1, 1);

    for recipient in ["Controller", "SourceController"] {
        assert!(
            compile(
                "unsupported_damage_recipient_rejected",
                crate::primitives::EffectContext::Spell,
                &body("2", recipient, "Creature"),
                binding(),
                &singleton,
            )
            .is_err(),
            "the rider recipient must name the selected group's controller, not {recipient}"
        );
    }

    for amount in ["X", "Count(CardsInHand(players: Relative(Controller)))"] {
        assert!(
            compile(
                "nonfixed_player_damage_rejected",
                crate::primitives::EffectContext::Spell,
                &body(
                    amount,
                    "ControllerOfTargetGroup(group_index: 0)",
                    "Creature"
                ),
                binding(),
                &singleton,
            )
            .is_err(),
            "nonfixed DamagePlayer amount {amount} is outside the fixed rider contract"
        );
    }

    assert!(
        compile(
            "out_of_range_recipient_group_rejected",
            crate::primitives::EffectContext::Spell,
            &body("2", "ControllerOfTargetGroup(group_index: 1)", "Creature"),
            binding(),
            &singleton,
        )
        .is_err(),
        "the recipient cannot reference a target group outside the owner schema"
    );
    assert!(
        compile(
            "nonsingleton_recipient_group_rejected",
            crate::primitives::EffectContext::Spell,
            &body("2", "ControllerOfTargetGroup(group_index: 0)", "Creature"),
            binding(),
            &targeting(1, 2),
        )
        .is_err(),
        "the recipient requires exactly one target in its group"
    );
    assert!(
        compile(
            "noncreature_recipient_group_rejected",
            crate::primitives::EffectContext::Spell,
            &body(
                "2",
                "ControllerOfTargetGroup(group_index: 0)",
                "AnyPermanent"
            ),
            binding(),
            &singleton,
        )
        .is_err(),
        "the recipient requires a Creature target role rather than any permanent"
    );

    let one_group = singleton.groups[0].clone();
    let two_singleton_groups = crate::primitives::TargetingDef {
        groups: vec![one_group.clone(), one_group],
    };
    let wrong_creature_role = compile(
        "unrelated_creature_role_group_rejected",
        crate::primitives::EffectContext::Spell,
        r#"(steps: [
            (id: "damage_target", kind: Perform(DamageTarget(
                amount: 4, target: (kind: Creature),
            ))),
            (id: "return_target", kind: Perform(ReturnToOwnersHand(
                subject: Chosen((kind: Creature)),
            ))),
            (id: "damage_player", kind: Perform(DamagePlayer(
                amount: 2, who: ControllerOfTargetGroup(group_index: 1),
            ))),
        ], yields: [])"#,
        vec![
            crate::primitives::ProgramTargetReference {
                instruction_id: "damage_target".into(),
                role: crate::primitives::ProgramTargetRole::Target,
                target_group_index: 0,
            },
            crate::primitives::ProgramTargetReference {
                instruction_id: "return_target".into(),
                role: crate::primitives::ProgramTargetRole::Subject,
                target_group_index: 1,
            },
        ],
        &two_singleton_groups,
    )
    .expect_err("a creature group bound to a different role cannot supply DamagePlayer");
    assert!(
        wrong_creature_role.contains("Creature DamageTarget role"),
        "the compiler requires the selected group's own DamageTarget binding: {wrong_creature_role}"
    );

    assert!(
        compile(
            "missing_damage_target_binding_rejected",
            crate::primitives::EffectContext::Spell,
            &body("2", "ControllerOfTargetGroup(group_index: 0)", "Creature"),
            Vec::new(),
            &singleton,
        )
        .is_err(),
        "the group must bind the existing DamageTarget role"
    );
    assert!(
        compile(
            "mismatched_damage_player_binding_rejected",
            crate::primitives::EffectContext::Spell,
            &body("2", "ControllerOfTargetGroup(group_index: 0)", "Creature"),
            vec![crate::primitives::ProgramTargetReference {
                instruction_id: "damage_player".into(),
                role: crate::primitives::ProgramTargetRole::Target,
                target_group_index: 0,
            }],
            &singleton,
        )
        .is_err(),
        "DamagePlayer does not gain a target role through the recipient reference"
    );
    assert!(
        compile(
            "out_of_range_target_binding_rejected",
            crate::primitives::EffectContext::Spell,
            &body("2", "ControllerOfTargetGroup(group_index: 0)", "Creature"),
            vec![crate::primitives::ProgramTargetReference {
                instruction_id: "damage_target".into(),
                role: crate::primitives::ProgramTargetRole::Target,
                target_group_index: 1,
            }],
            &singleton,
        )
        .is_err(),
        "the existing DamageTarget binding must point at an authored group"
    );
    assert!(
        compile(
            "ability_damage_rider_rejected",
            crate::primitives::EffectContext::Ability,
            &body("2", "ControllerOfTargetGroup(group_index: 0)", "Creature"),
            binding(),
            &singleton,
        )
        .is_err(),
        "ControllerOfTargetGroup DamagePlayer remains spell-only"
    );
}

#[test]
fn program_sacrifice_rejects_any_of_branch_exclusions_but_keeps_supported_filters() {
    let body = |cost: &str| {
        format!(
            r#"(steps: [(id: "choose", bind: [], input: None, kind: Choose(
                choice: "sacrifice", chooser: Controller, optional: true,
                branches: [(
                    branch_id: "pay_sacrifice", presentation: Fallback,
                    cost: {cost}, requirement: Always,
                    body: (steps: [], yields: []),
                )],
                otherwise: (steps: [], yields: []),
            ))], yields: [])"#
        )
    };

    for (card_suffix, exclusion) in [("source", "Source"), ("attached_object", "AttachedObject")] {
        let cost = format!(
            "SacrificePermanent(filter: (any_of: Some([\
                (kind: AnyPermanent, controller: You, excluded_objects: [{exclusion}]),\
                (kind: AnyPermanent, controller: You, permanent_types: [Land]),\
            ])), source_only: false)"
        );
        let error = ability_program_fixture(
            &format!("program_any_of_{card_suffix}_exclusion_fixture"),
            &body(&cost),
        )
        .expect_err("Program sacrifice filters reject exclusions on any_of branches");
        let diagnostic = error.to_ascii_lowercase();
        assert!(
            diagnostic.contains("program")
                && diagnostic.contains("any_of")
                && diagnostic.contains("excluded_objects"),
            "the typed Program rejection identifies the unsupported branch exclusion: {error}"
        );
    }

    for (card_suffix, exclusion) in [("source", "Source"), ("attached_object", "AttachedObject")] {
        let cost = format!(
            "SacrificePermanent(filter: (kind: AnyPermanent, controller: You, excluded_objects: [{exclusion}]), source_only: false)"
        );
        let compiled = ability_program_fixture(
            &format!("program_flat_{card_suffix}_exclusion_fixture"),
            &body(&cost),
        )
        .unwrap_or_else(|error| panic!("flat {exclusion} exclusions remain supported: {error}"));
        let [choose] = compiled.body.steps.as_slice() else {
            panic!("the flat sacrifice filter remains in one Choose instruction")
        };
        let crate::primitives::CompiledInstructionKind::Choose { branches, .. } = &choose.kind
        else {
            panic!("the flat sacrifice filter remains a Choose branch cost")
        };
        let [branch] = branches.as_slice() else {
            panic!("the flat sacrifice branch remains unique")
        };
        let expected: crate::primitives::ResolutionCost =
            ron::from_str(&cost).expect("the flat sacrifice cost parses");
        assert_eq!(branch.cost, expected, "flat {exclusion} filter");
    }
}

#[test]
fn program_choose_admits_fixed_blight_only_for_controller_ability_programs() {
    use crate::primitives::{CompiledProgram, EffectContext};

    let compile = |card_id: &str, context, body: &str| {
        let program: crate::CardProgram =
            ron::from_str(&format!("(body: {body})")).expect("program fixture parses");
        let owner = crate::ProgramOwnerId::ability(crate::AbilityDefinitionId {
            card_id: card_id.into(),
            face_id: crate::CardFaceId::new(card_id).unwrap(),
            ability_path: vec![crate::AbilityId::new("triggered_01").unwrap()],
        });
        CompiledProgram::compile(owner, context, &program)
    };
    let body = r#"(steps: [(id: "choose", bind: [], kind: Choose(
        choice: "blight", chooser: Controller, optional: true,
        branches: [(branch_id: "pay_blight", presentation: Fallback,
            cost: Blight(count: 2), requirement: Always,
            body: (steps: [(id: "draw", kind: Perform(Draw(who: Controller, count: 1)))],
                yields: []))],
        otherwise: (steps: [], yields: []),
    ))], yields: [])"#;
    let compiled = compile("blight_program_fixture", EffectContext::Ability, body)
        .expect("a fixed positive Blight branch is admitted on an ability program");
    let [choose] = compiled.body.steps.as_slice() else {
        panic!("the Blight cost remains inside one typed Choose instruction")
    };
    let crate::primitives::CompiledInstructionKind::Choose {
        chooser, branches, ..
    } = &choose.kind
    else {
        panic!("the Blight payment remains a Choose branch")
    };
    assert_eq!(*chooser, crate::primitives::PlayerRecipient::Controller);
    let [branch] = branches.as_slice() else {
        panic!("the authored paid branch remains unique")
    };
    assert_eq!(
        branch.cost,
        crate::primitives::ResolutionCost::Blight { count: 2 }
    );

    let source_controller = body.replace("chooser: Controller", "chooser: SourceController");
    assert!(
        compile(
            "source_controller_blight_fixture",
            EffectContext::Ability,
            &source_controller,
        )
        .is_err(),
        "a SourceController chooser is outside the narrow Blight payment contract"
    );
    let zero_blight = body.replace("Blight(count: 2)", "Blight(count: 0)");
    assert!(
        compile("zero_blight_fixture", EffectContext::Ability, &zero_blight).is_err(),
        "zero is not a positive fixed Blight payment"
    );
    assert!(
        compile("spell_blight_fixture", EffectContext::Spell, body).is_err(),
        "the Blight cost is not admitted on a spell program"
    );
}

#[test]
fn program_choose_admits_optional_filtered_discard_for_ability_controller() {
    use crate::primitives::{CompiledProgram, EffectContext};

    let compile = |context: EffectContext, body: &str| {
        let program: crate::CardProgram =
            ron::from_str(&format!("(body: {body})")).expect("program fixture parses");
        let card_id = "crypt_lurker_choose_fixture";
        let owner = match context {
            EffectContext::Spell => {
                crate::ProgramOwnerId::face(card_id, crate::CardFaceId::new(card_id).unwrap())
            }
            EffectContext::Ability => crate::ProgramOwnerId::ability(crate::AbilityDefinitionId {
                card_id: card_id.into(),
                face_id: crate::CardFaceId::new(card_id).unwrap(),
                ability_path: vec![crate::AbilityId::new("triggered_01").unwrap()],
            }),
        };
        CompiledProgram::compile(owner, context, &program)
    };
    let body = r#"(steps: [(id: "choose_crypt_lurker_branch", kind: Choose(
        choice: "crypt_lurker", chooser: Controller, optional: true,
        branches: [
            (branch_id: "sacrifice_a_creature", presentation: Fallback,
                cost: SacrificePermanent(filter: (kind: Creature, controller: You)),
                requirement: Always,
                body: (steps: [(id: "draw_after_sacrifice", kind: Perform(
                    Draw(who: Controller, count: 1)))], yields: [])),
            (branch_id: "discard_a_creature_card", presentation: Fallback,
                cost: DiscardCard(filter: Some(Creature)),
                requirement: Always,
                body: (steps: [(id: "draw_after_discard", kind: Perform(
                    Draw(who: Controller, count: 1)))], yields: [])),
        ],
        otherwise: (steps: [], yields: []),
    ))], yields: [])"#;

    let compiled = compile(EffectContext::Ability, body)
        .expect("the optional ability Choose admits Crypt Lurker's filtered discard cost");
    assert_eq!(compiled.context, EffectContext::Ability);
    assert!(compiled.body.yields.is_empty());
    let [choose] = compiled.body.steps.as_slice() else {
        panic!("Crypt Lurker has one resultless branch choice");
    };
    assert!(choose.outputs.is_empty());
    let crate::primitives::CompiledInstructionKind::Choose {
        choice,
        chooser,
        optional,
        branches,
        otherwise,
        ..
    } = &choose.kind
    else {
        panic!("Crypt Lurker's effect is a typed Choose");
    };
    assert_eq!(choice.as_str(), "crypt_lurker");
    assert_eq!(*chooser, crate::primitives::PlayerRecipient::Controller);
    assert!(*optional);
    assert!(otherwise.steps.is_empty() && otherwise.yields.is_empty());
    let [sacrifice, discard] = branches.as_slice() else {
        panic!("Crypt Lurker keeps its sacrifice and discard branches in order");
    };
    assert_eq!(sacrifice.branch_id.as_str(), "sacrifice_a_creature");
    assert_eq!(
        sacrifice.cost,
        crate::primitives::ResolutionCost::SacrificePermanent {
            filter: crate::primitives::TargetFilter {
                kind: crate::primitives::TargetKind::Creature,
                controller: crate::primitives::TargetController::You,
                ..Default::default()
            },
            source_only: false,
        }
    );
    assert_eq!(discard.branch_id.as_str(), "discard_a_creature_card");
    assert_eq!(
        discard.cost,
        crate::primitives::ResolutionCost::DiscardCard {
            filter: Some(crate::primitives::CardTypeFilter::Creature),
        }
    );
    for (branch, draw_id) in [
        (sacrifice, "draw_after_sacrifice"),
        (discard, "draw_after_discard"),
    ] {
        assert!(branch.body.yields.is_empty());
        let [draw] = branch.body.steps.as_slice() else {
            panic!("each paid branch draws exactly one card");
        };
        assert_eq!(draw.id.as_str(), draw_id);
        assert!(draw.outputs.is_empty());
        let crate::primitives::CompiledInstructionKind::Perform(effect) = &draw.kind else {
            panic!("each paid branch's resultless body is one Draw");
        };
        assert_eq!(
            **effect,
            crate::primitives::SpellEffectKind::Draw {
                who: crate::primitives::PlayerRecipient::Controller,
                count: crate::primitives::Amount::Fixed(1),
            }
        );
    }

    let discard_only = |chooser: &str, optional: &str, filter: &str| {
        format!(
            r#"(steps: [(id: "choose_discard", kind: Choose(
                choice: "discard", chooser: {chooser}, optional: {optional},
                branches: [(branch_id: "discard_creature", presentation: Fallback,
                    cost: DiscardCard(filter: {filter}), requirement: Always,
                    body: (steps: [(id: "draw", kind: Perform(
                        Draw(who: Controller, count: 1)))], yields: []))],
                otherwise: (steps: [], yields: []),
            ))], yields: [])"#
        )
    };
    for (label, context, invalid_body) in [
        (
            "mandatory payment",
            EffectContext::Ability,
            discard_only("Controller", "false", "Some(Creature)"),
        ),
        (
            "SourceController chooser",
            EffectContext::Ability,
            discard_only("SourceController", "true", "Some(Creature)"),
        ),
        (
            "unfiltered discard",
            EffectContext::Ability,
            discard_only("Controller", "true", "None"),
        ),
        (
            "noncreature filter",
            EffectContext::Ability,
            discard_only("Controller", "true", "Some(Artifact)"),
        ),
        (
            "spell program",
            EffectContext::Spell,
            discard_only("Controller", "true", "Some(Creature)"),
        ),
    ] {
        assert!(
            compile(context, &invalid_body).is_err(),
            "Program filtered-discard admission rejects {label}"
        );
    }
}

#[test]
fn program_discard_each_opponent_exact_one_binds_a_combined_unbounded_cohort() {
    use crate::primitives::{CompiledProgram, EffectContext};

    let compile = |context, quantity: &str| {
        let body = format!(
            r#"(steps: [(id: "discard", bind: ["cohort"], kind: Perform(Discard(
                who: EachOpponent, quantity: {quantity},
            )))], yields: [])"#
        );
        let program: crate::CardProgram =
            ron::from_str(&format!("(body: {body})")).expect("program fixture parses");
        let owner = crate::ProgramOwnerId::ability(crate::AbilityDefinitionId {
            card_id: "player_set_discard_fixture".into(),
            face_id: crate::CardFaceId::new("player_set_discard_fixture").unwrap(),
            ability_path: vec![crate::AbilityId::new("triggered_01").unwrap()],
        });
        CompiledProgram::compile(owner, context, &program)
    };

    let compiled = compile(EffectContext::Ability, "Exact(1)")
        .expect("a fixed one-card discard per opponent is admitted in an ability program");
    let [discard] = compiled.body.steps.as_slice() else {
        panic!("the player-set discard remains one typed operation")
    };
    let result = compiled
        .slot(discard.outputs[0])
        .expect("the discard result is bound");
    assert_eq!(
        result.result_type,
        crate::primitives::ResultType::DiscardResult {
            who: crate::primitives::PlayerRecipient::EachOpponent,
            cardinality: crate::primitives::Cardinality { min: 0, max: None },
            optional_payment: false,
        },
        "the single result contains every opponent's selected card, so it cannot claim max 1"
    );

    for unsupported_quantity in ["Exact(2)", "OptionalExact(1)"] {
        assert!(
            compile(EffectContext::Ability, unsupported_quantity).is_err(),
            "only EachOpponent Exact(1) is admitted, not {unsupported_quantity}"
        );
    }
    assert!(
        compile(EffectContext::Spell, "Exact(1)").is_err(),
        "the narrow EachOpponent contract does not widen spell programs"
    );
}

#[test]
fn program_discard_each_player_exact_one_feeds_controller_discard_count() {
    use crate::primitives::{CompiledProgram, EffectContext};

    let compile =
        |context, quantity: &str, players: &str, card_type: &str, min: &str, max: &str| {
            let body = format!(
                r#"(steps: [
                (id: "discard_each_player", bind: ["discarded"], input: None,
                 kind: Perform(Discard(who: EachPlayer, quantity: {quantity}))),
                (id: "draw_if_controller_discarded", bind: [], input: None,
                 kind: IfResult(
                    condition: DiscardCount(
                        result: "discarded", players: {players}, card_type: {card_type},
                        min: {min}, max: {max},
                    ),
                    yes: (steps: [(id: "draw", bind: [], input: None,
                        kind: Perform(Draw(who: Controller, count: 1)))], yields: []),
                    no: (steps: [], yields: []),
                 )),
            ], yields: [])"#
            );
            let program: crate::CardProgram =
                ron::from_str(&format!("(body: {body})")).expect("program fixture parses");
            let owner = match context {
                EffectContext::Spell => crate::ProgramOwnerId::face(
                    "fanatic_discard_fixture",
                    crate::CardFaceId::new("fanatic_discard_fixture").unwrap(),
                ),
                EffectContext::Ability => {
                    crate::ProgramOwnerId::ability(crate::AbilityDefinitionId {
                        card_id: "fanatic_discard_fixture".into(),
                        face_id: crate::CardFaceId::new("fanatic_discard_fixture").unwrap(),
                        ability_path: vec![crate::AbilityId::new("triggered_01").unwrap()],
                    })
                }
            };
            CompiledProgram::compile(owner, context, &program)
        };

    let compiled = compile(
        EffectContext::Ability,
        "Exact(1)",
        "Controller",
        "None",
        "Some(1)",
        "None",
    )
    .expect("an EachPlayer Exact(1) discard cohort feeds the controller's discard count");

    assert_eq!(compiled.context, EffectContext::Ability);
    let [discard, draw_check] = compiled.body.steps.as_slice() else {
        panic!("the Program discards once before checking its result")
    };
    assert_eq!(discard.id.as_str(), "discard_each_player");
    assert_eq!(discard.outputs.len(), 1);
    let result = compiled
        .slot(discard.outputs[0])
        .expect("the player-set discard result is bound");
    assert_eq!(
        result.result_type,
        crate::primitives::ResultType::DiscardResult {
            who: crate::primitives::PlayerRecipient::EachPlayer,
            cardinality: crate::primitives::Cardinality { min: 0, max: None },
            optional_payment: false,
        },
        "one aggregate result can include every player's choice"
    );

    let crate::primitives::CompiledInstructionKind::If { condition, yes, no } = &draw_check.kind
    else {
        panic!("the controller's discard count guards the draw")
    };
    let crate::primitives::CompiledProgramCondition::Result(
        crate::primitives::CompiledProgramResultCondition::DiscardCount {
            result: checked_result,
            players,
            card_type,
            min,
            max,
        },
    ) = condition.as_ref()
    else {
        panic!("the conditional reads the typed discard result")
    };
    assert_eq!(*checked_result, discard.outputs[0]);
    assert_eq!(*players, crate::primitives::RelativePlayerSet::Controller);
    assert_eq!(*card_type, None);
    assert_eq!(*min, Some(1));
    assert_eq!(*max, None);
    let [draw] = yes.steps.as_slice() else {
        panic!("the positive controller count draws one card")
    };
    assert!(draw.input.is_none() && draw.outputs.is_empty());
    let crate::primitives::CompiledInstructionKind::Perform(effect) = &draw.kind else {
        panic!("the positive controller count draws one card")
    };
    assert_eq!(
        **effect,
        crate::primitives::SpellEffectKind::Draw {
            who: crate::primitives::PlayerRecipient::Controller,
            count: crate::primitives::Amount::Fixed(1),
        },
    );
    assert!(no.steps.is_empty() && no.yields.is_empty());

    for quantity in ["Exact(2)", "OptionalExact(1)"] {
        assert!(
            compile(
                EffectContext::Ability,
                quantity,
                "Controller",
                "None",
                "Some(1)",
                "None",
            )
            .is_err(),
            "only EachPlayer Exact(1) is admitted, not {quantity}"
        );
    }
    let spell_error = compile(
        EffectContext::Spell,
        "Exact(1)",
        "Controller",
        "None",
        "Some(1)",
        "None",
    )
    .expect_err("EachPlayer Exact(1) discard remains ability-only");
    assert!(
        spell_error.contains("Discard recipient supports only the resolving spell controller"),
        "the valid face owner reaches spell-recipient validation, got: {spell_error}"
    );
    for (players, card_type, min, max, unsupported_shape) in [
        (
            "Opponents",
            "None",
            "Some(1)",
            "None",
            "other player cohort",
        ),
        (
            "Controller",
            "Some(Nonland)",
            "Some(1)",
            "None",
            "card filter",
        ),
        ("Controller", "None", "Some(2)", "None", "larger minimum"),
        (
            "Controller",
            "None",
            "Some(1)",
            "Some(1)",
            "bounded maximum",
        ),
    ] {
        assert!(
            compile(
                EffectContext::Ability,
                "Exact(1)",
                players,
                card_type,
                min,
                max,
            )
            .is_err(),
            "the EachPlayer result projection rejects a {unsupported_shape}"
        );
    }
}

#[test]
fn endure_program_constituent_contracts_fail_independently_before_admission() {
    use crate::primitives::{CompiledProgram, EffectContext};

    let compile_ability = |card_id: &str, body: &str| {
        let program: crate::CardProgram =
            ron::from_str(&format!("(body: {body})")).expect("program fixture parses");
        let owner = crate::ProgramOwnerId::ability(crate::AbilityDefinitionId {
            card_id: card_id.into(),
            face_id: crate::CardFaceId::new(card_id).unwrap(),
            ability_path: vec![crate::AbilityId::new("triggered_01").unwrap()],
        });
        CompiledProgram::compile(owner, EffectContext::Ability, &program)
    };

    let shapes = [
        (
            "endure_source_counters",
            "positive fixed source +1/+1 counters",
            r#"(steps: [(id: "put", kind: Perform(PutCounters(
                counter: PlusOnePlusOne, count: 1, subject: Source,
            )))], yields: [])"#,
        ),
        (
            "endure_source_controller_tokens",
            "ordinary source-controller token creation",
            r#"(steps: [(id: "token", kind: Perform(CreateTokens(
                token: "spirit_w_1_1", count: 1, who: SourceController,
                tapped: false, sacrifice_timing: None,
            )))], yields: [])"#,
        ),
        (
            "endure_effects_applicable_counter_guard",
            "exact single-counter EffectsApplicable branch",
            r#"(steps: [(id: "choose", kind: Choose(
                choice: "endure", chooser: Controller, optional: false,
                branches: [(branch_id: "counters", presentation: Fallback, cost: None,
                    requirement: EffectsApplicable,
                    body: (steps: [(id: "put", kind: Perform(PutCounters(
                        counter: PlusOnePlusOne, count: 1, subject: Source,
                    )))], yields: []))],
                otherwise: (steps: [], yields: []),
            ))], yields: [])"#,
        ),
    ];
    let rejected = shapes
        .iter()
        .filter_map(|(card_id, label, body)| {
            compile_ability(card_id, body)
                .err()
                .map(|error| format!("{label}: {error}"))
        })
        .collect::<Vec<_>>();
    assert!(
        rejected.is_empty(),
        "each reviewed constituent needs its own Ability-program admission; current rejections: {}",
        rejected.join("; ")
    );
}

#[test]
fn currency_discard_to_exile_program_admits_only_its_exact_linked_exile_choice() {
    use crate::primitives::{
        ChooseDeparturePolicy, CompiledChooseRequirement, CompiledInstructionKind, CompiledProgram,
        EffectContext, PlayerRecipient, ProgramRequiredInput, SpellEffectKind,
    };

    let body = r#"(steps: [(id: "choose_to_exile", kind: Choose(
        choice: "may_exile", chooser: Controller, optional: true,
        branches: [(branch_id: "exile", presentation: Fallback, cost: None,
            requirement: EffectsApplicable, body: (steps: [
                (id: "exile_discarded", kind: Perform(Exile(
                    subject: TriggerObject, linked_exile_id: Some("exiled_cards"),
                )))
            ], yields: []))],
        otherwise: (steps: [], yields: []),
    ))], yields: [])"#;
    let compile = |body: &str, context| {
        let program: crate::CardProgram =
            ron::from_str(&format!("(body: {body})")).expect("Currency Program fixture parses");
        let owner = crate::ProgramOwnerId::ability(crate::AbilityDefinitionId {
            card_id: "currency_converter".into(),
            face_id: crate::CardFaceId::new("currency_converter").unwrap(),
            ability_path: vec![crate::AbilityId::new("discard_to_exile").unwrap()],
        });
        CompiledProgram::compile(owner, context, &program)
    };

    let compiled = compile(body, EffectContext::Ability)
        .expect("the exact Currency discard_to_exile Program shape should compile");
    assert_eq!(
        compiled.required_inputs,
        vec![ProgramRequiredInput::TriggerLinkedExileObject]
    );
    let [choose] = compiled.body.steps.as_slice() else {
        panic!("the Currency trigger contains one optional linked-Exile choice")
    };
    let CompiledInstructionKind::Choose {
        chooser,
        optional,
        departure_policy,
        branches,
        otherwise,
        ..
    } = &choose.kind
    else {
        panic!("Currency's choice is a compiled Choose")
    };
    assert_eq!(*chooser, PlayerRecipient::Controller);
    assert!(*optional);
    assert_eq!(
        *departure_policy,
        ChooseDeparturePolicy::DelegateNonCostChoice
    );
    assert!(otherwise.steps.is_empty());
    assert!(otherwise.yields.is_empty());
    let [branch] = branches.as_slice() else {
        panic!("Currency's optional choice has one branch")
    };
    assert!(matches!(
        branch.requirement,
        CompiledChooseRequirement::TriggerObjectCanBeLinkedExiled
    ));
    assert!(matches!(
        branch.body.steps.as_slice(),
        [step]
            if step.outputs.is_empty()
                && matches!(
                    &step.kind,
                    CompiledInstructionKind::Perform(effect)
                        if matches!(
                            effect.as_ref(),
                            SpellEffectKind::Exile {
                                subject: crate::primitives::EffectSubject::TriggerObject,
                                linked_exile_id: Some(link),
                            } if link.as_str() == "exiled_cards"
                        )
                )
    ));

    for (label, invalid_body) in [
        (
            "wrong chooser",
            body.replace("chooser: Controller", "chooser: SourceController"),
        ),
        (
            "mandatory choice",
            body.replace("optional: true", "optional: false"),
        ),
        (
            "paid branch",
            body.replace("cost: None", "cost: Mana(\"{1}\")"),
        ),
        (
            "unlinked exile",
            body.replace("linked_exile_id: Some(\"exiled_cards\")", "linked_exile_id: None"),
        ),
        (
            "different subject",
            body.replace("subject: TriggerObject", "subject: Source"),
        ),
        (
            "bound result",
            body.replace(
                "id: \"exile_discarded\", kind: Perform(Exile(",
                "id: \"exile_discarded\", bind: [\"exiled\"], kind: Perform(Exile(",
            ),
        ),
        (
            "input-bound leaf",
            body.replace(
                "id: \"exile_discarded\", kind: Perform(Exile(",
                "id: \"exile_discarded\", input: Some(\"prior\"), kind: Perform(Exile(",
            ),
        ),
        (
            "yielded branch result",
            body.replace(
                "            ], yields: []))],",
                "            ], yields: [\"exiled\"]))],",
            ),
        ),
        (
            "extra branch operation",
            body.replace(
                "(id: \"exile_discarded\", kind: Perform(Exile(\n                    subject: TriggerObject, linked_exile_id: Some(\"exiled_cards\"),\n                )))",
                "(id: \"exile_discarded\", kind: Perform(Exile(\n                    subject: TriggerObject, linked_exile_id: Some(\"exiled_cards\"),\n                ))),\n                (id: \"extra\", kind: Perform(GainLife(amount: 1)))",
            ),
        ),
        (
            "nonempty Otherwise",
            body.replace(
                "otherwise: (steps: [], yields: [])",
                "otherwise: (steps: [(id: \"extra\", kind: Perform(GainLife(amount: 1)))], yields: [])",
            ),
        ),
        (
            "multiple branches",
            body.replace(
                "branches: [(branch_id: \"exile\", presentation: Fallback, cost: None,\n            requirement: EffectsApplicable, body: (steps: [\n                (id: \"exile_discarded\", kind: Perform(Exile(\n                    subject: TriggerObject, linked_exile_id: Some(\"exiled_cards\"),\n                )))\n            ], yields: []))],",
                "branches: [(branch_id: \"exile\", presentation: Fallback, cost: None,\n            requirement: EffectsApplicable, body: (steps: [\n                (id: \"exile_discarded\", kind: Perform(Exile(\n                    subject: TriggerObject, linked_exile_id: Some(\"exiled_cards\"),\n                )))\n            ], yields: [])),\n            (branch_id: \"empty\", presentation: Fallback, cost: None, requirement: Always, body: (steps: [], yields: []))],",
            ),
        ),
    ] {
        assert_ne!(invalid_body, body, "{label} fixture must alter the valid source");
        assert!(
            compile(&invalid_body, EffectContext::Ability).is_err(),
            "accepted invalid Currency EffectsApplicable profile: {label}"
        );
    }
    assert!(
        compile(body, EffectContext::Spell).is_err(),
        "the linked-Exile applicability guard remains ability-only"
    );
}

#[test]
fn mimic_vat_imprint_program_admits_its_exact_linked_exile_receipt_flow() {
    use crate::primitives::{
        ChooseDeparturePolicy, CompiledChooseRequirement, CompiledInstructionKind, CompiledProgram,
        EffectContext, ProgramRequiredInput, ResultType,
    };

    const BODY: &str = r#"(steps: [(id: "choose_imprint", kind: Choose(
        choice: "imprint_creature", chooser: Controller, optional: true,
        branches: [(branch_id: "imprint", presentation: Fallback, cost: None,
            requirement: EffectsApplicable, body: (steps: [
                (id: "exile_event", bind: ["imprint_receipt"], kind: Perform(Exile(
                    subject: TriggerObject, linked_exile_id: Some("imprinted_card"),
                ))),
                (id: "return_previous", input: Some("imprint_receipt"),
                    kind: Perform(ReturnOtherLinkedExiledCards(
                        producer_ability_id: "imprint", linked_exile_id: "imprinted_card",
                    )))
            ], yields: []))],
        otherwise: (steps: [], yields: []),
    ))], yields: [])"#;
    let program: crate::CardProgram =
        ron::from_str(&format!("(body: {BODY})")).expect("Mimic Vat Program fixture parses");
    let owner = crate::ProgramOwnerId::ability(crate::AbilityDefinitionId {
        card_id: "mimic_vat".into(),
        face_id: crate::CardFaceId::new("mimic_vat").unwrap(),
        ability_path: vec![crate::AbilityId::new("imprint").unwrap()],
    });

    let compile = |owner| CompiledProgram::compile(owner, EffectContext::Ability, &program);
    let compiled = compile(owner.clone()).unwrap_or_else(|error| {
        panic!("Mimic Vat's exact linked-Exile receipt flow was rejected: {error}")
    });
    assert_eq!(
        compiled.required_inputs,
        vec![ProgramRequiredInput::TriggerLinkedExileObject]
    );
    let [choose] = compiled.body.steps.as_slice() else {
        panic!("Mimic Vat's imprint Program has one optional choice");
    };
    let CompiledInstructionKind::Choose {
        chooser,
        optional,
        departure_policy,
        branches,
        otherwise,
        ..
    } = &choose.kind
    else {
        panic!("Mimic Vat's imprint Program compiles its optional choice");
    };
    assert_eq!(*chooser, crate::primitives::PlayerRecipient::Controller);
    assert!(*optional);
    assert_eq!(
        *departure_policy,
        ChooseDeparturePolicy::OptionalCostFallsThrough
    );
    assert!(otherwise.steps.is_empty());
    assert!(otherwise.yields.is_empty());
    let [branch] = branches.as_slice() else {
        panic!("Mimic Vat has one linked-Exile branch");
    };
    assert_eq!(
        branch.requirement,
        CompiledChooseRequirement::TriggerObjectCanBeLinkedExiled
    );
    let [exile, return_other] = branch.body.steps.as_slice() else {
        panic!("the accepted branch contains the exact Exile and return pair");
    };
    let [receipt_slot] = exile.outputs.as_slice() else {
        panic!("the Mimic Vat Exile binds its receipt");
    };
    assert_eq!(
        compiled.slot(*receipt_slot).unwrap().result_type,
        ResultType::TriggerObjectLinkedExileReceipt {
            producer_ability_id: crate::AbilityId::new("imprint").unwrap(),
            linked_exile_id: crate::AbilityLinkId::new("imprinted_card").unwrap(),
            cardinality: crate::primitives::Cardinality {
                min: 0,
                max: Some(1),
            },
        }
    );
    assert_eq!(return_other.input, Some(*receipt_slot));

    for (label, invalid_body) in [
        (
            "missing receipt input",
            BODY.replace("input: Some(\"imprint_receipt\")", "input: None"),
        ),
        (
            "crossed linked-exile owner",
            BODY.replace("producer_ability_id: \"imprint\"", "producer_ability_id: \"other\""),
        ),
        (
            "crossed linked-exile ID",
            BODY.replace(
                "linked_exile_id: \"imprinted_card\"",
                "linked_exile_id: \"other_link\"",
            ),
        ),
        (
            "non-TriggerObject producer",
            BODY.replace("subject: TriggerObject", "subject: Source"),
        ),
        (
            "intervening instruction",
            BODY.replace(
                "(id: \"return_previous\", input:",
                "(id: \"intervening\", kind: Perform(GainLife(amount: 1))),\n                (id: \"return_previous\", input:",
            ),
        ),
        (
            "branch result escape",
            BODY.replace("], yields: []))],", "], yields: [\"imprint_receipt\"]))],"),
        ),
        (
            "extra receipt use",
            BODY.replace(
                "                    )))\n            ], yields: []))],",
                "                    ))),\n                (id: \"return_again\", input: Some(\"imprint_receipt\"),\n                    kind: Perform(ReturnOtherLinkedExiledCards(\n                        producer_ability_id: \"imprint\", linked_exile_id: \"imprinted_card\",\n                    )))\n            ], yields: []))],",
            ),
        ),
        (
            "nonempty Otherwise",
            BODY.replace(
                "otherwise: (steps: [], yields: [])",
                "otherwise: (steps: [(id: \"other\", kind: Perform(GainLife(amount: 1)))], yields: [])",
            ),
        ),
    ] {
        assert_ne!(invalid_body, BODY, "{label} fixture changes the valid profile");
        let invalid: crate::CardProgram = ron::from_str(&format!("(body: {invalid_body})"))
            .unwrap_or_else(|error| panic!("{label} fixture parses: {error}"));
        assert!(
            compile(owner.clone()).is_ok(),
            "the base Mimic Vat profile remains valid for {label}"
        );
        let invalid_compiled = crate::primitives::CompiledProgram::compile(
            owner.clone(),
            EffectContext::Ability,
            &invalid,
        );
        assert!(
            invalid_compiled.is_err(),
            "unsupported Mimic Vat receipt profile was accepted: {label}"
        );
    }
    let other_owner = crate::ProgramOwnerId::ability(crate::AbilityDefinitionId {
        card_id: "other_card".into(),
        face_id: crate::CardFaceId::new("other_card").unwrap(),
        ability_path: vec![crate::AbilityId::new("imprint").unwrap()],
    });
    assert!(
        compile(other_owner).is_err(),
        "the specialized receipt cannot be claimed by another program owner"
    );
}

#[test]
fn linked_exile_program_input_requires_an_exact_event_object_source() {
    const BODY: &str = r#"(steps: [(id: "choose_to_exile", kind: Choose(
        choice: "may_exile", chooser: Controller, optional: true,
        branches: [(branch_id: "exile", presentation: Fallback, cost: None,
            requirement: EffectsApplicable, body: (steps: [
                (id: "exile_discarded", kind: Perform(Exile(
                    subject: TriggerObject, linked_exile_id: Some("exiled_cards"),
                )))
            ], yields: []))],
        otherwise: (steps: [], yields: []),
    ))], yields: [])"#;
    let make_card = |id: &str, trigger: &str| {
        format!(
            r#"(id: "{id}", name: "Linked Exile Program Input Fixture",
                face_id: "{id}", types: ["Artifact"],
                triggered_abilities: [(
                    ability_id: "triggered_01", presentation: Fallback,
                    trigger: {trigger}, effect: [],
                    program: Some((body: {BODY}, target_bindings: [])),
                    modal: None, targeting: None, may: false, intervening_if: None,
                triggers_only_once: false, max_triggers_per_turn: None,
                )], activated_abilities: [(
                    ability_id: "consume_link", presentation: Fallback, costs: [Tap],
                    effect: [ChooseLinkedExiledCard(
                        producer_ability_id: "triggered_01",
                        linked_exile_id: "exiled_cards",
                        filter: (any_of: Some([
                            (card_type: Some(Creature)),
                            (card_type: Some(Noncreature)),
                        ])),
                        destination: OwnerGraveyard, optional: false,
                    )],
                )])"#
        )
    };

    let discard = make_card(
        "linked_exile_program_discard_input",
        "WheneverPlayerDiscardsCard(player: Controller)",
    );
    crate::CardRegistry::from_chunks_and_tokens(&[&discard], &[])
        .expect("the discard trigger supplies its exact event object");

    let unrelated = make_card("linked_exile_program_no_input", "WheneverYouProliferate");
    let error = crate::CardRegistry::from_chunks_and_tokens(&[&unrelated], &[])
        .expect_err("an unrelated trigger cannot use the linked-Exile event object")
        .to_string();
    assert!(
        error.contains(
            "requires TriggerLinkedExileObject, but its trigger does not supply that input"
        ),
        "unexpected linked-Exile input error: {error}"
    );
}

#[test]
fn endure_program_admits_only_the_source_counter_choice_composition() {
    use crate::primitives::{
        CompiledInstructionKind, CompiledProgram, EffectContext, PlayerRecipient, SpellEffectKind,
    };

    let ability_owner = crate::ProgramOwnerId::ability(crate::AbilityDefinitionId {
        card_id: "endure_program_fixture".into(),
        face_id: crate::CardFaceId::new("endure_program_fixture").unwrap(),
        ability_path: vec![crate::AbilityId::new("triggered_01").unwrap()],
    });
    let ability_program = |body: &str| {
        let program: crate::CardProgram =
            ron::from_str(&format!("(body: {body})")).expect("program fixture parses");
        CompiledProgram::compile(ability_owner.clone(), EffectContext::Ability, &program)
    };
    let source = r#"(steps: [(id: "choose", kind: Choose(
        choice: "endure", chooser: SourceController, optional: false,
        branches: [
            (branch_id: "counters", presentation: Fallback, cost: None,
                requirement: EffectsApplicable,
                body: (steps: [(id: "counters", kind: Perform(PutCounters(
                    counter: PlusOnePlusOne, count: 1, subject: Source,
                )))], yields: [])),
            (branch_id: "spirit", presentation: Fallback, cost: None,
                requirement: Always,
                body: (steps: [(id: "spirit", kind: Perform(CreateTokens(
                    token: "spirit_w_1_1", count: 1, who: SourceController,
                    tapped: false, sacrifice_timing: None,
                )))], yields: [])),
        ], otherwise: (steps: [], yields: []),
    ))], yields: [])"#;
    let compiled = ability_program(source).expect(
        "the reviewed ability owner accepts source-counter eligibility and source-owned tokens",
    );
    let [choose] = compiled.body.steps.as_slice() else {
        panic!("expected one authored Endure choice");
    };
    let CompiledInstructionKind::Choose {
        chooser, branches, ..
    } = &choose.kind
    else {
        panic!("Endure is represented by the existing Choose instruction");
    };
    assert_eq!(*chooser, PlayerRecipient::SourceController);
    let [counters, spirit] = branches.as_slice() else {
        panic!("expected the two Endure alternatives");
    };
    assert!(matches!(
        counters.body.steps.as_slice(),
        [step]
            if matches!(
                &step.kind,
                CompiledInstructionKind::Perform(effect)
                    if matches!(
                        effect.as_ref(),
                        SpellEffectKind::PutCounters {
                            counter: crate::primitives::CounterKind::PlusOnePlusOne,
                            count: crate::primitives::Amount::Fixed(1),
                            subject: crate::primitives::EffectSubject::Source,
                        }
                    )
            ) && step.outputs.is_empty() && step.input.is_none()
    ));
    assert!(matches!(
        &spirit.body.steps[0].kind,
        CompiledInstructionKind::Perform(effect)
            if matches!(
                effect.as_ref(),
                SpellEffectKind::CreateTokens {
                    who: PlayerRecipient::SourceController,
                    count: crate::primitives::Amount::Fixed(1),
                    sacrifice_timing: None,
                    ..
                }
            )
    ));

    for (label, invalid_body) in [
        (
            "extra leaf",
            r#"(steps: [
                (id: "counters", kind: Perform(PutCounters(counter: PlusOnePlusOne, count: 1, subject: Source))),
                (id: "extra", kind: Perform(GainLife(amount: 1))),
            ], yields: [])"#,
        ),
        (
            "nested branch",
            r#"(steps: [(id: "nested", kind: If(
                condition: ControllerLibraryEmpty,
                yes: (steps: [(id: "counters", kind: Perform(PutCounters(counter: PlusOnePlusOne, count: 1, subject: Source)))], yields: []),
                no: (steps: [], yields: [])))], yields: [])"#,
        ),
        (
            "result binding",
            r#"(steps: [(id: "counters", bind: ["out"], kind: Perform(PutCounters(counter: PlusOnePlusOne, count: 1, subject: Source)))], yields: [])"#,
        ),
        (
            "zero amount",
            r#"(steps: [(id: "counters", kind: Perform(PutCounters(counter: PlusOnePlusOne, count: 0, subject: Source)))], yields: [])"#,
        ),
        (
            "dynamic amount",
            r#"(steps: [(id: "counters", kind: Perform(PutCounters(counter: PlusOnePlusOne, count: EventCount, subject: Source)))], yields: [])"#,
        ),
        (
            "unsupported EffectsApplicable counter kind",
            r#"(steps: [(id: "counters", kind: Perform(PutCounters(counter: Charge, count: 1, subject: Source)))], yields: [])"#,
        ),
        (
            "wrong subject",
            r#"(steps: [(id: "counters", kind: Perform(PutCounters(counter: PlusOnePlusOne, count: 1, subject: Chosen((kind: Creature)))))], yields: [])"#,
        ),
    ] {
        let body = format!(
            r#"(steps: [(id: "choose", kind: Choose(
                choice: "endure", chooser: SourceController, optional: false,
                branches: [(branch_id: "counters", presentation: Fallback, cost: None,
                    requirement: EffectsApplicable, body: {invalid_body})],
                otherwise: (steps: [], yields: []),
            ))], yields: [])"#
        );
        assert!(
            ability_program(&body).is_err(),
            "accepted invalid guard body: {label}"
        );
    }

    for (label, invalid_body) in [
        (
            "counter leaf input",
            r#"(steps: [(id: "counters", input: Some("prior"), kind: Perform(PutCounters(
                counter: PlusOnePlusOne, count: 1, subject: Source,
            )))], yields: [])"#,
        ),
        (
            "counter body yield",
            r#"(steps: [(id: "counters", kind: Perform(PutCounters(
                counter: PlusOnePlusOne, count: 1, subject: Source,
            )))], yields: ["counter_result"])"#,
        ),
    ] {
        let body = format!(
            r#"(steps: [(id: "choose", kind: Choose(
                choice: "endure", chooser: SourceController, optional: false,
                branches: [(branch_id: "counters", presentation: Fallback, cost: None,
                    requirement: EffectsApplicable, body: {invalid_body})],
                otherwise: (steps: [], yields: []),
            ))], yields: [])"#
        );
        let error = ability_program(&body).expect_err("the EffectsApplicable leaf shape is exact");
        assert!(
            error.contains("supports EffectsApplicable only for an ability branch containing one resultless fixed positive Source +1/+1 PutCounters instruction"),
            "{label}: {error}"
        );
    }

    let source_controller_token = r#"(steps: [(id: "token", kind: Perform(CreateTokens(
        token: "spirit_w_1_1", count: 1, who: SourceController,
        tapped: false, sacrifice_timing: None,
    )))], yields: [])"#;
    let token_program: crate::CardProgram =
        ron::from_str(&format!("(body: {source_controller_token})")).unwrap();
    let spell_error = CompiledProgram::compile(
        crate::ProgramOwnerId::face(
            "endure_spell_fixture",
            crate::CardFaceId::new("endure_spell_fixture").unwrap(),
        ),
        EffectContext::Spell,
        &token_program,
    )
    .expect_err("source-controller token creation stays unavailable to spell programs");
    assert!(spell_error.contains("SourceController"), "{spell_error}");
}

#[test]
fn replicating_ring_program_admits_source_night_threshold_remove_and_tokens() {
    use crate::primitives::{
        CompiledInstructionKind, CompiledProgram, CompiledProgramCondition, EffectContext,
        SpellEffectKind,
    };

    let owner = crate::ProgramOwnerId::ability(crate::AbilityDefinitionId {
        card_id: "replicating_ring".into(),
        face_id: crate::CardFaceId::new("replicating_ring").unwrap(),
        ability_path: vec![crate::AbilityId::new("triggered_01").unwrap()],
    });
    let ring_body = r#"(steps: [
        (id: "put_night", kind: Perform(PutCounters(
            counter: Night, count: 1, subject: Source,
        ))),
        (id: "threshold", kind: If(
            condition: SourceCounterCount(counter: Night, min: Some(8)),
            yes: (steps: [
                (id: "remove_night", kind: Perform(RemoveAllCounters(
                    counter: Night, subject: Source,
                ))),
                (id: "make_rings", kind: Perform(CreateTokens(
                    token: "replicated_ring", count: 8, who: Controller,
                    tapped: false, sacrifice_timing: None,
                ))),
            ], yields: []),
            no: (steps: [], yields: []),
        )),
    ], yields: [])"#;
    let authored: crate::CardProgram =
        ron::from_str(&format!("(body: {ring_body})")).expect("Ring body parses");
    let compiled = CompiledProgram::compile(owner, EffectContext::Ability, &authored)
        .expect("the exact Replicating Ring threshold program is admitted");

    let [put_night, threshold] = compiled.body.steps.as_slice() else {
        panic!("Ring increments Night before its source-threshold branch");
    };
    assert!(matches!(
        &put_night.kind,
        CompiledInstructionKind::Perform(effect)
            if matches!(effect.as_ref(), SpellEffectKind::PutCounters {
                counter: crate::primitives::CounterKind::Night,
                count: crate::primitives::Amount::Fixed(1),
                subject: crate::primitives::EffectSubject::Source,
            })
    ));
    let CompiledInstructionKind::If { condition, yes, no } = &threshold.kind else {
        panic!("Ring threshold is a ProgramIf");
    };
    assert!(matches!(
        condition.as_ref(),
        CompiledProgramCondition::Game(crate::primitives::GameCondition::SourceCounterCount {
            counter: crate::primitives::CounterKind::Night,
            min: Some(8),
            max: None,
        })
    ));
    assert!(no.steps.is_empty());
    assert!(matches!(
        yes.steps.as_slice(),
        [remove_night, make_rings]
            if matches!(
                &remove_night.kind,
                CompiledInstructionKind::Perform(effect)
                    if matches!(effect.as_ref(), SpellEffectKind::RemoveAllCounters {
                        counter: crate::primitives::CounterKind::Night,
                        subject: crate::primitives::EffectSubject::Source,
                    })
            ) && matches!(
                &make_rings.kind,
                CompiledInstructionKind::Perform(effect)
                    if matches!(effect.as_ref(), SpellEffectKind::CreateTokens {
                        token,
                        count: crate::primitives::Amount::Fixed(8),
                        who: crate::primitives::PlayerRecipient::Controller,
                        tapped: false,
                        sacrifice_timing: None,
                    } if token == "replicated_ring")
            )
    ));
}

#[test]
fn source_counter_program_support_is_typed_and_rejects_unsupported_contexts() {
    use crate::primitives::{CompiledProgram, EffectContext};

    let compile = |_label: &str, context, body: &str| {
        let card_id = "source_counter_fixture";
        let program: crate::CardProgram =
            ron::from_str(&format!("(body: {body})")).expect("program fixture parses");
        let owner = match context {
            EffectContext::Spell => {
                crate::ProgramOwnerId::face(card_id, crate::CardFaceId::new(card_id).unwrap())
            }
            EffectContext::Ability => crate::ProgramOwnerId::ability(crate::AbilityDefinitionId {
                card_id: card_id.into(),
                face_id: crate::CardFaceId::new(card_id).unwrap(),
                ability_path: vec![crate::AbilityId::new("triggered_01").unwrap()],
            }),
        };
        CompiledProgram::compile(owner, context, &program)
    };

    let charge_removal = r#"(steps: [(id: "remove_charge", kind: Perform(RemoveAllCounters(
        counter: Charge, subject: Source,
    )))], yields: [])"#;
    compile(
        "charge_removal_fixture",
        EffectContext::Ability,
        charge_removal,
    )
    .expect("the same source-bound removal operation supports Charge counters");

    let ring_program = r#"(steps: [
        (id: "put_night", kind: Perform(PutCounters(
            counter: Night, count: 1, subject: Source,
        ))),
        (id: "threshold", kind: If(
            condition: SourceCounterCount(counter: Night, min: Some(8)),
            yes: (steps: [(id: "remove_night", kind: Perform(RemoveAllCounters(
                counter: Night, subject: Source,
            )))], yields: []),
            no: (steps: [], yields: []),
        )),
    ], yields: [])"#;
    assert!(
        compile("ring_spell_fixture", EffectContext::Spell, ring_program).is_err(),
        "the source counter operation and condition require an ability owner"
    );

    let spell_threshold = r#"(steps: [(id: "threshold", kind: If(
        condition: SourceCounterCount(counter: Night, min: Some(8)),
        yes: (steps: [], yields: []), no: (steps: [], yields: []),
    ))], yields: [])"#;
    assert!(
        compile(
            "ring_condition_spell_fixture",
            EffectContext::Spell,
            spell_threshold
        )
        .is_err(),
        "SourceCounterCount is not a context-free spell predicate"
    );

    let nested_choose_if = r#"(steps: [(id: "choose", kind: Choose(
        choice: "mode", chooser: Controller, optional: false,
        branches: [(branch_id: "mode", presentation: Fallback,
            cost: None, requirement: Always,
            body: (steps: [(id: "if", kind: If(
                condition: SourceCounterCount(counter: Night, min: Some(8)),
                yes: (steps: [], yields: []), no: (steps: [], yields: []),
            ))], yields: []))],
        otherwise: (steps: [], yields: []),
    ))], yields: [])"#;
    compile(
        "nested_ring_condition_fixture",
        EffectContext::Ability,
        nested_choose_if,
    )
    .expect("SourceCounterCount remains available to a direct ProgramIf inside a Choose body");

    let choose_source_counter_requirement = r#"(steps: [(id: "choose", kind: Choose(
        choice: "mode", chooser: Controller, optional: false,
        branches: [(branch_id: "mode", presentation: Fallback,
            cost: None,
            requirement: GameCondition(SourceCounterCount(
                counter: Night, min: Some(8),
            )),
            body: (steps: [], yields: []))],
        otherwise: (steps: [], yields: []),
    ))], yields: [])"#;
    compile(
        "choose_source_counter_requirement_fixture",
        EffectContext::Ability,
        choose_source_counter_requirement,
    )
    .expect("bounded source-counter conditions are also available to ability Choose requirements");

    for (label, body) in [
        (
            "non-source counter placement",
            r#"(steps: [(id: "put", kind: Perform(PutCounters(
                counter: Night, count: 1, subject: Chosen((kind: Creature)),
            )))], yields: [])"#,
        ),
        (
            "zero counter placement",
            r#"(steps: [(id: "put", kind: Perform(PutCounters(
                counter: Night, count: 0, subject: Source,
            )))], yields: [])"#,
        ),
        (
            "dynamic counter placement",
            r#"(steps: [(id: "put", kind: Perform(PutCounters(
                counter: Night, count: X, subject: Source,
            )))], yields: [])"#,
        ),
        (
            "non-source all-counter removal",
            r#"(steps: [(id: "remove", kind: Perform(RemoveAllCounters(
                counter: Charge, subject: Chosen((kind: Creature)),
            )))], yields: [])"#,
        ),
        (
            "missing source-counter bound",
            r#"(steps: [(id: "if", kind: If(
                condition: SourceCounterCount(counter: Night),
                yes: (steps: [], yields: []), no: (steps: [], yields: []),
            ))], yields: [])"#,
        ),
        (
            "reversed source-counter bounds",
            r#"(steps: [(id: "if", kind: If(
                condition: SourceCounterCount(counter: Night, min: Some(8), max: Some(7)),
                yes: (steps: [], yields: []), no: (steps: [], yields: []),
            ))], yields: [])"#,
        ),
    ] {
        assert!(
            compile(label, EffectContext::Ability, body).is_err(),
            "unsupported source-counter form was admitted: {label}"
        );
    }
}

#[test]
fn source_counter_program_rejects_invalid_direct_counter_shapes_and_spell_context() {
    use crate::primitives::{CompiledProgram, EffectContext};

    let ability_owner = crate::ProgramOwnerId::ability(crate::AbilityDefinitionId {
        card_id: "source_counter_fixture".into(),
        face_id: crate::CardFaceId::new("source_counter_fixture").unwrap(),
        ability_path: vec![crate::AbilityId::new("triggered_01").unwrap()],
    });
    let compile_ability = |leaf: &str| {
        let program: crate::CardProgram = ron::from_str(&format!(
            r#"(body: (steps: [(id: "counter", kind: Perform({leaf}))], yields: []))"#
        ))
        .expect("direct counter fixture parses");
        CompiledProgram::compile(ability_owner.clone(), EffectContext::Ability, &program)
    };

    compile_ability("PutCounters(counter: Charge, count: 1, subject: Source)")
        .expect("ability programs admit positive fixed Source counters of any valid kind");

    for (label, leaf) in [
        (
            "zero amount",
            "PutCounters(counter: PlusOnePlusOne, count: 0, subject: Source)",
        ),
        (
            "dynamic amount",
            "PutCounters(counter: PlusOnePlusOne, count: EventCount, subject: Source)",
        ),
        (
            "wrong subject",
            "PutCounters(counter: PlusOnePlusOne, count: 1, subject: Chosen((kind: Creature)))",
        ),
    ] {
        let error = compile_ability(leaf)
            .expect_err("only fixed positive Source counter placements are admitted");
        assert!(
            error.contains("effect is not in the resolution-program operation whitelist"),
            "{label}: {error}"
        );
    }

    let spell_program: crate::CardProgram = ron::from_str(
        r#"(body: (steps: [(id: "counter", kind: Perform(PutCounters(
            counter: PlusOnePlusOne, count: 1, subject: Source,
        )))], yields: []))"#,
    )
    .expect("spell counter fixture parses");
    let spell_error = CompiledProgram::compile(
        crate::ProgramOwnerId::face(
            "source_counter_spell_fixture",
            crate::CardFaceId::new("source_counter_spell_fixture").unwrap(),
        ),
        EffectContext::Spell,
        &spell_program,
    )
    .expect_err("Source counter placement remains ability-only");
    assert!(
        spell_error.contains("effect is not in the resolution-program operation whitelist"),
        "{spell_error}"
    );
}

#[test]
fn program_choose_rejects_mixed_free_paid_and_x_cost_siblings() {
    let fixed_mana = program_fixture(
        r#"(steps: [(id: "choose", kind: Choose(
            choice: "choice_01", chooser: Controller, optional: false,
            branches: [(branch_id: "paid", presentation: Fallback,
                cost: Mana("{2}"), body: (steps: [], yields: []))],
            otherwise: (steps: [], yields: []),
        ))], yields: [])"#,
    );
    crate::CardRegistry::from_chunks_and_tokens(&[&fixed_mana], &[])
        .expect("a fixed non-X Mana branch is admitted");

    let homogeneous_paid = program_fixture(
        r#"(steps: [(id: "choose", kind: Choose(
            choice: "choice_01", chooser: Controller, optional: false,
            branches: [
                (branch_id: "mana", presentation: Fallback, cost: Mana("{2}"),
                    body: (steps: [], yields: [])),
                (branch_id: "waterbend", presentation: Fallback,
                    cost: Waterbend("{2}"), body: (steps: [], yields: [])),
            ],
            otherwise: (steps: [], yields: []),
        ))], yields: [])"#,
    );
    let registry = crate::CardRegistry::from_chunks_and_tokens(&[&homogeneous_paid], &[])
        .expect("Mana and Waterbend siblings are both admitted in an all-paid Choose");
    let owner = crate::ProgramOwnerId::face(
        "program_fixture",
        crate::CardFaceId::new("program_fixture").unwrap(),
    );
    let compiled = registry
        .compiled_program_for_owner(&owner)
        .expect("homogeneous paid Choose compiles");
    let crate::primitives::CompiledInstructionKind::Choose { branches, .. } =
        &compiled.body.steps[0].kind
    else {
        panic!("program fixture retains its Choose instruction")
    };
    assert_eq!(branches.len(), 2);
    assert!(matches!(
        branches[0].cost,
        crate::primitives::ResolutionCost::Mana(_)
    ));
    assert!(matches!(
        branches[1].cost,
        crate::primitives::ResolutionCost::Waterbend(_)
    ));

    let mixed = program_fixture(
        r#"(steps: [(id: "choose", kind: Choose(
            choice: "choice_01", chooser: Controller, optional: false,
            branches: [
                (branch_id: "free", presentation: Fallback, cost: None,
                    body: (steps: [], yields: [])),
                (branch_id: "paid", presentation: Fallback, cost: Mana("{1}"),
                    body: (steps: [], yields: [])),
            ],
            otherwise: (steps: [], yields: []),
        ))], yields: [])"#,
    );
    let mixed_error = invalid_program_reason(&mixed);
    assert!(
        mixed_error.contains("cannot mix free and paid sibling branches"),
        "{mixed_error}"
    );

    for cost in [r#"Mana("{X}")"#, r#"Waterbend("{X}")"#] {
        let definition = program_fixture(&format!(
            r#"(steps: [(id: "choose", kind: Choose(
                choice: "choice_01", chooser: Controller, optional: false,
                branches: [(branch_id: "paid", presentation: Fallback, cost: {cost},
                    body: (steps: [], yields: []))],
                otherwise: (steps: [], yields: []),
            ))], yields: [])"#
        ));
        let error = invalid_program_reason(&definition);
        assert!(
            error.contains("requires a nonempty fixed mana cost without X"),
            "{cost}: {error}"
        );
    }
}

#[test]
fn program_choose_enforces_program_and_choice_id_boundaries() {
    let one_choose = |instruction_id: &str, choice_id: &str, branch_id: &str| {
        format!(
            r#"(id: "{instruction_id}", bind: [], kind: Choose(
                choice: "{choice_id}", chooser: Controller, optional: false,
                branches: [(branch_id: "{branch_id}", presentation: Fallback,
                    cost: None, body: (steps: [], yields: []))],
                otherwise: (steps: [], yields: []),
            ))"#
        )
    };

    let duplicate_instruction = program_fixture(&format!(
        "(steps: [{}, {}], yields: [])",
        one_choose("same_instruction", "choice_one", "continue"),
        one_choose("same_instruction", "choice_two", "continue"),
    ));
    let error = invalid_program_reason(&duplicate_instruction);
    assert!(
        error.contains("duplicate or empty instruction id"),
        "{error}"
    );

    let duplicate_choice = program_fixture(&format!(
        "(steps: [{}, {}], yields: [])",
        one_choose("first", "same_choice", "continue"),
        one_choose("second", "same_choice", "continue"),
    ));
    let error = invalid_program_reason(&duplicate_choice);
    assert!(error.contains("duplicate Choose choice id"), "{error}");

    let duplicate_branch = program_fixture(
        r#"(steps: [(id: "choose", kind: Choose(
            choice: "choice_01", chooser: Controller, optional: false,
            branches: [
                (branch_id: "same_branch", presentation: Fallback, cost: None,
                    body: (steps: [], yields: [])),
                (branch_id: "same_branch", presentation: Fallback, cost: None,
                    body: (steps: [], yields: [])),
            ],
            otherwise: (steps: [], yields: []),
        ))], yields: [])"#,
    );
    let error = invalid_program_reason(&duplicate_branch);
    assert!(error.contains("has duplicate branch id"), "{error}");

    let malformed_choice = program_fixture(&format!(
        "(steps: [{}], yields: [])",
        one_choose("choose", "Bad-ID", "valid_branch"),
    ));
    let error = invalid_program_reason(&malformed_choice);
    assert!(error.contains("canonical snake_case"), "{error}");

    let malformed_branch = program_fixture(&format!(
        "(steps: [{}], yields: [])",
        one_choose("choose", "valid_choice", "Bad-ID"),
    ));
    let error = invalid_program_reason(&malformed_branch);
    assert!(error.contains("canonical snake_case"), "{error}");

    let empty_instruction = program_fixture(&format!(
        "(steps: [{}], yields: [])",
        one_choose("", "valid_choice", "valid_branch"),
    ));
    let error = invalid_program_reason(&empty_instruction);
    assert!(
        error.contains("duplicate or empty instruction id"),
        "{error}"
    );

    let repeated_branch_id_in_distinct_choices = program_fixture(&format!(
        "(steps: [{}, {}], yields: [])",
        one_choose("first", "choice_one", "continue"),
        one_choose("second", "choice_two", "continue"),
    ));
    crate::CardRegistry::from_chunks_and_tokens(&[&repeated_branch_id_in_distinct_choices], &[])
        .expect("branch IDs are local to each Choose; instruction and choice IDs are program-wide");
}

#[test]
fn program_choose_blocks_do_not_leak_sibling_or_branch_locals() {
    let sibling_leak = program_fixture(
        r#"(steps: [(id: "choose", kind: Choose(
            choice: "choice_01", chooser: Controller, optional: false,
            branches: [
                (branch_id: "produce", presentation: Fallback, cost: None,
                    body: (steps: [
                        (id: "pick_lands", bind: ["branch_lands"],
                            kind: Perform(ChoosePermanents(
                                chooser: Controller,
                                filter: (kind: AnyPermanent, permanent_types: [Land]),
                                min: 0, max: 3,
                            ))),
                    ], yields: [])),
                (branch_id: "consume", presentation: Fallback, cost: None,
                    body: (steps: [
                        (id: "reuse", input: Some("branch_lands"),
                            kind: Perform(UntapChosenPermanents)),
                    ], yields: [])),
            ],
            otherwise: (steps: [], yields: []),
        ))], yields: [])"#,
    );
    let error = invalid_program_reason(&sibling_leak);
    assert!(
        error.contains("input result 'branch_lands' is not in scope"),
        "{error}"
    );

    let otherwise_leak = program_fixture(
        r#"(steps: [(id: "choose", kind: Choose(
            choice: "choice_01", chooser: Controller, optional: false,
            branches: [(branch_id: "produce", presentation: Fallback, cost: None,
                body: (steps: [
                    (id: "pick_lands", bind: ["branch_lands"],
                        kind: Perform(ChoosePermanents(
                            chooser: Controller,
                            filter: (kind: AnyPermanent, permanent_types: [Land]),
                            min: 0, max: 3,
                        ))),
                ], yields: []))],
            otherwise: (steps: [], yields: ["branch_lands"]),
        ))], yields: [])"#,
    );
    let error = invalid_program_reason(&otherwise_leak);
    assert!(
        error.contains("block yields result 'branch_lands' outside its scope"),
        "{error}"
    );
}

#[test]
fn program_choose_joins_matching_typed_yields_from_every_exit() {
    let valid = program_fixture(
        r#"(steps: [
            (id: "choose", bind: ["lands"], kind: Choose(
                choice: "choose_lands", chooser: Controller, optional: true,
                branches: [
                    (branch_id: "first", presentation: Fallback, cost: None,
                        requirement: GameCondition(ControllerLibraryEmpty),
                        body: (steps: [
                            (id: "pick_first", bind: ["first_lands"],
                                kind: Perform(ChoosePermanents(
                                    chooser: Controller,
                                    filter: (kind: AnyPermanent, permanent_types: [Land]),
                                    min: 0, max: 3,
                                ))),
                        ], yields: ["first_lands"])),
                    (branch_id: "second", presentation: Fallback, cost: None,
                        body: (steps: [
                            (id: "pick_second", bind: ["second_lands"],
                                kind: Perform(ChoosePermanents(
                                    chooser: Controller,
                                    filter: (kind: AnyPermanent, permanent_types: [Land]),
                                    min: 0, max: 3,
                                ))),
                        ], yields: ["second_lands"])),
                ],
                otherwise: (steps: [
                    (id: "pick_otherwise", bind: ["otherwise_lands"],
                        kind: Perform(ChoosePermanents(
                            chooser: Controller,
                            filter: (kind: AnyPermanent, permanent_types: [Land]),
                            min: 0, max: 3,
                        ))),
                ], yields: ["otherwise_lands"]),
            )),
            (id: "untap", input: Some("lands"),
                kind: Perform(UntapChosenPermanents)),
        ], yields: [])"#,
    );
    let registry = crate::CardRegistry::from_chunks_and_tokens(&[&valid], &[])
        .expect("every Choose exit explicitly yields one compatible permanent selection");
    let owner = crate::ProgramOwnerId::face(
        "program_fixture",
        crate::CardFaceId::new("program_fixture").unwrap(),
    );
    let compiled = registry
        .compiled_program_for_owner(&owner)
        .expect("the joined Choose program is registered");
    assert_eq!(
        compiled.body.steps[0].outputs.as_slice(),
        &[compiled.body.steps[1]
            .input
            .expect("joined result feeds the parent")]
    );

    let missing_yield = program_fixture(
        r#"(steps: [(id: "choose", bind: ["joined"], kind: Choose(
            choice: "choose_lands", chooser: Controller, optional: true,
            branches: [(branch_id: "pick", presentation: Fallback, cost: None,
                body: (steps: [
                    (id: "pick_lands", bind: ["lands"], kind: Perform(ChoosePermanents(
                        chooser: Controller,
                        filter: (kind: AnyPermanent, permanent_types: [Land]),
                        min: 0, max: 3,
                    ))),
                ], yields: ["lands"]))],
            otherwise: (steps: [], yields: []),
        ))], yields: [])"#,
    );
    let error = invalid_program_reason(&missing_yield);
    assert!(error.contains("mismatched explicit yield arity"), "{error}");

    let mismatched_type = program_fixture(
        r#"(steps: [(id: "choose", bind: ["joined"], kind: Choose(
            choice: "choose_lands", chooser: Controller, optional: true,
            branches: [(branch_id: "pick", presentation: Fallback, cost: None,
                body: (steps: [
                    (id: "pick_lands", bind: ["lands"], kind: Perform(ChoosePermanents(
                        chooser: Controller,
                        filter: (kind: AnyPermanent, permanent_types: [Land]),
                        min: 0, max: 3,
                    ))),
                ], yields: ["lands"]))],
            otherwise: (steps: [
                (id: "draw", bind: ["drawn"], kind: Perform(Draw(count: 1))),
            ], yields: ["drawn"]),
        ))], yields: [])"#,
    );
    let error = invalid_program_reason(&mismatched_type);
    assert!(error.contains("mismatched explicit yield types"), "{error}");
}

#[test]
fn program_choose_target_bindings_cover_nested_branch_and_otherwise_leaves() {
    let definition = |card_id: &str, choice: &str, target_instruction: &str| {
        format!(
            r#"(id: "{card_id}", name: "Nested Choose Target Fixture",
                face_id: "{card_id}", types: ["Instant"],
                spell_program: Some((body: (steps: [{choice}], yields: []),
                    target_bindings: [(
                        instruction_id: "{target_instruction}",
                        role: Target,
                        target_group_index: 0,
                    )])),
                targeting: Some((groups: [(
                    min: 1, max: 1, prompt: "Choose target creature",
                    effect_indices: [],
                )])))"#
        )
    };
    let choices = [
        (
            "choose_branch_target_fixture",
            r#"(id: "choose", kind: Choose(
                choice: "choose_damage", chooser: Controller, optional: false,
                branches: [(branch_id: "damage", presentation: Fallback, cost: None,
                    body: (steps: [(id: "branch_damage", kind: Perform(DamageTarget(
                        amount: 3, target: (kind: Creature),
                    )))], yields: []))],
                otherwise: (steps: [], yields: []),
            ))"#,
            "branch_damage",
        ),
        (
            "choose_otherwise_target_fixture",
            r#"(id: "choose", kind: Choose(
                choice: "choose_noop", chooser: Controller, optional: false,
                branches: [(branch_id: "noop", presentation: Fallback, cost: None,
                    body: (steps: [], yields: []))],
                otherwise: (steps: [(id: "otherwise_damage", kind: Perform(DamageTarget(
                    amount: 3, target: (kind: Creature),
                )))], yields: []),
            ))"#,
            "otherwise_damage",
        ),
    ];

    for (card_id, choice, target_instruction) in choices {
        let definition = definition(card_id, choice, target_instruction);
        let registry = crate::CardRegistry::from_chunks_and_tokens(&[&definition], &[])
            .expect("nested target role resolves through the exact Choose arm");
        let owner = crate::ProgramOwnerId::face(card_id, crate::CardFaceId::new(card_id).unwrap());
        let compiled = registry
            .compiled_program_for_owner(&owner)
            .expect("nested target program is registered");
        assert_eq!(compiled.target_bindings.len(), 1);
        assert_eq!(compiled.target_bindings[0].instruction_address, 1);
        assert_eq!(compiled.target_bindings[0].target_group_index, 0);

        let wrong_role = definition.replace("role: Target", "role: Source");
        let error = crate::CardRegistry::from_chunks_and_tokens(&[&wrong_role], &[])
            .expect_err("nested target references retain their leaf's exact target role")
            .to_string();
        assert!(error.contains("does not expose target role"), "{error}");
    }
}

#[test]
fn choose_cast_cost_receipt_accepts_expected_unselected_and_rejects_ability_owner() {
    let program = choose_cast_cost_program("kicker_group", "kicker_option")
        .replace("expected_selected: true", "expected_selected: false");
    let definition = format!(
        r#"(id: "choose_unselected_receipt", name: "Choose Unselected Receipt",
            face_id: "choose_unselected_receipt", mana_cost: "{{0}}", types: ["Instant"],
            cast_cost_groups: [(group_id: "kicker_group", presentation: Fallback,
                min: 0, max: 1, options: [Mana(option_id: "kicker_option",
                    presentation: Fallback, kind: Kicker, cost: "{{1}}")])],
            spell_program: {program})"#
    );
    let registry = crate::CardRegistry::from_chunks_and_tokens(&[&definition], &[])
        .expect("the exact spell owner may test the captured unselected value");
    let owner = crate::ProgramOwnerId::face(
        "choose_unselected_receipt",
        crate::CardFaceId::new("choose_unselected_receipt").unwrap(),
    );
    let compiled = registry
        .compiled_program_for_owner(&owner)
        .expect("the receipt-bound spell program is registered");
    let crate::primitives::CompiledInstructionKind::Choose { branches, .. } =
        &compiled.body.steps[0].kind
    else {
        panic!("receipt requirement remains on its Choose branch")
    };
    assert!(matches!(
        &branches[0].requirement,
        crate::primitives::CompiledChooseRequirement::CastCostReceipt(condition)
            if condition.group_id.as_str() == "kicker_group"
                && condition.option_id.as_str() == "kicker_option"
                && !condition.expected_selected
    ));

    let body: super::AuthoredBlock = ron::from_str(
        r#"(steps: [(id: "choose", kind: Choose(
            choice: "ability_receipt", chooser: Controller, optional: false,
            branches: [(branch_id: "selected", presentation: Fallback, cost: None,
                requirement: CastCostReceipt((group_id: "kicker_group",
                    option_id: "kicker_option", expected_selected: false)),
                body: (steps: [], yields: []))],
            otherwise: (steps: [], yields: []),
        ))], yields: [])"#,
    )
    .expect("ability-owner receipt fixture parses");
    let program = super::CardProgram {
        body,
        target_bindings: Vec::new(),
    };
    let owner = crate::ProgramOwnerId::ability(crate::AbilityDefinitionId {
        card_id: "ability_receipt_fixture".into(),
        face_id: crate::CardFaceId::new("ability_receipt_fixture").unwrap(),
        ability_path: vec![crate::AbilityId::new("triggered_01").unwrap()],
    });
    let error = super::CompiledProgram::compile(owner, super::EffectContext::Ability, &program)
        .expect_err("a triggered ability cannot read a spell cast-cost receipt");
    assert!(
        error.contains("cast-cost requirements require a spell program owner"),
        "{error}"
    );
}

#[test]
fn lavabrink_program_admits_affected_choice_and_recursive_source_conditions() {
    use crate::primitives::{
        CompiledChooseRequirement, CompiledInstructionKind, GameCondition, PlayerRecipient,
        ProgramRequiredInput, SpellEffectKind, TargetKind,
    };

    const BODY: &str = r#"(steps: [
        (id: "upkeep_choice", kind: Choose(
            choice: "doom_counter_choice", chooser: AffectedPlayer, optional: true,
            branches: [
                (branch_id: "add_doom", presentation: Fallback, cost: None,
                    requirement: GameCondition(ObjectMatches(
                        object: Source, filter: (kind: AnyPermanent),
                    )),
                    body: (steps: [(id: "add_doom_counter", kind: Perform(PutCounters(
                        counter: Doom, count: 1, subject: Source,
                    )))], yields: [])),
                (branch_id: "remove_doom", presentation: Fallback, cost: None,
                    requirement: GameCondition(AllOf([
                        ObjectMatches(object: Source, filter: (kind: AnyPermanent)),
                        SourceCounterCount(counter: Doom, min: Some(1)),
                    ])),
                    body: (steps: [(id: "remove_doom_counter", kind: Perform(RemoveCounters(
                        counter: Doom, count: 1, subject: Source,
                    )))], yields: [])),
            ],
            otherwise: (steps: [], yields: []),
        )),
        (id: "threshold", kind: If(
            condition: AllOf([
                ObjectMatches(object: Source, filter: (kind: AnyPermanent)),
                AnyOf([
                    SourceCounterCount(counter: Doom, min: Some(3)),
                    SourceCounterCount(counter: Doom, min: Some(4)),
                ]),
            ]),
            yes: (steps: [
                (id: "sacrifice_source", bind: ["sacrificed"], kind: Perform(
                    Sacrifice(subject: Source),
                )),
                (id: "create_reflexive_damage", input: Some("sacrificed"), kind: Perform(
                    CreateReflexiveTrigger(
                        when: Some(Sacrificed(object: Source)),
                        ability: (
                            ability_id: "reflexive_damage", presentation: Fallback,
                            effect: [DamageAll(amount: 6, players: All,
                                kind: (kind: Creature))],
                            targeting: None, intervening_if: None,
                        ),
                    ),
                )),
            ], yields: []),
            no: (steps: [], yields: []),
        )),
    ], yields: [])"#;

    let _: crate::CardProgram = ron::from_str(&format!("(body: {BODY})"))
        .expect("the complete Floodgates Program fixture parses before admission");
    let definition = |trigger: &str, body: &str| {
        format!(
            r#"(id: "lavabrink_program_admission_fixture",
                name: "Lavabrink Program Admission Fixture",
                face_id: "lavabrink_program_admission_fixture", types: ["Artifact"],
                triggered_abilities: [(
                    ability_id: "triggered_01", presentation: Fallback,
                    trigger: {trigger}, effect: [],
                    program: Some((body: {body})), modal: None, targeting: None,
                    may: false, intervening_if: None, triggers_only_once: false,
                    max_triggers_per_turn: None,
                )])"#
        )
    };
    let upkeep_definition = definition("AtBeginningOfUpkeep(player: AnyPlayer)", BODY);
    let registry = crate::CardRegistry::from_chunks_and_tokens(&[&upkeep_definition], &[])
        .expect("an upkeep trigger supplies the affected player and admits the complete shape");
    let owner = crate::ProgramOwnerId::ability(crate::AbilityDefinitionId {
        card_id: "lavabrink_program_admission_fixture".into(),
        face_id: crate::CardFaceId::new("lavabrink_program_admission_fixture").unwrap(),
        ability_path: vec![crate::AbilityId::new("triggered_01").unwrap()],
    });
    let compiled = registry
        .compiled_program_for_owner(&owner)
        .expect("the exact upkeep ability owns its compiled Program");
    assert_eq!(
        compiled.required_inputs,
        vec![ProgramRequiredInput::TriggerAffectedPlayer],
        "the chooser is bound to the captured upkeep player before instruction zero"
    );

    let [choose, threshold] = compiled.body.steps.as_slice() else {
        panic!("the upkeep choice precedes one source-threshold If");
    };
    let CompiledInstructionKind::Choose {
        chooser,
        optional,
        branches,
        otherwise,
        ..
    } = &choose.kind
    else {
        panic!("the upkeep player makes the add/remove/decline choice");
    };
    assert_eq!(*chooser, PlayerRecipient::AffectedPlayer);
    assert!(*optional);
    assert!(otherwise.steps.is_empty());
    let [add, remove] = branches.as_slice() else {
        panic!("the choice offers one add branch and one remove branch");
    };
    assert!(matches!(
        &add.requirement,
        CompiledChooseRequirement::GameCondition(GameCondition::ObjectMatches {
            object: crate::primitives::ConditionObjectRef::Source,
            filter,
        }) if filter.kind == TargetKind::AnyPermanent
    ));
    assert!(matches!(
        &remove.requirement,
        CompiledChooseRequirement::GameCondition(GameCondition::AllOf(conditions))
            if conditions.len() == 2
                && conditions.iter().any(|condition| matches!(
                    condition,
                    GameCondition::SourceCounterCount {
                        counter: crate::primitives::CounterKind::Doom,
                        min: Some(1),
                        max: None,
                    }
                ))
                && conditions.iter().any(|condition| matches!(
                    condition,
                    GameCondition::ObjectMatches {
                        object: crate::primitives::ConditionObjectRef::Source,
                        filter,
                    } if filter.kind == TargetKind::AnyPermanent
                ))
    ));
    assert_eq!(
        add.fallback_label(),
        "Put a doom counter on this permanent."
    );
    assert_eq!(
        remove.fallback_label(),
        "Remove a doom counter from this permanent.",
        "fixed source-counter removal needs a useful public branch label"
    );
    assert!(matches!(
        remove.body.steps.as_slice(),
        [step]
            if matches!(
                &step.kind,
                CompiledInstructionKind::Perform(effect)
                    if matches!(effect.as_ref(), SpellEffectKind::RemoveCounters {
                        counter: crate::primitives::CounterKind::Doom,
                        count: 1,
                        subject: crate::primitives::EffectSubject::Source,
                    })
            )
    ));

    let CompiledInstructionKind::If { condition, yes, no } = &threshold.kind else {
        panic!("the post-choice threshold remains an ordered Program If");
    };
    assert!(matches!(
        condition.as_ref(),
        crate::primitives::CompiledProgramCondition::Game(GameCondition::AllOf(conditions))
            if conditions.len() == 2
                && conditions.iter().any(|condition| matches!(
                    condition,
                    GameCondition::AnyOf(branches)
                        if branches.len() == 2
                            && branches.iter().all(|branch| matches!(
                                branch,
                                GameCondition::SourceCounterCount {
                                    counter: crate::primitives::CounterKind::Doom,
                                    min: Some(3 | 4),
                                    max: None,
                                }
                            ))
                ))
    ));
    assert!(no.steps.is_empty());
    assert!(matches!(
        yes.steps.as_slice(),
        [sacrifice, reflexive]
            if sacrifice.outputs.len() == 1
                && compiled.slot(sacrifice.outputs[0]).is_some_and(|slot|
                    slot.result_type == crate::primitives::ResultType::CardResultCohort {
                        action: crate::primitives::CardResultAction::Sacrifice,
                        who: PlayerRecipient::Controller,
                        cardinality: crate::primitives::Cardinality {
                            min: 0,
                            max: Some(1),
                        },
                    }
                )
                && reflexive.input == Some(sacrifice.outputs[0])
    ));
    let CompiledInstructionKind::Perform(reflexive_effect) = &yes.steps[1].kind else {
        unreachable!("the immediate receipt consumer is a reflexive trigger");
    };
    assert!(matches!(
        reflexive_effect.as_ref(),
        SpellEffectKind::CreateReflexiveTrigger {
            when: Some(crate::primitives::ResolutionReceiptCondition::Sacrificed {
                object: crate::primitives::ConditionObjectRef::Source,
            }),
            ability,
        } if matches!(
            ability.effect.as_slice(),
            [SpellEffectKind::DamageAll {
                amount: crate::primitives::Amount::Fixed(6),
                players: crate::primitives::MassPlayerSet::All,
                kind,
            }] if kind.kind == TargetKind::Creature
        )
    ));

    let non_upkeep = definition("WhenSelfEntersBattlefield", BODY);
    let error = crate::CardRegistry::from_chunks_and_tokens(&[&non_upkeep], &[])
        .expect_err("the AffectedPlayer chooser needs a trigger that captures that player")
        .to_string();
    assert!(
        error
            .contains("requires TriggerAffectedPlayer, but its trigger does not supply that input"),
        "the precise producer/consumer mismatch is reported: {error}"
    );

    let cross_object = BODY.replace(
        "ObjectMatches(object: Source, filter: (kind: AnyPermanent))",
        "ObjectMatches(object: ChosenTarget(group_index: 0, target_index: 0), filter: (kind: AnyPermanent))",
    );
    assert!(
        crate::CardRegistry::from_chunks_and_tokens(
            &[&definition(
                "AtBeginningOfUpkeep(player: AnyPlayer)",
                &cross_object
            )],
            &[],
        )
        .is_err(),
        "the Floodgates source predicate cannot become a chosen or cross-object condition"
    );

    let unbounded_source_counter = BODY.replace(
        "SourceCounterCount(counter: Doom, min: Some(1))",
        "SourceCounterCount(counter: Doom)",
    );
    assert!(
        crate::CardRegistry::from_chunks_and_tokens(
            &[&definition(
                "AtBeginningOfUpkeep(player: AnyPlayer)",
                &unbounded_source_counter,
            )],
            &[],
        )
        .is_err(),
        "recursive source-count predicates retain the ordinary bounded-count contract"
    );

    let empty_nested_condition = BODY.replace(
        "AnyOf([\n                    SourceCounterCount(counter: Doom, min: Some(3)),\n                    SourceCounterCount(counter: Doom, min: Some(4)),\n                ])",
        "AnyOf([])",
    );
    assert!(
        crate::CardRegistry::from_chunks_and_tokens(
            &[&definition(
                "AtBeginningOfUpkeep(player: AnyPlayer)",
                &empty_nested_condition,
            )],
            &[],
        )
        .is_err(),
        "recursive AllOf/AnyOf source predicates still require nonempty bounded composition"
    );

    let compile_ability_body = |body: &str| {
        let program: crate::CardProgram = ron::from_str(&format!("(body: {body})"))
            .expect("counter-removal Program fixture parses");
        crate::primitives::CompiledProgram::compile(
            owner.clone(),
            crate::primitives::EffectContext::Ability,
            &program,
        )
    };
    for (label, effect) in [
        (
            "zero count",
            "RemoveCounters(counter: Doom, count: 0, subject: Source)",
        ),
        (
            "non-source subject",
            "RemoveCounters(counter: Doom, count: 1, subject: Chosen((kind: AnyPermanent)))",
        ),
    ] {
        let body = format!("(steps: [(id: \"remove\", kind: Perform({effect}))], yields: [])");
        assert!(
            compile_ability_body(&body).is_err(),
            "fixed-positive Source RemoveCounters rejects {label}"
        );
    }

    let spell = format!(
        r#"(id: "lavabrink_program_spell_fixture",
            name: "Lavabrink Program Spell Fixture",
            face_id: "lavabrink_program_spell_fixture", mana_cost: "{{0}}",
            types: ["Instant"], spell_program: Some((body: {BODY})))"#
    );
    assert!(
        crate::CardRegistry::from_chunks_and_tokens(&[&spell], &[]).is_err(),
        "captured upkeep-player and live source predicates remain ability-only"
    );
}

#[test]
fn program_source_counter_removal_branch_fallback_is_readable() {
    use crate::primitives::{
        CompiledBlock, CompiledChooseBranch, CompiledChooseRequirement, CompiledInstruction,
        CompiledInstructionKind, CounterKind, EffectSubject, ResolutionCost, SpellEffectKind,
    };

    let branch = CompiledChooseBranch {
        branch_id: crate::ChoiceId::new("remove_doom").unwrap(),
        presentation: crate::AbilityPresentation::Fallback,
        cost: ResolutionCost::None,
        requirement: CompiledChooseRequirement::Always,
        body: CompiledBlock {
            steps: vec![CompiledInstruction {
                instruction_address: 0,
                id: "remove_doom_counter".into(),
                outputs: vec![],
                input: None,
                kind: CompiledInstructionKind::Perform(Box::new(SpellEffectKind::RemoveCounters {
                    counter: CounterKind::Doom,
                    count: 1,
                    subject: EffectSubject::Source,
                })),
            }],
            yields: vec![],
        },
    };

    assert_eq!(
        branch.fallback_label(),
        "Remove a doom counter from this permanent."
    );
}

#[test]
fn program_source_sacrifice_receipt_is_typed_immediate_and_unambiguous() {
    use crate::primitives::{CompiledInstructionKind, CompiledProgram, EffectContext};

    let owner = crate::ProgramOwnerId::ability(crate::AbilityDefinitionId {
        card_id: "source_sacrifice_receipt_fixture".into(),
        face_id: crate::CardFaceId::new("source_sacrifice_receipt_fixture").unwrap(),
        ability_path: vec![crate::AbilityId::new("triggered_01").unwrap()],
    });
    let compile = |body: &str| {
        let program: crate::CardProgram =
            ron::from_str(&format!("(body: {body})")).expect("sacrifice receipt fixture parses");
        CompiledProgram::compile(owner.clone(), EffectContext::Ability, &program)
    };
    let reflexive = r#"(id: "reflexive", input: Some("sacrificed"), kind: Perform(
        CreateReflexiveTrigger(
            when: Some(Sacrificed(object: Source)),
            ability: (
                ability_id: "reflexive_damage", presentation: Fallback,
                effect: [DamageAll(amount: 6, players: All,
                    kind: (kind: Creature))],
                targeting: None, intervening_if: None,
            ),
        ),
    ))"#;
    let direct_source_sacrifice = format!(
        r#"(steps: [
            (id: "sacrifice", bind: ["sacrificed"], kind: Perform(
                Sacrifice(subject: Source),
            )),
            {reflexive},
        ], yields: [])"#
    );
    let compiled = compile(&direct_source_sacrifice)
        .expect("a reflexive child consumes its immediately preceding source sacrifice result");
    let [sacrifice, child] = compiled.body.steps.as_slice() else {
        panic!("the source sacrifice directly precedes its reflexive child");
    };
    assert!(matches!(
        &sacrifice.kind,
        CompiledInstructionKind::Perform(effect)
            if matches!(effect.as_ref(), crate::primitives::SpellEffectKind::Sacrifice {
                subject: crate::primitives::EffectSubject::Source,
            })
    ));
    let result = compiled
        .slot(sacrifice.outputs[0])
        .expect("the committed sacrifice exposes its own typed result");
    assert_eq!(
        result.result_type,
        crate::primitives::ResultType::CardResultCohort {
            action: crate::primitives::CardResultAction::Sacrifice,
            who: crate::primitives::PlayerRecipient::Controller,
            cardinality: crate::primitives::Cardinality {
                min: 0,
                max: Some(1),
            },
        }
    );
    assert_eq!(child.input, Some(sacrifice.outputs[0]));

    let wrong_type = direct_source_sacrifice.replace(
        r#"(id: "sacrifice", bind: ["sacrificed"], kind: Perform(
                Sacrifice(subject: Source),
            ))"#,
        r#"(id: "sacrifice", bind: ["sacrificed"], kind: Perform(
                Draw(who: Controller, count: 1),
            ))"#,
    );
    assert!(
        compile(&wrong_type).is_err(),
        "a DrawReceipt cannot authorize the Sacrificed(Source) child"
    );

    let unrelated_object = direct_source_sacrifice.replace(
        "Sacrificed(object: Source)",
        "Sacrificed(object: PreviousEffectObject)",
    );
    assert!(
        compile(&unrelated_object).is_err(),
        "the receipt predicate must name the exact sacrificed source"
    );

    let non_immediate = direct_source_sacrifice.replace(
        &format!("{reflexive},"),
        &format!(
            r#"(id: "unrelated", kind: Perform(GainLife(amount: 1))),
            {reflexive},"#
        ),
    );
    assert!(
        compile(&non_immediate).is_err(),
        "an unrelated instruction cannot separate the receipt from its consumer"
    );

    let branch_ambiguous = format!(
        r#"(steps: [
            (id: "choose_sacrifice", bind: ["sacrificed"], kind: Choose(
                choice: "sacrifice_choice", chooser: Controller, optional: false,
                branches: [
                    (branch_id: "first", presentation: Fallback, cost: None,
                        requirement: Always,
                        body: (steps: [(id: "sacrifice_first", bind: ["first_result"],
                            kind: Perform(Sacrifice(subject: Source)))],
                            yields: ["first_result"])),
                    (branch_id: "second", presentation: Fallback, cost: None,
                        requirement: Always,
                        body: (steps: [(id: "sacrifice_second", bind: ["second_result"],
                            kind: Perform(Sacrifice(subject: Source)))],
                            yields: ["second_result"])),
                ],
                otherwise: (steps: [(id: "sacrifice_otherwise", bind: ["otherwise_result"],
                    kind: Perform(Sacrifice(subject: Source)))],
                    yields: ["otherwise_result"]),
            )),
            {reflexive},
        ], yields: [])"#
    );
    assert!(
        compile(&branch_ambiguous).is_err(),
        "a joined Choose output is not an explicit immediate Sacrifice(Source) result"
    );

    let resultless_reflexive = direct_source_sacrifice.replace("input: Some(\"sacrificed\"), ", "");
    assert!(
        compile(&resultless_reflexive).is_err(),
        "Sacrificed(Source) cannot read ambient prior-effect state"
    );

    let direct_damage = r#"(steps: [(id: "damage_all", kind: Perform(DamageAll(
        amount: 6, players: All, kind: (kind: Creature),
    )))], yields: [])"#;
    assert!(
        compile(direct_damage).is_err(),
        "the specialized reflexive child does not admit a direct Program DamageAll leaf"
    );
}

#[test]
fn source_sacrifice_reflexive_damage_accepts_any_positive_fixed_all_creature_amount() {
    let compile = |amount: &str, players: &str, kind: &str| {
        let body = format!(
            r#"(steps: [
                (id: "sacrifice", bind: ["sacrificed"], kind: Perform(
                    Sacrifice(subject: Source),
                )),
                (id: "reflexive", input: Some("sacrificed"), kind: Perform(
                    CreateReflexiveTrigger(
                        when: Some(Sacrificed(object: Source)),
                        ability: (
                            ability_id: "reflexive_damage", presentation: Fallback,
                            effect: [DamageAll(amount: {amount}, players: {players},
                                kind: {kind})],
                            targeting: None, intervening_if: None,
                        ),
                    ),
                )),
            ], yields: [])"#
        );
        let _: crate::CardProgram =
            ron::from_str(&format!("(body: {body})")).unwrap_or_else(|error| {
                panic!("the source-sacrifice reflexive CardProgram fixture parses: {error}")
            });
        ability_program_fixture("source_sacrifice_damage_amount_fixture", &body)
    };

    compile("5", "All", "(kind: Creature)")
        .expect("the bounded reflexive family accepts a different positive fixed amount");

    for (label, amount, players, kind) in [
        ("zero", "0", "All", "(kind: Creature)"),
        ("cast-time X", "X", "All", "(kind: Creature)"),
        (
            "variable amount",
            "Conditional(condition: ControllerLibraryEmpty, when_true: 5, otherwise: 4)",
            "All",
            "(kind: Creature)",
        ),
        ("non-creature filter", "5", "All", "(kind: AnyPermanent)"),
        (
            "restricted player recipient",
            "5",
            "Controller",
            "(kind: Creature)",
        ),
    ] {
        assert!(
            compile(amount, players, kind).is_err(),
            "Sacrificed(Source) reflexive DamageAll remains closed to {label}"
        );
    }
}

#[test]
fn ability_program_binds_same_graveyard_exile_result_to_its_target_group() {
    let program: crate::CardProgram = ron::from_str(
        r#"(body: (steps: [
            (id: "exile", bind: ["exiled"], input: None,
                kind: Perform(MoveGraveyardCards(
                    filter: (owner: AnyPlayer), destination: Exile,
                    linked_exile_id: None, entry_modifiers: [],
                ))),
            (id: "drain_if_creature", bind: [], input: None,
                kind: IfResult(
                    condition: CardResultCount(result: "exiled",
                        card_type: Some(Creature), min: Some(1), max: None),
                    yes: (steps: [
                        (id: "lose_life", bind: [], input: None,
                            kind: Perform(LoseLife(amount: Fixed(2), who: EachOpponent))),
                        (id: "gain_life", bind: [], input: None,
                            kind: Perform(GainLife(amount: 2))),
                    ], yields: []),
                    no: (steps: [], yields: []),
                )),
        ], yields: []), target_bindings: [
            (instruction_id: "exile", role: Target, target_group_index: 0),
        ])"#,
    )
    .expect("Soul-Shackled Zombie's authored Program parses");
    let targeting: crate::primitives::TargetingDef = ron::from_str(
        r#"(groups: [(
            min: 0, max: 2,
            prompt: "Choose up to two target cards from a single graveyard",
            effect_indices: [], distinct_from: [], same_graveyard: true,
            cast_cost_expansion: None,
        )])"#,
    )
    .expect("one owner-local same-graveyard group parses");
    let owner = crate::ProgramOwnerId::ability(crate::AbilityDefinitionId {
        card_id: "program_graveyard_exile_fixture".into(),
        face_id: crate::CardFaceId::new("program_graveyard_exile_fixture").unwrap(),
        ability_path: vec![crate::AbilityId::new("triggered_01").unwrap()],
    });
    let compiled = crate::primitives::CompiledProgram::compile_with_targeting(
        owner,
        crate::primitives::EffectContext::Ability,
        &program,
        Some(&targeting),
    )
    .expect("an ability Program may bind its synchronous targeted graveyard Exile result");

    assert_eq!(compiled.target_bindings.len(), 1);
    assert_eq!(compiled.target_bindings[0].target_group_index, 0);
    assert_eq!(compiled.target_bindings[0].role_index, 0);
    let [exile, count_gate] = compiled.body.steps.as_slice() else {
        panic!("the Exile result is followed by its count gate");
    };
    let [exile_result] = exile.outputs.as_slice() else {
        panic!("the Exile instruction binds its typed result");
    };
    assert_eq!(
        compiled.slot(*exile_result).unwrap().result_type,
        crate::primitives::ResultType::TargetCardResultCohort {
            action: crate::primitives::CardResultAction::Exile,
            cardinality: crate::primitives::Cardinality {
                min: 0,
                max: Some(2),
            },
            target_group_index: 0,
        }
    );
    let crate::primitives::CompiledInstructionKind::If { condition, .. } = &count_gate.kind else {
        panic!("the typed Exile cohort feeds the named creature count");
    };
    assert!(matches!(
        condition.as_ref(),
        crate::primitives::CompiledProgramCondition::Result(
            crate::primitives::CompiledProgramResultCondition::CardResultCount {
                result,
                card_type: Some(crate::primitives::CardTypeFilter::Creature),
                min: Some(1),
                max: None,
            }
        ) if *result == *exile_result
    ));
}

#[test]
fn ability_graveyard_exile_result_rejects_unsupported_profiles() {
    let compile = |context: crate::primitives::EffectContext,
                   effect: &str,
                   output_bindings: &str,
                   target_bindings: &str,
                   target_group: Option<&str>,
                   count_result: &str,
                   count_min: &str,
                   count_max: &str|
     -> Result<crate::primitives::CompiledProgram, String> {
        let source = r#"(body: (steps: [
            (id: "exile", bind: [__OUTPUT_BINDINGS__], input: None,
                kind: Perform(__EFFECT__)),
            (id: "count", bind: [], input: None,
                kind: IfResult(condition: CardResultCount(result: "__COUNT_RESULT__",
                    card_type: Some(Creature), min: __COUNT_MIN__, max: __COUNT_MAX__),
                    yes: (steps: [], yields: []), no: (steps: [], yields: [])))
        ], yields: []), target_bindings: [__TARGET_BINDINGS__])"#
            .replace("__OUTPUT_BINDINGS__", output_bindings)
            .replace("__EFFECT__", effect)
            .replace("__COUNT_RESULT__", count_result)
            .replace("__COUNT_MIN__", count_min)
            .replace("__COUNT_MAX__", count_max)
            .replace("__TARGET_BINDINGS__", target_bindings);
        let program: crate::CardProgram =
            ron::from_str(&source).expect("Program fixture parses before compiler validation");
        let targeting = target_group.map(|group| {
            ron::from_str::<crate::primitives::TargetingDef>(
                &r#"(groups: [__GROUP__])"#.replace("__GROUP__", group),
            )
            .expect("target-group fixture parses before compiler validation")
        });
        let owner = match context {
            crate::primitives::EffectContext::Ability => {
                crate::ProgramOwnerId::ability(crate::AbilityDefinitionId {
                    card_id: "graveyard_exile_profile_fixture".into(),
                    face_id: crate::CardFaceId::new("graveyard_exile_profile_fixture").unwrap(),
                    ability_path: vec![crate::AbilityId::new("triggered_01").unwrap()],
                })
            }
            crate::primitives::EffectContext::Spell => crate::ProgramOwnerId::face(
                "graveyard_exile_profile_fixture",
                crate::CardFaceId::new("graveyard_exile_profile_fixture").unwrap(),
            ),
        };
        crate::primitives::CompiledProgram::compile_with_targeting(
            owner,
            context,
            &program,
            targeting.as_ref(),
        )
    };

    const EXILE: &str = "MoveGraveyardCards(filter: (owner: AnyPlayer), destination: Exile, linked_exile_id: None, entry_modifiers: [])";
    const TARGET: &str = "(instruction_id: \"exile\", role: Target, target_group_index: 0)";
    const GROUP: &str = "(min: 0, max: 2, prompt: \"Choose up to two cards from one graveyard\", effect_indices: [], same_graveyard: true)";
    let compile_default = |context, effect, bindings, group| {
        compile(
            context,
            effect,
            "\"exiled\"",
            bindings,
            group,
            "exiled",
            "Some(1)",
            "None",
        )
    };

    assert!(
        compile_default(
            crate::primitives::EffectContext::Ability,
            EXILE,
            TARGET,
            Some(GROUP)
        )
        .is_ok(),
        "the exact ability-owned Exile signature remains admitted"
    );

    let cases = [
        (
            "spell owner",
            crate::primitives::EffectContext::Spell,
            EXILE,
            TARGET,
            Some(GROUP),
        ),
        (
            "non-Exile destination",
            crate::primitives::EffectContext::Ability,
            "MoveGraveyardCards(filter: (owner: AnyPlayer), destination: Hand, linked_exile_id: None, entry_modifiers: [])",
            TARGET,
            Some(GROUP),
        ),
        (
            "linked Exile",
            crate::primitives::EffectContext::Ability,
            "MoveGraveyardCards(filter: (owner: AnyPlayer), destination: Exile, linked_exile_id: Some(\"linked\"), entry_modifiers: [])",
            TARGET,
            Some(GROUP),
        ),
        (
            "entry modifiers",
            crate::primitives::EffectContext::Ability,
            "MoveGraveyardCards(filter: (owner: AnyPlayer), destination: Exile, linked_exile_id: None, entry_modifiers: [AddTypes((creature_types: [\"Phyrexian\"]))])",
            TARGET,
            Some(GROUP),
        ),
        (
            "event-dependent graveyard filter",
            crate::primitives::EffectContext::Ability,
            "MoveGraveyardCards(filter: (owner: AnyPlayer, mana_value_less_than_trigger_object: true), destination: Exile, linked_exile_id: None, entry_modifiers: [])",
            TARGET,
            Some(GROUP),
        ),
        (
            "missing Target binding",
            crate::primitives::EffectContext::Ability,
            EXILE,
            "",
            Some(GROUP),
        ),
        (
            "wrong target role",
            crate::primitives::EffectContext::Ability,
            EXILE,
            "(instruction_id: \"exile\", role: Subject, target_group_index: 0)",
            Some(GROUP),
        ),
        (
            "repeated Target binding",
            crate::primitives::EffectContext::Ability,
            EXILE,
            "(instruction_id: \"exile\", role: Target, target_group_index: 0), (instruction_id: \"exile\", role: Target, target_group_index: 0)",
            Some(GROUP),
        ),
        (
            "mismatched instruction binding",
            crate::primitives::EffectContext::Ability,
            EXILE,
            "(instruction_id: \"other\", role: Target, target_group_index: 0)",
            Some(GROUP),
        ),
        (
            "unknown target group",
            crate::primitives::EffectContext::Ability,
            EXILE,
            "(instruction_id: \"exile\", role: Target, target_group_index: 1)",
            Some(GROUP),
        ),
        (
            "unknown instruction reference",
            crate::primitives::EffectContext::Ability,
            EXILE,
            "(instruction_id: \"exile\", role: Target, target_group_index: 0), (instruction_id: \"missing\", role: Target, target_group_index: 0)",
            Some(GROUP),
        ),
        (
            "legacy effect-index group",
            crate::primitives::EffectContext::Ability,
            EXILE,
            TARGET,
            Some("(min: 0, max: 2, prompt: \"Choose cards\", effect_indices: [0], same_graveyard: true)"),
        ),
        (
            "too many targets",
            crate::primitives::EffectContext::Ability,
            EXILE,
            TARGET,
            Some("(min: 0, max: 3, prompt: \"Choose cards\", effect_indices: [], same_graveyard: true)"),
        ),
        (
            "mandatory target minimum",
            crate::primitives::EffectContext::Ability,
            EXILE,
            TARGET,
            Some("(min: 1, max: 2, prompt: \"Choose cards\", effect_indices: [], same_graveyard: true)"),
        ),
        (
            "cross-graveyard selection",
            crate::primitives::EffectContext::Ability,
            EXILE,
            TARGET,
            Some("(min: 0, max: 2, prompt: \"Choose cards\", effect_indices: [], same_graveyard: false)"),
        ),
        (
            "opponent-chosen group",
            crate::primitives::EffectContext::Ability,
            EXILE,
            TARGET,
            Some("(chooser: ChosenOpponent, min: 0, max: 2, prompt: \"Choose cards\", effect_indices: [], same_graveyard: true)"),
        ),
    ];
    for (label, context, effect, bindings, group) in cases {
        assert!(
            compile_default(context, effect, bindings, group).is_err(),
            "the Exile result profile must reject {label}"
        );
    }
    assert!(
        compile_default(
            crate::primitives::EffectContext::Ability,
            EXILE,
            TARGET,
            None,
        )
        .is_err(),
        "the Exile result requires an authored target group"
    );
    assert!(
        compile_default(
            crate::primitives::EffectContext::Ability,
            EXILE,
            TARGET,
            Some("(min: 0, max: 2, prompt: \"First group\", effect_indices: [], same_graveyard: true), (min: 0, max: 1, prompt: \"Second group\", effect_indices: [], same_graveyard: true)"),
        )
        .is_err(),
        "the Exile result profile admits exactly one target group"
    );

    assert!(
        compile(
            crate::primitives::EffectContext::Ability,
            EXILE,
            "",
            TARGET,
            Some(GROUP),
            "exiled",
            "Some(1)",
            "None",
        )
        .is_err(),
        "the Exile producer must bind its single typed result"
    );
    assert!(
        compile(
            crate::primitives::EffectContext::Ability,
            EXILE,
            "\"exiled\", \"extra\"",
            TARGET,
            Some(GROUP),
            "exiled",
            "Some(1)",
            "None",
        )
        .is_err(),
        "the Exile producer cannot bind duplicate or additional result slots"
    );
    assert!(
        compile(
            crate::primitives::EffectContext::Ability,
            EXILE,
            "\"exiled\"",
            TARGET,
            Some(GROUP),
            "missing",
            "Some(1)",
            "None",
        )
        .is_err(),
        "the result count must read the exact Exile result slot"
    );
    assert!(
        compile(
            crate::primitives::EffectContext::Ability,
            EXILE,
            "\"exiled\"",
            TARGET,
            Some(GROUP),
            "exiled",
            "Some(2)",
            "Some(1)",
        )
        .is_err(),
        "the named count must have valid lower and upper bounds"
    );
}

#[test]
fn registry_admits_closed_spell_delayed_program_with_captured_target_controller() {
    let fixture = delayed_spell_program_fixture(ARCANE_DELAYED_PROGRAM_BODY);

    let registry =
        crate::CardRegistry::from_chunks_and_tokens(&[&fixture], &[]).unwrap_or_else(|error| {
            let rendered = error.to_string();
            assert!(
                !rendered.starts_with("ron parse:"),
                "the positive profile fixture must parse before admission is checked: {rendered}"
            );
            panic!(
                "the valid direct spell-delayed Program profile is not admitted yet: {rendered}"
            );
        });
    let owner = crate::ProgramOwnerId::ability(crate::AbilityDefinitionId {
        card_id: "delayed_spell_program_fixture".into(),
        face_id: crate::CardFaceId::new("delayed_spell_program_fixture").unwrap(),
        ability_path: vec![crate::AbilityId::new("delayed_01").unwrap()],
    });
    let compiled = registry
        .compiled_program_for_owner(&owner)
        .expect("the direct delayed child owns the compiled Program");
    assert_eq!(compiled.owner, owner);
    assert_eq!(
        compiled.required_inputs,
        vec![crate::primitives::ProgramRequiredInput::TriggerAffectedPlayer]
    );
    assert!(matches!(
        compiled.body.steps.as_slice(),
        [step] if matches!(
            &step.kind,
            crate::primitives::CompiledInstructionKind::Choose { branches, .. }
                if branches.len() == 3
        )
    ));
}

const ARCANE_DELAYED_PROGRAM_BODY: &str = r#"(steps: [
    (id: "choose_count", kind: Choose(
        choice: "draw_count", chooser: AffectedPlayer, optional: false,
        branches: [
            (branch_id: "draw_zero", presentation: Fallback,
                cost: None, requirement: Always,
                body: (steps: [(id: "draw_zero",
                    kind: Perform(Draw(who: AffectedPlayer, count: 0)))], yields: [])),
            (branch_id: "draw_one", presentation: Fallback,
                cost: None, requirement: Always,
                body: (steps: [(id: "draw_one",
                    kind: Perform(Draw(who: AffectedPlayer, count: 1)))], yields: [])),
            (branch_id: "draw_two", presentation: Fallback,
                cost: None, requirement: Always,
                body: (steps: [(id: "draw_two",
                    kind: Perform(Draw(who: AffectedPlayer, count: 2)))], yields: [])),
        ], otherwise: (steps: [], yields: []),
    )),
], yields: [])"#;

fn delayed_spell_program_fixture(body: &str) -> String {
    delayed_spell_program_fixture_with_face_fields(body, "[\"Instant\"]", "")
}

fn delayed_spell_program_fixture_with_face_fields(body: &str, types: &str, fields: &str) -> String {
    r#"(id: "delayed_spell_program_fixture",
        name: "Delayed Spell Program Fixture", face_id: "delayed_spell_program_fixture",
        mana_cost: "{1}{U}", types: __FACE_TYPES__,
        __FACE_FIELDS__
        spell_effect: [
            CounterTargetSpell(spell_filter: (), unless_controller_pays: None,
                unless_controller_pays_by_cast_cost: None),
            CreateDelayedTrigger(subject: None,
                affected_player: Some(PreviousTargetedSpellController),
                ability: (
                    ability_id: "delayed_01", presentation: Fallback,
                    trigger: AtBeginningOfNextTurnUpkeep, effect: [],
                    program: Some((body: __DELAYED_BODY__, target_bindings: [])),
                    modal: None, targeting: None, may: false, intervening_if: None,
                    triggers_only_once: false, max_triggers_per_turn: None,
                ),
            ),
        ],
        targeting: Some((groups: [(min: 1, max: 1, prompt: "Choose target spell",
            effect_indices: [0])]))
    )"#
    .replace("__DELAYED_BODY__", body)
    .replace("__FACE_TYPES__", types)
    .replace("__FACE_FIELDS__", fields)
}

fn assert_delayed_spell_program_rejected(fixture: &str, expected_reason: &str) {
    let error = crate::CardRegistry::from_chunks_and_tokens(&[fixture], &[])
        .expect_err("the invalid direct face delayed Program profile must be rejected")
        .to_string();
    assert!(
        !error.starts_with("ron parse:"),
        "the near-miss fixture must parse before admission is checked: {error}"
    );
    assert!(
        error.contains(expected_reason),
        "expected {expected_reason:?} in admission error: {error}"
    );
}

#[test]
fn registry_reuses_the_direct_delayed_program_profile_for_a_different_body() {
    let body = r#"(steps: [(id: "gain_life",
        kind: Perform(GainLife(who: Controller, amount: 1)))], yields: [])"#;
    let fixture = delayed_spell_program_fixture(body);
    let registry = crate::CardRegistry::from_chunks_and_tokens(&[&fixture], &[])
        .expect("the exact capture profile admits another generally supported Program body");
    let owner = crate::ProgramOwnerId::ability(crate::AbilityDefinitionId {
        card_id: "delayed_spell_program_fixture".into(),
        face_id: crate::CardFaceId::new("delayed_spell_program_fixture").unwrap(),
        ability_path: vec![crate::AbilityId::new("delayed_01").unwrap()],
    });
    let compiled = registry
        .compiled_program_for_owner(&owner)
        .expect("the delayed child retains its exact owner");
    assert!(compiled.required_inputs.is_empty());
    assert!(matches!(
        compiled.body.steps.as_slice(),
        [step] if matches!(
            &step.kind,
            crate::primitives::CompiledInstructionKind::Perform(effect)
                if matches!(**effect, crate::primitives::SpellEffectKind::GainLife { .. })
        )
    ));
}

#[test]
fn registry_rejects_direct_delayed_program_near_miss_profiles() {
    let base = delayed_spell_program_fixture(ARCANE_DELAYED_PROGRAM_BODY);
    let cases = [
        (
            "wrong captured player",
            base.replace(
                "affected_player: Some(PreviousTargetedSpellController)",
                "affected_player: None",
            ),
            "requires the captured PreviousTargetedSpellController",
        ),
        (
            "wrong delayed subject",
            base.replace(
                "CreateDelayedTrigger(subject: None,",
                "CreateDelayedTrigger(subject: Some(PreviousEffectObject),",
            ),
            "requires a subjectless delayed trigger",
        ),
        (
            "wrong trigger",
            base.replace(
                "trigger: AtBeginningOfNextTurnUpkeep",
                "trigger: AtBeginningOfNextEndStep",
            ),
            "requires AtBeginningOfNextTurnUpkeep",
        ),
        (
            "paid counter",
            base.replace(
                "unless_controller_pays: None",
                "unless_controller_pays: Some(2)",
            ),
            "must have no payment form",
        ),
        (
            "cast-cost-paid counter",
            base.replace(
                "unless_controller_pays_by_cast_cost: None",
                "unless_controller_pays_by_cast_cost: Some((condition: (group_id: \"kick\", option_id: \"kick_option\"), if_selected: 1, otherwise: 1))",
            ),
            "must have no payment form",
        ),
        (
            "wrong producer",
            base.replace(
                "CounterTargetSpell(spell_filter: (), unless_controller_pays: None,\n                unless_controller_pays_by_cast_cost: None),",
                "CounterTargetAbility,",
            ),
            "requires an immediately preceding CounterTargetSpell",
        ),
        (
            "non-adjacent producer",
            base.replace(
                "unless_controller_pays_by_cast_cost: None),\n            CreateDelayedTrigger",
                "unless_controller_pays_by_cast_cost: None),\n            Draw(who: Controller, count: 1),\n            CreateDelayedTrigger",
            ),
            "requires an immediately preceding CounterTargetSpell",
        ),
        (
            "optional delayed trigger",
            base.replace("may: false", "may: true"),
            "requires default delayed-ability metadata",
        ),
        (
            "intervening-if delayed trigger",
            base.replace("intervening_if: None", "intervening_if: Some(Void)"),
            "requires default delayed-ability metadata",
        ),
        (
            "trigger-only-once delayed trigger",
            base.replace("triggers_only_once: false", "triggers_only_once: true"),
            "requires default delayed-ability metadata",
        ),
        (
            "capped delayed trigger",
            base.replace(
                "max_triggers_per_turn: None",
                "max_triggers_per_turn: Some(1)",
            ),
            "requires default delayed-ability metadata",
        ),
        (
            "targeted delayed trigger",
            base.replace(
                "modal: None, targeting: None",
                "modal: None, targeting: Some((groups: []))",
            ),
            "requires default delayed-ability metadata",
        ),
        (
            "implicit face targeting",
            base.replace(
                "targeting: Some((groups: [(min: 1, max: 1, prompt: \"Choose target spell\",\n            effect_indices: [0])]))",
                "targeting: None",
            ),
            "requires an explicit face-local target group",
        ),
        (
            "minimum other than one",
            base.replace("min: 1, max: 1", "min: 0, max: 1"),
            "requires one 1-of-1 StackSpell binding",
        ),
        (
            "maximum other than one",
            base.replace("min: 1, max: 1", "min: 1, max: 2"),
            "requires one 1-of-1 StackSpell binding",
        ),
        (
            "target group bound to the wrong effect",
            base.replace("effect_indices: [0]", "effect_indices: [1]"),
            "target group references an untargeted effect",
        ),
        (
            "counter group also bound to a stack ability role",
            base.replace(
                "            ),\n        ],\n        targeting:",
                "            ),\n            CounterTargetAbility,\n        ],\n        targeting:",
            )
            .replace("effect_indices: [0]", "effect_indices: [0, 2]"),
            "requires one 1-of-1 StackSpell binding",
        ),
    ];

    for (label, fixture, expected_reason) in cases {
        let error = crate::CardRegistry::from_chunks_and_tokens(&[&fixture], &[])
            .expect_err("a near-miss delayed Program profile must be rejected")
            .to_string();
        assert!(
            !error.starts_with("ron parse:"),
            "{label} fixture must parse before admission is checked: {error}"
        );
        assert!(
            error.contains(expected_reason),
            "{label}: expected {expected_reason:?}, got {error}"
        );
    }
}

#[test]
fn registry_keeps_delayed_program_owner_ids_face_local_and_rejects_same_face_collisions() {
    let printed_activated = delayed_spell_program_fixture_with_face_fields(
        ARCANE_DELAYED_PROGRAM_BODY,
        "[\"Instant\"]",
        r#"activated_abilities: [(ability_id: "delayed_01", presentation: Fallback,
            costs: [], effect: [Draw(who: Controller, count: 1)])],"#,
    );
    let printed_triggered = delayed_spell_program_fixture_with_face_fields(
        ARCANE_DELAYED_PROGRAM_BODY,
        "[\"Instant\"]",
        r#"triggered_abilities: [(ability_id: "delayed_01", presentation: Fallback,
            trigger: AtBeginningOfNextTurnUpkeep,
            effect: [Draw(who: Controller, count: 1)])],"#,
    );
    let class_level_ability = delayed_spell_program_fixture_with_face_fields(
        ARCANE_DELAYED_PROGRAM_BODY,
        "[\"Enchantment\", \"Class\"]",
        &class_level_bars("delayed_01", ""),
    );
    let class_activated = delayed_spell_program_fixture_with_face_fields(
        ARCANE_DELAYED_PROGRAM_BODY,
        "[\"Enchantment\", \"Class\"]",
        &class_level_bars(
            "level_two",
            r#"activated_abilities: [(ability_id: "delayed_01", presentation: Fallback,
                costs: [], effect: [Draw(who: Controller, count: 1)])],"#,
        ),
    );
    let class_triggered = delayed_spell_program_fixture_with_face_fields(
        ARCANE_DELAYED_PROGRAM_BODY,
        "[\"Enchantment\", \"Class\"]",
        &class_level_bars(
            "level_two",
            r#"triggered_abilities: [(ability_id: "delayed_01", presentation: Fallback,
                trigger: WhenThisClassBecomesLevel(level: 2),
                effect: [Draw(who: Controller, count: 1)])],"#,
        ),
    );

    for (label, fixture) in [
        ("printed activated", printed_activated),
        ("printed triggered", printed_triggered),
        ("Class level-up", class_level_ability),
        ("Class activated", class_activated),
        ("Class triggered", class_triggered),
    ] {
        let error = crate::CardRegistry::from_chunks_and_tokens(&[&fixture], &[])
            .expect_err("a delayed Program owner cannot collide with a same-face ability owner")
            .to_string();
        assert!(
            !error.starts_with("ron parse:"),
            "{label} collision fixture must parse: {error}"
        );
        assert!(
            error.contains("conflicts with a face-local runtime ability owner"),
            "{label}: expected runtime owner collision, got {error}"
        );
    }

    let mut split = String::from(
        r#"(id: "delayed_spell_split_fixture", name: "Delayed Spell Split Fixture",
            layout: Split, faces: [
                (name: "Ability Face", face_id: "ability_face", types: ["Instant"],
                    triggered_abilities: [(ability_id: "delayed_01",
                        presentation: Fallback, trigger: WhenSelfEntersBattlefield,
                        effect: [Draw(who: Controller, count: 1)])]),
        "#,
    );
    let spell_face = delayed_spell_program_fixture(ARCANE_DELAYED_PROGRAM_BODY).replace(
        "(id: \"delayed_spell_program_fixture\",\n        name: \"Delayed Spell Program Fixture\", face_id: \"delayed_spell_program_fixture\",",
        "(name: \"Spell Face\", face_id: \"spell_face\",",
    );
    assert!(!spell_face.contains("id: \"delayed_spell_program_fixture\""));
    split.push_str(&spell_face);
    split.push_str("])");
    let registry = crate::CardRegistry::from_chunks_and_tokens(&[&split], &[])
        .expect("another face may use the same local ability ID");
    let owner = crate::ProgramOwnerId::ability(crate::AbilityDefinitionId {
        card_id: "delayed_spell_split_fixture".into(),
        face_id: crate::CardFaceId::new("spell_face").unwrap(),
        ability_path: vec![crate::AbilityId::new("delayed_01").unwrap()],
    });
    assert!(registry.compiled_program_for_owner(&owner).is_some());
}

#[test]
fn registry_accepts_the_counter_target_group_among_unrelated_face_groups() {
    let base = delayed_spell_program_fixture(ARCANE_DELAYED_PROGRAM_BODY);
    let fixture = base
        .replace(
            "        spell_effect: [\n            CounterTargetSpell",
            "        spell_effect: [\n            Exile(subject: Chosen((kind: Creature))),\n            CounterTargetSpell",
        )
        .replace(
            "effect_indices: [0])]))",
            "effect_indices: [0]), (min: 1, max: 1, prompt: \"Choose target spell\", effect_indices: [1])]))",
        );
    let registry = crate::CardRegistry::from_chunks_and_tokens(&[&fixture], &[])
        .expect("an unrelated target group does not obscure the counter's unique spell group");
    let owner = crate::ProgramOwnerId::ability(crate::AbilityDefinitionId {
        card_id: "delayed_spell_program_fixture".into(),
        face_id: crate::CardFaceId::new("delayed_spell_program_fixture").unwrap(),
        ability_path: vec![crate::AbilityId::new("delayed_01").unwrap()],
    });
    assert!(registry.compiled_program_for_owner(&owner).is_some());
}

fn class_level_bars(level_two_owner_id: &str, extra_level_two_fields: &str) -> String {
    r#"class_level_bars: [(
        level: 2,
        level_ability: (ability_id: "__LEVEL_TWO_ID__", presentation: Fallback,
            costs: [Mana("{2}{U}")], effect: [SetClassLevel(level: 2)], timing: SorcerySpeed),
        __LEVEL_TWO_FIELDS__
    ), (
        level: 3,
        level_ability: (ability_id: "level_three", presentation: Fallback,
            costs: [Mana("{4}{U}")], effect: [SetClassLevel(level: 3)], timing: SorcerySpeed),
    )],"#
        .replace("__LEVEL_TWO_ID__", level_two_owner_id)
        .replace("__LEVEL_TWO_FIELDS__", extra_level_two_fields)
}

#[test]
fn registry_keeps_nested_delayed_programs_unowned_and_upkeep_inputs_closed() {
    let base = delayed_spell_program_fixture(ARCANE_DELAYED_PROGRAM_BODY);
    let nested = base
        .replace(
            "CreateDelayedTrigger(subject: None,",
            "Conditional(condition: Void, effect: CreateDelayedTrigger(subject: None,",
        )
        .replace(
            "            ),\n        ],\n        targeting:",
            "            )),\n        ],\n        targeting:",
        );
    assert_delayed_spell_program_rejected(
        &nested,
        "resolving-created abilities cannot use authored programs",
    );

    let generic_upkeep = r#"(id: "generic_upkeep_program_fixture",
        name: "Generic Upkeep Program Fixture",
        face_id: "generic_upkeep_program_fixture", types: ["Artifact"],
        triggered_abilities: [(ability_id: "triggered_01", presentation: Fallback,
            trigger: AtBeginningOfNextTurnUpkeep, effect: [],
            program: Some((body: (steps: [(id: "draw",
                kind: Perform(Draw(who: AffectedPlayer, count: 1)))], yields: []),
                target_bindings: [])),
            modal: None, targeting: None, may: false, intervening_if: None,
            triggers_only_once: false, max_triggers_per_turn: None)])"#;
    let error = crate::CardRegistry::from_chunks_and_tokens(&[generic_upkeep], &[])
        .expect_err("upkeep timing alone does not provide the typed affected-player input")
        .to_string();
    assert!(
        !error.starts_with("ron parse:"),
        "the generic upkeep Program fixture must parse before admission: {error}"
    );
    assert!(
        error.contains("delayed trigger conditions require CreateDelayedTrigger"),
        "a delayed-only trigger is not a generic triggered-ability owner: {error}"
    );

    let ordinary_upkeep =
        generic_upkeep.replace("AtBeginningOfNextTurnUpkeep", "WhenSelfEntersBattlefield");
    let error = crate::CardRegistry::from_chunks_and_tokens(&[&ordinary_upkeep], &[])
        .expect_err("ordinary triggers without an affected-player binding cannot supply it")
        .to_string();
    assert!(
        error.contains("TriggerAffectedPlayer")
            && error.contains("trigger does not supply that input"),
        "the delayed capture must not satisfy an unrelated triggered Program: {error}"
    );
}

#[test]
fn registry_rejects_duplicate_direct_delayed_ids_across_program_and_legacy_siblings() {
    let base = delayed_spell_program_fixture(ARCANE_DELAYED_PROGRAM_BODY);
    let duplicate = base.replace(
        "        ],\n        targeting:",
        r#"            CreateDelayedTrigger(subject: None, affected_player: None,
                ability: (ability_id: "delayed_01", presentation: Fallback,
                    trigger: AtBeginningOfNextTurnUpkeep,
                    effect: [Draw(who: Controller, count: 1)])),
        ],
        targeting:"#,
    );
    assert_delayed_spell_program_rejected(
        &duplicate,
        "duplicate direct delayed child ability id 'delayed_01'",
    );
}

mod combustible_gearhulk_program_compiler_tests {
    use crate::primitives::{
        AuthoredBlock, CardProgram, CompiledInstructionKind, CompiledProgram, EffectContext,
        PlayerRecipient, ProgramTargetReference, ProgramTargetRole, TargetChooser, TargetFilter,
        TargetGroupDef, TargetKind, TargetRole, TargetSchema, TargetingDef,
    };

    fn owner() -> crate::ProgramOwnerId {
        crate::ProgramOwnerId::ability(crate::AbilityDefinitionId {
            card_id: "combustible_gearhulk_program_fixture".into(),
            face_id: crate::CardFaceId::new("combustible_gearhulk_program_fixture").unwrap(),
            ability_path: vec![crate::AbilityId::new("triggered_01").unwrap()],
        })
    }

    fn targeting_with_bounds(min: u32, max: u32) -> TargetingDef {
        TargetingDef {
            groups: vec![TargetGroupDef {
                chooser: TargetChooser::Controller,
                min,
                max,
                prompt: "Choose target opponent".into(),
                effect_indices: Vec::new(),
                distinct_from: Vec::new(),
                same_graveyard: false,
                cast_cost_expansion: None,
            }],
        }
    }

    fn opponent_targeting() -> TargetingDef {
        targeting_with_bounds(1, 1)
    }

    fn target_binding(
        instruction_id: &str,
        role: ProgramTargetRole,
        target_group_index: u32,
    ) -> ProgramTargetReference {
        ProgramTargetReference {
            instruction_id: instruction_id.into(),
            role,
            target_group_index,
        }
    }

    fn damage_target_binding() -> Vec<ProgramTargetReference> {
        vec![target_binding("damage", ProgramTargetRole::Target, 0)]
    }

    fn compile_with_targeting(
        body: &str,
        target_bindings: Vec<ProgramTargetReference>,
        targeting: &TargetingDef,
    ) -> Result<CompiledProgram, String> {
        compile_with_context(body, target_bindings, targeting, EffectContext::Ability)
    }

    fn compile_with_context(
        body: &str,
        target_bindings: Vec<ProgramTargetReference>,
        targeting: &TargetingDef,
        context: EffectContext,
    ) -> Result<CompiledProgram, String> {
        let program = CardProgram {
            body: ron::from_str::<AuthoredBlock>(body)
                .expect("the authored Program fixture parses"),
            target_bindings,
        };
        CompiledProgram::compile_with_targeting(
            match context {
                EffectContext::Ability => owner(),
                EffectContext::Spell => crate::ProgramOwnerId::face(
                    "combustible_gearhulk_program_fixture",
                    crate::CardFaceId::new("combustible_gearhulk_program_fixture").unwrap(),
                ),
            },
            context,
            &program,
            Some(targeting),
        )
    }

    fn compile(
        body: &str,
        target_bindings: Vec<ProgramTargetReference>,
    ) -> Result<CompiledProgram, String> {
        compile_with_targeting(body, target_bindings, &opponent_targeting())
    }

    fn targeted_choose_damage_body(
        chooser_group: u32,
        chooser_kind: &str,
        damage_group: u32,
    ) -> String {
        format!(
            r#"(steps: [
                (id: "choose", bind: [], input: None, kind: Choose(
                    choice: "draw_or_mill", chooser: TargetedPlayer(
                        group_index: {chooser_group}, kind: {chooser_kind},
                    ), optional: true,
                    branches: [(
                        branch_id: "draw", presentation: Fallback,
                        cost: None, requirement: Always,
                        body: (steps: [
                            (id: "draw_three", bind: [], input: None,
                                kind: Perform(Draw(who: Controller, count: 3))),
                        ], yields: []),
                    )],
                    otherwise: (steps: [
                        (id: "damage", bind: [], input: None,
                            kind: Perform(DamagePlayer(amount: 2, who: TargetedPlayer(
                                group_index: {damage_group}, kind: OpponentPlayer,
                            )))),
                    ], yields: []),
                )),
            ], yields: [])"#
        )
    }

    fn compile_targeted_choose_with_branch_cost(cost: &str) -> Result<CompiledProgram, String> {
        let body = targeted_choose_damage_body(0, "OpponentPlayer", 0).replace(
            "cost: None, requirement: Always",
            &format!("cost: {cost}, requirement: Always"),
        );
        compile(&body, damage_target_binding())
    }

    fn targeted_mill_damage_body(amount: &str, input: &str) -> String {
        format!(
            r#"(steps: [
                (id: "choose", bind: [], input: None, kind: Choose(
                    choice: "draw_or_mill", chooser: TargetedPlayer(
                        group_index: 0, kind: OpponentPlayer,
                    ), optional: true,
                    branches: [(
                        branch_id: "draw", presentation: Fallback,
                        cost: None, requirement: Always,
                        body: (steps: [
                            (id: "draw_three", bind: [], input: None,
                                kind: Perform(Draw(who: Controller, count: 3))),
                        ], yields: []),
                    )],
                    otherwise: (steps: [
                        (id: "mill", bind: ["milled"], input: None,
                            kind: Perform(Mill(count: 3, who: Controller))),
                        (id: "damage", bind: [], input: {input},
                            kind: Perform(DamagePlayer(
                                amount: {amount},
                                who: TargetedPlayer(group_index: 0, kind: OpponentPlayer),
                            ))),
                    ], yields: []),
                )),
            ], yields: [])"#
        )
    }

    #[test]
    fn program_compiler_accepts_gearhulk_targeted_choose_and_damage_group() {
        let compiled = compile(
            &targeted_choose_damage_body(0, "OpponentPlayer", 0),
            damage_target_binding(),
        )
        .expect("a triggered Program can pair the target chooser and damage recipient");

        assert_eq!(compiled.target_bindings.len(), 1);
        assert_eq!(compiled.target_bindings[0].instruction_address, 2);
        assert_eq!(compiled.target_bindings[0].target_group_index, 0);
        assert_eq!(compiled.target_bindings[0].role_index, 0);

        let targeting = opponent_targeting();
        let schema = TargetSchema::compile_program(&compiled, Some(&targeting))
            .expect("the compiled target group has one supported binding");
        assert_eq!(schema.groups.len(), 1);
        assert_eq!(schema.groups[0].bindings.len(), 1);
        assert_eq!(schema.groups[0].bindings[0].effect_index, 2);
        assert!(matches!(
            schema.groups[0].bindings[0].role,
            TargetRole::Filtered(filter) if filter == &TargetFilter {
                kind: TargetKind::OpponentPlayer,
                ..TargetFilter::default()
            }
        ));

        let spell = compile_with_context(
            &targeted_choose_damage_body(0, "OpponentPlayer", 0),
            damage_target_binding(),
            &targeting,
            EffectContext::Spell,
        )
        .expect("the closed opponent target profile supports spell programs too");
        assert_eq!(spell.target_bindings[0].instruction_address, 2);

        let [choose] = compiled.body.steps.as_slice() else {
            panic!("the target choice is the Program's only top-level instruction");
        };
        let CompiledInstructionKind::Choose {
            chooser:
                PlayerRecipient::TargetedPlayer {
                    group_index: 0,
                    kind: TargetKind::OpponentPlayer,
                },
            otherwise,
            ..
        } = &choose.kind
        else {
            panic!("the opponent chosen by the target group controls the resolution choice");
        };
        assert!(matches!(
            otherwise.steps.as_slice(),
            [instruction] if instruction.instruction_address == 2
                && matches!(
                    &instruction.kind,
                    CompiledInstructionKind::Perform(effect)
                        if matches!(effect.as_ref(), crate::primitives::SpellEffectKind::DamagePlayer {
                            who: PlayerRecipient::TargetedPlayer {
                                group_index: 0,
                                kind: TargetKind::OpponentPlayer,
                            },
                            ..
                        })
                )
        ));
    }

    #[test]
    fn program_compiler_rejects_paid_mana_targeted_optional_choose() {
        let error = compile_targeted_choose_with_branch_cost("Mana(\"{1}\")")
            .expect_err("a targeted optional Choose branch must not carry a Mana cost");
        assert!(error.contains("only supports a free cost"), "{error}");
    }

    #[test]
    fn program_compiler_rejects_paid_waterbend_targeted_optional_choose() {
        let error = compile_targeted_choose_with_branch_cost("Waterbend(\"{1}\")")
            .expect_err("a targeted optional Choose branch must not carry a Waterbend cost");
        assert!(error.contains("only supports a free cost"), "{error}");
    }

    #[test]
    fn program_compiler_recursively_accepts_gearhulk_targeted_choose_in_otherwise() {
        let compiled = compile(
            r#"(steps: [
                (id: "outer_choice", bind: [], input: None, kind: Choose(
                    choice: "outer", chooser: Controller, optional: true,
                    branches: [(
                        branch_id: "outer_draw", presentation: Fallback,
                        cost: None, requirement: Always,
                        body: (steps: [
                            (id: "outer_draw_one", bind: [], input: None,
                                kind: Perform(Draw(who: Controller, count: 1))),
                        ], yields: []),
                    )],
                    otherwise: (steps: [
                        (id: "choose", bind: [], input: None, kind: Choose(
                            choice: "draw_or_mill", chooser: TargetedPlayer(
                                group_index: 0, kind: OpponentPlayer,
                            ), optional: true,
                            branches: [(
                                branch_id: "draw", presentation: Fallback,
                                cost: None, requirement: Always,
                                body: (steps: [
                                    (id: "draw_three", bind: [], input: None,
                                        kind: Perform(Draw(who: Controller, count: 3))),
                                ], yields: []),
                            )],
                            otherwise: (steps: [
                                (id: "damage", bind: [], input: None,
                                    kind: Perform(DamagePlayer(amount: 2, who: TargetedPlayer(
                                        group_index: 0, kind: OpponentPlayer,
                                    )))),
                            ], yields: []),
                        )),
                    ], yields: []),
                )),
            ], yields: [])"#,
            damage_target_binding(),
        )
        .expect("targeted choice and matching damage compile recursively in Otherwise");

        assert_eq!(compiled.target_bindings[0].instruction_address, 4);
        let [outer] = compiled.body.steps.as_slice() else {
            panic!("the outer choice is the only top-level instruction");
        };
        let CompiledInstructionKind::Choose { otherwise, .. } = &outer.kind else {
            panic!("the outer instruction is a choice");
        };
        let [inner] = otherwise.steps.as_slice() else {
            panic!("the target choice is in the outer Otherwise block");
        };
        let CompiledInstructionKind::Choose { otherwise, .. } = &inner.kind else {
            panic!("the nested instruction is the targeted choice");
        };
        assert!(matches!(
            otherwise.steps.as_slice(),
            [damage] if damage.instruction_address == 4
                && matches!(
                    &damage.kind,
                    CompiledInstructionKind::Perform(effect)
                        if matches!(effect.as_ref(), crate::primitives::SpellEffectKind::DamagePlayer { .. })
                )
        ));
    }

    #[test]
    fn program_compiler_accepts_gearhulk_typed_mill_input_for_targeted_damage() {
        let compiled = compile(
            &targeted_mill_damage_body("Count(PreviousMillManaValueSum)", "Some(\"milled\")"),
            damage_target_binding(),
        )
        .expect("the direct mana-value sum consumes its named immediate Mill result");

        assert_eq!(compiled.target_bindings.len(), 1);
        assert_eq!(compiled.target_bindings[0].instruction_address, 3);
        let Some(milled_slot) = compiled.slots.iter().find(|slot| slot.name == "milled") else {
            panic!("the Otherwise Mill instruction binds its typed result");
        };
        assert!(matches!(
            &milled_slot.result_type,
            crate::primitives::ResultType::CardResultCohort {
                action: crate::primitives::CardResultAction::Mill,
                who: PlayerRecipient::Controller,
                ..
            }
        ));

        let [choose] = compiled.body.steps.as_slice() else {
            panic!("the target choice is the Program's only top-level instruction");
        };
        let CompiledInstructionKind::Choose { otherwise, .. } = &choose.kind else {
            panic!("the target choice owns the Otherwise block");
        };
        let [mill, damage] = otherwise.steps.as_slice() else {
            panic!("the Otherwise block contains the Mill and its direct damage consumer");
        };
        assert_eq!(mill.id, "mill");
        assert!(mill.outputs.contains(&milled_slot.id));
        assert_eq!(damage.id, "damage");
        assert_eq!(damage.input, Some(milled_slot.id));
        assert!(matches!(
            &damage.kind,
            CompiledInstructionKind::Perform(effect)
                if matches!(effect.as_ref(), crate::primitives::SpellEffectKind::DamagePlayer {
                    amount: crate::primitives::Amount::Count(
                        crate::primitives::CountExpression::PreviousMillManaValueSum
                    ),
                    who: PlayerRecipient::TargetedPlayer {
                        group_index: 0,
                        kind: TargetKind::OpponentPlayer,
                    },
                })
        ));
    }

    #[test]
    fn program_compiler_rejects_unbound_or_wrapped_gearhulk_mill_sum() {
        for (label, amount, input) in [
            (
                "unbound direct sum",
                "Count(PreviousMillManaValueSum)",
                "None",
            ),
            (
                "affine-wrapped sum",
                "Count(Affine(terms: [(coefficient: 1, quantity: PreviousMillManaValueSum)]))",
                "None",
            ),
            (
                "divide-wrapped sum",
                "DivideRoundedDown(amount: Count(PreviousMillManaValueSum), divisor: 2)",
                "None",
            ),
        ] {
            let error = compile(
                &targeted_mill_damage_body(amount, input),
                damage_target_binding(),
            )
            .expect_err(&format!("the Program compiler admitted {label}"));
            if label == "unbound direct sum" {
                assert!(
                    error.contains("instruction requires an explicitly bound input result"),
                    "{error}"
                );
            }
        }
    }

    #[test]
    fn program_compiler_requires_the_immediately_preceding_controller_mill_slot() {
        let valid =
            targeted_mill_damage_body("Count(PreviousMillManaValueSum)", "Some(\"milled\")");
        let intervening_operation = valid.replace(
            "(id: \"damage\"",
            "(id: \"intervening_life\", bind: [], input: None, kind: Perform(GainLife(amount: 1))),\n                        (id: \"damage\"",
        );
        let error = compile(&intervening_operation, damage_target_binding())
            .expect_err("an intervening operation breaks the immediate Mill contract");
        assert!(
            error.contains("immediately preceding Controller Mill"),
            "{error}"
        );

        let older_named_slot = valid.replace(
            "(id: \"damage\"",
            "(id: \"second_mill\", bind: [\"recent\"], input: None, kind: Perform(Mill(count: 1, who: Controller))),\n                        (id: \"damage\"",
        );
        let error = compile(&older_named_slot, damage_target_binding())
            .expect_err("the consumer cannot select an older Mill slot");
        assert!(
            error.contains("result bound by the immediately preceding Controller Mill"),
            "{error}"
        );

        let inherited_branch_slot = r#"(steps: [
            (id: "outer_mill", bind: ["milled"], input: None,
                kind: Perform(Mill(count: 3, who: Controller))),
            (id: "choose", bind: [], input: None, kind: Choose(
                choice: "draw_or_mill", chooser: TargetedPlayer(
                    group_index: 0, kind: OpponentPlayer,
                ), optional: true,
                branches: [(
                    branch_id: "draw", presentation: Fallback,
                    cost: None, requirement: Always,
                    body: (steps: [
                        (id: "draw_three", bind: [], input: None,
                            kind: Perform(Draw(who: Controller, count: 3))),
                    ], yields: []),
                )],
                otherwise: (steps: [
                    (id: "damage", bind: [], input: Some("milled"),
                        kind: Perform(DamagePlayer(
                            amount: Count(PreviousMillManaValueSum),
                            who: TargetedPlayer(group_index: 0, kind: OpponentPlayer),
                        ))),
                ], yields: []),
            )),
        ], yields: [])"#;
        let error = compile(inherited_branch_slot, damage_target_binding())
            .expect_err("a branch cannot consume an inherited older Mill result");
        assert!(error.contains("same program block"), "{error}");
    }

    #[test]
    fn program_compiler_limits_mill_sum_input_to_controller_mill_targeted_damage() {
        let valid =
            targeted_mill_damage_body("Count(PreviousMillManaValueSum)", "Some(\"milled\")");
        let wrong_action = valid.replace(
            "kind: Perform(Mill(count: 3, who: Controller))",
            "kind: Perform(Draw(who: Controller, count: 3))",
        );
        let error = compile(&wrong_action, damage_target_binding())
            .expect_err("a non-Mill result cannot supply PreviousMillManaValueSum");
        assert!(error.contains("wrong type for this instruction"), "{error}");

        let wrong_mill_recipient = valid.replace(
            "kind: Perform(Mill(count: 3, who: Controller))",
            "kind: Perform(Mill(count: 3, who: AffectedPlayer))",
        );
        let error = compile(&wrong_mill_recipient, damage_target_binding())
            .expect_err("only the controller's Mill cohort supplies this amount");
        assert!(error.contains("wrong type for this instruction"), "{error}");

        let wrong_damage_recipient = valid.replace(
            "who: TargetedPlayer(group_index: 0, kind: OpponentPlayer),",
            "who: Controller,",
        );
        let error = compile(&wrong_damage_recipient, damage_target_binding())
            .expect_err("the typed input is limited to targeted-opponent DamagePlayer");
        assert!(error.contains("does not accept an input result"), "{error}");

        let ambient_reader = valid.replace(
            "(id: \"damage\"",
            "(id: \"ambient_gain\", bind: [], input: None, kind: Perform(GainLife(amount: Count(PreviousMillManaValueSum)))),\n                        (id: \"damage\"",
        );
        let error = compile(&ambient_reader, damage_target_binding())
            .expect_err("other Program operations cannot read the implicit Mill sum");
        assert!(
            error.contains("immediate controller Mill result"),
            "{error}"
        );
    }

    #[test]
    fn program_compiler_rejects_invalid_gearhulk_target_group_contracts() {
        let valid_body = targeted_choose_damage_body(0, "OpponentPlayer", 0);
        let valid_binding = damage_target_binding();
        let invalid_role = vec![target_binding("damage", ProgramTargetRole::Spell, 0)];
        let unknown_group = vec![target_binding("damage", ProgramTargetRole::Target, 1)];
        let extra_damage_binding = vec![
            target_binding("damage", ProgramTargetRole::Target, 0),
            target_binding("choose", ProgramTargetRole::Target, 0),
        ];
        let mismatched_group_body = targeted_choose_damage_body(1, "OpponentPlayer", 1);
        let wrong_kind_body = targeted_choose_damage_body(0, "AnyPlayer", 0);
        let unpaired_damage_body = targeted_choose_damage_body(0, "OpponentPlayer", 1);
        let damage_without_chooser_body = r#"(steps: [
            (id: "damage", bind: [], input: None,
                kind: Perform(DamagePlayer(amount: 2, who: TargetedPlayer(
                    group_index: 0, kind: OpponentPlayer,
                )))),
        ], yields: [])"#;

        let mut wrong_group_chooser = opponent_targeting();
        wrong_group_chooser.groups[0].chooser = TargetChooser::ChosenOpponent;
        let mut extra_group = opponent_targeting();
        extra_group.groups.push(extra_group.groups[0].clone());
        let mut distinct_target_group = opponent_targeting();
        distinct_target_group.groups[0].distinct_from.push(1);
        let mut same_graveyard_target_group = opponent_targeting();
        same_graveyard_target_group.groups[0].same_graveyard = true;
        let mut cast_cost_expansion_target_group = opponent_targeting();
        cast_cost_expansion_target_group.groups[0].cast_cost_expansion =
            Some(crate::primitives::CastCostTargetExpansion {
                condition: crate::primitives::CastCostReceiptCondition {
                    group_id: crate::ChoiceId::new("cost_group").unwrap(),
                    option_id: crate::ChoiceId::new("cost_option").unwrap(),
                    expected_selected: true,
                },
                without_cost: TargetFilter::default(),
            });

        let cases = [
            (
                "missing target-role binding",
                valid_body.as_str(),
                Vec::new(),
                opponent_targeting(),
            ),
            (
                "duplicate target-role binding",
                valid_body.as_str(),
                vec![valid_binding[0].clone(), valid_binding[0].clone()],
                opponent_targeting(),
            ),
            (
                "wrong target-role kind",
                valid_body.as_str(),
                invalid_role,
                opponent_targeting(),
            ),
            (
                "unknown target group",
                valid_body.as_str(),
                unknown_group,
                opponent_targeting(),
            ),
            (
                "unsupported player target kind",
                wrong_kind_body.as_str(),
                valid_binding.clone(),
                opponent_targeting(),
            ),
            (
                "plural player target group",
                valid_body.as_str(),
                valid_binding.clone(),
                targeting_with_bounds(1, 2),
            ),
            (
                "extra Choose target binding",
                valid_body.as_str(),
                extra_damage_binding,
                opponent_targeting(),
            ),
            (
                "extra target group",
                valid_body.as_str(),
                valid_binding.clone(),
                extra_group,
            ),
            (
                "distinct target groups",
                valid_body.as_str(),
                valid_binding.clone(),
                distinct_target_group,
            ),
            (
                "same graveyard target constraint",
                valid_body.as_str(),
                valid_binding.clone(),
                same_graveyard_target_group,
            ),
            (
                "cast-cost target expansion",
                valid_body.as_str(),
                valid_binding.clone(),
                cast_cost_expansion_target_group,
            ),
            (
                "chooser and bound group mismatch",
                mismatched_group_body.as_str(),
                valid_binding.clone(),
                opponent_targeting(),
            ),
            (
                "unpaired targeted DamagePlayer",
                unpaired_damage_body.as_str(),
                valid_binding.clone(),
                opponent_targeting(),
            ),
            (
                "targeted DamagePlayer without a matching chooser",
                damage_without_chooser_body,
                valid_binding.clone(),
                opponent_targeting(),
            ),
            (
                "opponent-selected rather than controller-selected target group",
                valid_body.as_str(),
                valid_binding,
                wrong_group_chooser,
            ),
        ];

        for (label, body, bindings, targeting) in cases {
            let result = compile_with_targeting(body, bindings, &targeting);
            assert!(result.is_err(), "the Program compiler admitted {label}");
        }

        let zero_damage_body =
            targeted_choose_damage_body(0, "OpponentPlayer", 0).replace("amount: 2", "amount: 0");
        let zero_damage_error = compile(&zero_damage_body, damage_target_binding())
            .expect_err("TargetedPlayer damage must have a positive amount");
        assert!(
            zero_damage_error.contains("positive fixed amount"),
            "{zero_damage_error}"
        );
    }
}
