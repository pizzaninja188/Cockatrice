//! Actual-card coverage for Arcane Denial's target-controller capture and two independently
//! ordered delayed triggers at the next actual turn's upkeep.
use crate::helpers::*;
use tricerules_cards::primitives::{PlayerRecipient, SpellEffectKind, TriggerCondition};
use tricerules_cards::AbilityPresentation;
use tricerules_core::{TurnStep, Zone};
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, ChoiceKind, ResolutionChoiceDecision, RuledCommand, SubmitResolutionChoice,
    TargetRef, TargetRefKind,
};

const PLAYER_IDS: [i32; 3] = [4, 9, 27];
const FOUR_PLAYER_IDS: [i32; 4] = [4, 9, 27, 35];

fn stack_target(object_id: u32) -> Vec<TargetRef> {
    vec![TargetRef {
        object_id,
        damage_amount: 0,
        group_index: 0,
        kind: TargetRefKind::Stack as i32,
    }]
}

fn choose_branch(branch_index: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            decision: ResolutionChoiceDecision::SelectBranch as i32,
            selected_branch_index: branch_index,
            ..Default::default()
        })),
    }
}

fn set_library_top(engine: &mut GameEngine, player: usize, card_ids: &[&str]) -> Vec<u32> {
    let object_ids = card_ids
        .iter()
        .map(|card_id| inject_library_card(engine, player, card_id))
        .collect::<Vec<_>>();
    engine.state.players[player]
        .library
        .retain(|object_id| !object_ids.contains(object_id));
    for object_id in object_ids.iter().rev() {
        engine.state.players[player].library.push_front(*object_id);
    }
    object_ids
}

fn prepare_arcane_denial_against_opt(seed: u64) -> (GameEngine, u32) {
    prepare_arcane_denial_against_opt_with_players(seed, &PLAYER_IDS)
}

fn prepare_arcane_denial_against_opt_with_players(
    seed: u64,
    player_ids: &[i32],
) -> (GameEngine, u32) {
    let decks = Some(vec![vec!["island".into(); 20]; player_ids.len()]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        player_ids,
        20,
        decks,
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);

    inject_card_into_hand(&mut engine, 1, "opt");
    inject_card_into_hand(&mut engine, 2, "arcane_denial");
    give_mana(
        &mut engine,
        player_ids[1],
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    give_mana(
        &mut engine,
        player_ids[2],
        ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        },
    );

    engine
        .apply_command(player_ids[0], &pass())
        .expect("active player passes");
    let opt_slot = hand_index_for_card(&engine, 1, "opt");
    engine
        .apply_command(player_ids[1], &cast_spell(opt_slot, vec![]))
        .expect("target player casts Opt");
    let opt = engine.state.stack.last().expect("Opt on stack").id;
    engine
        .apply_command(player_ids[1], &pass())
        .expect("target player passes");
    let denial_slot = hand_index_for_card(&engine, 2, "arcane_denial");
    engine
        .apply_command(player_ids[2], &cast_spell(denial_slot, stack_target(opt)))
        .expect("third player casts Arcane Denial");
    (engine, opt)
}

fn cast_arcane_denial_against_opt(seed: u64) -> (GameEngine, u32) {
    cast_arcane_denial_against_opt_with_players(seed, &PLAYER_IDS)
}

fn cast_arcane_denial_against_opt_with_players(seed: u64, player_ids: &[i32]) -> (GameEngine, u32) {
    let (mut engine, opt) = prepare_arcane_denial_against_opt_with_players(seed, player_ids);
    pass_priority_round(&mut engine);
    assert!(engine.state.stack.is_empty(), "Arcane Denial counters Opt");
    assert_eq!(engine.state.objects[&opt].zone, Zone::Graveyard);
    assert_eq!(engine.state.active_event_observers.len(), 2);
    assert!(engine.state.active_event_observers.iter().all(|observer| {
        matches!(
            observer.matcher,
            tricerules_core::state::EventObserverMatcher::AtBeginningOfNextTurnUpkeep {
                target_turn_instance: None,
                ..
            }
        )
    }));
    (engine, opt)
}

