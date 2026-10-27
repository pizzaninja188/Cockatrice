//! Actual Mycosynth Lattice spending permission; mana is not recolored (CR 609.4b).
use super::helpers::*;
use crate::helpers::ScenarioCommandExt;
use tricerules_cards::{ContinuousEffectKind, EffectDuration};
use tricerules_core::{AffectedScope, ContinuousEffect};
use tricerules_proto::ruled::v1 as rv1;

fn game(seed: u64, players: &[i32]) -> GameEngine {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        players,
        20,
        None,
        true,
    )
    .expect("create game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn enter_lattice(engine: &mut GameEngine) -> u32 {
    let card = inject_card_into_hand(engine, 0, "mycosynth_lattice");
    assert_eq!(
        move_ready_to_battlefield(engine, 0, "mycosynth_lattice"),
        card
    );
    card
}

fn object_ref(engine: &GameEngine, oid: u32) -> rv1::CostObjectRef {
    rv1::CostObjectRef {
        object_id: oid,
        zone_change_generation: engine
            .state
            .zone_change_generation
            .get(&oid)
            .copied()
            .unwrap_or(0),
    }
}

fn convoke_draft(
    engine: &GameEngine,
    card: &str,
    creatures: &[u32],
    mana: rv1::PaymentMana,
) -> rv1::CastSpell {
    let hand = hand_index_for_card(engine, 0, card);
    rv1::CastSpell {
        cast_method: rv1::CastMethod::Normal as i32,
        source: Some(hand_cast_source(hand)),
        payment: Some(rv1::PaymentSelection {
            source: Some(object_ref(engine, engine.state.players[0].hand[hand])),
            expected_state_revision: engine.state.command_index,
            convoke: creatures
                .iter()
                .map(|&oid| rv1::ObjectPaymentContribution {
                    object: Some(object_ref(engine, oid)),
                    kind: rv1::ObjectPaymentKind::Generic as i32,
                })
                .collect(),
            mana: Some(mana),
            ..Default::default()
        }),
        ..Default::default()
    }
}

fn preview(engine: &GameEngine, cast: &rv1::CastSpell) -> rv1::PaymentPreview {
    engine.preview_payment_scenario(
        0,
        &rv1::PreviewPayment {
            transaction_id: 13,
            revision: 2,
            cast_spell: Some(cast.clone()),
            ..Default::default()
        },
    )
}

#[test]
fn lattice_all_four_players_can_spend_actual_blue_for_red_spell_costs() {
    let players = [4, 9, 17, 27];
    for (seat, player) in players.into_iter().enumerate() {
        for with_lattice in [false, true] {
            let mut engine = game(
                26_102_720 + seat as u64 * 2 + u64::from(with_lattice),
                &players,
            );
            if with_lattice {
                enter_lattice(&mut engine);
            }
            let bolt = inject_card_into_hand(&mut engine, seat, "lightning_bolt");
            give_mana(
                &mut engine,
                player,
                ManaGift {
                    u: 1,
                    ..Default::default()
                },
            );
            for prior in players.iter().take(seat) {
                engine
                    .apply_scenario_command(*prior, &pass())
                    .expect("pass priority in seat order");
            }
            let slot = engine.state.players[seat]
                .hand
                .iter()
                .position(|oid| *oid == bolt)
                .unwrap();
            let target = if player == 4 { 9 } else { 4 };
            let result =
                engine.apply_scenario_command(player, &cast_spell(slot, target_player(target)));
            if !with_lattice {
                assert!(
                    result.is_err(),
                    "off-color mana is insufficient without Lattice"
                );
                assert_eq!(
                    engine.state.objects[&bolt].zone,
                    tricerules_core::Zone::Hand
                );
                assert_eq!(engine.state.players[seat].mana_pool.blue, 1);
            } else {
                result.expect("every player can pay {R} with blue mana while Lattice is active");
                assert_eq!(
                    engine.state.objects[&bolt].zone,
                    tricerules_core::Zone::Stack
                );
                assert_eq!(engine.state.players[seat].mana_pool.blue, 0);
                assert_eq!(
                    serde_json::to_value(
                        engine.state.stack.last().unwrap().mana_colors_spent_to_cast
                    )
                    .unwrap(),
                    serde_json::json!([false, true, false, false, false]),
                    "the receipt records actual blue, not red"
                );
            }
        }
    }
}

#[test]
fn lattice_allows_actual_blue_mana_to_pay_an_activated_red_cost() {
    let mut engine = game(26_102_730, &[0, 1]);
    let hound = inject_creature_on_battlefield(&mut engine, 0, "fiery_hellhound");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    assert!(apply_ability(&mut engine, 0, hound, 0, vec![]).is_err());
    assert_eq!(engine.state.players[0].mana_pool.blue, 1);
    enter_lattice(&mut engine);
    apply_ability(&mut engine, 0, hound, 0, vec![])
        .expect("blue mana pays Fiery Hellhound's {R} activation");
    assert_eq!(engine.state.players[0].mana_pool.blue, 0);
    pass_both_players(&mut engine);
    assert_eq!(engine.characteristics(hound).unwrap().power, Some(3));
}

