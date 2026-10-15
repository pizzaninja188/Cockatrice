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
fn return_other_linked_exiled_cards_requires_its_exact_imprint_producer_and_receipt() {
    let definition = |producer: &str, link: &str, previous: &str| {
        format!(
            r#"(
                id: "mimic_fixture",
                name: "Mimic Fixture",
                face_id: "mimic_fixture",
                types: ["Artifact"],
                triggered_abilities: [(
                    ability_id: "imprint",
                    presentation: Fallback,
                    trigger: WheneverCreatureDies(
                        controller: AnyPlayer,
                        filter: (permanent_type: Some(Creature), token: Some(false)),
                    ),
                    effect: [ChooseResolutionBranch(
                        chooser: Controller,
                        optional: true,
                        selection: PlayerChoice,
                        branches: [(
                            branch_id: "imprint_creature",
                            presentation: Fallback,
                            cost: None,
                            requirement: EffectsApplicable,
                            effects: [{previous}, ReturnOtherLinkedExiledCards(
                                producer_ability_id: "{producer}",
                                linked_exile_id: "{link}",
                            )],
                        )],
                    )],
                )],
            )"#
        )
    };

    let valid = definition(
        "imprint",
        "imprinted_card",
        "Exile(subject: TriggerObject, linked_exile_id: Some(\"imprinted_card\"))",
    );
    crate::CardRegistry::from_chunks_and_tokens(&[&valid], &[])
        .expect("the exact adjacent linked Exile can preserve the new imprint");

    for (producer, link, previous) in [
        (
            "missing_producer",
            "imprinted_card",
            "Exile(subject: TriggerObject, linked_exile_id: Some(\"imprinted_card\"))",
        ),
        (
            "imprint",
            "other_card",
            "Exile(subject: TriggerObject, linked_exile_id: Some(\"imprinted_card\"))",
        ),
        (
            "imprint",
            "imprinted_card",
            "Exile(subject: TriggerObject, linked_exile_id: None)",
        ),
    ] {
        let invalid = definition(producer, link, previous);
        assert!(
            crate::CardRegistry::from_chunks_and_tokens(&[&invalid], &[]).is_err(),
            "invalid return linkage must fail closed: {invalid}"
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
fn triggered_mana_output_detection_follows_resolution_choices() {
    let direct: super::SpellEffectKind =
        ron::from_str("AddMana(amount: (g: 1), retention: EndOfStep)").unwrap();
    assert!(direct.could_add_mana_to_pool());

    let conditional: super::SpellEffectKind = ron::from_str(
        "Conditional(condition: ActivePlayer(players: Controller), effect: AddMana(amount: (g: 1), retention: EndOfStep))",
    )
    .unwrap();
    assert!(conditional.could_add_mana_to_pool());

    let branch: super::SpellEffectKind = ron::from_str(
        "ChooseResolutionBranch(optional: false, branches: [(branch_id: \"add_mana\", presentation: Fallback, cost: None, effects: [AddMana(amount: (g: 1), retention: EndOfStep)])])",
    )
    .unwrap();
    assert!(branch.could_add_mana_to_pool());

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

fn mirrorworks_event_object_copy_branch() -> SpellEffectKind {
    SpellEffectKind::ChooseResolutionBranch {
        chooser: PlayerRecipient::Controller,
        optional: true,
        selection: ResolutionBranchSelection::PlayerChoice,
        branches: vec![ResolutionBranchDef {
            branch_id: crate::ChoiceId::new("pay_to_copy").unwrap(),
            presentation: AbilityPresentation::Fallback,
            runtime_fallback: None,
            cost: ResolutionCost::Mana(ManaCost::parse("{2}").unwrap()),
            requirement: ResolutionBranchRequirement::Always,
            effects: vec![SpellEffectKind::CreateTokenCopies {
                count: Amount::Fixed(1),
                source: TokenCopySource::TriggerObject,
            }],
        }],
        otherwise: vec![],
    }
}

#[test]
fn trigger_object_copy_is_nested_trigger_context_scoped() {
    let branch = mirrorworks_event_object_copy_branch();
    assert!(branch.uses_trigger_object_reference());
    assert!(branch
        .validate(EffectContext::Spell)
        .unwrap_err()
        .contains("trigger object"));

    let mut ability = TriggeredAbilityDef {
        ability_id: AbilityId::new("triggered_01").unwrap(),
        presentation: AbilityPresentation::Fallback,
        trigger: TriggerCondition::WhenSelfEntersBattlefield,
        effect: vec![branch.clone()],
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
        effect: vec![branch.clone()],
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
        effect: vec![branch],
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
fn first_applicable_resolution_branches_require_a_costless_final_fallback() {
    let conditional = ResolutionBranchDef {
        branch_id: crate::ChoiceId::new("branch_01").unwrap(),
        presentation: crate::AbilityPresentation::Fallback,
        runtime_fallback: None,
        cost: ResolutionCost::None,
        requirement: ResolutionBranchRequirement::GameCondition(
            GameCondition::CreatureDeathsThisTurn {
                min: Some(1),
                max: None,
            },
        ),
        effects: vec![SpellEffectKind::Draw {
            who: PlayerRecipient::Controller,
            count: Amount::Fixed(1),
        }],
    };
    let fallback = ResolutionBranchDef {
        branch_id: crate::ChoiceId::new("branch_02").unwrap(),
        presentation: crate::AbilityPresentation::Fallback,
        runtime_fallback: None,
        cost: ResolutionCost::None,
        requirement: ResolutionBranchRequirement::Always,
        effects: vec![SpellEffectKind::GainLife {
            amount: Amount::Fixed(1),
        }],
    };
    let automatic = |optional, chooser, branches| SpellEffectKind::ChooseResolutionBranch {
        chooser,
        optional,
        selection: ResolutionBranchSelection::FirstApplicable,
        branches,
        otherwise: Vec::new(),
    };

    assert!(automatic(
        false,
        PlayerRecipient::Controller,
        vec![conditional.clone(), fallback.clone()],
    )
    .validate(EffectContext::Ability)
    .is_ok());
    assert!(automatic(
        true,
        PlayerRecipient::Controller,
        vec![conditional.clone(), fallback.clone()],
    )
    .validate(EffectContext::Ability)
    .is_err());
    assert!(automatic(
        false,
        PlayerRecipient::SourceController,
        vec![conditional.clone(), fallback.clone()],
    )
    .validate(EffectContext::Ability)
    .is_err());
    assert!(automatic(
        false,
        PlayerRecipient::Controller,
        vec![conditional.clone()],
    )
    .validate(EffectContext::Ability)
    .is_err());

    let mut costed = conditional;
    costed.cost = ResolutionCost::DiscardCard { filter: None };
    assert!(
        automatic(false, PlayerRecipient::Controller, vec![costed, fallback],)
            .validate(EffectContext::Ability)
            .is_err()
    );
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
fn issue_241_pay_or_otherwise_supports_exact_delayed_objects() {
    let effects: Vec<SpellEffectKind> = ron::from_str(
        r#"[
          Exile(subject: Chosen((kind: Creature))),
          CreateDelayedTrigger(
            subject: Some(PreviousEffectObject),
            ability: (
              ability_id: "delayed_01",
              presentation: Fallback,
              trigger: AtBeginningOfNextEndStep,
              effect: [ChooseResolutionBranch(
                optional: true,
                branches: [(
                  branch_id: "pay",
                  presentation: Fallback,
                  cost: Mana("{3}{B}"),
                  effects: [],
                )],
                otherwise: [ReturnTriggeredCard(
                  reference: ExactTriggerObject,
                  from: [Exile],
                  controller: Owner,
                )],
              )],
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
fn issue_241_otherwise_requires_an_optional_resolution_choice() {
    let effect: SpellEffectKind = ron::from_str(
        r#"ChooseResolutionBranch(
          optional: false,
          branches: [(
            branch_id: "pay",
            presentation: Fallback,
            cost: Mana("{1}"),
            effects: [],
          )],
          otherwise: [GainLife(amount: 1)],
        )"#,
    )
    .expect("otherwise is typed authoring vocabulary");

    assert!(effect.validate(EffectContext::Ability).is_err());
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
fn nested_resolution_branch_rejects_unknown_target_controller_group() {
    let card = r#"(
      id: "test", name: "Test", face_id: "test", types: ["Instant"],
      spell_effect: [
        DamageTarget(amount: 1, target: (kind: Creature)),
        ChooseResolutionBranch(optional: true, branches: [(
          branch_id: "branch_01", presentation: Fallback, cost: None,
          effects: [DamagePlayer(amount: 1, who: ControllerOfTargetGroup(group_index: 1))],
        )]),
      ],
    )"#;
    assert!(crate::CardRegistry::from_chunks_and_tokens(&[card], &[]).is_err());
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

fn source_counter_scaled_mana_branch(effect: SpellEffectKind) -> SpellEffectKind {
    SpellEffectKind::ChooseResolutionBranch {
        chooser: PlayerRecipient::Controller,
        optional: false,
        selection: ResolutionBranchSelection::FirstApplicable,
        branches: vec![ResolutionBranchDef {
            branch_id: crate::ChoiceId::new("continue").unwrap(),
            presentation: crate::AbilityPresentation::Fallback,
            runtime_fallback: None,
            cost: ResolutionCost::None,
            requirement: ResolutionBranchRequirement::Always,
            effects: vec![effect],
        }],
        otherwise: vec![],
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
    nested.effect = vec![source_counter_scaled_mana_branch(
        opponent_land_mana_effect(),
    )];
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
        effect: vec![source_counter_scaled_mana_branch(
            source_counter_scaled_mana_effect(),
        )],
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
        effect: vec![source_counter_scaled_mana_branch(
            source_counter_scaled_mana_effect(),
        )],
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
    let condition = ron::from_str::<crate::primitives::ProgramResultCondition>(
        r#"CounterPayment(result: "payment", outcome: Paid)"#,
    );
    assert!(
        condition.is_ok(),
        "a fixed-cost counter result should expose its typed payment status"
    );
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
            "owner-local chosen target",
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
            spell_effect: [ChooseResolutionBranch(optional: false, branches: [(
                branch_id: "create", presentation: Fallback, cost: None,
                effects: [CreateReflexiveTrigger(ability: (
                    ability_id: "reflexive", presentation: Fallback,
                    effect: [GrantTriggeredAbility(subject: Source,
                        ability: (ability_id: "granted", presentation: Fallback,
                            trigger: WhenSelfDies, effect: [], program: {program},
                            modal: None, targeting: None))], targeting: None))],
            )])])"#
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
            reason.contains("If condition uses context outside controller hand and library state"),
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
