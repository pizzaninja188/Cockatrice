//! Actual cast-color, entry-copy and mana-ability coverage for Pentad Prism.
use super::helpers::*;
use tricerules_cards::{
    AbilityCost, AbilityPresentation, AbilitySourceZone, Amount, CountExpression, CounterKind,
    ManaAmount, SpellEffectKind,
};
use tricerules_core::{GameEngine, Zone};
use tricerules_proto::ruled::v1::{
    cost_selection::Selection, ruled_command::Cmd, CounterRemovalSelection, PaymentMana,
    PreviewPayment,
};

fn setup(seed: u64) -> GameEngine {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        Some(vec![deck_with("island", &[]), deck_with("forest", &[])]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn paid_cast(engine: &mut GameEngine, card: &str, mana: PaymentMana) -> u32 {
    let source = inject_card_into_hand(engine, 0, card);
    give_mana(
        engine,
        0,
        ManaGift {
            w: mana.w,
            u: mana.u,
            b: mana.b,
            r: mana.r,
            g: mana.g,
            c: mana.c,
        },
    );
    let mut command = cast_spell(hand_index_for_card(engine, 0, card), vec![]);
    let Some(Cmd::CastSpell(cast)) = command.cmd.as_mut() else {
        unreachable!()
    };
    let preview = engine.preview_payment(
        0,
        &PreviewPayment {
            cast_spell: Some(cast.clone()),
            ..Default::default()
        },
    );
    assert!(preview.valid, "{preview:?}");
    let mut selection = preview.selection.unwrap();
    selection.mana = Some(mana);
    cast.payment = Some(selection);
    semantic::accepted(engine, 0, &command);
    source
}

fn finish(engine: &mut GameEngine) {
    semantic::complete(engine, 12, |_| None).require_exercised();
}

fn illegal(engine: &mut GameEngine, player: i32, command: &RuledCommand) {
    let before = format!("{:?}", engine.state);
    assert!(engine.apply_command(player, command).is_err());
    assert_eq!(
        format!("{:?}", engine.state),
        before,
        "rejection must be atomic"
    );
}

fn object_ref(engine: &GameEngine, object_id: u32) -> tricerules_proto::ruled::v1::CostObjectRef {
    tricerules_proto::ruled::v1::CostObjectRef {
        object_id,
        zone_change_generation: semantic::generation(engine, object_id),
    }
}

fn activation(engine: &mut GameEngine, source: u32, option: u32) -> RuledCommand {
    let batch = engine.initial_response_batch();
    let choice =
        &batch.legal_by_player[&0].cost_choices_by_ability[&(u64::from(source) << 32)].choices[0];
    let removal = choice.counter_removal.as_ref().unwrap();
    assert_eq!(removal.count, 1);
    assert_eq!(removal.options.len(), 1);
    assert_eq!(removal.options[0].label, "charge");
    let mut command = activate_ability_for(engine, source, 0, vec![]);
    let Some(Cmd::ActivateAbility(ability)) = command.cmd.as_mut() else {
        unreachable!()
    };
    ability.mana_option_index = option;
    ability.cost_selections = vec![CostSelection {
        cost_index: choice.cost_index,
        selection: Some(Selection::CounterRemoval(CounterRemovalSelection {
            source: removal.source,
            option_id: removal.options[0].option_id,
        })),
    }];
    command
}

#[test]
fn pentad_prism_complete_registry_identity_exists() {
    let card = tricerules_cards::registry::global()
        .get("pentad_prism")
        .expect("complete Prism definition");
    assert_eq!(card.name, "Pentad Prism");
    assert_eq!(card.face_count(), 1);
    assert_eq!(card.primary_face().mana_cost.to_string(), "{2}");
    assert_eq!(card.primary_face().types, ["Artifact"]);
    assert_eq!(card.primary_face().static_abilities.len(), 1);
    assert_eq!(card.primary_face().activated_abilities.len(), 1);
    let face = card.primary_face();
    assert!(face.supertypes.is_empty());
    assert!(face.colors().is_empty());
    assert!(face.power.is_none() && face.toughness.is_none());
    let entry = &face.static_abilities[0];
    assert_eq!(entry.ability_id.as_str(), "static_01");
    assert_eq!(
        entry.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert!(matches!(
        &entry.definition,
        tricerules_cards::primitives::StaticAbilityDef::EntersWithCounters {
            affected: tricerules_cards::primitives::EntersWithCountersAffected::Self_,
            counter: CounterKind::Charge,
            amount: Amount::Count(CountExpression::ManaColorsSpentToCast),
            cast_cost_condition: None,
        }
    ));
    let ability = &face.activated_abilities[0];
    assert_eq!(ability.ability_id.as_str(), "activated_01");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(ability.source_zone, AbilitySourceZone::Battlefield);
    assert_eq!(
        ability.costs,
        vec![AbilityCost::RemoveCounters {
            counter: Some(CounterKind::Charge),
            count: 1,
            payment_source: Default::default()
        }]
    );
    assert_eq!(
        ability.effect,
        vec![SpellEffectKind::ProduceMana {
            commander_color_identity: false,
            restriction: None,
            conditional: None,
            options: vec![
                ManaAmount {
                    w: 1,
                    ..Default::default()
                },
                ManaAmount {
                    u: 1,
                    ..Default::default()
                },
                ManaAmount {
                    b: 1,
                    ..Default::default()
                },
                ManaAmount {
                    r: 1,
                    ..Default::default()
                },
                ManaAmount {
                    g: 1,
                    ..Default::default()
                },
            ]
        }]
    );
}

#[test]
fn pentad_prism_counts_actual_distinct_colors_and_excludes_unused_pool() {
    for (i, mana, expected) in [
        (
            PaymentMana {
                w: 1,
                u: 1,
                ..Default::default()
            },
            2,
        ),
        (
            PaymentMana {
                w: 2,
                ..Default::default()
            },
            1,
        ),
        (
            PaymentMana {
                w: 1,
                c: 1,
                ..Default::default()
            },
            1,
        ),
        (
            PaymentMana {
                c: 2,
                ..Default::default()
            },
            0,
        ),
    ]
    .into_iter()
    .enumerate()
    .map(|(i, (m, e))| (i, m, e))
    {
        let mut engine = setup(202_610_200 + i as u64);
        give_mana(
            &mut engine,
            0,
            ManaGift {
                b: 2,
                r: 1,
                g: 3,
                ..Default::default()
            },
        );
        let source = paid_cast(&mut engine, "pentad_prism", mana);
        finish(&mut engine);
        assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
        assert_eq!(
            engine.state.objects[&source].counter_count(CounterKind::Charge),
            expected
        );
        let pool = &engine.state.players[0].mana_pool;
        assert_eq!((pool.black, pool.red, pool.green), (2, 1, 3));
    }
}

#[test]
fn pentad_prism_each_color_is_immediate_and_zero_charge_is_unusable() {
    for option in 0..5 {
        let mut engine = setup(202_610_210 + option as u64);
        let source = paid_cast(
            &mut engine,
            "pentad_prism",
            PaymentMana {
                w: 1,
                u: 1,
                ..Default::default()
            },
        );
        finish(&mut engine);
        let command = activation(&mut engine, source, option);
        illegal(&mut engine, 1, &command);
        let mut bad = command.clone();
        let Some(Cmd::ActivateAbility(ability)) = bad.cmd.as_mut() else {
            unreachable!()
        };
        ability.mana_option_index = 5;
        illegal(&mut engine, 0, &bad);
        semantic::accepted(&mut engine, 0, &command);
        assert_eq!(
            engine.state.objects[&source].counter_count(CounterKind::Charge),
            1
        );
        assert!(engine.state.stack.is_empty());
        semantic::accepted(&mut engine, 0, &command);
        assert_eq!(
            engine.state.objects[&source].counter_count(CounterKind::Charge),
            0
        );
        assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
        assert!(!engine.state.objects[&source].tapped);
        assert!(engine.state.stack.is_empty());
        let pool = &engine.state.players[0].mana_pool;
        let actual = [
            pool.white,
            pool.blue,
            pool.black,
            pool.red,
            pool.green,
            pool.colorless,
        ];
        let mut expected = [0; 6];
        expected[option as usize] = 2;
        assert_eq!(actual, expected);
        illegal(&mut engine, 0, &command);
    }
}

#[test]
fn pentad_prism_paid_entry_copies_use_their_own_colors_through_parked_choices() {
    for (i, card, mana, expected) in [
        (
            "sculpting_steel",
            PaymentMana {
                w: 1,
                u: 1,
                g: 1,
                ..Default::default()
            },
            3,
        ),
        (
            "mirrormade",
            PaymentMana {
                u: 2,
                g: 1,
                ..Default::default()
            },
            2,
        ),
    ]
    .into_iter()
    .enumerate()
    .map(|(i, (c, m, e))| (i, c, m, e))
    {
        let mut engine = setup(202_610_220 + i as u64);
        let model = paid_cast(
            &mut engine,
            "pentad_prism",
            PaymentMana {
                w: 1,
                u: 1,
                ..Default::default()
            },
        );
        finish(&mut engine);
        for _ in 0..2 {
            let command = activation(&mut engine, model, 4);
            semantic::accepted(&mut engine, 0, &command);
        }
        assert_eq!(
            engine.state.objects[&model].counter_count(CounterKind::Charge),
            0
        );
        let copy = paid_cast(&mut engine, card, mana);
        assert_eq!(
            engine
                .state
                .stack
                .last()
                .unwrap()
                .mana_colors_spent_to_cast
                .count(),
            expected
        );
        pass_both_players(&mut engine);
        let pending = engine.state.pending_resolution.as_ref().unwrap();
        assert!(pending.presentation.candidates.contains(&model));
        let command = submit_resolution_choice(vec![model]);
        illegal(&mut engine, 1, &command);
        illegal(&mut engine, 0, &submit_resolution_choice(vec![999_999]));
        semantic::accepted(&mut engine, 0, &command);
        finish(&mut engine);
        assert_eq!(engine.state.objects[&copy].zone, Zone::Battlefield);
        assert_eq!(
            engine.characteristics(copy).unwrap().names,
            ["Pentad Prism"]
        );
        assert_eq!(
            engine.state.objects[&copy].counter_count(CounterKind::Charge),
            expected
        );
        assert_eq!(
            engine.state.objects[&model].counter_count(CounterKind::Charge),
            0
        );
    }
}

#[test]
fn pentad_prism_restricted_blue_and_ordinary_white_both_count() {
    let mut engine = setup(202_610_223);
    let helper = inject_permanent_on_battlefield(&mut engine, 0, "hydraulic_helper");
    let command = activate_ability_for(&engine, helper, 0, vec![]);
    semantic::accepted(&mut engine, 0, &command);
    let group = engine.state.players[0]
        .restricted_mana
        .last()
        .unwrap()
        .restriction_group_id;
    assert_eq!(engine.state.players[0].mana_pool.blue, 0);
    let prism = inject_card_into_hand(&mut engine, 0, "pentad_prism");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            ..Default::default()
        },
    );
    let mut command = cast_spell(hand_index_for_card(&engine, 0, "pentad_prism"), vec![]);
    let Some(Cmd::CastSpell(cast)) = command.cmd.as_mut() else {
        unreachable!()
    };
    cast.restricted_mana.push(ManaSpendSelection {
        restriction_group_id: group,
        u: 1,
        ..Default::default()
    });
    semantic::accepted(&mut engine, 0, &command);
    assert_eq!(
        engine
            .state
            .stack
            .last()
            .unwrap()
            .mana_colors_spent_to_cast
            .count(),
        2
    );
    assert!(engine.state.players[0].restricted_mana.is_empty());
    assert_eq!(engine.state.players[0].mana_pool.white, 0);
    finish(&mut engine);
    assert_eq!(
        engine.state.objects[&prism].counter_count(CounterKind::Charge),
        2
    );
}

#[test]
fn pentad_prism_reductions_count_only_actual_payment_and_free_cast_counts_zero() {
    for reducers in 1..=2 {
        let mut engine = setup(202_610_224 + reducers);
        for _ in 0..reducers {
            inject_permanent_on_battlefield(&mut engine, 0, "voyager_quickwelder");
        }
        let prism = paid_cast(
            &mut engine,
            "pentad_prism",
            PaymentMana {
                g: 2 - reducers as u32,
                ..Default::default()
            },
        );
        finish(&mut engine);
        assert_eq!(
            engine.state.objects[&prism].counter_count(CounterKind::Charge),
            2 - reducers as u32
        );
    }
}

#[test]
fn pentad_prism_noncast_reanimation_does_not_inherit_resolving_spells_three_colors() {
    let mut engine = setup(202_610_227);
    let prism = inject_graveyard_card(&mut engine, 0, "pentad_prism");
    let fodder = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
    let trash = inject_card_into_hand(&mut engine, 0, "trash_for_treasure");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            u: 1,
            r: 1,
            ..Default::default()
        },
    );
    let command = cast_spell_with_costs(
        hand_index_for_card(&engine, 0, "trash_for_treasure"),
        vec![TargetRef {
            kind: TargetRefKind::Graveyard as i32,
            object_id: prism,
            group_index: 0,
            ..Default::default()
        }],
        vec![permanent_cost_selection(0, fodder)],
    );
    semantic::accepted(&mut engine, 0, &command);
    assert_eq!(
        engine
            .state
            .stack
            .last()
            .unwrap()
            .mana_colors_spent_to_cast
            .count(),
        3
    );
    finish(&mut engine);
    assert_eq!(engine.state.objects[&prism].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.objects[&prism].counter_count(CounterKind::Charge),
        0
    );
    assert_eq!(engine.state.objects[&trash].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&fodder].zone, Zone::Graveyard);
}