#[test]
fn lattice_convoke_uses_colorless_creatures_for_generic_and_actual_mana_for_residual() {
    for (index, card) in ["unexpected_assistance", "merrow_skyswimmer"]
        .into_iter()
        .enumerate()
    {
        let mut engine = game(26_102_731 + index as u64, &[0, 1]);
        inject_card_into_hand(&mut engine, 0, card);
        let creatures: Vec<_> = (0..3)
            .map(|_| inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears"))
            .collect();
        give_mana(
            &mut engine,
            0,
            ManaGift {
                r: 1,
                g: 1,
                ..Default::default()
            },
        );
        let mana = rv1::PaymentMana {
            r: 1,
            g: 1,
            ..Default::default()
        };
        let absent = preview(&engine, &convoke_draft(&engine, card, &creatures, mana));
        assert!(
            !absent.complete,
            "the same off-color pool is insufficient without Lattice"
        );
        for &creature in &creatures {
            assert!(!engine.state.objects[&creature].tapped);
        }
        enter_lattice(&mut engine);
        let empty = preview(
            &engine,
            &convoke_draft(&engine, card, &[], rv1::PaymentMana::default()),
        );
        let options = &empty
            .candidates
            .iter()
            .find(|candidate| {
                candidate
                    .object
                    .as_ref()
                    .is_some_and(|object| object.object_id == creatures[0])
            })
            .expect("colorless creature remains available for generic convoke")
            .options;
        assert_eq!(options, &[rv1::ObjectPaymentKind::Generic as i32]);

        let mut cast = convoke_draft(&engine, card, &creatures, mana);
        let offer = preview(&engine, &cast);
        assert!(offer.valid && offer.complete, "{card}: {offer:?}");
        let selected = offer.selection.expect("completed exact selection");
        assert_eq!(selected.convoke.len(), 3);
        let selected_mana = selected.mana.as_ref().expect("selected mana");
        assert_eq!((selected_mana.r, selected_mana.g), (1, 1));
        cast.payment = Some(selected);
        engine
            .apply_scenario_command(
                0,
                &RuledCommand {
                    cmd: Some(Cmd::CastSpell(cast)),
                },
            )
            .expect("commit the previewed mixed convoke and off-color mana");
        for &creature in &creatures {
            assert!(engine.state.objects[&creature].tapped);
        }
        assert_eq!(
            (
                engine.state.players[0].mana_pool.red,
                engine.state.players[0].mana_pool.green
            ),
            (0, 0)
        );
        assert_eq!(
            serde_json::to_value(engine.state.stack.last().unwrap().mana_colors_spent_to_cast)
                .unwrap(),
            serde_json::json!([false, false, false, true, true]),
            "the receipt records actual red and green"
        );
    }
}

#[test]
fn lattice_does_not_remove_hydraulic_helper_spending_restrictions() {
    let mut engine = game(26_102_733, &[0, 1]);
    enter_lattice(&mut engine);
    let helper = inject_creature_on_battlefield(&mut engine, 0, "hydraulic_helper");
    apply_ability(&mut engine, 0, helper, 0, vec![]).expect("produce restricted blue mana");
    let group = engine.state.players[0]
        .restricted_mana
        .last()
        .unwrap()
        .restriction_group_id;
    inject_card_into_hand(&mut engine, 0, "lightning_bolt");
    let mut forbidden = cast_spell(
        hand_index_for_card(&engine, 0, "lightning_bolt"),
        target_player(1),
    );
    let Some(Cmd::CastSpell(cast)) = forbidden.cmd.as_mut() else {
        unreachable!()
    };
    cast.restricted_mana.push(ManaSpendSelection {
        restriction_group_id: group,
        u: 1,
        ..Default::default()
    });
    assert!(
        engine.apply_scenario_command(0, &forbidden).is_err(),
        "nonartifact spell remains forbidden"
    );
    assert_eq!(engine.state.players[0].restricted_mana.len(), 1);

    inject_card_into_hand(&mut engine, 0, "wretched_doll");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    let mut artifact = cast_spell(hand_index_for_card(&engine, 0, "wretched_doll"), vec![]);
    let Some(Cmd::CastSpell(cast)) = artifact.cmd.as_mut() else {
        unreachable!()
    };
    cast.restricted_mana.push(ManaSpendSelection {
        restriction_group_id: group,
        u: 1,
        ..Default::default()
    });
    engine
        .apply_scenario_command(0, &artifact)
        .expect("restricted blue pays {B} on an artifact spell");
    assert!(engine.state.players[0].restricted_mana.is_empty());
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    assert_eq!(
        serde_json::to_value(engine.state.stack.last().unwrap().mana_colors_spent_to_cast).unwrap(),
        serde_json::json!([false, true, false, false, false])
    );
}

fn begin_lightning_bolt(engine: &mut GameEngine) -> u64 {
    let bolt = inject_card_into_hand(engine, 0, "lightning_bolt");
    let slot = engine.state.players[0]
        .hand
        .iter()
        .position(|oid| *oid == bolt)
        .unwrap();
    let Some(Cmd::CastSpell(cast)) = cast_spell(slot, target_player(1)).cmd else {
        unreachable!()
    };
    engine
        .apply_scenario_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::BeginSpellCast(rv1::BeginSpellCast {
                    announcement: Some(rv1::SpellCastAnnouncement {
                        source: cast.source,
                        targets: cast.targets,
                        face_index: cast.face_index,
                        cast_method: cast.cast_method,
                        ..Default::default()
                    }),
                })),
            },
        )
        .expect("begin casting Lightning Bolt");
    engine
        .state
        .pending_spell_cast
        .as_ref()
        .unwrap()
        .transaction_id
}

