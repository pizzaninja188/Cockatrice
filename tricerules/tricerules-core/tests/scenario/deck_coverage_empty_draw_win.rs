use super::helpers::*;
use prost::Message;
use tricerules_core::{TurnStep, Zone};
use tricerules_proto::ruled::v1 as rv1;

fn setup(seed: u64) -> GameEngine {
    let deck = deck_with("island", &["laboratory_maniac"]);
    let mut engine = GameEngine::new(
        seed,
        &[4, 9, 27],
        20,
        Some(vec![deck.clone(), deck.clone(), deck]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn paid_spell(engine: &mut GameEngine, card: &str) -> u32 {
    let caster = engine.state.players[0].id;
    let oid = inject_card_into_hand(engine, 0, card);
    give_mana(
        engine,
        caster,
        ManaGift {
            u: 3,
            c: 4,
            ..Default::default()
        },
    );
    let slot = engine.state.players[0]
        .hand
        .iter()
        .position(|&id| id == oid)
        .unwrap();
    engine
        .apply_command(caster, &cast_spell(slot, Vec::new()))
        .unwrap();
    oid
}

fn resolve(engine: &mut GameEngine, oid: u32) -> Vec<rv1::RuledEvent> {
    let mut events = Vec::new();
    for _ in 0..16 {
        if engine.state.winner().is_some() || !engine.state.stack.iter().any(|item| item.id == oid)
        {
            return events;
        }
        let actor = engine.state.priority_player_id();
        events.extend(engine.apply_command(actor, &pass()).unwrap().events);
    }
    panic!("resolution did not finish");
}

#[test]
fn maniac_paid_cast_empty_draw_wins_without_actual_draw_or_spell_exit() {
    let mut engine = setup(507_010);
    let maniac = paid_spell(&mut engine, "laboratory_maniac");
    resolve(&mut engine, maniac);
    assert_eq!(engine.state.objects[&maniac].zone, Zone::Battlefield);
    engine.state.players[0].library.clear();
    let spell = paid_spell(&mut engine, "divination");
    let hand = engine.state.players[0].hand.clone();
    let events = resolve(&mut engine, spell);
    assert_eq!(engine.state.winner(), Some(4));
    assert_eq!(engine.state.players[0].hand, hand);
    assert!(!engine.state.players[0].pending_library_loss);
    assert!(!engine.state.players[0].has_lost);
    assert_eq!(engine.state.objects[&spell].zone, Zone::Stack);
    assert!(!events.iter().any(|event| matches!(&event.ev,
        Some(rv1::ruled_event::Ev::StackResolved(exit)) if exit.object_id == spell)));
}

#[test]
fn jace_paid_minus_eight_checks_empty_library_after_source_leaves_before_losses() {
    for remaining in [0usize, 6, 7, 8] {
        let mut engine = setup(507_020 + remaining as u64);
        let jace = paid_spell(&mut engine, "jace,_wielder_of_mysteries");
        resolve(&mut engine, jace);
        assert_eq!(engine.state.objects[&jace].zone, Zone::Battlefield);
        engine
            .state
            .objects
            .get_mut(&jace)
            .unwrap()
            .counters
            .insert(tricerules_cards::CounterKind::Loyalty, 8);
        engine.state.players[0].library.truncate(remaining);
        let hand = engine.state.players[0].hand.len();
        let command = activate_ability_for(&engine, jace, 1, Vec::new());
        engine.apply_command(4, &command).unwrap();
        assert_eq!(engine.state.objects[&jace].zone, Zone::Graveyard);
        let ability = engine.state.stack.last().unwrap().id;
        let events = resolve(&mut engine, ability);
        assert_eq!(engine.state.players[0].hand.len(), hand + remaining.min(7));
        assert_eq!(engine.state.winner(), (remaining <= 7).then_some(4));
        assert_eq!(
            engine.state.players[0].library.len(),
            remaining.saturating_sub(7)
        );
        assert!(!engine.state.players[0].has_lost);
        assert_eq!(engine.state.players[0].pending_library_loss, remaining < 7);
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(&event.ev,
                    Some(rv1::ruled_event::Ev::StackResolved(exit)) if exit.object_id == ability
                ))
                .count(),
            usize::from(remaining == 8)
        );
    }
}

fn resolve_until_choice(engine: &mut GameEngine) -> rv1::ResolutionChoiceRequired {
    for _ in 0..16 {
        let actor = engine.state.priority_player_id();
        let batch = engine.apply_command(actor, &pass()).unwrap();
        if let Some(choice) = find_resolution_choice(&batch) {
            return choice.clone();
        }
    }
    panic!("no resolution choice");
}

#[test]
fn empty_draw_win_both_replacement_orders_stop_children_and_reject_invalid_answers() {
    for card in ["laboratory_maniac", "jace,_wielder_of_mysteries"] {
        for reflection_first in [false, true] {
            let mut engine = setup(507_030 + u64::from(reflection_first));
            let source = inject_permanent_on_battlefield(&mut engine, 0, card);
            // A planeswalker fixture must be alive at the pre-resolution SBA boundary.
            if card.starts_with("jace") {
                engine
                    .state
                    .objects
                    .get_mut(&source)
                    .unwrap()
                    .counters
                    .insert(tricerules_cards::CounterKind::Loyalty, 4);
            }
            let reflection = inject_permanent_on_battlefield(&mut engine, 0, "thought_reflection");
            engine.state.players[0].library.clear();
            let spell = paid_spell(&mut engine, "divination");
            let hand = engine.state.players[0].hand.clone();
            let choice = resolve_until_choice(&mut engine);
            assert_eq!(choice.deciding_player_id, 4);
            assert_eq!(choice.replacement_options.len(), 2);
            let selected = choice
                .replacement_options
                .iter()
                .find(|option| {
                    option.source_object_id == if reflection_first { reflection } else { source }
                })
                .unwrap()
                .application_id;
            let before = engine.diagnostic_snapshot().unwrap();
            for (actor, command) in [
                (9, submit_resolution_choice(vec![selected])),
                (4, submit_resolution_choice(vec![selected, selected])),
                (4, submit_resolution_choice(vec![u32::MAX])),
            ] {
                assert!(engine.apply_command(actor, &command).is_err());
                assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
            }
            let mut extra = submit_resolution_choice(vec![selected]);
            if let Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(answer)) =
                extra.cmd.as_mut()
            {
                answer.chosen_player_ids.push(9);
            }
            assert!(engine.apply_command(4, &extra).is_err());
            assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
            *engine
                .state
                .zone_change_generation
                .entry(source)
                .or_default() += 2;
            let stale = engine.diagnostic_snapshot().unwrap();
            let old_win = choice
                .replacement_options
                .iter()
                .find(|option| option.source_object_id == source)
                .unwrap()
                .application_id;
            assert!(engine
                .apply_command(4, &submit_resolution_choice(vec![old_win]))
                .is_err());
            assert_eq!(engine.diagnostic_snapshot().unwrap(), stale);
            *engine
                .state
                .zone_change_generation
                .get_mut(&source)
                .unwrap() -= 2;
            let batch = engine
                .apply_command(4, &submit_resolution_choice(vec![selected]))
                .unwrap();
            assert_eq!(engine.state.winner(), Some(4));
            assert!(find_resolution_choice(&batch).is_none());
            assert!(engine.state.pending_resolution.is_none());
            assert!(batch.legal_by_player.is_empty());
            assert_eq!(engine.state.players[0].hand, hand);
            assert!(!engine.state.players[0].pending_library_loss);
            assert_eq!(engine.state.objects[&spell].zone, Zone::Stack);
        }
    }
}