fn resolve_arcane_denial_against_uncounterable_eject(seed: u64) -> (GameEngine, u32) {
    let decks = Some(vec![vec!["island".into(); 20]; PLAYER_IDS.len()]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &PLAYER_IDS,
        20,
        decks,
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let creature = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    inject_card_into_hand(&mut engine, 1, "eject");
    inject_card_into_hand(&mut engine, 2, "arcane_denial");
    give_mana(
        &mut engine,
        PLAYER_IDS[1],
        ManaGift {
            u: 1,
            c: 3,
            ..Default::default()
        },
    );
    give_mana(
        &mut engine,
        PLAYER_IDS[2],
        ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        },
    );

    engine
        .apply_command(PLAYER_IDS[0], &pass())
        .expect("active player passes");
    let eject_slot = hand_index_for_card(&engine, 1, "eject");
    engine
        .apply_command(
            PLAYER_IDS[1],
            &cast_spell(eject_slot, target_object(creature)),
        )
        .expect("target player casts the uncounterable Eject");
    let eject = engine.state.stack.last().expect("Eject on stack").id;
    engine
        .apply_command(PLAYER_IDS[1], &pass())
        .expect("target player passes");
    let denial_slot = hand_index_for_card(&engine, 2, "arcane_denial");
    engine
        .apply_command(PLAYER_IDS[2], &cast_spell(denial_slot, stack_target(eject)))
        .expect("third player casts Arcane Denial at Eject");
    pass_priority_round(&mut engine);
    assert_eq!(
        engine.state.stack.len(),
        1,
        "uncounterable Eject remains on the stack"
    );
    assert_eq!(engine.state.stack[0].id, eject);
    assert_eq!(engine.state.objects[&creature].zone, Zone::Battlefield);
    pass_priority_round(&mut engine);

    assert!(engine.state.stack.is_empty(), "Eject resolves normally");
    assert_eq!(engine.state.objects[&creature].zone, Zone::Hand);
    assert_eq!(engine.state.active_event_observers.len(), 2);
    assert!(engine.state.active_event_observers.iter().all(|observer| {
        matches!(
            observer.matcher,
            tricerules_core::state::EventObserverMatcher::AtBeginningOfNextTurnUpkeep {
                target_turn_instance: None,
                ..
            }
        )
    }));
    (engine, creature)
}

fn order_arcane_delayed_triggers(engine: &mut GameEngine, target_resolves_first: bool) {
    let order = engine
        .state
        .pending_trigger_order
        .as_ref()
        .expect("the Arcane Denial caster orders both simultaneous delayed triggers");
    assert_eq!(order.deciding_player, PLAYER_IDS[2]);
    assert_eq!(order.candidates.len(), 2);
    let target_trigger = order
        .candidates
        .iter()
        .find(|candidate| {
            matches!(
                candidate.ability.effect.as_slice(),
                [SpellEffectKind::ChooseResolutionBranch {
                    chooser: PlayerRecipient::AffectedPlayer,
                    ..
                }]
            )
        })
        .expect("target controller's draw-choice trigger");
    let own_trigger = order
        .candidates
        .iter()
        .find(|candidate| candidate.object_id != target_trigger.object_id)
        .expect("Arcane Denial controller's draw trigger");
    let first_to_place = if target_resolves_first {
        own_trigger.object_id
    } else {
        target_trigger.object_id
    };
    engine
        .apply_command(PLAYER_IDS[2], &submit_trigger_order(first_to_place))
        .expect("choose the first delayed trigger to put on the stack");
    assert!(engine.state.pending_trigger_order.is_none());
    let top = engine
        .state
        .stack
        .last()
        .expect("both delayed abilities are on the stack");
    let top_is_target_trigger = top.triggered_ability.as_ref().is_some_and(|ability| {
        matches!(
            ability.effect.as_slice(),
            [SpellEffectKind::ChooseResolutionBranch {
                chooser: PlayerRecipient::AffectedPlayer,
                ..
            }]
        )
    });
    assert_eq!(top_is_target_trigger, target_resolves_first);
    if top_is_target_trigger {
        assert_eq!(top.trigger_context.affected_player, Some(PLAYER_IDS[1]));
    }
}

fn resolve_arcane_delayed_triggers(
    engine: &mut GameEngine,
    draw_count: u32,
    target_left_game: bool,
) {
    let mut target_choice_seen = false;
    while !engine.state.stack.is_empty() || engine.state.pending_resolution.is_some() {
        if let Some(pending) = engine.state.pending_resolution.as_ref() {
            assert_eq!(
                pending.presentation.choice_kind,
                ChoiceKind::ResolutionBranch
            );
            let deciding_player = if target_left_game {
                PLAYER_IDS[0]
            } else {
                PLAYER_IDS[1]
            };
            assert_eq!(pending.deciding_player, deciding_player);
            target_choice_seen = true;
            assert!(
                engine
                    .apply_command(PLAYER_IDS[2], &choose_branch(0))
                    .is_err(),
                "the trigger controller cannot choose the draw count"
            );
            engine
                .apply_command(deciding_player, &choose_branch(draw_count))
                .expect("the target player or their rules-selected delegate chooses 0, 1, or 2");
        } else {
            pass_priority_round(engine);
        }
    }
    assert!(
        target_choice_seen,
        "the delayed draw choice is delegated when its chooser left"
    );
}