fn exile_cast(engine: &mut GameEngine, source: u32) -> RuledCommand {
    let batch = engine.initial_response_batch();
    let action = batch.legal_by_player[&0]
        .zone_cast_actions
        .iter()
        .find(|a| a.object_id == source)
        .unwrap();
    RuledCommand {
        cmd: Some(Cmd::CastSpell(tricerules_proto::ruled::v1::CastSpell {
            source: Some(exile_cast_source(source, action.zone_change_generation)),
            face_index: action.face_index,
            casting_permission_id: action.casting_permission_id,
            cast_method: action.cast_method,
            ..Default::default()
        })),
    }
}

#[test]
fn pentad_prism_palette_preparation_is_paid_but_its_observer_does_not_match_artifacts() {
    let mut engine = setup(202_610_228);
    inject_card_into_hand(&mut engine, 0, "pigment_wrangler_striking_palette");
    let wrangler = move_ready_to_battlefield(&mut engine, 0, "pigment_wrangler_striking_palette");
    let palette = engine.state.prepared_permanents[&wrangler];
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let command = exile_cast(&mut engine, palette);
    semantic::accepted(&mut engine, 0, &command);
    assert!(
        engine.state.stack.last().unwrap().is_copy,
        "preparation copy is really cast"
    );
    assert_eq!(
        engine
            .state
            .stack
            .last()
            .unwrap()
            .mana_colors_spent_to_cast
            .count(),
        1
    );
    finish(&mut engine);
    assert_eq!(engine.state.active_event_observers.len(), 1);
    let prism = paid_cast(
        &mut engine,
        "pentad_prism",
        PaymentMana {
            w: 1,
            u: 1,
            ..Default::default()
        },
    );
    assert_eq!(
        engine.state.stack.len(),
        1,
        "Palette cannot copy artifact spells"
    );
    assert_eq!(engine.state.stack.last().unwrap().id, prism);
    finish(&mut engine);
    assert_eq!(
        engine.state.objects[&prism].counter_count(CounterKind::Charge),
        2
    );
    assert_eq!(engine.state.active_event_observers.len(), 1);
    assert!(engine.state.captured_spell_copies.is_empty());
}