#[test]
fn lattice_staged_spell_payment_rechecks_after_ironworks_sacrifices_its_source() {
    for second_lattice in [false, true] {
        let mut engine = game(26_102_734 + u64::from(second_lattice), &[0, 1]);
        let lattice = enter_lattice(&mut engine);
        if second_lattice {
            enter_lattice(&mut engine);
        }
        let ironworks = inject_permanent_on_battlefield(&mut engine, 0, "krark-clan_ironworks");
        give_mana(
            &mut engine,
            0,
            ManaGift {
                u: 1,
                ..Default::default()
            },
        );
        let transaction_id = begin_lightning_bolt(&mut engine);
        let preview = engine.preview_payment_scenario(
            0,
            &rv1::PreviewPayment {
                commit_spell_cast: Some(rv1::CommitSpellCast {
                    transaction_id,
                    payment: Some(rv1::PaymentSelection {
                        mana: Some(rv1::PaymentMana {
                            u: 1,
                            ..Default::default()
                        }),
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
                ..Default::default()
            },
        );
        assert!(
            preview.valid && preview.complete,
            "blue payment was legal before the sacrifice: {preview:?}"
        );
        let mut selection = preview.selection.expect("exact blue selection");
        let sacrifice = activate_ability_with_costs(
            ironworks,
            0,
            vec![],
            vec![permanent_cost_selection(0, lattice)],
        );
        engine
            .apply_scenario_command(0, &sacrifice)
            .expect("sacrifice Lattice for mana during payment");
        assert_eq!(
            engine.state.objects[&lattice].zone,
            tricerules_core::Zone::Graveyard
        );
        assert_eq!(
            (
                engine.state.players[0].mana_pool.blue,
                engine.state.players[0].mana_pool.colorless
            ),
            (1, 2)
        );
        selection.expected_state_revision = engine.state.command_index;
        let commit = RuledCommand {
            cmd: Some(Cmd::CommitSpellCast(rv1::CommitSpellCast {
                transaction_id,
                payment: Some(selection),
                ..Default::default()
            })),
        };
        let result = engine.apply_scenario_command(0, &commit);
        if second_lattice {
            result.expect("another Lattice still permits the same blue payment");
            assert_eq!(engine.state.players[0].mana_pool.blue, 0);
            assert_eq!(engine.state.players[0].mana_pool.colorless, 2);
        } else {
            assert!(
                result.is_err(),
                "a stale off-color selection cannot commit after the last Lattice leaves"
            );
            assert_eq!(
                (
                    engine.state.players[0].mana_pool.blue,
                    engine.state.players[0].mana_pool.colorless
                ),
                (1, 2)
            );
            assert!(engine.state.pending_spell_cast.is_some());
        }
    }
}

fn parked_sting_slinger_case(allow_second_lattice: bool, off_color_plan: bool) {
    use tricerules_cards::CounterKind;
    let mut engine = game(
        26_102_736 + u64::from(allow_second_lattice) + 2 * u64::from(off_color_plan),
        &[0, 1, 2],
    );
    let departing_lattice = inject_card_into_hand(&mut engine, 2, "mycosynth_lattice");
    assert_eq!(
        move_ready_to_battlefield(&mut engine, 2, "mycosynth_lattice"),
        departing_lattice
    );
    if allow_second_lattice {
        enter_lattice(&mut engine);
    }
    inject_permanent_on_battlefield(&mut engine, 0, "laezel,_vlaakiths_champion");
    for _ in 0..3 {
        inject_permanent_on_battlefield(&mut engine, 0, "doubling_season");
    }
    let source = inject_creature_on_battlefield(&mut engine, 0, "sting-slinger");
    let recipient = inject_creature_with_stats(&mut engine, 0, "ancient_brontodon", 20, 20);
    // With Lattice the automatic solver spends both blue first and leaves red untouched.
    // The matched-color control has one blue and one red, naturally paying {1}{R}.
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: if off_color_plan { 2 } else { 1 },
            r: 1,
            ..Default::default()
        },
    );
    let original_blue = engine.state.players[0].mana_pool.blue;
    let command = activate_ability_with_costs(
        source,
        0,
        vec![],
        vec![rv1::CostSelection {
            cost_index: 2,
            selection: Some(rv1::cost_selection::Selection::BattlefieldObjects(
                rv1::CostObjectRefs {
                    objects: vec![object_ref(&engine, recipient)],
                },
            )),
        }],
    );
    engine
        .apply_scenario_command(0, &command)
        .expect("stage actual Sting-Slinger activation");
    assert!(
        engine.state.pending_resolution.is_some(),
        "counter replacement ordering parks the unpaid cost"
    );
    assert_eq!(
        (
            engine.state.players[0].mana_pool.blue,
            engine.state.players[0].mana_pool.red
        ),
        (original_blue, 1)
    );
    assert!(!engine.state.objects[&source].tapped);
    assert_eq!(
        engine.state.objects[&recipient].counter_count(CounterKind::MinusOneMinusOne),
        0
    );
    assert!(engine.state.stack.is_empty());
    engine
        .apply_scenario_command(2, &concede())
        .expect("Lattice owner leaves while the cost is parked");
    assert!(!engine.state.objects.contains_key(&departing_lattice));
    assert!(
        engine.state.pending_resolution.is_some(),
        "the live payer still orders replacement effects"
    );
    let mut failure = None;
    let mut cancellation_logged = false;
    for _ in 0..5 {
        let Some(pending) = engine.state.pending_resolution.as_ref() else {
            break;
        };
        assert_eq!(
            pending.presentation.choice_kind,
            ChoiceKind::ReplacementEffect
        );
        let selected = pending.presentation.candidates[0];
        match engine.apply_scenario_command(0, &submit_resolution_choice(vec![selected])) {
            Ok(batch) => cancellation_logged |= batch.events.iter().any(|event| {
                matches!(&event.ev, Some(Ev::Log(log)) if log.text.to_ascii_lowercase().contains("cancel"))
            }),
            Err(error) => { failure = Some(error); break; }
        }
    }
    let should_commit = allow_second_lattice || !off_color_plan;
    if should_commit {
        assert!(
            failure.is_none(),
            "the retained exact debit is still legal: {failure:?}"
        );
        assert!(engine.state.pending_resolution.is_none());
        assert!(engine.state.objects[&source].tapped);
        assert_eq!(
            engine.state.objects[&recipient].counter_count(CounterKind::MinusOneMinusOne),
            16
        );
        assert_eq!(engine.state.stack.len(), 1);
        assert_eq!(engine.state.players[0].mana_pool.blue, 0);
        assert_eq!(
            engine.state.players[0].mana_pool.red,
            u32::from(off_color_plan),
            "the second-Lattice control proves which exact debit was parked"
        );
    } else {
        assert!(failure.is_none(), "the valid final choice must accept and record the cancelled unpaid activation: {failure:?}");
        assert!(
            cancellation_logged,
            "the player is told the activation was cancelled"
        );
        assert_eq!(
            (
                engine.state.players[0].mana_pool.blue,
                engine.state.players[0].mana_pool.red
            ),
            (original_blue, 1)
        );
        assert!(!engine.state.objects[&source].tapped);
        assert_eq!(
            engine.state.objects[&recipient].counter_count(CounterKind::MinusOneMinusOne),
            0
        );
        assert!(engine.state.stack.is_empty());
        assert!(
            engine.state.pending_resolution.is_none(),
            "the rejected continuation is retired"
        );
        assert!(engine.state.pending_ability_activation.is_none());
        assert!(
            engine.state.blocking_choice().is_none(),
            "the rejected activation cannot strand a choice"
        );
        assert!(
            engine.diagnostic_snapshot().unwrap()["state"]["pending_replacement_event"].is_null()
        );
    }
}

#[test]
fn lattice_parked_exact_off_color_debit_is_cancelled_after_its_owner_leaves() {
    parked_sting_slinger_case(false, true);
}

#[test]
fn lattice_parked_natural_color_debit_survives_source_departure() {
    parked_sting_slinger_case(false, false);
}

#[test]
fn lattice_parked_off_color_debit_survives_with_another_lattice() {
    parked_sting_slinger_case(true, true);
}

#[test]
fn lattice_flame_javelin_mono_hybrid_generic_alternatives_reduce_before_payment() {
    let mut engine = game(26_102_740, &[0, 1]);
    enter_lattice(&mut engine);
    for _ in 0..4 {
        inject_creature_on_battlefield(&mut engine, 0, "mocking_sprite");
    }
    let javelin = inject_card_into_hand(&mut engine, 0, "flame_javelin");
    let slot = engine.state.players[0]
        .hand
        .iter()
        .position(|oid| *oid == javelin)
        .unwrap();
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 2,
            ..Default::default()
        },
    );
    let command = cast_spell(slot, target_player(1));
    let Some(Cmd::CastSpell(mut selected)) = command.cmd.clone() else {
        unreachable!()
    };
    selected.payment = Some(rv1::PaymentSelection {
        source: Some(object_ref(&engine, javelin)),
        expected_state_revision: engine.state.command_index,
        mana: Some(rv1::PaymentMana {
            u: 2,
            ..Default::default()
        }),
        ..Default::default()
    });
    let explicit = preview(&engine, &selected);
    assert!(explicit.valid && explicit.complete,
        "three generic alternatives {{6}}, reduced by four, admit exactly two blue mana: {explicit:?}");
    assert_eq!(explicit.selection.unwrap().mana.unwrap().u, 2);
    engine
        .apply_scenario_command(0, &command)
        .expect("automatic payment must agree with the explicit reduced mono-hybrid demand");
    // Automatic payment may choose two generic branches and one red branch: the four generic
    // mana is reduced away and the remaining red pip takes only one actual blue mana.
    assert_eq!(engine.state.players[0].mana_pool.blue, 1);
    assert_eq!(
        engine.state.objects[&javelin].zone,
        tricerules_core::Zone::Stack
    );
    assert_eq!(
        serde_json::to_value(engine.state.stack.last().unwrap().mana_colors_spent_to_cast).unwrap(),
        serde_json::json!([false, true, false, false, false])
    );
}