#[test]
fn arcane_denial_is_admitted_with_both_delayed_effects_and_exact_choice_ownership() {
    let face = tricerules_cards::registry::global()
        .get("arcane_denial")
        .expect("Arcane Denial is admitted as a complete card")
        .primary_face();

    assert_eq!(face.name, "Arcane Denial");
    assert_eq!(face.types, ["Instant"]);
    assert!(matches!(
        face.spell_effect.as_slice(),
        [
            SpellEffectKind::CounterTargetSpell { .. },
            SpellEffectKind::CreateDelayedTrigger {
                subject: None,
                affected_player: Some(PlayerRecipient::PreviousTargetedSpellController),
                ability: first,
            },
            SpellEffectKind::CreateDelayedTrigger {
                subject: None,
                affected_player: None,
                ability: second,
            }
        ] if first.presentation == AbilityPresentation::OracleLines(vec![1])
            && second.presentation == AbilityPresentation::OracleLines(vec![2])
            && first.trigger == TriggerCondition::AtBeginningOfNextTurnUpkeep
            && second.trigger == TriggerCondition::AtBeginningOfNextTurnUpkeep
            && matches!(
                first.effect.as_slice(),
                [SpellEffectKind::ChooseResolutionBranch {
                    chooser: PlayerRecipient::AffectedPlayer,
                    optional: false,
                    branches,
                    ..
                }] if branches.len() == 3
                    && branches.iter().enumerate().all(|(index, branch)| {
                        branch.presentation == AbilityPresentation::OracleLines(vec![1])
                            && branch.fallback_label()
                                == [
                                    "The affected player draws no cards.",
                                    "The affected player draws a card.",
                                    "The affected player draws 2 cards.",
                                ][index]
                    })
                    && branches.iter().all(|branch| matches!(
                        branch.effects.as_slice(),
                        [SpellEffectKind::Draw {
                            who: PlayerRecipient::AffectedPlayer,
                            ..
                        }]
                    ))
            )
            && matches!(
                second.effect.as_slice(),
                [SpellEffectKind::Draw {
                    who: PlayerRecipient::Controller,
                    ..
                }]
            )
    ));
}