#[test]
fn maniac_partial_draw_commits_one_card_but_abandons_brainstorm_and_frantic_tails() {
    for card in ["brainstorm", "frantic_search"] {
        let mut engine = setup(507_040);
        inject_permanent_on_battlefield(&mut engine, 0, "laboratory_maniac");
        let observer = inject_permanent_on_battlefield(&mut engine, 0, "ominous_seas");
        let land = inject_permanent_on_battlefield(&mut engine, 0, "island");
        engine.state.objects.get_mut(&land).unwrap().tapped = true;
        engine.state.players[0].library.truncate(1);
        let drawn = engine.state.players[0].library[0];
        let generation = engine
            .state
            .zone_change_generation
            .get(&drawn)
            .copied()
            .unwrap_or(0);
        let graveyard = engine.state.players[0].graveyard.clone();
        let spell = paid_spell(&mut engine, card);
        let hand = engine.state.players[0].hand.len();
        let events = resolve(&mut engine, spell);
        assert_eq!(engine.state.winner(), Some(4));
        assert_eq!(engine.state.players[0].hand.len(), hand + 1);
        assert!(engine.state.players[0].hand.contains(&drawn));
        assert_eq!(engine.state.zone_change_generation[&drawn], generation + 1);
        assert_eq!(engine.state.objects[&drawn].zone, Zone::Hand);
        assert_eq!(engine.state.players[0].graveyard, graveyard);
        assert!(engine.state.objects[&land].tapped);
        assert!(engine.state.objects[&observer].counters.is_empty());
        assert!(engine.state.staged_trigger_groups.is_empty());
        assert!(engine.state.pending_resolution.is_none());
        assert_eq!(engine.state.objects[&spell].zone, Zone::Stack);
        assert!(!events.iter().any(|event| {
            matches!(&event.ev, Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(_)))
                || matches!(&event.ev, Some(rv1::ruled_event::Ev::StackResolved(exit)) if exit.object_id == spell)
        }));
    }
}