#[test]
fn pentad_prism_airbend_alternative_recast_resets_identity_and_uses_actual_two_colors() {
    let mut engine = setup(202_610_229);
    let model = paid_cast(
        &mut engine,
        "pentad_prism",
        PaymentMana {
            c: 2,
            ..Default::default()
        },
    );
    finish(&mut engine);
    let steel = paid_cast(
        &mut engine,
        "sculpting_steel",
        PaymentMana {
            c: 3,
            ..Default::default()
        },
    );
    pass_both_players(&mut engine);
    semantic::accepted(&mut engine, 0, &submit_resolution_choice(vec![model]));
    finish(&mut engine);
    let old_generation = semantic::generation(&engine, steel);
    inject_card_into_hand(&mut engine, 0, "airbending_lesson");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            c: 2,
            ..Default::default()
        },
    );
    let command = cast_spell(
        hand_index_for_card(&engine, 0, "airbending_lesson"),
        target_object(steel),
    );
    semantic::accepted(&mut engine, 0, &command);
    finish(&mut engine);
    assert_eq!(engine.state.objects[&steel].zone, Zone::Exile);
    assert_eq!(
        engine.state.objects[&steel].counter_count(CounterKind::Charge),
        0
    );
    assert!(semantic::generation(&engine, steel) > old_generation);
    assert!(engine.state.objects[&steel].copiable_values.is_none());
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            g: 1,
            ..Default::default()
        },
    );
    let command = exile_cast(&mut engine, steel);
    semantic::accepted(&mut engine, 0, &command);
    assert_eq!(
        engine
            .state
            .stack
            .last()
            .unwrap()
            .mana_colors_spent_to_cast
            .count(),
        2
    );
    pass_both_players(&mut engine);
    semantic::accepted(&mut engine, 0, &submit_resolution_choice(vec![model]));
    finish(&mut engine);
    assert_eq!(engine.state.objects[&steel].zone, Zone::Battlefield);
    assert_eq!(
        engine.characteristics(steel).unwrap().names,
        ["Pentad Prism"]
    );
    assert_eq!(
        engine.state.objects[&steel].counter_count(CounterKind::Charge),
        2
    );
}