fn parked_replay_fixture() -> (GameEngine, u32, u32) {
    let mut engine = game(26_102_741, &[0, 1, 2]);
    let lattice = inject_card_into_hand(&mut engine, 2, "mycosynth_lattice");
    assert_eq!(
        move_ready_to_battlefield(&mut engine, 2, "mycosynth_lattice"),
        lattice
    );
    inject_permanent_on_battlefield(&mut engine, 0, "laezel,_vlaakiths_champion");
    for _ in 0..3 {
        inject_permanent_on_battlefield(&mut engine, 0, "doubling_season");
    }
    let source = inject_creature_on_battlefield(&mut engine, 0, "sting-slinger");
    let recipient = inject_creature_with_stats(&mut engine, 0, "ancient_brontodon", 20, 20);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 2,
            r: 1,
            ..Default::default()
        },
    );
    (engine, source, recipient)
}

#[test]
fn lattice_parked_payment_cancellation_replays_only_accepted_wire_commands() {
    use prost::Message;
    use tricerules_cards::CounterKind;

    fn record_accepted(
        engine: &mut GameEngine,
        actor: i32,
        command: RuledCommand,
        accepted: &mut Vec<(i32, Vec<u8>, RuledEventBatch)>,
    ) -> bool {
        let bytes = command.encode_to_vec();
        let decoded = RuledCommand::decode(bytes.as_slice()).unwrap();
        match engine.apply_scenario_command(actor, &decoded) {
            Ok(batch) => {
                accepted.push((actor, bytes, batch));
                true
            }
            Err(_) => false,
        }
    }

    let (mut engine, source, recipient) = parked_replay_fixture();
    let mut accepted = Vec::new();
    let activation = activate_ability_with_costs(
        source,
        0,
        vec![],
        vec![rv1::CostSelection {
            cost_index: 2,
            selection: Some(rv1::cost_selection::Selection::BattlefieldObjects(
                rv1::CostObjectRefs {
                    objects: vec![object_ref(&engine, recipient)],
                },
            )),
        }],
    );
    assert!(record_accepted(&mut engine, 0, activation, &mut accepted));
    assert!(engine.state.pending_resolution.is_some());
    assert!(record_accepted(&mut engine, 2, concede(), &mut accepted));

    let pending = engine.state.pending_resolution.as_ref().unwrap();
    let mut stale = submit_resolution_choice(vec![pending.presentation.candidates[0]]);
    let Some(Cmd::SubmitResolutionChoice(answer)) = stale.cmd.as_mut() else {
        unreachable!()
    };
    answer.resolution_prompt_id = pending.resolution_prompt_id.get() + 1;
    let before_stale = engine.diagnostic_snapshot().unwrap();
    let before_stale_index = engine.state.command_index;
    assert!(
        engine.apply_scenario_command(0, &stale).is_err(),
        "a stale answer stays rejected"
    );
    assert_eq!(engine.state.command_index, before_stale_index);
    assert_eq!(
        engine.diagnostic_snapshot().unwrap(),
        before_stale,
        "a stale answer preserves the prompt"
    );

    let mut final_choice_accepted = false;
    for _ in 0..5 {
        let Some(pending) = engine.state.pending_resolution.as_ref() else {
            break;
        };
        let mut choice = submit_resolution_choice(vec![pending.presentation.candidates[0]]);
        let Some(Cmd::SubmitResolutionChoice(answer)) = choice.cmd.as_mut() else {
            unreachable!()
        };
        answer.resolution_prompt_id = pending.resolution_prompt_id.get();
        let before_index = engine.state.command_index;
        let accepted_choice = record_accepted(&mut engine, 0, choice, &mut accepted);
        if engine.state.pending_resolution.is_none() {
            final_choice_accepted = accepted_choice;
            if accepted_choice {
                assert_eq!(engine.state.command_index, before_index + 1);
            }
            break;
        }
        assert!(
            accepted_choice,
            "an intermediate replacement choice must be accepted"
        );
    }
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.blocking_choice().is_none());
    assert!(!engine.state.objects[&source].tapped);
    assert_eq!(
        engine.state.objects[&recipient].counter_count(CounterKind::MinusOneMinusOne),
        0
    );
    assert_eq!(
        (
            engine.state.players[0].mana_pool.blue,
            engine.state.players[0].mana_pool.red
        ),
        (2, 1)
    );
    assert!(engine.state.stack.is_empty());

    let (mut replay, replay_source, replay_recipient) = parked_replay_fixture();
    assert_eq!((source, recipient), (replay_source, replay_recipient));
    for (actor, bytes, expected) in accepted {
        let command = RuledCommand::decode(bytes.as_slice()).unwrap();
        assert_eq!(
            replay.apply_scenario_command(actor, &command).unwrap(),
            expected
        );
    }
    assert_eq!(
        (engine.state.pending_resolution.is_none(), engine.state.blocking_choice().is_none(),
            engine.diagnostic_snapshot().unwrap()["state"]["pending_replacement_event"].is_null()),
        (replay.state.pending_resolution.is_none(), replay.state.blocking_choice().is_none(),
            replay.diagnostic_snapshot().unwrap()["state"]["pending_replacement_event"].is_null()),
        "replaying accepted wire commands must reconstruct the cancellation rather than strand its choice",
    );
    assert!(
        final_choice_accepted,
        "the state-changing cancellation must itself be accepted and logged"
    );
    assert_eq!(
        engine.diagnostic_snapshot().unwrap(),
        replay.diagnostic_snapshot().unwrap()
    );
    let actor = engine.state.priority_player_id();
    assert_eq!(actor, replay.state.priority_player_id());
    let wire = pass().encode_to_vec();
    let next = RuledCommand::decode(wire.as_slice()).unwrap();
    assert_eq!(
        engine.apply_scenario_command(actor, &next).unwrap(),
        replay.apply_scenario_command(actor, &next).unwrap(),
        "subsequent legal actions also replay"
    );
    assert_eq!(
        engine.diagnostic_snapshot().unwrap(),
        replay.diagnostic_snapshot().unwrap()
    );
}