#[test]
fn maniac_nonactive_apnap_winner_stops_later_players_and_staged_draw_triggers() {
    let mut engine = setup(507_041);
    let beleren = inject_permanent_on_battlefield(&mut engine, 0, "jace_beleren");
    inject_permanent_on_battlefield(&mut engine, 0, "ominous_seas");
    for seat in [1, 2] {
        inject_permanent_on_battlefield(&mut engine, seat, "laboratory_maniac");
        engine.state.players[seat].library.clear();
    }
    engine
        .apply_command(4, &activate_ability(beleren, 0, Vec::new()))
        .unwrap();
    let ability = engine.state.stack.last().unwrap().id;
    let hands = engine
        .state
        .players
        .iter()
        .map(|p| p.hand.len())
        .collect::<Vec<_>>();
    resolve(&mut engine, ability);
    assert_eq!(engine.state.winner(), Some(9));
    assert_eq!(engine.state.players[0].hand.len(), hands[0] + 1);
    assert_eq!(engine.state.players[1].hand.len(), hands[1]);
    assert_eq!(engine.state.players[2].hand.len(), hands[2]);
    assert!(engine
        .state
        .players
        .iter()
        .all(|p| !p.pending_library_loss && !p.has_lost));
    assert!(engine.state.staged_trigger_groups.is_empty());
    assert!(engine.state.pending_triggers.is_empty());
}

#[test]
fn maniac_scheduled_empty_draw_wins_without_step_completion_priority_or_loss() {
    let mut engine = setup(507_042);
    inject_permanent_on_battlefield(&mut engine, 0, "laboratory_maniac");
    engine.state.players[0].library.clear();
    engine.state.turn_step = TurnStep::Upkeep;
    engine.state.priority_idx = 0;
    let hands = engine
        .state
        .players
        .iter()
        .map(|p| p.hand.clone())
        .collect::<Vec<_>>();
    let mut last = None;
    for _ in 0..3 {
        let actor = engine.state.priority_player_id();
        last = Some(engine.apply_command(actor, &pass()).unwrap());
    }
    let batch = last.unwrap();
    assert_eq!(engine.state.winner(), Some(4));
    assert!(batch.legal_by_player.is_empty());
    assert!(!batch
        .events
        .iter()
        .any(|e| matches!(e.ev, Some(rv1::ruled_event::Ev::PriorityChanged(_)))));
    for (player, hand) in engine.state.players.iter().zip(hands) {
        assert_eq!(player.hand, hand);
        assert!(!player.pending_library_loss && !player.has_lost);
    }
}

#[test]
fn jace_plus_one_mills_chosen_player_before_controller_draw_and_illegal_target_cancels_all() {
    for target in [4, 9] {
        for departed in [false, true] {
            if departed && target == 4 {
                continue;
            }
            let mut engine = setup(507_050);
            let jace = paid_spell(&mut engine, "jace,_wielder_of_mysteries");
            resolve(&mut engine, jace);
            let target_index = engine.state.player_idx(target).unwrap();
            let milled = engine.state.players[target_index]
                .library
                .iter()
                .take(2)
                .copied()
                .collect::<Vec<_>>();
            let expected_draw = engine.state.players[0].library[usize::from(target == 4) * 2];
            let hand = engine.state.players[0].hand.clone();
            let command = activate_ability_for(&engine, jace, 0, target_player(target));
            engine.apply_command(4, &command).unwrap();
            assert_eq!(
                engine.state.objects[&jace].counters[&tricerules_cards::CounterKind::Loyalty],
                5
            );
            let ability = engine.state.stack.last().unwrap().id;
            if departed {
                engine.apply_command(9, &concede()).unwrap();
            }
            resolve(&mut engine, ability);
            assert!(engine.state.winner().is_none());
            if departed {
                assert_eq!(engine.state.players[0].hand, hand);
            } else {
                assert_eq!(engine.state.players[0].hand.len(), hand.len() + 1);
                assert!(engine.state.players[0].hand.contains(&expected_draw));
                for oid in milled {
                    assert_eq!(engine.state.objects[&oid].zone, Zone::Graveyard);
                }
            }
        }
    }
}