#[test]
fn pentad_prism_dev_entry_is_zero_and_convoke_taps_are_not_mana_colors() {
    let mut engine = setup(202_610_230);
    inject_card_into_hand(&mut engine, 0, "pentad_prism");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            u: 1,
            ..Default::default()
        },
    );
    let prism = move_ready_to_battlefield(&mut engine, 0, "pentad_prism");
    assert_eq!(
        engine.state.objects[&prism].counter_count(CounterKind::Charge),
        0
    );
    assert_eq!(
        (
            engine.state.players[0].mana_pool.white,
            engine.state.players[0].mana_pool.blue
        ),
        (1, 1)
    );
    let command = activate_ability_for(&engine, prism, 0, vec![]);
    illegal(&mut engine, 0, &command);

    let mut engine = setup(202_610_231);
    let contributions = ["serra_angel", "air_elemental", "grizzly_bears"]
        .map(|card| inject_creature_on_battlefield(&mut engine, 0, card));
    let target = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let spell = inject_card_into_hand(&mut engine, 0, "vote_out");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 1,
            ..Default::default()
        },
    );
    let mut command = cast_spell(
        hand_index_for_card(&engine, 0, "vote_out"),
        target_object(target),
    );
    let Some(Cmd::CastSpell(cast)) = command.cmd.as_mut() else {
        unreachable!()
    };
    cast.payment = Some(tricerules_proto::ruled::v1::PaymentSelection {
        expected_state_revision: engine.state.command_index,
        source: Some(object_ref(&engine, spell)),
        convoke: contributions
            .iter()
            .map(
                |&source| tricerules_proto::ruled::v1::ObjectPaymentContribution {
                    object: Some(object_ref(&engine, source)),
                    kind: tricerules_proto::ruled::v1::ObjectPaymentKind::Generic as i32,
                },
            )
            .collect(),
        mana: Some(PaymentMana {
            b: 1,
            ..Default::default()
        }),
        ..Default::default()
    });
    semantic::accepted(&mut engine, 0, &command);
    assert_eq!(
        engine
            .state
            .stack
            .last()
            .unwrap()
            .mana_colors_spent_to_cast
            .count(),
        1,
        "W/U/G creatures are nonmana payments"
    );
    assert!(contributions
        .iter()
        .all(|id| engine.state.objects[id].tapped));
    finish(&mut engine);
    assert_eq!(engine.state.objects[&target].zone, Zone::Graveyard);
}