#[test]
fn lattice_parked_missing_permission_does_not_mask_a_stale_mana_snapshot() {
    use tricerules_cards::CounterKind;
    let (mut engine, source, recipient) = parked_replay_fixture();
    let activation = activate_ability_with_costs(
        source,
        0,
        vec![],
        vec![rv1::CostSelection {
            cost_index: 2,
            selection: Some(rv1::cost_selection::Selection::BattlefieldObjects(
                rv1::CostObjectRefs {
                    objects: vec![object_ref(&engine, recipient)],
                },
            )),
        }],
    );
    engine.apply_scenario_command(0, &activation).unwrap();
    engine.apply_scenario_command(2, &concede()).unwrap();
    // Synthetic stale-resource guard: independently invalidate the parked debit after the
    // actual Lattice owner departs. Missing permission must not mask this unrelated failure.
    engine.state.players[0].mana_pool.blue = 1;
    let mut failure = None;
    for _ in 0..5 {
        let Some(pending) = engine.state.pending_resolution.as_ref() else {
            break;
        };
        let selected = pending.presentation.candidates[0];
        if let Err(error) =
            engine.apply_scenario_command(0, &submit_resolution_choice(vec![selected]))
        {
            failure = Some(error);
            break;
        }
    }
    assert!(
        matches!(
            failure,
            Some(tricerules_core::EngineError::Illegal(
                "cost transaction became stale"
            ))
        ),
        "a separately stale resource must remain an error"
    );
    assert_eq!(
        (
            engine.state.players[0].mana_pool.blue,
            engine.state.players[0].mana_pool.red
        ),
        (1, 1)
    );
    assert!(!engine.state.objects[&source].tapped);
    assert_eq!(
        engine.state.objects[&recipient].counter_count(CounterKind::MinusOneMinusOne),
        0
    );
    assert!(engine.state.stack.is_empty());
}

const MIXED_COLORLESS_COST_FIXTURE: &str = r#"(
    id: "lattice_mixed_cost_fixture", name: "Lattice Mixed Cost Fixture",
    face_id: "lattice_mixed_cost_fixture", mana_cost: "{W}{U}{C}", types: ["Instant"],
    spell_effect: [GainLife(amount: 1)],
)"#;

const BLUE_COLORLESS_COST_FIXTURE: &str = r#"(
    id: "lattice_blue_colorless_fixture", name: "Lattice Blue Colorless Fixture",
    face_id: "lattice_blue_colorless_fixture", mana_cost: "{U}{C}", types: ["Instant"],
    spell_effect: [GainLife(amount: 1)],
)"#;

const MIXED_WATERBEND_FIXTURE: &str = r#"(
    id: "lattice_mixed_waterbend_fixture", name: "Lattice Mixed Waterbend Fixture",
    face_id: "lattice_mixed_waterbend_fixture", mana_cost: "{1}",
    types: ["Creature"], power: 1, toughness: 1,
    activated_abilities: [(
        ability_id: "activated_01", presentation: Fallback,
        costs: [Mana("{W}{U}"), Waterbend("{2}")], effect: [GainLife(amount: 1)],
    )],
)"#;

fn fixture_game(chunks: &[&str], seed: u64) -> GameEngine {
    let mut definitions = vec![
        include_str!("../../../tricerules-cards/data/mycosynth_lattice.ron"),
        include_str!("../../../tricerules-cards/data/forest.ron"),
        include_str!("../../../tricerules-cards/data/island.ron"),
    ];
    definitions.extend_from_slice(chunks);
    let registry = Box::leak(Box::new(
        tricerules_cards::CardRegistry::from_chunks_and_tokens(&definitions, &[])
            .expect("labeled payment fixture registry"),
    ));
    let mut engine = GameEngine::new(
        registry,
        seed,
        &[0, 1],
        20,
        Some(vec![deck_with("forest", &[]), deck_with("island", &[])]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    engine
}

#[test]
fn lattice_mixed_cost_reserves_actual_colorless_mana() {
    let mut engine = fixture_game(&[MIXED_COLORLESS_COST_FIXTURE], 26_102_742);
    enter_lattice(&mut engine);
    let card = inject_card_into_hand(&mut engine, 0, "lattice_mixed_cost_fixture");
    let slot = engine.state.players[0]
        .hand
        .iter()
        .position(|oid| *oid == card)
        .unwrap();
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 3,
            ..Default::default()
        },
    );
    let command = cast_spell(slot, vec![]);
    assert!(
        engine.apply_scenario_command(0, &command).is_err(),
        "colored mana cannot pay {{C}}"
    );
    assert_eq!(engine.state.players[0].mana_pool.red, 3);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    engine
        .apply_scenario_command(0, &command)
        .expect("actual C and two red pay {W}{U}{C}");
    assert_eq!(
        (
            engine.state.players[0].mana_pool.red,
            engine.state.players[0].mana_pool.colorless
        ),
        (1, 0)
    );
    assert_eq!(
        serde_json::to_value(engine.state.stack.last().unwrap().mana_colors_spent_to_cast).unwrap(),
        serde_json::json!([false, false, false, true, false])
    );
}