#[test]
fn jace_insufficient_loyalty_is_atomic_and_self_mill_can_reach_empty_draw_win() {
    let mut engine = setup(507_051);
    let jace = paid_spell(&mut engine, "jace,_wielder_of_mysteries");
    resolve(&mut engine, jace);
    let before = engine.diagnostic_snapshot().unwrap();
    let command = activate_ability_for(&engine, jace, 1, Vec::new());
    assert!(engine.apply_command(4, &command).is_err());
    assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
    engine.state.players[0].library.truncate(2);
    let hand = engine.state.players[0].hand.clone();
    let command = activate_ability_for(&engine, jace, 0, target_player(4));
    engine.apply_command(4, &command).unwrap();
    let ability = engine.state.stack.last().unwrap().id;
    resolve(&mut engine, ability);
    assert_eq!(engine.state.winner(), Some(4));
    assert_eq!(engine.state.players[0].hand, hand);
    assert!(!engine.state.players[0].pending_library_loss);
}

#[test]
fn serialized_terminal_draw_choices_use_fresh_handles_and_replay_exact_batches() {
    fn fresh() -> GameEngine {
        let mut engine = setup(507_060);
        inject_permanent_on_battlefield(&mut engine, 0, "thought_reflection");
        inject_permanent_on_battlefield(&mut engine, 0, "laboratory_maniac");
        inject_permanent_on_battlefield(&mut engine, 0, "laboratory_maniac");
        engine.state.players[0].library.clear();
        paid_spell(&mut engine, "divination");
        engine
    }
    let mut engine = fresh();
    let mut recorded = Vec::new();
    let initial = loop {
        let actor = engine.state.priority_player_id();
        let command = pass();
        let batch = engine.apply_command(actor, &command).unwrap();
        let choice = find_resolution_choice(&batch);
        recorded.push((actor, command, batch));
        if let Some(choice) = choice {
            break choice;
        }
    };
    let reflection = initial
        .replacement_options
        .iter()
        .find(|o| o.source_card_name == "Thought Reflection")
        .unwrap()
        .application_id;
    let old_win = initial
        .replacement_options
        .iter()
        .find(|o| o.source_card_name == "Laboratory Maniac")
        .unwrap()
        .application_id;
    let command = submit_resolution_choice(vec![reflection]);
    let batch = engine.apply_command(4, &command).unwrap();
    let child = find_resolution_choice(&batch).unwrap().clone();
    recorded.push((4, command, batch));
    assert_eq!(child.replacement_options.len(), 2);
    assert!(child
        .candidate_object_ids
        .iter()
        .all(|id| !initial.candidate_object_ids.contains(id)));
    let before = engine.diagnostic_snapshot().unwrap();
    assert!(engine
        .apply_command(4, &submit_resolution_choice(vec![old_win]))
        .is_err());
    assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
    let command = submit_resolution_choice(vec![child.candidate_object_ids[0]]);
    let batch = engine.apply_command(4, &command).unwrap();
    assert_eq!(engine.state.winner(), Some(4));
    assert!(batch.legal_by_player.is_empty());
    recorded.push((4, command, batch));
    let mut replay = fresh();
    for (actor, command, batch) in recorded {
        let decoded = rv1::RuledCommand::decode(command.encode_to_vec().as_slice()).unwrap();
        assert_eq!(replay.apply_command(actor, &decoded).unwrap(), batch);
    }
    assert_eq!(
        replay.diagnostic_snapshot().unwrap(),
        engine.diagnostic_snapshot().unwrap()
    );
    let before = engine.diagnostic_snapshot().unwrap();
    assert!(engine.apply_command(4, &pass()).is_err());
    assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
}