#[test]
fn pentad_prism_clone_excludes_unanimated_artifact_and_animation_retains_charge_kind() {
    use tricerules_cards::primitives::{
        ContinuousEffectKind, EffectDuration, PermanentTypeFilter, TargetFilter, TargetKind,
        TypeLineAddition,
    };
    use tricerules_core::state::{AffectedScope, ContinuousEffect};
    let mut engine = setup(202_610_232);
    let prism = paid_cast(
        &mut engine,
        "pentad_prism",
        PaymentMana {
            c: 2,
            ..Default::default()
        },
    );
    finish(&mut engine);
    let bear = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let clone = paid_cast(
        &mut engine,
        "clone",
        PaymentMana {
            u: 1,
            c: 3,
            ..Default::default()
        },
    );
    pass_both_players(&mut engine);
    let candidates = &engine
        .state
        .pending_resolution
        .as_ref()
        .unwrap()
        .presentation
        .candidates;
    assert!(candidates.contains(&bear));
    assert!(!candidates.contains(&prism));
    semantic::accepted(&mut engine, 0, &submit_resolution_choice(vec![bear]));
    finish(&mut engine);
    assert_eq!(
        engine.characteristics(clone).unwrap().names,
        ["Grizzly Bears"]
    );
    for kind in [
        ContinuousEffectKind::Layer4AddTypes(TypeLineAddition {
            land_types: vec![],
            card_types: vec![PermanentTypeFilter::Creature],
            creature_types: vec![],
        }),
        ContinuousEffectKind::Layer7bSetPt {
            power: 4,
            toughness: 4,
        },
    ] {
        engine.state.continuous_effects.push(ContinuousEffect {
            trigger_grant_origin: None,
            source_id: None,
            affected: AffectedScope::PermanentsMatching {
                reference_player: 0,
                filter: Box::new(TargetFilter {
                    kind: TargetKind::AnyPermanent,
                    permanent_types: vec![PermanentTypeFilter::Artifact],
                    ..Default::default()
                }),
                exclude: None,
            },
            kind,
            condition: None,
            duration: EffectDuration::Indefinite,
            timestamp: engine.state.command_index,
        });
    }
    assert!(engine.characteristics(prism).unwrap().has_type("Creature"));
    let steel = paid_cast(
        &mut engine,
        "sculpting_steel",
        PaymentMana {
            w: 1,
            u: 1,
            g: 1,
            ..Default::default()
        },
    );
    pass_both_players(&mut engine);
    semantic::accepted(&mut engine, 0, &submit_resolution_choice(vec![prism]));
    finish(&mut engine);
    assert!(engine.characteristics(steel).unwrap().has_type("Creature"));
    assert_eq!(
        engine.state.objects[&steel].counter_count(CounterKind::Charge),
        3
    );
    assert_eq!(
        engine.state.objects[&steel].counter_count(CounterKind::PlusOnePlusOne),
        0
    );
}

