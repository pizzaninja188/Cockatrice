//! Actual-card coverage for Flameshadow Conjuring's nontoken-creature entry trigger.

use super::helpers::*;
use tricerules_core::{GameEngine, Zone};
use tricerules_proto::ruled::v1::{dev_command, DevCommand, DevMoveCard, DevZone};
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, ChoiceKind, ChooseTriggerTarget, ResolutionChoiceDecision, RuledCommand,
    SubmitResolutionChoice, TargetRef, TargetRefKind,
};

fn hand_index_for_object(engine: &GameEngine, player: usize, object_id: u32) -> usize {
    engine.state.players[player]
        .hand
        .iter()
        .position(|candidate| *candidate == object_id)
        .expect("the exact inserted card remains in hand")
}

fn select_branch() -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            decision: ResolutionChoiceDecision::SelectBranch as i32,
            selected_branch_index: 0,
            ..Default::default()
        })),
    }
}

fn choose_trigger_target(object_id: u32, kind: TargetRefKind) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: Vec::new(),
            targets: vec![TargetRef {
                object_id,
                group_index: 0,
                kind: kind as i32,
                ..Default::default()
            }],
        })),
    }
}

fn pass_until_resolution_choice(engine: &mut GameEngine) {
    for _ in 0..8 {
        if engine.state.pending_resolution.is_some() {
            return;
        }
        pass_priority_round(engine);
    }
    panic!("Flameshadow Conjuring must offer its optional copy branch");
}

fn move_card_to_zone(engine: &mut GameEngine, player_id: i32, card_name: &str, zone: DevZone) {
    engine.enable_dev_commands();
    engine
        .apply_command(
            player_id,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: player_id,
                    dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                        card_name: card_name.into(),
                        zone: zone as i32,
                        ready: false,
                    })),
                })),
            },
        )
        .expect("move a card through the dev command");
}

#[test]
fn flameshadow_copies_the_entry_object_and_schedules_exact_token_exile() {
    let mut engine =
        GameEngine::new(202_610_702, &[4, 9, 27], 20, None, true).expect("new three-player game");
    advance_to_main1_from_game_start(&mut engine);
    inject_permanent_on_battlefield(&mut engine, 0, "flameshadow_conjuring");

    let creature = inject_card_into_hand(&mut engine, 0, "grizzly_bears");
    give_mana(
        &mut engine,
        4,
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
    );
    let hand_index = hand_index_for_object(&engine, 0, creature);
    engine
        .apply_command(4, &cast_spell(hand_index, vec![]))
        .expect("cast a nontoken creature");
    pass_priority_round(&mut engine);

    assert_eq!(engine.state.objects[&creature].zone, Zone::Battlefield);
    assert_eq!(engine.state.stack.len(), 1);
    let trigger = engine.state.stack.last().unwrap();
    assert_eq!(
        trigger.trigger_context.observed_object.unwrap().object_id,
        creature
    );

    let before_battlefield = engine.state.players[0]
        .battlefield
        .iter()
        .copied()
        .collect::<std::collections::HashSet<_>>();
    pass_until_resolution_choice(&mut engine);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .deciding_player,
        4
    );
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .presentation
            .choice_kind,
        ChoiceKind::ResolutionBranch
    );
    give_mana(
        &mut engine,
        4,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    engine
        .apply_command(4, &select_branch())
        .expect("choose to pay {R} for the token copy");
    submit_mana_resolution_decision(&mut engine, 4, ResolutionChoiceDecision::PayMana)
        .expect("pay {R} while the trigger resolves");

    let created_tokens = engine.state.players[0]
        .battlefield
        .iter()
        .copied()
        .filter(|object_id| {
            !before_battlefield.contains(object_id)
                && engine.state.objects[object_id].token_origin.is_some()
        })
        .collect::<Vec<_>>();
    assert_eq!(created_tokens.len(), 1);
    let token = created_tokens[0];
    assert_eq!(engine.state.objects[&token].zone, Zone::Battlefield);
    assert!(
        engine.effective_has_keyword(token, tricerules_cards::Keyword::Haste),
        "the returned exact token reference receives indefinite haste"
    );
    assert!(
        engine.state.active_event_observers.iter().any(|observer| {
            observer.watched.is_some_and(|watched| {
                watched.object_id == token
                    && watched.zone_change_generation == engine.state.zone_change_generation[&token]
            })
        }),
        "the delayed trigger watches the exact copied-token generation"
    );
    assert!(engine.state.stack.is_empty());
}