#[test]
fn arcane_denial_discards_delayed_triggers_when_the_caster_leaves_before_upkeep() {
    let ids = FOUR_PLAYER_IDS;
    let (mut engine, _) = cast_arcane_denial_against_opt_with_players(84270, &ids);
    assert_eq!(engine.state.active_event_observers.len(), 2);

    engine
        .apply_command(ids[2], &concede())
        .expect("Arcane Denial's controller leaves before the next upkeep");

    assert!(
        engine.state.active_event_observers.is_empty(),
        "a delayed triggered ability controlled by a departed player cannot later be put on the stack"
    );
    end_active_turn(&mut engine, ids[0]);
    assert_eq!(engine.state.active_player_id(), ids[1]);
    assert!(engine.state.pending_trigger_order.is_none());
    assert!(engine.state.pending_triggers.is_empty());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn arcane_denial_discards_staged_triggers_when_the_caster_leaves_before_placement() {
    let ids = FOUR_PLAYER_IDS;
    let (mut engine, _) = cast_arcane_denial_against_opt_with_players(84271, &ids);
    end_active_turn(&mut engine, ids[0]);
    let order = engine
        .state
        .pending_trigger_order
        .as_ref()
        .expect("Arcane Denial's simultaneous delayed triggers await the caster's order");
    assert_eq!(order.deciding_player, ids[2]);
    assert_eq!(order.candidates.len(), 2);

    engine
        .apply_command(ids[2], &concede())
        .expect("the delayed-trigger controller leaves before placing either ability");

    assert!(engine.state.pending_trigger_order.is_none());
    assert!(engine.state.staged_trigger_groups.is_empty());
    assert!(engine.state.pending_triggers.is_empty());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn arcane_denial_orders_both_triggers_and_the_target_chooses_zero_one_or_two() {
    for target_resolves_first in [false, true] {
        for draw_count in 0..=2 {
            let seed = 84200 + u64::from(target_resolves_first) * 10 + u64::from(draw_count);
            let (mut engine, _) = cast_arcane_denial_against_opt(seed);
            let target_cards = set_library_top(&mut engine, 1, &["forest", "mountain", "plains"]);
            let controller_cards =
                set_library_top(&mut engine, 2, &["grizzly_bears", "storm_crow"]);
            let target_hand_before = engine.state.players[1].hand.len();
            let controller_hand_before = engine.state.players[2].hand.len();

            end_active_turn(&mut engine, PLAYER_IDS[0]);
            assert_eq!(engine.state.active_player_id(), PLAYER_IDS[1]);
            assert_eq!(engine.state.turn_step, TurnStep::Upkeep);
            order_arcane_delayed_triggers(&mut engine, target_resolves_first);
            resolve_arcane_delayed_triggers(&mut engine, draw_count, false);

            assert_eq!(
                engine.state.players[1].hand.len(),
                target_hand_before + draw_count as usize,
            );
            assert_eq!(
                engine.state.players[2].hand.len(),
                controller_hand_before + 1,
            );
            assert!(target_cards[..draw_count as usize]
                .iter()
                .all(|object_id| engine.state.players[1].hand.contains(object_id)));
            assert!(engine.state.players[2].hand.contains(&controller_cards[0]));
        }
    }
}

#[test]
fn arcane_denial_still_uses_an_uncounterable_targets_controller_for_the_delayed_draw() {
    let (mut engine, creature) = resolve_arcane_denial_against_uncounterable_eject(84240);
    assert_eq!(engine.state.objects[&creature].owner, PLAYER_IDS[1]);
    let target_hand_before = engine.state.players[1].hand.len();
    let controller_hand_before = engine.state.players[2].hand.len();
    let target_cards = set_library_top(&mut engine, 1, &["forest", "mountain", "plains"]);
    let controller_cards = set_library_top(&mut engine, 2, &["grizzly_bears", "storm_crow"]);

    end_active_turn(&mut engine, PLAYER_IDS[0]);
    assert_eq!(engine.state.active_player_id(), PLAYER_IDS[1]);
    assert_eq!(engine.state.turn_step, TurnStep::Upkeep);
    order_arcane_delayed_triggers(&mut engine, true);
    resolve_arcane_delayed_triggers(&mut engine, 2, false);

    assert_eq!(engine.state.players[1].hand.len(), target_hand_before + 2);
    assert_eq!(
        engine.state.players[2].hand.len(),
        controller_hand_before + 1
    );
    assert!(target_cards[..2]
        .iter()
        .all(|object_id| engine.state.players[1].hand.contains(object_id)));
    assert!(engine.state.players[2].hand.contains(&controller_cards[0]));
}

#[test]
fn arcane_denial_with_all_targets_illegal_creates_no_delayed_triggers() {
    let (mut engine, opt) = prepare_arcane_denial_against_opt(84241);
    let denial = engine
        .state
        .stack
        .last()
        .expect("Arcane Denial on stack")
        .id;
    engine.state.stack.retain(|item| item.id != opt);
    engine.state.players[1].graveyard.push(opt);
    engine.state.objects.get_mut(&opt).expect("Opt object").zone = Zone::Graveyard;
    *engine.state.zone_change_generation.entry(opt).or_default() += 1;

    pass_priority_round(&mut engine);

    assert!(engine.state.stack.is_empty());
    assert_eq!(engine.state.objects[&denial].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&opt].zone, Zone::Graveyard);
    assert!(engine.state.active_event_observers.is_empty());
}

#[test]
fn arcane_denials_target_player_leaving_before_upkeep_does_not_stall_the_controllers_draw() {
    let (mut engine, _) = cast_arcane_denial_against_opt(84230);
    engine
        .apply_command(PLAYER_IDS[1], &concede())
        .expect("target spell controller concedes before the upkeep");
    let controller_hand_before = engine.state.players[2].hand.len();
    let controller_cards = set_library_top(&mut engine, 2, &["grizzly_bears", "storm_crow"]);

    end_active_turn(&mut engine, PLAYER_IDS[0]);
    assert_eq!(engine.state.active_player_id(), PLAYER_IDS[2]);
    assert_eq!(engine.state.turn_step, TurnStep::Upkeep);
    order_arcane_delayed_triggers(&mut engine, true);
    resolve_arcane_delayed_triggers(&mut engine, 0, true);

    assert_eq!(
        engine.state.players[2].hand.len(),
        controller_hand_before + 1
    );
    assert!(engine.state.players[2].hand.contains(&controller_cards[0]));
}

fn resolve_target_draw_trigger(engine: &mut GameEngine, controller: i32) {
    pass_priority_round(engine);
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the target-player draw trigger asks for a delegate");
    assert_eq!(pending.deciding_player, controller);
    assert!(matches!(
        pending.continuation,
        tricerules_core::state::ResolutionContinuation::AuthoredBranch {
            branch: tricerules_core::state::PendingResolutionBranch {
                stage: tricerules_core::state::PendingResolutionBranchStage::ChoosingDelegate { .. },
                ..
            },
            ..
        }
    ));
}

#[test]
fn arcane_denial_refreshes_delegate_candidates_without_reusing_stale_indices() {
    let ids = FOUR_PLAYER_IDS;
    let (mut engine, _) = cast_arcane_denial_against_opt_with_players(84250, &ids);
    engine
        .apply_command(ids[1], &concede())
        .expect("target spell controller leaves before the upkeep");
    end_active_turn(&mut engine, ids[0]);
    assert_eq!(engine.state.active_player_id(), ids[2]);
    order_arcane_delayed_triggers(&mut engine, true);
    resolve_target_draw_trigger(&mut engine, ids[2]);

    let pending = engine.state.pending_resolution.as_ref().unwrap();
    let tricerules_core::state::ResolutionContinuation::AuthoredBranch { branch, .. } =
        &pending.continuation
    else {
        panic!("authored branch is parked")
    };
    let tricerules_core::state::PendingResolutionBranchStage::ChoosingDelegate { candidates } =
        &branch.stage
    else {
        panic!("controller selects between two surviving delegates")
    };
    assert_eq!(candidates, &[Some(ids[0]), Some(ids[3])]);
    assert!(engine.apply_command(ids[0], &choose_branch(0)).is_err());

    let events = engine
        .apply_command(ids[0], &concede())
        .expect("one delegate candidate leaves while the controller is choosing");
    let pending = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(pending.deciding_player, ids[2]);
    let choice = events
        .events
        .iter()
        .find_map(|event| match event.ev.as_ref() {
            Some(tricerules_proto::ruled::v1::ruled_event::Ev::ResolutionChoiceRequired(
                choice,
            )) => Some(choice),
            _ => None,
        })
        .expect("the delegate prompt is refreshed");
    assert_eq!(choice.resolution_branches.len(), 2);
    assert!(!choice.resolution_branches[0].selectable);
    assert!(choice.resolution_branches[1].selectable);
    assert_eq!(
        choice.resolution_branches[1]
            .presentation
            .as_ref()
            .unwrap()
            .fallback_text,
        "P35 makes the choice"
    );
    assert!(engine.apply_command(ids[2], &choose_branch(0)).is_err());
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .deciding_player,
        ids[2],
        "a stale candidate index cannot change the pending chooser"
    );

    engine
        .apply_command(ids[2], &choose_branch(1))
        .expect("the preserved index selects the remaining delegate");
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .deciding_player,
        ids[3]
    );
    assert!(engine.apply_command(ids[2], &choose_branch(2)).is_err());
    engine
        .apply_command(ids[3], &choose_branch(2))
        .expect("the selected delegate chooses the count for the departed target");

    let target_hand_before = engine.state.players[1].hand.len();
    let controller_hand_before = engine.state.players[2].hand.len();
    pass_priority_round(&mut engine);
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.players[1].hand.len(), target_hand_before);
    assert_eq!(
        engine.state.players[2].hand.len(),
        controller_hand_before + 1
    );
}