#[test]
fn lattice_blue_colorless_payment_stages_in_either_order_without_losing_true_c() {
    for colorless_first in [false, true] {
        let mut engine = fixture_game(
            &[BLUE_COLORLESS_COST_FIXTURE],
            26_102_752 + u64::from(colorless_first),
        );
        enter_lattice(&mut engine);
        let card = inject_card_into_hand(&mut engine, 0, "lattice_blue_colorless_fixture");
        let slot = engine.state.players[0]
            .hand
            .iter()
            .position(|oid| *oid == card)
            .unwrap();
        give_mana(
            &mut engine,
            0,
            ManaGift {
                r: 1,
                c: 1,
                ..Default::default()
            },
        );
        let Some(Cmd::CastSpell(mut cast)) = cast_spell(slot, vec![]).cmd else {
            unreachable!()
        };
        cast.payment = Some(rv1::PaymentSelection {
            source: Some(object_ref(&engine, card)),
            expected_state_revision: engine.state.command_index,
            mana: Some(if colorless_first {
                rv1::PaymentMana {
                    c: 1,
                    ..Default::default()
                }
            } else {
                rv1::PaymentMana {
                    r: 1,
                    ..Default::default()
                }
            }),
            ..Default::default()
        });
        let partial = preview(&engine, &cast);
        assert!(partial.valid && !partial.complete, "{partial:?}");
        assert_eq!(
            partial.remaining_cost,
            if colorless_first { "{U}" } else { "{C}" }
        );
        cast.payment = partial.selection;
        let mana = cast.payment.as_mut().unwrap().mana.as_mut().unwrap();
        mana.r = 1;
        mana.c = 1;
        let complete = preview(&engine, &cast);
        assert!(complete.valid && complete.complete, "{complete:?}");
        let selected = complete.selection.unwrap();
        assert_eq!(
            (
                selected.mana.as_ref().unwrap().r,
                selected.mana.as_ref().unwrap().c
            ),
            (1, 1)
        );
        cast.payment = Some(selected);
        engine
            .apply_scenario_command(
                0,
                &RuledCommand {
                    cmd: Some(Cmd::CastSpell(cast)),
                },
            )
            .unwrap();
        assert_eq!(
            (
                engine.state.players[0].mana_pool.red,
                engine.state.players[0].mana_pool.colorless
            ),
            (0, 0)
        );
        assert_eq!(
            serde_json::to_value(engine.state.stack.last().unwrap().mana_colors_spent_to_cast)
                .unwrap(),
            serde_json::json!([false, false, false, true, false])
        );
    }
}

#[test]
fn lattice_waterbend_taps_artifacts_for_generic_then_spends_actual_mana_on_the_residual() {
    use tricerules_cards::CounterKind;
    for synthetic_mixed in [false, true] {
        let mut engine = if synthetic_mixed {
            fixture_game(&[MIXED_WATERBEND_FIXTURE], 26_102_755)
        } else {
            game(26_102_754, &[0, 1])
        };
        enter_lattice(&mut engine);
        let source = inject_creature_on_battlefield(
            &mut engine,
            0,
            if synthetic_mixed {
                "lattice_mixed_waterbend_fixture"
            } else {
                "foggy_swamp_vinebender"
            },
        );
        let islands = [
            inject_permanent_on_battlefield(&mut engine, 0, "island"),
            inject_permanent_on_battlefield(&mut engine, 0, "island"),
        ];
        assert!(islands
            .iter()
            .all(|oid| engine.characteristics(*oid).unwrap().is_artifact()));
        let red = if synthetic_mixed { 2 } else { 3 };
        give_mana(
            &mut engine,
            0,
            ManaGift {
                r: red,
                ..Default::default()
            },
        );
        let Some(Cmd::ActivateAbility(mut activation)) =
            activate_ability_for(&engine, source, 0, vec![]).cmd
        else {
            unreachable!()
        };
        activation.payment = Some(rv1::PaymentSelection {
            source: Some(object_ref(&engine, source)),
            expected_state_revision: engine.state.command_index,
            waterbend: islands
                .iter()
                .map(|oid| object_ref(&engine, *oid))
                .collect(),
            mana: Some(rv1::PaymentMana {
                r: red,
                ..Default::default()
            }),
            ..Default::default()
        });
        let offer = engine.preview_payment_scenario(
            0,
            &rv1::PreviewPayment {
                activate_ability: Some(activation.clone()),
                ..Default::default()
            },
        );
        assert!(offer.valid && offer.complete, "{offer:?}");
        let selected = offer.selection.unwrap();
        assert_eq!(selected.waterbend.len(), 2);
        assert_eq!(selected.mana.as_ref().unwrap().r, red);
        activation.payment = Some(selected);
        engine
            .apply_scenario_command(
                0,
                &RuledCommand {
                    cmd: Some(Cmd::ActivateAbility(activation)),
                },
            )
            .expect("commit the previewed Waterbend and mana");
        assert!(islands.iter().all(|oid| engine.state.objects[oid].tapped));
        assert_eq!(engine.state.players[0].mana_pool.red, 0);
        pass_both_players(&mut engine);
        if synthetic_mixed {
            assert_eq!(engine.state.players[0].life, 21);
        } else {
            assert_eq!(
                engine.state.objects[&source].counter_count(CounterKind::PlusOnePlusOne),
                1
            );
        }
    }
}