#[test]
fn flameshadow_ignores_tokens_and_noncreature_entries_and_can_decline_the_copy() {
    let mut engine =
        GameEngine::new(202_610_703, &[4, 9], 20, None, true).expect("new two-player game");
    advance_to_main1_from_game_start(&mut engine);
    inject_permanent_on_battlefield(&mut engine, 0, "flameshadow_conjuring");

    let sol_ring = inject_card_into_hand(&mut engine, 0, "sol_ring");
    give_mana(
        &mut engine,
        4,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    let sol_ring_index = hand_index_for_object(&engine, 0, sol_ring);
    engine
        .apply_command(4, &cast_spell(sol_ring_index, vec![]))
        .expect("cast a noncreature artifact");
    pass_priority_round(&mut engine);
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_triggers.is_empty());

    let raise_alarm = inject_card_into_hand(&mut engine, 0, "raise_the_alarm");
    give_mana(
        &mut engine,
        4,
        ManaGift {
            w: 1,
            c: 1,
            ..Default::default()
        },
    );
    let hand_index = hand_index_for_object(&engine, 0, raise_alarm);
    engine
        .apply_command(4, &cast_spell(hand_index, vec![]))
        .expect("cast Raise the Alarm");
    pass_priority_round(&mut engine);
    assert_eq!(
        engine.state.players[0]
            .battlefield
            .iter()
            .filter(|object_id| engine.state.objects[object_id].token_origin.is_some())
            .count(),
        2,
        "Raise the Alarm created two creature tokens"
    );
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_triggers.is_empty());

    let creature = inject_card_into_hand(&mut engine, 0, "grizzly_bears");
    give_mana(
        &mut engine,
        4,
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
    );
    let hand_index = hand_index_for_object(&engine, 0, creature);
    engine
        .apply_command(4, &cast_spell(hand_index, vec![]))
        .expect("cast a nontoken creature");
    pass_priority_round(&mut engine);
    pass_until_resolution_choice(&mut engine);
    engine
        .apply_command(
            4,
            &RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                    decision: ResolutionChoiceDecision::Decline as i32,
                    ..Default::default()
                })),
            },
        )
        .expect("decline the optional {R} payment");

    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(
        engine.state.players[0]
            .battlefield
            .iter()
            .filter(|object_id| engine.state.objects[object_id].token_origin.is_some())
            .count(),
        2,
        "declining the optional copy creates no additional token"
    );
}

#[test]
fn flameshadow_trigger_uses_source_and_event_object_lki_after_they_leave() {
    let mut engine =
        GameEngine::new(202_610_704, &[4, 9], 20, None, true).expect("new two-player game");
    advance_to_main1_from_game_start(&mut engine);
    inject_permanent_on_battlefield(&mut engine, 0, "flameshadow_conjuring");

    let creature = inject_card_into_hand(&mut engine, 0, "grizzly_bears");
    give_mana(
        &mut engine,
        4,
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
    );
    let hand_index = hand_index_for_object(&engine, 0, creature);
    engine
        .apply_command(4, &cast_spell(hand_index, vec![]))
        .expect("cast a nontoken creature");
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.stack.len(), 1, "entry trigger is on the stack");

    move_card_to_zone(&mut engine, 4, "Flameshadow Conjuring", DevZone::Graveyard);
    assert!(
        !engine.state.players[0]
            .battlefield
            .iter()
            .any(|object_id| engine.state.objects[object_id].card_id == "flameshadow_conjuring"),
        "the trigger's source has left before resolution"
    );
    let murder = inject_card_into_hand(&mut engine, 0, "murder");
    give_mana(
        &mut engine,
        4,
        ManaGift {
            b: 2,
            c: 1,
            ..Default::default()
        },
    );
    let hand_index = hand_index_for_object(&engine, 0, murder);
    engine
        .apply_command(4, &cast_spell(hand_index, target_object(creature)))
        .expect("cast Murder at the observed creature before the trigger resolves");
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.objects[&creature].zone, Zone::Graveyard);

    pass_until_resolution_choice(&mut engine);
    give_mana(
        &mut engine,
        4,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    engine
        .apply_command(4, &select_branch())
        .expect("choose the copy branch");
    submit_mana_resolution_decision(&mut engine, 4, ResolutionChoiceDecision::PayMana)
        .expect("pay {R} with the source in the graveyard");

    let token = engine.state.players[0]
        .battlefield
        .iter()
        .copied()
        .find(|object_id| engine.state.objects[object_id].token_origin.is_some())
        .expect("the resolving trigger still creates its token");
    assert_eq!(
        engine.state.objects[&token].card_id, "grizzly_bears",
        "the copy uses the observed creature's exact last-known copiable values"
    );
    assert!(engine.effective_has_keyword(token, tricerules_cards::Keyword::Haste));
    assert!(engine.state.active_event_observers.iter().any(|observer| {
        observer.watched.is_some_and(|watched| {
            watched.object_id == token
                && watched.zone_change_generation == engine.state.zone_change_generation[&token]
        })
    }));
}