#[test]
fn arcane_denial_delegate_choice_replays_accepted_commands_from_identical_state() {
    use prost::Message;

    type RecordedCommand = (
        i32,
        Vec<u8>,
        tricerules_proto::ruled::v1::RuledEventBatch,
        serde_json::Value,
    );

    fn record_command(
        engine: &mut GameEngine,
        log: &mut Vec<RecordedCommand>,
        actor: i32,
        command: RuledCommand,
    ) {
        let batch = engine
            .apply_command(actor, &command)
            .expect("record only accepted commands");
        log.push((
            actor,
            command.encode_to_vec(),
            batch,
            engine.diagnostic_snapshot().unwrap(),
        ));
    }

    let ids = FOUR_PLAYER_IDS;
    let (mut engine, _) = cast_arcane_denial_against_opt_with_players(84272, &ids);
    let (mut replay, _) = cast_arcane_denial_against_opt_with_players(84272, &ids);
    engine
        .apply_command(ids[1], &concede())
        .expect("target player leaves before the delayed upkeep");
    replay
        .apply_command(ids[1], &concede())
        .expect("replay the target player's concession");
    end_active_turn(&mut engine, ids[0]);
    end_active_turn(&mut replay, ids[0]);
    assert_eq!(
        engine.diagnostic_snapshot().unwrap(),
        replay.diagnostic_snapshot().unwrap()
    );
    assert_eq!(engine.state.active_player_id(), ids[2]);
    assert_eq!(engine.state.turn_step, TurnStep::Upkeep);
    let mut accepted = Vec::new();

    let order = engine.state.pending_trigger_order.as_ref().unwrap();
    let target_trigger = order
        .candidates
        .iter()
        .find(|candidate| {
            matches!(
                candidate.ability.effect.as_slice(),
                [SpellEffectKind::ChooseResolutionBranch {
                    chooser: PlayerRecipient::AffectedPlayer,
                    ..
                }]
            )
        })
        .expect("the target draw-choice trigger");
    let own_trigger = order
        .candidates
        .iter()
        .find(|candidate| candidate.object_id != target_trigger.object_id)
        .expect("the caster's draw trigger");
    let own_trigger_object_id = own_trigger.object_id;
    record_command(
        &mut engine,
        &mut accepted,
        ids[2],
        submit_trigger_order(own_trigger_object_id),
    );

    let live_players = engine
        .state
        .players
        .iter()
        .filter(|player| !player.has_lost)
        .count();
    let passes_needed = live_players - engine.state.passes_since_stack_change as usize;
    for _ in 0..passes_needed {
        let actor = engine.state.priority_player_id();
        record_command(&mut engine, &mut accepted, actor, pass());
    }
    assert!(matches!(
        &engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .continuation,
        tricerules_core::state::ResolutionContinuation::AuthoredBranch {
            branch: tricerules_core::state::PendingResolutionBranch {
                stage: tricerules_core::state::PendingResolutionBranchStage::ChoosingDelegate { .. },
                ..
            },
            ..
        }
    ));
    record_command(&mut engine, &mut accepted, ids[2], choose_branch(0));
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .deciding_player,
        ids[0]
    );
    record_command(&mut engine, &mut accepted, ids[0], choose_branch(1));

    for (actor, encoded, expected_batch, expected_state) in accepted {
        let command = RuledCommand::decode(encoded.as_slice()).unwrap();
        assert_eq!(
            replay.apply_command(actor, &command).unwrap(),
            expected_batch
        );
        assert_eq!(replay.diagnostic_snapshot().unwrap(), expected_state);
    }
}