#[test]
fn winning_draw_static_uses_copied_face_and_current_control_or_suppression() {
    use tricerules_cards::primitives::{BasicLandType, ControllerReference};
    use tricerules_cards::{CardRegistry, ContinuousEffectKind, EffectDuration};
    use tricerules_core::{AffectedScope, ContinuousEffect};
    for mode in 0..5 {
        let mut engine = setup(507_061);
        let source = inject_permanent_on_battlefield(
            &mut engine,
            0,
            if mode == 4 {
                "grizzly_bears"
            } else {
                "laboratory_maniac"
            },
        );
        if mode == 1 {
            engine.state.objects.get_mut(&source).unwrap().face_down = true;
        } else if mode == 2 || mode == 3 {
            engine.state.continuous_effects.push(ContinuousEffect {
                source_id: None,
                trigger_grant_origin: None,
                affected: AffectedScope::Single(source),
                kind: if mode == 2 {
                    ContinuousEffectKind::Layer4SetTypeLine(tricerules_cards::TypeLineReplacement {
                        card_types: vec![tricerules_cards::PermanentTypeFilter::Land],
                        creature_types: Vec::new(),
                        land_types: vec![BasicLandType::Forest],
                    })
                } else {
                    ContinuousEffectKind::Layer2Control {
                        controller: ControllerReference::Fixed(9),
                    }
                },
                condition: None,
                duration: EffectDuration::UntilEndOfTurn,
                timestamp: engine.state.command_index,
            });
        } else if mode == 4 {
            let face = CardRegistry::global()
                .get("laboratory_maniac")
                .unwrap()
                .primary_face()
                .clone();
            engine
                .state
                .objects
                .get_mut(&source)
                .unwrap()
                .copiable_values = Some(tricerules_core::state::CopiableValues {
                source_card_id: "laboratory_maniac".into(),
                source_face_index: 0,
                face,
                room_faces: None,
                display_name: "Laboratory Maniac".into(),
            });
        }
        engine.state.players[0].library.clear();
        let spell = paid_spell(&mut engine, "divination");
        resolve(&mut engine, spell);
        if mode == 0 || mode == 4 {
            assert_eq!(engine.state.winner(), Some(4));
            assert!(!engine.state.players[0].has_lost);
        } else {
            assert!(engine.state.winner().is_none());
            assert!(engine.state.players[0].has_lost);
        }
    }
}

#[test]
fn parked_empty_draw_win_revalidates_face_control_suppression_copy_and_library() {
    use tricerules_cards::primitives::ControllerReference;
    use tricerules_cards::{CardRegistry, ContinuousEffectKind, EffectDuration};
    use tricerules_core::{AffectedScope, ContinuousEffect};
    for mode in 0..5 {
        let mut engine = setup(507_070 + mode);
        let source = inject_permanent_on_battlefield(&mut engine, 0, "laboratory_maniac");
        let reflection = inject_permanent_on_battlefield(&mut engine, 0, "thought_reflection");
        let refill = engine.state.players[0].library[0];
        engine.state.players[0].library.clear();
        paid_spell(&mut engine, "divination");
        let hand = engine.state.players[0].hand.clone();
        let choice = resolve_until_choice(&mut engine);
        let old = choice
            .replacement_options
            .iter()
            .find(|o| o.source_object_id == source)
            .unwrap()
            .application_id;
        let draws_before = engine.state.turn_history.current.player(4).cards_drawn;
        match mode {
            0 => engine.state.objects.get_mut(&source).unwrap().face_down = true,
            1 | 2 => engine.state.continuous_effects.push(ContinuousEffect {
                source_id: None,
                trigger_grant_origin: None,
                affected: AffectedScope::Single(source),
                kind: if mode == 1 {
                    ContinuousEffectKind::Layer6RemoveAllAbilities
                } else {
                    ContinuousEffectKind::Layer2Control {
                        controller: ControllerReference::Fixed(9),
                    }
                },
                condition: None,
                duration: EffectDuration::UntilEndOfTurn,
                timestamp: engine.state.command_index,
            }),
            3 => {
                let face = CardRegistry::global()
                    .get("jace,_wielder_of_mysteries")
                    .unwrap()
                    .primary_face()
                    .clone();
                let object = engine.state.objects.get_mut(&source).unwrap();
                object.copiable_values = Some(tricerules_core::state::CopiableValues {
                    source_card_id: "jace,_wielder_of_mysteries".into(),
                    source_face_index: 0,
                    display_name: face.name.clone(),
                    face,
                    room_faces: None,
                });
                object.copy_revision += 1;
                object
                    .counters
                    .insert(tricerules_cards::CounterKind::Loyalty, 4);
            }
            4 => engine.state.players[0].library.push_back(refill),
            _ => unreachable!(),
        }
        let before = engine.diagnostic_snapshot().unwrap();
        assert!(engine
            .apply_command(4, &submit_resolution_choice(vec![old]))
            .is_err());
        assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
        if mode == 4 {
            let double = choice
                .replacement_options
                .iter()
                .find(|o| o.source_object_id == reflection)
                .unwrap()
                .application_id;
            let batch = engine
                .apply_command(4, &submit_resolution_choice(vec![double]))
                .unwrap();
            assert_eq!(engine.state.winner(), Some(4));
            assert_eq!(engine.state.players[0].hand.len(), hand.len() + 1);
            assert!(engine.state.players[0].hand.contains(&refill));
            assert_eq!(
                engine.state.turn_history.current.player(4).cards_drawn,
                draws_before + 1
            );
            assert!(!engine.state.players[0].pending_library_loss);
            assert!(batch.legal_by_player.is_empty());
        }
    }
}