#[test]
fn pentad_prism_serialized_paid_cast_and_parked_copy_choice_replay_exactly() {
    use prost::Message;
    fn fresh() -> (GameEngine, u32, u32) {
        let mut engine = setup(202_610_233);
        let prism = inject_permanent_on_battlefield(&mut engine, 0, "pentad_prism");
        let steel = inject_card_into_hand(&mut engine, 0, "sculpting_steel");
        give_mana(
            &mut engine,
            0,
            ManaGift {
                w: 1,
                u: 1,
                g: 1,
                ..Default::default()
            },
        );
        (engine, prism, steel)
    }
    let (mut engine, prism, steel) = fresh();
    let cast = cast_spell(hand_index_for_card(&engine, 0, "sculpting_steel"), vec![]);
    let mut recorded = Vec::new();
    for command in [cast, pass(), pass(), submit_resolution_choice(vec![prism])] {
        let actor = if engine.state.pending_resolution.is_some() {
            0
        } else {
            engine.state.priority_player_id()
        };
        let batch = semantic::accepted(&mut engine, actor, &command);
        recorded.push((actor, command, batch));
    }
    let (mut replay, ..) = fresh();
    for (actor, command, batch) in recorded {
        let decoded = RuledCommand::decode(command.encode_to_vec().as_slice()).unwrap();
        assert_eq!(replay.apply_command(actor, &decoded).unwrap(), batch);
    }
    assert_eq!(
        replay.diagnostic_snapshot().unwrap(),
        engine.diagnostic_snapshot().unwrap()
    );
    assert_eq!(
        engine.state.objects[&steel].counter_count(CounterKind::Charge),
        3
    );
    assert!(engine.state.stack.is_empty() && engine.state.pending_resolution.is_none());
}