#[test]
fn lattice_applies_to_actual_flameshadow_resolution_payment_and_offer() {
    for with_lattice in [false, true] {
        let mut engine = game(26_102_756 + u64::from(with_lattice), &[0, 1]);
        if with_lattice {
            enter_lattice(&mut engine);
        }
        inject_permanent_on_battlefield(&mut engine, 0, "flameshadow_conjuring");
        let creature = inject_card_into_hand(&mut engine, 0, "grizzly_bears");
        let slot = engine.state.players[0]
            .hand
            .iter()
            .position(|oid| *oid == creature)
            .unwrap();
        give_mana(
            &mut engine,
            0,
            ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
        );
        engine
            .apply_scenario_command(0, &cast_spell(slot, vec![]))
            .unwrap();
        pass_both_players(&mut engine);
        assert_eq!(
            engine.state.objects[&creature].zone,
            tricerules_core::Zone::Battlefield
        );
        give_mana(
            &mut engine,
            0,
            ManaGift {
                u: 1,
                ..Default::default()
            },
        );
        let trigger = pass_both_players_with_batch(&mut engine);
        assert_eq!(
            find_resolution_choice(&trigger).unwrap().choice_kind(),
            ChoiceKind::ResolutionBranch
        );
        let payment_batch = engine
            .apply_scenario_command(
                0,
                &submit_resolution_decision(ResolutionChoiceDecision::SelectBranch),
            )
            .unwrap();
        let payment = find_resolution_choice(&payment_batch).expect("Flameshadow payment offer");
        assert_eq!(payment.payment_currently_legal, with_lattice);
        let pending = engine.state.pending_resolution.as_ref().unwrap();
        let mut answer = rv1::SubmitResolutionChoice {
            resolution_prompt_id: pending.resolution_prompt_id.get(),
            decision: rv1::ResolutionChoiceDecision::PayMana as i32,
            payment: Some(rv1::PaymentSelection {
                source: Some(object_ref(&engine, pending.presentation.source_object_id)),
                expected_state_revision: engine.state.command_index,
                mana: Some(rv1::PaymentMana {
                    u: 1,
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        };
        let offer = engine.preview_payment_scenario(
            0,
            &rv1::PreviewPayment {
                resolution_choice: Some(answer.clone()),
                ..Default::default()
            },
        );
        assert_eq!(offer.complete, with_lattice, "{offer:?}");
        if with_lattice {
            assert!(offer.valid);
            answer.payment = offer.selection;
            engine
                .apply_scenario_command(
                    0,
                    &RuledCommand {
                        cmd: Some(Cmd::SubmitResolutionChoice(answer)),
                    },
                )
                .expect("blue pays the red cost during resolution");
            assert_eq!(engine.state.players[0].mana_pool.blue, 0);
            assert_eq!(
                engine
                    .state
                    .objects
                    .values()
                    .filter(|object| object.zone == tricerules_core::Zone::Battlefield
                        && object.token_origin.is_some())
                    .count(),
                1
            );
        } else {
            assert!(engine
                .apply_scenario_command(
                    0,
                    &RuledCommand {
                        cmd: Some(Cmd::SubmitResolutionChoice(answer))
                    }
                )
                .is_err());
            assert_eq!(engine.state.players[0].mana_pool.blue, 1);
            assert!(engine.state.pending_resolution.is_some());
        }
    }
}

#[test]
fn lattice_spends_actual_off_color_mana_for_hybrid_and_phyrexian_alternatives() {
    let mut engine = game(26_102_743, &[0, 1]);
    enter_lattice(&mut engine);
    let recruit = inject_card_into_hand(&mut engine, 0, "boros_recruit");
    let slot = engine.state.players[0]
        .hand
        .iter()
        .position(|oid| *oid == recruit)
        .unwrap();
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    engine
        .apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("blue pays the hybrid pip");
    assert_eq!(engine.state.players[0].mana_pool.blue, 0);
    assert_eq!(
        serde_json::to_value(engine.state.stack.last().unwrap().mana_colors_spent_to_cast).unwrap(),
        serde_json::json!([false, true, false, false, false])
    );

    for pay_life in [false, true] {
        let mut engine = game(26_102_744 + u64::from(pay_life), &[0, 1]);
        enter_lattice(&mut engine);
        let creature = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
        let spell = inject_card_into_hand(&mut engine, 0, "mutagenic_growth");
        let slot = engine.state.players[0]
            .hand
            .iter()
            .position(|oid| *oid == spell)
            .unwrap();
        give_mana(
            &mut engine,
            0,
            ManaGift {
                u: 1,
                ..Default::default()
            },
        );
        let flex = if pay_life {
            vec![FlexPipPayment {
                pip_index: 0,
                pay_life: true,
            }]
        } else {
            vec![]
        };
        engine
            .apply_scenario_command(0, &cast_spell_flex(slot, target_object(creature), flex))
            .expect("Phyrexian pip retains either the mana or life alternative");
        assert_eq!(engine.state.players[0].life, if pay_life { 18 } else { 20 });
        assert_eq!(engine.state.players[0].mana_pool.blue, u32::from(pay_life));
    }
}

#[test]
fn lattice_convoke_staging_is_independent_of_mana_and_creature_selection_order() {
    for mana_first in [false, true] {
        let mut engine = game(26_102_746 + u64::from(mana_first), &[0, 1]);
        enter_lattice(&mut engine);
        inject_card_into_hand(&mut engine, 0, "unexpected_assistance");
        let creatures: Vec<_> = (0..3)
            .map(|_| inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears"))
            .collect();
        give_mana(
            &mut engine,
            0,
            ManaGift {
                r: 2,
                ..Default::default()
            },
        );
        let mut cast = convoke_draft(
            &engine,
            "unexpected_assistance",
            &[],
            rv1::PaymentMana::default(),
        );
        for stage in 0..4 {
            let select_mana = if mana_first { stage == 0 } else { stage == 3 };
            if select_mana {
                cast.payment.as_mut().unwrap().mana = Some(rv1::PaymentMana {
                    r: 2,
                    ..Default::default()
                });
            } else {
                let index = if mana_first { stage - 1 } else { stage };
                cast.payment
                    .as_mut()
                    .unwrap()
                    .convoke
                    .push(rv1::ObjectPaymentContribution {
                        object: Some(object_ref(&engine, creatures[index])),
                        kind: rv1::ObjectPaymentKind::Generic as i32,
                    });
            }
            let offer = preview(&engine, &cast);
            assert!(offer.valid, "stage {stage}: {offer:?}");
            let selected = offer.selection.unwrap();
            if mana_first || select_mana {
                assert_eq!(
                    selected.mana.as_ref().unwrap().r,
                    2,
                    "staged mana is not discarded greedily"
                );
            }
            cast.payment = Some(selected);
        }
        assert!(preview(&engine, &cast).complete);
        engine
            .apply_scenario_command(
                0,
                &RuledCommand {
                    cmd: Some(Cmd::CastSpell(cast)),
                },
            )
            .unwrap();
        assert!(creatures.iter().all(|oid| engine.state.objects[oid].tapped));
        assert_eq!(engine.state.players[0].mana_pool.red, 0);
        assert_eq!(
            serde_json::to_value(engine.state.stack.last().unwrap().mana_colors_spent_to_cast)
                .unwrap(),
            serde_json::json!([false, false, false, true, false])
        );
    }
}

#[test]
fn lattice_spending_permission_stops_when_its_ability_is_removed_and_returns_when_restored() {
    let mut engine = game(26_102_748, &[0, 1]);
    let lattice = enter_lattice(&mut engine);
    let creature = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let bolt = inject_card_into_hand(&mut engine, 0, "lightning_bolt");
    let slot = engine.state.players[0]
        .hand
        .iter()
        .position(|oid| *oid == bolt)
        .unwrap();
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(lattice),
        kind: ContinuousEffectKind::Layer6RemoveAllAbilities,
        condition: None,
        duration: EffectDuration::Indefinite,
        timestamp: engine.state.command_index + 1,
    });
    let chars = engine.characteristics(creature).unwrap();
    assert!(
        chars.is_artifact() && chars.colors.is_empty(),
        "the earlier-layer effects already started"
    );
    assert!(engine
        .apply_scenario_command(0, &cast_spell(slot, target_player(1)))
        .is_err());
    assert_eq!(engine.state.players[0].mana_pool.blue, 1);
    engine.state.continuous_effects.pop();
    engine
        .apply_scenario_command(0, &cast_spell(slot, target_player(1)))
        .expect("restoring the ability restores the live permission");
    assert_eq!(engine.state.players[0].mana_pool.blue, 0);
}

#[test]
fn lattice_copied_by_sculpting_steel_retains_spending_after_the_original_leaves() {
    let mut engine = game(26_102_749, &[0, 1]);
    let lattice = enter_lattice(&mut engine);
    let steel = inject_card_into_hand(&mut engine, 0, "sculpting_steel");
    let slot = engine.state.players[0]
        .hand
        .iter()
        .position(|oid| *oid == steel)
        .unwrap();
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 3,
            ..Default::default()
        },
    );
    engine
        .apply_scenario_command(0, &cast_spell(slot, vec![]))
        .unwrap();
    pass_both_players(&mut engine);
    assert!(engine
        .state
        .pending_resolution
        .as_ref()
        .unwrap()
        .presentation
        .candidates
        .contains(&lattice));
    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![lattice]))
        .unwrap();
    assert_eq!(
        engine.characteristics(steel).unwrap().names,
        ["Mycosynth Lattice"]
    );
    engine.enable_dev_commands();
    engine
        .apply_scenario_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(rv1::DevCommand {
                    target_player_id: 0,
                    dev: Some(rv1::dev_command::Dev::MoveCard(rv1::DevMoveCard {
                        card_name: "Mycosynth Lattice".into(),
                        zone: rv1::DevZone::Graveyard as i32,
                        ready: false,
                    })),
                })),
            },
        )
        .unwrap();
    assert_eq!(
        engine.state.objects[&lattice].zone,
        tricerules_core::Zone::Graveyard
    );
    let bolt = inject_card_into_hand(&mut engine, 0, "lightning_bolt");
    let slot = engine.state.players[0]
        .hand
        .iter()
        .position(|oid| *oid == bolt)
        .unwrap();
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    engine
        .apply_scenario_command(0, &cast_spell(slot, target_player(1)))
        .expect("the independent copied Lattice grants the permission");
    assert_eq!(engine.state.players[0].mana_pool.blue, 0);
}