#[test]
fn discard_destination_commits_before_empty_draw_win_with_library_of_leng() {
    for destination in [0, 1] {
        let mut engine = setup(507_080 + destination as u64);
        inject_permanent_on_battlefield(&mut engine, 0, "laboratory_maniac");
        inject_permanent_on_battlefield(&mut engine, 0, "library_of_leng");
        let discarded = inject_card_into_hand(&mut engine, 0, "grizzly_bears");
        engine.state.players[0].library.clear();
        give_mana(
            &mut engine,
            4,
            ManaGift {
                r: 1,
                ..Default::default()
            },
        );
        let spell = paid_spell(&mut engine, "romantic_rendezvous");
        let hand_before = engine.state.players[0].hand.len();
        let draws_before = engine.state.turn_history.current.player(4).cards_drawn;
        let discard = resolve_until_choice(&mut engine);
        assert_eq!(discard.choice_kind(), rv1::ChoiceKind::HandCards);
        let batch = engine
            .apply_command(4, &submit_resolution_choice(vec![discarded]))
            .unwrap();
        let choice = find_resolution_choice(&batch).unwrap();
        assert_eq!(choice.choice_kind(), rv1::ChoiceKind::PrivateReplacement);
        assert_eq!(engine.state.objects[&discarded].zone, Zone::Hand);
        engine
            .apply_command(4, &submit_resolution_choice(vec![destination]))
            .unwrap();
        assert_eq!(engine.state.winner(), Some(4));
        assert_eq!(
            engine.state.objects[&discarded].zone,
            if destination == 0 {
                Zone::Graveyard
            } else {
                Zone::Hand
            }
        );
        assert_eq!(
            engine.state.players[0].hand.len(),
            hand_before - usize::from(destination == 0)
        );
        assert_eq!(
            engine.state.turn_history.current.player(4).cards_drawn,
            draws_before + destination
        );
        assert!(!engine.state.players[0].pending_library_loss);
        assert!(engine.state.pending_resolution.is_none());
        assert_eq!(engine.state.objects[&spell].zone, Zone::Stack);
    }
}

#[test]
fn final_concession_publishes_one_terminal_result() {
    let mut engine = GameEngine::new(507_090, &[4, 9], 20, None, true).unwrap();
    let batch = engine.apply_command(9, &concede()).unwrap();
    assert_eq!(engine.state.winner(), Some(4));
    assert_eq!(
        batch
            .events
            .iter()
            .filter(|event| matches!(&event.ev,
                Some(rv1::ruled_event::Ev::Log(log)) if log.text == "Game over. Winner: 4"
            ))
            .count(),
        1
    );
    assert!(batch.legal_by_player.is_empty());
}

#[test]
fn failed_draw_terminal_loss_does_not_flush_survivors_staged_triggers() {
    let deck = deck_with("island", &["divination"]);
    let mut engine =
        GameEngine::new(507_091, &[4, 9], 20, Some(vec![deck.clone(), deck]), true).unwrap();
    advance_to_main1_from_game_start(&mut engine);
    inject_permanent_on_battlefield(&mut engine, 1, "consecrated_sphinx");
    engine.state.players[0].library.truncate(1);
    let spell = paid_spell(&mut engine, "divination");
    let before = engine.state.players[0].hand.len();
    let events = resolve(&mut engine, spell);
    assert_eq!(engine.state.winner(), Some(9));
    assert_eq!(engine.state.players[0].hand.len(), before + 1);
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.staged_trigger_groups.is_empty());
    assert!(engine.state.pending_triggers.is_empty());
    assert!(!events
        .iter()
        .any(|event| matches!(&event.ev, Some(rv1::ruled_event::Ev::StackPushed(_)))));
}