#[test]
fn flameshadow_exiles_the_exact_token_at_end_step_after_control_changes() {
    let mut engine =
        GameEngine::new(202_610_705, &[4, 9], 20, None, true).expect("new two-player game");
    advance_to_main1_from_game_start(&mut engine);
    inject_permanent_on_battlefield(&mut engine, 0, "flameshadow_conjuring");

    let creature = inject_card_into_hand(&mut engine, 0, "grizzly_bears");
    give_mana(
        &mut engine,
        4,
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
    );
    let hand_index = hand_index_for_object(&engine, 0, creature);
    engine
        .apply_command(4, &cast_spell(hand_index, vec![]))
        .expect("cast a nontoken creature");
    pass_priority_round(&mut engine);
    pass_until_resolution_choice(&mut engine);
    give_mana(
        &mut engine,
        4,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    engine
        .apply_command(4, &select_branch())
        .expect("choose the copy branch");
    submit_mana_resolution_decision(&mut engine, 4, ResolutionChoiceDecision::PayMana)
        .expect("pay {R}");

    let token = engine.state.players[0]
        .battlefield
        .iter()
        .copied()
        .find(|object_id| {
            engine.state.objects[object_id].token_origin.is_some()
                && engine.state.objects[object_id].card_id == "grizzly_bears"
        })
        .expect("the copied token is on the battlefield");
    let generation = engine.state.zone_change_generation[&token];

    // Pass priority to P1 during P0's turn. Ray of Command changes control only until cleanup,
    // allowing the next end-step delayed trigger to prove it follows the object identity.
    engine
        .apply_command(4, &pass())
        .expect("active player passes priority");
    let ray = inject_card_into_hand(&mut engine, 1, "ray_of_command");
    give_mana(
        &mut engine,
        9,
        ManaGift {
            u: 1,
            c: 3,
            ..Default::default()
        },
    );
    let ray_index = hand_index_for_object(&engine, 1, ray);
    engine
        .apply_command(9, &cast_spell(ray_index, target_object(token)))
        .expect("P1 casts Ray of Command on the token");
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.objects[&token].controller, 9);
    assert_eq!(engine.state.zone_change_generation[&token], generation);

    engine
        .apply_command(4, &primitive_yield())
        .expect("main one to combat");
    engine
        .apply_command(4, &primitive_yield())
        .expect("begin combat");
    if engine.state.turn_step == tricerules_core::TurnStep::DeclareAttackers {
        engine
            .apply_command(4, &primitive_yield())
            .expect("skip attackers");
    }
    engine
        .apply_command(4, &primitive_yield())
        .expect("end combat to main two");
    engine
        .apply_command(4, &primitive_yield())
        .expect("main two to end step");
    assert_eq!(engine.state.turn_step, tricerules_core::TurnStep::EndStep);
    assert_eq!(
        engine.state.stack.len(),
        1,
        "the end-step exile trigger is staged"
    );
    assert_eq!(engine.state.objects[&token].controller, 9);
    resolve_entire_stack_two_player(&mut engine);
    assert!(
        !engine.state.objects.contains_key(&token),
        "the token left the battlefield by exile and ceased to exist"
    );
    assert!(engine.state.active_event_observers.is_empty());
}