#[test]
fn lattice_sunburst_counts_actual_mana_colors_on_prism_and_a_mixed_cost_fixture() {
    use tricerules_cards::CounterKind;
    let prism = include_str!("../../../tricerules-cards/data/pentad_prism.ron");
    // A renamed test-only Prism definition supplies colored pips; the actual card remains {2}.
    let mixed = prism
        .replace("\"pentad_prism\"", "\"lattice_sunburst_fixture\"")
        .replace(
            "name: \"Pentad Prism\"",
            "name: \"Lattice Sunburst Fixture\"",
        )
        .replace("mana_cost: \"{2}\"", "mana_cost: \"{W}{U}\"");
    for (index, card) in ["lattice_sunburst_fixture", "pentad_prism", "pentad_prism"]
        .into_iter()
        .enumerate()
    {
        let mut engine = fixture_game(&[prism, &mixed], 26_102_750 + index as u64);
        enter_lattice(&mut engine);
        let oid = inject_card_into_hand(&mut engine, 0, card);
        let slot = engine.state.players[0]
            .hand
            .iter()
            .position(|object| *object == oid)
            .unwrap();
        let mana = match index {
            0 => rv1::PaymentMana {
                r: 2,
                ..Default::default()
            },
            1 => rv1::PaymentMana {
                r: 1,
                g: 1,
                ..Default::default()
            },
            _ => rv1::PaymentMana {
                c: 2,
                ..Default::default()
            },
        };
        give_mana(
            &mut engine,
            0,
            ManaGift {
                r: mana.r,
                g: mana.g,
                c: mana.c,
                ..Default::default()
            },
        );
        let Some(Cmd::CastSpell(mut cast)) = cast_spell(slot, vec![]).cmd else {
            unreachable!()
        };
        cast.payment = Some(rv1::PaymentSelection {
            source: Some(object_ref(&engine, oid)),
            expected_state_revision: engine.state.command_index,
            mana: Some(mana),
            ..Default::default()
        });
        let offer = preview(&engine, &cast);
        assert!(offer.valid && offer.complete, "{card}: {offer:?}");
        cast.payment = offer.selection;
        engine
            .apply_scenario_command(
                0,
                &RuledCommand {
                    cmd: Some(Cmd::CastSpell(cast)),
                },
            )
            .unwrap();
        let expected = match index {
            0 => serde_json::json!([false, false, false, true, false]),
            1 => serde_json::json!([false, false, false, true, true]),
            _ => serde_json::json!([false, false, false, false, false]),
        };
        assert_eq!(
            serde_json::to_value(engine.state.stack.last().unwrap().mana_colors_spent_to_cast)
                .unwrap(),
            expected
        );
        pass_both_players(&mut engine);
        assert_eq!(
            engine.state.objects[&oid].counter_count(CounterKind::Charge),
            [1, 2, 0][index]
        );
    }
}