#[test]
fn arcane_denial_reassigns_when_the_selected_delegate_leaves_during_their_choice() {
    let ids = FOUR_PLAYER_IDS;
    let (mut engine, _) = cast_arcane_denial_against_opt_with_players(84251, &ids);
    engine
        .apply_command(ids[1], &concede())
        .expect("target spell controller leaves before the upkeep");
    end_active_turn(&mut engine, ids[0]);
    order_arcane_delayed_triggers(&mut engine, true);
    resolve_target_draw_trigger(&mut engine, ids[2]);
    engine
        .apply_command(ids[2], &choose_branch(0))
        .expect("controller delegates to the first surviving opponent");
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .deciding_player,
        ids[0]
    );

    let refreshed = engine
        .apply_command(ids[0], &concede())
        .expect("selected delegate leaves before answering the branch choice");
    let pending = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(pending.deciding_player, ids[3]);
    assert!(refreshed.events.iter().any(|event| matches!(
        &event.ev,
        Some(tricerules_proto::ruled::v1::ruled_event::Ev::ResolutionChoiceRequired(choice))
            if choice.deciding_player_id == ids[3]
                && choice.resolution_branches.len() == 3
    )));
    assert!(matches!(
        pending.continuation,
        tricerules_core::state::ResolutionContinuation::AuthoredBranch {
            branch: tricerules_core::state::PendingResolutionBranch {
                stage: tricerules_core::state::PendingResolutionBranchStage::Selecting,
                ..
            },
            ..
        }
    ));
    engine
        .apply_command(ids[3], &choose_branch(1))
        .expect("the remaining eligible player receives the refreshed branch prompt");
}