#[test]
fn flameshadow_copy_created_during_end_step_waits_for_the_next_turn_end_step() {
    let deck = deck_with("island", &[]);
    let mut engine = GameEngine::new(
        202_610_706,
        &[4, 9],
        20,
        Some(vec![deck.clone(), deck]),
        true,
    )
    .expect("new two-player game");
    advance_to_main1_from_game_start(&mut engine);
    inject_permanent_on_battlefield(&mut engine, 0, "flameshadow_conjuring");

    // Reach the real end step first, then create the entrant. Its copy observer is registered
    // after this step's beginning, so the card's delayed ability must wait for a later end step.
    engine
        .apply_command(4, &primitive_yield())
        .expect("main one to combat");
    engine
        .apply_command(4, &primitive_yield())
        .expect("begin combat");
    if engine.state.turn_step == tricerules_core::TurnStep::DeclareAttackers {
        engine
            .apply_command(4, &primitive_yield())
            .expect("skip attackers");
    }
    engine
        .apply_command(4, &primitive_yield())
        .expect("end combat to main two");
    engine
        .apply_command(4, &primitive_yield())
        .expect("main two to end step");
    assert_eq!(engine.state.turn_step, tricerules_core::TurnStep::EndStep);
    inject_card_into_hand(&mut engine, 0, "grizzly_bears");
    move_card_to_zone(&mut engine, 4, "Grizzly Bears", DevZone::Battlefield);
    let creature = battlefield_object_for_card(&engine, 0, "grizzly_bears");
    pass_until_resolution_choice(&mut engine);
    give_mana(
        &mut engine,
        4,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    engine
        .apply_command(4, &select_branch())
        .expect("choose to create the copy during the end step");
    submit_mana_resolution_decision(&mut engine, 4, ResolutionChoiceDecision::PayMana)
        .expect("pay {R} during the end step");

    let token = engine.state.players[0]
        .battlefield
        .iter()
        .copied()
        .find(|object_id| {
            *object_id != creature
                && engine.state.objects[object_id].token_origin.is_some()
                && engine.state.objects[object_id].card_id == "grizzly_bears"
        })
        .expect("the copy is still present in the current end step");
    assert_eq!(engine.state.turn_step, tricerules_core::TurnStep::EndStep);
    assert!(engine.effective_has_keyword(token, tricerules_cards::Keyword::Haste));
    assert!(engine.state.active_event_observers.iter().any(|observer| {
        observer.watched.is_some_and(|watched| {
            watched.object_id == token
                && watched.zone_change_generation == engine.state.zone_change_generation[&token]
        })
    }));
    assert!(engine.state.stack.is_empty());

    engine
        .apply_command(4, &primitive_yield())
        .expect("finish the end step where the copy was created");
    resolve_cleanup_discards_if_any(&mut engine);
    assert_eq!(engine.state.active_player_id(), 9);
    assert_eq!(engine.state.turn_step, tricerules_core::TurnStep::Upkeep);
    advance_to_main1_from_game_start(&mut engine);
    engine
        .apply_command(9, &primitive_yield())
        .expect("main one to combat in the next turn");
    engine
        .apply_command(9, &primitive_yield())
        .expect("begin combat in the next turn");
    if engine.state.turn_step == tricerules_core::TurnStep::DeclareAttackers {
        engine
            .apply_command(9, &primitive_yield())
            .expect("skip attackers in the next turn");
    }
    engine
        .apply_command(9, &primitive_yield())
        .expect("end combat to main two in the next turn");
    engine
        .apply_command(9, &primitive_yield())
        .expect("main two to the next turn's end step");
    assert_eq!(engine.state.turn_step, tricerules_core::TurnStep::EndStep);
    assert_eq!(
        engine.state.stack.len(),
        1,
        "the delayed exile trigger is staged"
    );
    resolve_entire_stack_two_player(&mut engine);
    assert!(
        !engine.state.objects.contains_key(&token),
        "the token remains through its creation end step and is exiled at the next one"
    );
}

#[test]
fn flameshadow_copy_entry_choice_fixture_resumes_with_the_exact_token_reference() {
    let mut engine =
        GameEngine::new(202_610_707, &[4, 9], 20, None, true).expect("new two-player game");
    advance_to_main1_from_game_start(&mut engine);
    inject_permanent_on_battlefield(&mut engine, 0, "flameshadow_conjuring");
    let copy_target = inject_permanent_on_battlefield(&mut engine, 0, "prodigal_sorcerer");

    let creature = inject_card_into_hand(&mut engine, 0, "grizzly_bears");
    give_mana(
        &mut engine,
        4,
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
    );
    let hand_index = hand_index_for_object(&engine, 0, creature);
    engine
        .apply_command(4, &cast_spell(hand_index, vec![]))
        .expect("cast a nontoken creature");
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.stack.len(), 1, "entry trigger is on the stack");

    // Engine fixture: retain Grizzly Bears' body while adding Clone's as-enters ability to the
    // observed object's copiable face. This isolates the generic parked-copy receipt path; it is
    // not a claim that a normal Clone copy retains that ability after choosing a source.
    let registry = tricerules_cards::CardRegistry::global();
    let mut face = registry
        .get("grizzly_bears")
        .expect("Grizzly Bears definition")
        .primary_face()
        .clone();
    face.static_abilities = registry
        .get("clone")
        .expect("Clone definition")
        .primary_face()
        .static_abilities
        .clone();
    engine
        .state
        .objects
        .get_mut(&creature)
        .expect("the observed creature")
        .copiable_values = Some(tricerules_core::state::CopiableValues {
        source_card_id: "grizzly_bears".into(),
        source_face_index: 0,
        display_name: face.name.clone(),
        face,
        room_faces: None,
    });

    pass_until_resolution_choice(&mut engine);
    give_mana(
        &mut engine,
        4,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    engine
        .apply_command(4, &select_branch())
        .expect("choose the copy branch");
    submit_mana_resolution_decision(&mut engine, 4, ResolutionChoiceDecision::PayMana)
        .expect("pay {R} for the token copy");

    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the copied Clone entry choice parks token creation");
    assert_eq!(pending.presentation.choice_kind, ChoiceKind::CopySource);
    assert!(pending.presentation.candidates.contains(&copy_target));
    engine
        .apply_command(4, &submit_resolution_choice(vec![copy_target]))
        .expect("choose Prodigal Sorcerer for the copied token");

    let token = engine.state.players[0]
        .battlefield
        .iter()
        .copied()
        .find(|object_id| engine.state.objects[object_id].token_origin.is_some())
        .expect("the copied token committed after its entry choice");
    assert_eq!(engine.effective_power(token), Some(1));
    assert_eq!(
        engine.state.objects[&token]
            .copiable_values
            .as_ref()
            .map(|values| values.source_card_id.as_str()),
        Some("prodigal_sorcerer"),
        "the entry choice installs the selected creature's copiable values"
    );
    assert!(engine.effective_has_keyword(token, tricerules_cards::Keyword::Haste));
    assert!(engine.state.active_event_observers.iter().any(|observer| {
        observer.watched.is_some_and(|watched| {
            watched.object_id == token
                && watched.zone_change_generation == engine.state.zone_change_generation[&token]
        })
    }));
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn flameshadow_token_and_haste_remain_when_its_delayed_exile_is_countered() {
    let mut engine =
        GameEngine::new(202_610_708, &[4, 9], 20, None, true).expect("new two-player game");
    advance_to_main1_from_game_start(&mut engine);
    inject_permanent_on_battlefield(&mut engine, 0, "flameshadow_conjuring");

    let creature = inject_card_into_hand(&mut engine, 0, "grizzly_bears");
    give_mana(
        &mut engine,
        4,
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
    );
    let hand_index = hand_index_for_object(&engine, 0, creature);
    engine
        .apply_command(4, &cast_spell(hand_index, vec![]))
        .expect("cast a nontoken creature");
    pass_priority_round(&mut engine);
    pass_until_resolution_choice(&mut engine);
    give_mana(
        &mut engine,
        4,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    engine
        .apply_command(4, &select_branch())
        .expect("choose the copy branch");
    submit_mana_resolution_decision(&mut engine, 4, ResolutionChoiceDecision::PayMana)
        .expect("pay {R}");
    let token = engine.state.players[0]
        .battlefield
        .iter()
        .copied()
        .find(|object_id| engine.state.objects[object_id].token_origin.is_some())
        .expect("Flameshadow's token");

    engine
        .apply_command(4, &primitive_yield())
        .expect("main one to combat");
    engine
        .apply_command(4, &primitive_yield())
        .expect("begin combat");
    if engine.state.turn_step == tricerules_core::TurnStep::DeclareAttackers {
        engine
            .apply_command(4, &primitive_yield())
            .expect("skip attackers");
    }
    engine
        .apply_command(4, &primitive_yield())
        .expect("end combat to main two");
    engine
        .apply_command(4, &primitive_yield())
        .expect("main two to end step");
    assert_eq!(engine.state.turn_step, tricerules_core::TurnStep::EndStep);
    let delayed_exile = engine.state.stack.last().expect("delayed exile trigger").id;

    engine
        .apply_command(4, &pass())
        .expect("pass priority to the opponent");
    let tidebinder = inject_card_into_hand(&mut engine, 1, "tishanas_tidebinder");
    give_mana(
        &mut engine,
        9,
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    let tidebinder_index = hand_index_for_object(&engine, 1, tidebinder);
    engine
        .apply_command(9, &cast_spell(tidebinder_index, vec![]))
        .expect("cast Tidebinder in response to the delayed exile");
    pass_both_players(&mut engine);
    assert_eq!(engine.state.pending_triggers.len(), 1);
    engine
        .apply_command(
            9,
            &choose_trigger_target(delayed_exile, TargetRefKind::Stack),
        )
        .expect("Tidebinder targets the delayed exile ability");
    pass_both_players(&mut engine);

    assert!(!engine
        .state
        .stack
        .iter()
        .any(|item| item.id == delayed_exile));
    assert_eq!(engine.state.objects[&token].zone, Zone::Battlefield);
    assert!(engine.state.players[0].battlefield.contains(&token));
    assert!(
        engine.effective_has_keyword(token, tricerules_cards::Keyword::Haste),
        "countering the delayed trigger does not remove the granted haste"
    );
}

#[test]
fn flameshadow_stages_one_independent_trigger_per_simultaneous_nontoken_creature() {
    let mut engine =
        GameEngine::new(202_610_709, &[4, 9], 20, None, true).expect("new two-player game");
    advance_to_main1_from_game_start(&mut engine);
    inject_permanent_on_battlefield(&mut engine, 0, "flameshadow_conjuring");
    let entrants = [
        inject_graveyard_card(&mut engine, 0, "darksteel_myr"),
        inject_graveyard_card(&mut engine, 0, "iron_myr"),
    ];
    let scrap_mastery = inject_card_into_hand(&mut engine, 0, "scrap_mastery");
    give_mana(
        &mut engine,
        4,
        ManaGift {
            r: 2,
            c: 3,
            ..Default::default()
        },
    );
    let hand_index = hand_index_for_object(&engine, 0, scrap_mastery);
    engine
        .apply_command(4, &cast_spell(hand_index, vec![]))
        .expect("cast Scrap Mastery");
    pass_priority_round(&mut engine);
    answer_simultaneous_entry_order_in_engine_order(&mut engine);
    answer_trigger_order_in_engine_order(&mut engine);

    for entrant in entrants {
        assert_eq!(engine.state.objects[&entrant].zone, Zone::Battlefield);
    }
    assert_eq!(engine.state.stack.len(), 2);
    let observed: std::collections::HashSet<_> = engine
        .state
        .stack
        .iter()
        .filter_map(|item| {
            item.trigger_context
                .observed_object
                .map(|object| object.object_id)
        })
        .collect();
    assert_eq!(observed, entrants.into_iter().collect());

    pass_until_resolution_choice(&mut engine);
    give_mana(
        &mut engine,
        4,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    engine
        .apply_command(4, &select_branch())
        .expect("choose one trigger's copy branch");
    submit_mana_resolution_decision(&mut engine, 4, ResolutionChoiceDecision::PayMana)
        .expect("pay for exactly one simultaneous entrant's copy");

    pass_until_resolution_choice(&mut engine);
    engine
        .apply_command(
            4,
            &RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                    decision: ResolutionChoiceDecision::Decline as i32,
                    ..Default::default()
                })),
            },
        )
        .expect("decline the other entrant's independent copy trigger");

    let tokens: Vec<_> = engine.state.players[0]
        .battlefield
        .iter()
        .copied()
        .filter(|object_id| engine.state.objects[object_id].token_origin.is_some())
        .collect();
    assert_eq!(tokens.len(), 1);
    assert!(engine.effective_has_keyword(tokens[0], tricerules_cards::Keyword::Haste));
    assert!(engine.state.stack.is_empty());
}
