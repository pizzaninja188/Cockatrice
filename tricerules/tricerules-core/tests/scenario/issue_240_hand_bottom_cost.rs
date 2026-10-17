use super::helpers::*;
use crate::helpers::ScenarioCommandExt;
use tricerules_core::EngineDeck;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    dev_command, permanent_moved::Destination, ruled_command::Cmd, ruled_event::Ev, ChoiceKind,
    DevCommand, DevMoveCard, DevZone, ResolutionChoiceDecision, RuledCommand, RuledEventBatch,
    SubmitResolutionChoice,
};

fn select_branch() -> RuledCommand {
    select_branch_index(0)
}

fn select_branch_index(index: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            resolution_prompt_id: 0,
            decision: ResolutionChoiceDecision::SelectBranch as i32,
            selected_branch_index: index,
            ..Default::default()
        })),
    }
}

fn resolve_top(engine: &mut GameEngine) -> RuledEventBatch {
    let mut batch = None;
    for _ in 0..engine.state.players.len() {
        batch = Some(
            engine
                .apply_scenario_command(engine.state.priority_player_id(), &pass())
                .expect("priority pass"),
        );
    }
    batch.expect("resolution batch")
}

fn choice_count(batch: &RuledEventBatch, choice_kind: ChoiceKind) -> usize {
    batch
        .events
        .iter()
        .filter(|event| {
            matches!(
                &event.ev,
                Some(Ev::ResolutionChoiceRequired(required))
                    if required.choice_kind == choice_kind as i32
            )
        })
        .count()
}

fn setup(seed: u64) -> (GameEngine, u32) {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        Some(vec![
            deck_with("mountain", &["manhole_missile"]),
            forest_only_deck(),
        ]),
        true,
    )
    .expect("Manhole Missile is registered");
    advance_to_main1_from_game_start(&mut engine);
    ensure_card_in_hand(&mut engine, 0, "manhole_missile");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            c: 1,
            ..Default::default()
        },
    );
    let target = inject_creature_with_stats(&mut engine, 1, "indomitable_ancients", 2, 10);
    (engine, target)
}

fn setup_three_player(seed: u64) -> (GameEngine, u32) {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1, 2],
        20,
        Some(vec![
            deck_with("mountain", &["manhole_missile"]),
            forest_only_deck(),
            forest_only_deck(),
        ]),
        true,
    )
    .expect("three-player Manhole Missile fixture");
    advance_to_main1_from_game_start(&mut engine);
    ensure_card_in_hand(&mut engine, 0, "manhole_missile");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            c: 1,
            ..Default::default()
        },
    );
    let target = inject_creature_with_stats(&mut engine, 1, "indomitable_ancients", 2, 10);
    (engine, target)
}

fn cast_and_choose_branch(engine: &mut GameEngine, target: u32) -> RuledEventBatch {
    let slot = hand_index_for_card(engine, 0, "manhole_missile");
    engine
        .apply_scenario_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast Manhole Missile");
    resolve_top(engine);
    assert_eq!(engine.state.objects[&target].damage, 3);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .expect("optional branch")
            .presentation
            .choice_kind,
        ChoiceKind::ResolutionBranch
    );
    engine.apply_scenario_command(0, &select_branch()).unwrap()
}

#[test]
fn issue_240_payment_moves_exact_private_card_to_bottom_then_draws() {
    let (mut engine, target) = setup(240_001);
    let choice = cast_and_choose_branch(&mut engine, target);
    let pending = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(pending.presentation.choice_kind, ChoiceKind::HandCards);
    let chosen = pending
        .presentation
        .candidates
        .iter()
        .copied()
        .find(|oid| engine.state.objects[oid].card_id == "mountain")
        .expect("a private hand card candidate");
    assert!(choice.events.iter().any(|event| matches!(
        &event.ev,
        Some(Ev::ResolutionChoiceRequired(required))
            if required.choice_kind == ChoiceKind::HandCards as i32
                && required.candidate_object_ids.contains(&chosen)
    )));

    let drawn = *engine.state.players[0]
        .library
        .front()
        .expect("card available to draw");
    let generation = engine
        .state
        .zone_change_generation
        .get(&chosen)
        .copied()
        .unwrap_or(0);
    let paid = engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![chosen]))
        .expect("pay by putting the chosen card on the bottom");

    assert_eq!(engine.state.objects[&chosen].zone, Zone::Library);
    assert_eq!(engine.state.zone_change_generation[&chosen], generation + 1);
    assert_eq!(engine.state.players[0].library.back(), Some(&chosen));
    assert!(engine.state.players[0].hand.contains(&drawn));
    assert!(paid.events.iter().any(|event| matches!(
        &event.ev,
        Some(Ev::PermanentMoved(moved))
            if moved.object_id == chosen && moved.destination == Destination::Library as i32
    )));
    assert!(paid.events.iter().all(|event| match &event.ev {
        Some(Ev::Log(log)) if log.visible_to_player_id.is_none() => {
            !log.text.contains("Mountain")
        }
        _ => true,
    }));
}

#[test]
fn issue_240_decline_and_illegal_target_skip_bottoming_and_draw() {
    let (mut declined, target) = setup(240_002);
    cast_and_choose_branch(&mut declined, target);
    let hand_before = declined.state.players[0].hand.clone();
    let library_before = declined.state.players[0].library.clone();
    declined
        .apply_scenario_command(0, &submit_resolution_choice(vec![]))
        .unwrap();
    assert_eq!(declined.state.players[0].hand, hand_before);
    assert_eq!(declined.state.players[0].library, library_before);

    let (mut fizzled, target) = setup(240_003);
    let slot = hand_index_for_card(&fizzled, 0, "manhole_missile");
    fizzled
        .apply_scenario_command(0, &cast_spell(slot, target_object(target)))
        .unwrap();
    fizzled.state.players[1]
        .battlefield
        .retain(|oid| *oid != target);
    fizzled.state.players[1].graveyard.push(target);
    fizzled.state.objects.get_mut(&target).unwrap().zone = Zone::Graveyard;
    *fizzled
        .state
        .zone_change_generation
        .entry(target)
        .or_default() += 1;
    let hand_before = fizzled.state.players[0].hand.clone();
    let library_before = fizzled.state.players[0].library.clone();
    resolve_top(&mut fizzled);
    assert!(fizzled.state.pending_resolution.is_none());
    assert_eq!(fizzled.state.players[0].hand, hand_before);
    assert_eq!(fizzled.state.players[0].library, library_before);
}

#[test]
fn issue_240_stale_wrong_player_and_replayed_payments_are_atomic() {
    let (mut engine, target) = setup(240_004);
    cast_and_choose_branch(&mut engine, target);
    let chosen = engine
        .state
        .pending_resolution
        .as_ref()
        .unwrap()
        .presentation
        .candidates[0];

    let before_wrong_player = serde_json::to_value(&engine.state).unwrap();
    assert!(engine
        .apply_scenario_command(1, &submit_resolution_choice(vec![chosen]))
        .is_err());
    assert_eq!(
        serde_json::to_value(&engine.state).unwrap(),
        before_wrong_player
    );

    *engine
        .state
        .zone_change_generation
        .entry(chosen)
        .or_default() += 1;
    let before_stale = serde_json::to_value(&engine.state).unwrap();
    assert!(engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![chosen]))
        .is_err());
    assert_eq!(serde_json::to_value(&engine.state).unwrap(), before_stale);

    *engine
        .state
        .zone_change_generation
        .get_mut(&chosen)
        .unwrap() -= 1;
    let command = submit_resolution_choice(vec![chosen]);
    engine.apply_scenario_command(0, &command).unwrap();
    let after_payment = serde_json::to_value(&engine.state).unwrap();
    assert!(engine.apply_scenario_command(0, &command).is_err());
    assert_eq!(serde_json::to_value(&engine.state).unwrap(), after_payment);
}

#[test]
fn issue_240_empty_hand_skips_the_branch_and_empty_library_draws_the_paid_card() {
    let (mut no_hand, target) = setup(240_005);
    let slot = hand_index_for_card(&no_hand, 0, "manhole_missile");
    let spell = no_hand.state.players[0].hand[slot];
    let removed = no_hand.state.players[0]
        .hand
        .iter()
        .copied()
        .filter(|oid| *oid != spell)
        .collect::<Vec<_>>();
    no_hand.state.players[0].hand.retain(|oid| *oid == spell);
    for oid in removed {
        no_hand.state.players[0].graveyard.push(oid);
        no_hand.state.objects.get_mut(&oid).unwrap().zone = Zone::Graveyard;
        *no_hand.state.zone_change_generation.entry(oid).or_default() += 1;
    }
    no_hand
        .apply_scenario_command(0, &cast_spell(0, target_object(target)))
        .unwrap();
    resolve_top(&mut no_hand);
    assert_eq!(no_hand.state.objects[&target].damage, 3);
    assert!(no_hand.state.pending_resolution.is_none());
    assert!(no_hand.state.players[0].hand.is_empty());

    let (mut empty_library, target) = setup(240_006);
    cast_and_choose_branch(&mut empty_library, target);
    let chosen = empty_library
        .state
        .pending_resolution
        .as_ref()
        .unwrap()
        .presentation
        .candidates[0];
    let drained = empty_library.state.players[0]
        .library
        .drain(..)
        .collect::<Vec<_>>();
    for oid in drained {
        empty_library.state.players[0].graveyard.push(oid);
        empty_library.state.objects.get_mut(&oid).unwrap().zone = Zone::Graveyard;
        *empty_library
            .state
            .zone_change_generation
            .entry(oid)
            .or_default() += 1;
    }
    empty_library
        .apply_scenario_command(0, &submit_resolution_choice(vec![chosen]))
        .unwrap();
    assert_eq!(empty_library.state.objects[&chosen].zone, Zone::Hand);
    assert!(empty_library.state.players[0].hand.contains(&chosen));
    assert!(empty_library.state.players[0].library.is_empty());
    assert!(!empty_library.state.players[0].has_lost);
}

#[test]
fn issue_240_accepted_commands_replay_identically() {
    fn run() -> (Vec<RuledEventBatch>, serde_json::Value) {
        let (mut engine, target) = setup(240_007);
        let slot = hand_index_for_card(&engine, 0, "manhole_missile");
        let cast = engine
            .apply_scenario_command(0, &cast_spell(slot, target_object(target)))
            .unwrap();
        let resolve = resolve_top(&mut engine);
        let branch = engine.apply_scenario_command(0, &select_branch()).unwrap();
        let chosen = engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .presentation
            .candidates[0];
        let payment = engine
            .apply_scenario_command(0, &submit_resolution_choice(vec![chosen]))
            .unwrap();
        (
            vec![cast, resolve, branch, payment],
            serde_json::to_value(&engine.state).unwrap(),
        )
    }

    assert_eq!(run(), run());
}

#[test]
fn issue_240_declared_commander_hand_bottom_cost_waits_for_owner_before_draw() {
    let mut p0_mainboard = vec!["mountain".into(); 12];
    p0_mainboard.push("manhole_missile".into());
    let p0_deck = EngineDeck {
        mainboard: p0_mainboard,
        commanders: vec!["captain_storm,_cosmium_raider".into()],
    };
    let p1_deck = EngineDeck {
        mainboard: vec!["forest".into(); 12],
        commanders: Vec::new(),
    };
    let mut engine = GameEngine::new_with_commander_decks(
        tricerules_cards::registry::global(),
        240_008,
        &[0, 1],
        20,
        Some(vec![p0_deck, p1_deck]),
        true,
    )
    .expect("declared commander fixture");
    advance_to_main1_from_game_start(&mut engine);
    let commander = *engine.state.players[0]
        .command_zone
        .first()
        .expect("Captain Storm began as P0's declared commander");
    assert!(engine.state.players[0]
        .declared_commander_object_ids
        .contains(&commander));

    // Return that exact declared object to hand, then pay with it. The dev move creates the
    // already-returned state without replacing it with a same-name noncommander.
    engine.enable_dev_commands();
    engine
        .apply_scenario_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: 0,
                    dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                        card_name: "Captain Storm, Cosmium Raider".into(),
                        zone: DevZone::Hand as i32,
                        ready: false,
                    })),
                })),
            },
        )
        .expect("return the declared commander object to its owner's hand");
    assert_eq!(engine.state.objects[&commander].zone, Zone::Hand);
    ensure_card_in_hand(&mut engine, 0, "manhole_missile");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            c: 1,
            ..Default::default()
        },
    );
    let target = inject_creature_with_stats(&mut engine, 1, "indomitable_ancients", 2, 10);
    let slot = hand_index_for_card(&engine, 0, "manhole_missile");
    engine
        .apply_scenario_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast Manhole Missile");
    resolve_top(&mut engine);
    engine
        .apply_scenario_command(0, &select_branch())
        .expect("choose the optional payment");
    let candidate = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("hand-card payment prompt")
        .presentation
        .candidates
        .iter()
        .copied()
        .find(|candidate| *candidate == commander)
        .expect("the exact declared commander is an eligible hand-card cost");

    let hand_before_owner_replacement = engine.state.players[0].hand.clone();
    let library_before_owner_replacement = engine.state.players[0].library.clone();
    let payment = engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![candidate]))
        .expect("pay the hand-card cost");
    assert!(
        payment.events.iter().any(|event| matches!(
            &event.ev,
            Some(Ev::ResolutionChoiceRequired(required))
                if required.deciding_player_id == 0
                    && required.choice_kind == ChoiceKind::ResolutionBranch as i32
                    && required.resolution_branches.len() == 2
                    && required.resolution_branches[0].label == "Command zone"
                    && required.resolution_branches[1].label.contains("library")
        )),
        "the commander owner must choose Command or Library before the child draw"
    );
    assert_eq!(engine.state.objects[&commander].zone, Zone::Hand);
    assert_eq!(engine.state.players[0].hand, hand_before_owner_replacement);
    assert_eq!(
        engine.state.players[0].library,
        library_before_owner_replacement
    );
}

#[test]
fn issue_240_declared_commander_destination_is_owner_only_atomic_and_resumes_once() {
    for destination in 0..=1 {
        let mut p0_mainboard = vec!["mountain".into(); 12];
        p0_mainboard.push("manhole_missile".into());
        let p0_deck = EngineDeck {
            mainboard: p0_mainboard,
            commanders: vec!["captain_storm,_cosmium_raider".into()],
        };
        let p1_deck = EngineDeck {
            mainboard: vec!["forest".into(); 12],
            commanders: Vec::new(),
        };
        let mut engine = GameEngine::new_with_commander_decks(
            tricerules_cards::registry::global(),
            240_009 + u64::from(destination),
            &[0, 1],
            20,
            Some(vec![p0_deck, p1_deck]),
            true,
        )
        .expect("declared commander fixture");
        advance_to_main1_from_game_start(&mut engine);
        let commander = *engine.state.players[0]
            .command_zone
            .first()
            .expect("Captain Storm began as P0's declared commander");
        engine.enable_dev_commands();
        engine
            .apply_scenario_command(
                0,
                &RuledCommand {
                    cmd: Some(Cmd::DevCommand(DevCommand {
                        target_player_id: 0,
                        dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                            card_name: "Captain Storm, Cosmium Raider".into(),
                            zone: DevZone::Hand as i32,
                            ready: false,
                        })),
                    })),
                },
            )
            .expect("return the declared commander to its owner's hand");
        ensure_card_in_hand(&mut engine, 0, "manhole_missile");
        give_mana(
            &mut engine,
            0,
            ManaGift {
                r: 1,
                c: 1,
                ..Default::default()
            },
        );
        let target = inject_creature_with_stats(&mut engine, 1, "indomitable_ancients", 2, 10);
        let spell =
            engine.state.players[0].hand[hand_index_for_card(&engine, 0, "manhole_missile")];
        let slot = hand_index_for_card(&engine, 0, "manhole_missile");
        let cast = engine
            .apply_scenario_command(0, &cast_spell(slot, target_object(target)))
            .expect("cast Manhole Missile");
        let resolve = resolve_top(&mut engine);
        let choose = engine
            .apply_scenario_command(0, &select_branch())
            .expect("choose the optional payment");
        let before_payment = engine.state.players[0].hand.clone();
        let candidate = *engine
            .state
            .pending_resolution
            .as_ref()
            .expect("private hand-card payment prompt")
            .presentation
            .candidates
            .iter()
            .find(|candidate| **candidate == commander)
            .expect("the declared commander is an eligible hand-card cost");
        let selected = engine
            .apply_scenario_command(0, &submit_resolution_choice(vec![candidate]))
            .expect("select the commander as payment");
        let pending = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("owner destination prompt is pending");
        assert_eq!(pending.deciding_player, 0);
        assert_eq!(
            pending.presentation.choice_kind,
            ChoiceKind::ResolutionBranch
        );
        assert_eq!(engine.state.players[0].hand, before_payment);
        assert_eq!(engine.state.objects[&commander].zone, Zone::Hand);
        let library_before_destination = engine.state.players[0].library.clone();
        assert_eq!(
            selected
                .events
                .iter()
                .filter(|event| matches!(
                    &event.ev,
                    Some(Ev::ResolutionChoiceRequired(required))
                        if required.deciding_player_id == 0
                            && required.choice_kind == ChoiceKind::ResolutionBranch as i32
                            && required.resolution_branches.len() == 2
                            && required.resolution_branches[0].label == "Command zone"
                            && required.resolution_branches[1].label == "Bottom of library"
                            && required.candidate_object_ids.is_empty()
                            && required.candidate_card_ids.is_empty()
                ))
                .count(),
            1,
            "the commander name stays out of the destination prompt"
        );

        let command = select_branch_index(destination);
        let before_wrong_player = serde_json::to_value(&engine.state).unwrap();
        assert!(engine.apply_scenario_command(1, &command).is_err());
        assert_eq!(
            serde_json::to_value(&engine.state).unwrap(),
            before_wrong_player
        );
        let active_prompt_id = engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .resolution_prompt_id
            .get();
        let mut stale_prompt = command.clone();
        let Some(Cmd::SubmitResolutionChoice(answer)) = stale_prompt.cmd.as_mut() else {
            panic!("the commander destination answer is a resolution choice");
        };
        answer.resolution_prompt_id = active_prompt_id + 1;
        let before_stale_prompt = serde_json::to_value(&engine.state).unwrap();
        assert!(engine.apply_scenario_command(0, &stale_prompt).is_err());
        assert_eq!(
            serde_json::to_value(&engine.state).unwrap(),
            before_stale_prompt
        );
        let generation = engine.state.zone_change_generation[&commander];
        *engine
            .state
            .zone_change_generation
            .get_mut(&commander)
            .unwrap() += 1;
        let before_stale = serde_json::to_value(&engine.state).unwrap();
        assert!(engine.apply_scenario_command(0, &command).is_err());
        assert_eq!(serde_json::to_value(&engine.state).unwrap(), before_stale);
        engine
            .state
            .zone_change_generation
            .insert(commander, generation);
        let malformed = select_branch_index(2);
        let before_malformed = serde_json::to_value(&engine.state).unwrap();
        assert!(engine.apply_scenario_command(0, &malformed).is_err());
        assert_eq!(
            serde_json::to_value(&engine.state).unwrap(),
            before_malformed
        );
        let answer = engine
            .apply_scenario_command(0, &command)
            .expect("commander owner chooses the destination");
        let expected_payment_log = if destination == 0 {
            "P0 pays by putting their commander into the command zone."
        } else {
            "P0 pays by putting a card on the bottom of their library."
        };
        assert_eq!(
            answer
                .events
                .iter()
                .filter(|event| matches!(
                    &event.ev,
                    Some(Ev::Log(log)) if log.text == expected_payment_log
                ))
                .count(),
            1,
            "the public payment receipt names the selected destination without the private card name"
        );
        let after_answer = serde_json::to_value(&engine.state).unwrap();
        assert!(engine.apply_scenario_command(0, &command).is_err());
        assert_eq!(serde_json::to_value(&engine.state).unwrap(), after_answer);

        if destination == 0 {
            assert_eq!(engine.state.objects[&commander].zone, Zone::Command);
            assert!(engine.state.players[0].command_zone.contains(&commander));
            assert_eq!(engine.state.players[0].hand.len(), before_payment.len());
            assert_eq!(
                engine.state.players[0].library.len() + 1,
                library_before_destination.len()
            );
        } else {
            assert_eq!(engine.state.objects[&commander].zone, Zone::Library);
            assert_eq!(engine.state.players[0].library.back(), Some(&commander));
            assert_eq!(engine.state.players[0].hand.len(), before_payment.len());
            assert_eq!(
                engine.state.players[0].library.len(),
                library_before_destination.len()
            );
        }
        let terminal_events = [cast, resolve, choose, selected, answer];
        let stack_resolved = terminal_events
            .iter()
            .flat_map(|batch| &batch.events)
            .filter(|event| matches!(&event.ev, Some(Ev::StackResolved(resolved)) if resolved.object_id == spell))
            .count();
        assert_eq!(
            stack_resolved, 1,
            "the spell resolves once after the owner answer"
        );
        let draw_logs = terminal_events
            .iter()
            .flat_map(|batch| &batch.events)
            .filter(|event| matches!(&event.ev, Some(Ev::Log(log)) if log.text.contains("draws 1 card")))
            .count();
        assert_eq!(draw_logs, 1, "the paid branch draws exactly once");
        assert!(engine.state.pending_resolution.is_none());
    }
}

#[test]
fn issue_240_departed_controller_cannot_delegate_hand_payment() {
    let (mut engine, target) = setup_three_player(240_013);
    cast_and_choose_branch(&mut engine, target);
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the controller is choosing a hand-card payment");
    assert_eq!(pending.presentation.choice_kind, ChoiceKind::HandCards);
    let chosen = *pending
        .presentation
        .candidates
        .iter()
        .find(|candidate| engine.state.objects[candidate].card_id == "mountain")
        .expect("an exact hand-card payment is available");
    let before_draws = engine.state.turn_history.current.player(0).cards_drawn;

    let departure = engine
        .apply_scenario_command(0, &concede())
        .expect("the active payment chooser may concede during the parked spell");

    assert!(engine.state.players[0].has_lost);
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert_eq!(
        engine.state.turn_history.current.player(0).cards_drawn,
        before_draws
    );
    assert!(!departure.events.iter().any(|event| matches!(
        &event.ev,
        Some(Ev::PermanentMoved(moved))
            if moved.object_id == chosen
                && matches!(moved.destination, x if x == Destination::Library as i32 || x == Destination::Command as i32)
    )));
    assert!(
        !departure.events.iter().any(|event| matches!(
            &event.ev,
            Some(Ev::ResolutionChoiceRequired(required))
                if required.choice_kind == ChoiceKind::HandCards as i32
                    && required.deciding_player_id != 0
        )),
        "the payment prompt is not delegated to a surviving player"
    );
}

#[test]
fn issue_240_two_player_controller_departure_at_each_payment_wait_is_terminal() {
    for commander_destination_wait in [false, true] {
        let seed = if commander_destination_wait {
            240_016
        } else {
            240_015
        };
        let (mut engine, target, commander) = if commander_destination_wait {
            let mut p0_mainboard = vec!["mountain".into(); 12];
            p0_mainboard.push("manhole_missile".into());
            let p0_deck = EngineDeck {
                mainboard: p0_mainboard,
                commanders: vec!["captain_storm,_cosmium_raider".into()],
            };
            let p1_deck = EngineDeck {
                mainboard: vec!["forest".into(); 12],
                commanders: Vec::new(),
            };
            let mut engine = GameEngine::new_with_commander_decks(
                tricerules_cards::registry::global(),
                seed,
                &[0, 1],
                20,
                Some(vec![p0_deck, p1_deck]),
                true,
            )
            .expect("two-player declared-commander fixture");
            advance_to_main1_from_game_start(&mut engine);
            let commander = *engine.state.players[0]
                .command_zone
                .first()
                .expect("Captain Storm began as P0's declared commander");
            engine.enable_dev_commands();
            engine
                .apply_scenario_command(
                    0,
                    &RuledCommand {
                        cmd: Some(Cmd::DevCommand(DevCommand {
                            target_player_id: 0,
                            dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                                card_name: "Captain Storm, Cosmium Raider".into(),
                                zone: DevZone::Hand as i32,
                                ready: false,
                            })),
                        })),
                    },
                )
                .expect("return the declared commander to its owner's hand");
            ensure_card_in_hand(&mut engine, 0, "manhole_missile");
            give_mana(
                &mut engine,
                0,
                ManaGift {
                    r: 1,
                    c: 1,
                    ..Default::default()
                },
            );
            let target = inject_creature_with_stats(&mut engine, 1, "indomitable_ancients", 2, 10);
            (engine, target, Some(commander))
        } else {
            let (engine, target) = setup(seed);
            (engine, target, None)
        };

        let slot = hand_index_for_card(&engine, 0, "manhole_missile");
        engine
            .apply_scenario_command(0, &cast_spell(slot, target_object(target)))
            .expect("cast the admitted Controller spell");
        resolve_top(&mut engine);
        engine
            .apply_scenario_command(0, &select_branch())
            .expect("choose the optional payment");
        let chosen = if let Some(commander) = commander {
            engine
                .apply_scenario_command(0, &submit_resolution_choice(vec![commander]))
                .expect("select the declared commander as payment");
            commander
        } else {
            let pending = engine
                .state
                .pending_resolution
                .as_ref()
                .expect("hand-card payment prompt");
            assert_eq!(pending.presentation.choice_kind, ChoiceKind::HandCards);
            *pending
                .presentation
                .candidates
                .iter()
                .find(|candidate| engine.state.objects[candidate].card_id == "mountain")
                .expect("a hand-card candidate is available")
        };
        let before_draws = engine.state.turn_history.current.player(0).cards_drawn;

        let departure = engine
            .apply_scenario_command(0, &concede())
            .expect("the Controller may concede during either parked payment prompt");

        assert!(engine.state.is_terminal());
        assert_eq!(engine.state.winner(), Some(1));
        assert!(engine.state.pending_resolution.is_none());
        assert!(engine.state.stack.is_empty());
        assert_eq!(
            engine.state.turn_history.current.player(0).cards_drawn,
            before_draws
        );
        assert!(!departure.events.iter().any(|event| matches!(
            &event.ev,
            Some(Ev::PermanentMoved(moved))
                if moved.object_id == chosen
                    && (moved.destination == Destination::Library as i32
                        || moved.destination == Destination::Command as i32)
        )));
        assert!(
            !departure.events.iter().any(|event| matches!(
                &event.ev,
                Some(Ev::ResolutionChoiceRequired(required))
                    if required.deciding_player_id != 0
                        && (required.choice_kind == ChoiceKind::HandCards as i32
                            || required.choice_kind == ChoiceKind::ResolutionBranch as i32)
            )),
            "terminal cleanup does not hand the payment or destination choice to another player"
        );
    }
}

#[test]
fn issue_240_departed_controller_cannot_delegate_commander_destination() {
    let mut p0_mainboard = vec!["mountain".into(); 12];
    p0_mainboard.push("manhole_missile".into());
    let p0_deck = EngineDeck {
        mainboard: p0_mainboard,
        commanders: vec!["captain_storm,_cosmium_raider".into()],
    };
    let other_deck = EngineDeck {
        mainboard: vec!["forest".into(); 12],
        commanders: Vec::new(),
    };
    let mut engine = GameEngine::new_with_commander_decks(
        tricerules_cards::registry::global(),
        240_014,
        &[0, 1, 2],
        20,
        Some(vec![p0_deck, other_deck.clone(), other_deck]),
        true,
    )
    .expect("three-player declared-commander fixture");
    advance_to_main1_from_game_start(&mut engine);
    let commander = *engine.state.players[0]
        .command_zone
        .first()
        .expect("Captain Storm began as P0's declared commander");
    engine.enable_dev_commands();
    engine
        .apply_scenario_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: 0,
                    dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                        card_name: "Captain Storm, Cosmium Raider".into(),
                        zone: DevZone::Hand as i32,
                        ready: false,
                    })),
                })),
            },
        )
        .expect("return the declared commander object to its owner's hand");
    ensure_card_in_hand(&mut engine, 0, "manhole_missile");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            c: 1,
            ..Default::default()
        },
    );
    let target = inject_creature_with_stats(&mut engine, 1, "indomitable_ancients", 2, 10);
    let slot = hand_index_for_card(&engine, 0, "manhole_missile");
    engine
        .apply_scenario_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast Manhole Missile");
    resolve_top(&mut engine);
    engine
        .apply_scenario_command(0, &select_branch())
        .expect("choose the optional payment");
    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![commander]))
        .expect("select the declared commander as the hand-card payment");
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
    let before_draws = engine.state.turn_history.current.player(0).cards_drawn;

    let departure = engine
        .apply_scenario_command(0, &concede())
        .expect("the commander destination chooser may concede while the payment is parked");

    assert!(engine.state.players[0].has_lost);
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert_eq!(
        engine.state.turn_history.current.player(0).cards_drawn,
        before_draws
    );
    assert!(
        !departure.events.iter().any(|event| matches!(
            &event.ev,
            Some(Ev::PermanentMoved(moved))
                if moved.object_id == commander
                    && (moved.destination == Destination::Library as i32
                        || moved.destination == Destination::Command as i32)
        )),
        "the departed owner cannot choose a destination or transfer the cost"
    );
    assert!(
        !departure.events.iter().any(|event| matches!(
            &event.ev,
            Some(Ev::ResolutionChoiceRequired(required))
                if required.choice_kind == ChoiceKind::ResolutionBranch as i32
                    && required.deciding_player_id != 0
        )),
        "the Commander destination prompt is not delegated to a surviving player"
    );
}

#[test]
fn issue_240_damage_replacement_resumes_one_paid_branch_and_lexical_tail() {
    use tricerules_core::state::{
        ActiveDamagePrevention, DamagePreventionAmount, DamagePreventionScope,
        ReplacementSourcePresentation,
    };

    let program = r#"(
        id: "hand_bottom_damage_replacement_fixture",
        name: "Hand Bottom Damage Replacement Fixture",
        face_id: "hand_bottom_damage_replacement_fixture",
        mana_cost: "{0}",
        types: ["Instant"],
        spell_effect: [],
        spell_program: Some((body: (
            steps: [
                (id: "damage", kind: Perform(DamageTarget(
                    amount: 1, target: (kind: Creature)
                ))),
                (id: "bottom_choice", kind: Choose(
                    choice: "bottom_choice", chooser: Controller, optional: true,
                    branches: [(
                        branch_id: "bottom", presentation: Fallback,
                        cost: PutHandCardOnLibraryBottom, requirement: Always,
                        body: (steps: [(id: "draw_one", kind: Perform(Draw(
                            who: Controller, count: 1
                        )))], yields: []),
                    )],
                    otherwise: (steps: [], yields: []),
                )),
                (id: "lexical_tail", kind: Perform(GainLife(amount: 1))),
            ], yields: []
        ), target_bindings: [
            (instruction_id: "damage", role: Target, target_group_index: 0)
        ])),
        targeting: Some((groups: [(min: 1, max: 1,
            prompt: "Choose target creature", effect_indices: [])]))
    )"#;
    let registry = Box::leak(Box::new(
        tricerules_card_model::CardRegistry::from_chunks_and_tokens(
            &[
                program,
                include_str!("../../../tricerules-cards/data/mountain.ron"),
                include_str!("../../../tricerules-cards/data/forest.ron"),
                include_str!("../../../tricerules-cards/data/generated/i/indomitable_ancients.ron"),
            ],
            &[],
        )
        .expect("the synthetic paid Program fixture compiles"),
    ));
    let decks = Some(vec![vec!["mountain".into(); 20], vec!["forest".into(); 20]]);
    let mut engine = GameEngine::new(registry, 240_012, &[0, 1], 20, decks, true)
        .expect("hand-bottom replacement fixture engine");
    advance_to_main1_from_game_start(&mut engine);
    let target = inject_creature_with_stats(&mut engine, 1, "indomitable_ancients", 2, 10);
    let first_effect_id = engine.state.next_damage_prevention_effect_id;
    engine.state.damage_prevention_effects = [first_effect_id, first_effect_id + 1]
        .into_iter()
        .map(|id| ActiveDamagePrevention {
            id,
            static_origin: None,
            source_id: None,
            source_label: format!("test prevention {id}"),
            source_presentation: ReplacementSourcePresentation::default(),
            scope: DamagePreventionScope::Recipient(target),
            amount: DamagePreventionAmount::All,
            duration: tricerules_card_model::primitives::EffectDuration::UntilEndOfTurn,
            additional_effect: None,
        })
        .collect();
    engine.state.next_damage_prevention_effect_id = first_effect_id + 2;
    let fixture_card =
        inject_card_into_hand(&mut engine, 0, "hand_bottom_damage_replacement_fixture");
    let spell_slot = hand_index_for_card(&engine, 0, "hand_bottom_damage_replacement_fixture");
    let before_draws = engine.state.turn_history.current.player(0).cards_drawn;
    let before_life = engine.state.players[0].life;
    let cast = engine
        .apply_scenario_command(0, &cast_spell(spell_slot, target_object(target)))
        .expect("cast the damage and hand-bottom Program");
    let resolution = resolve_top(&mut engine);
    assert_eq!(choice_count(&resolution, ChoiceKind::ReplacementEffect), 1);
    assert_eq!(choice_count(&resolution, ChoiceKind::ResolutionBranch), 0);
    let replacement = resolution
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::ResolutionChoiceRequired(required))
                if required.choice_kind == ChoiceKind::ReplacementEffect as i32 =>
            {
                Some(required)
            }
            _ => None,
        })
        .expect("the target's applicable replacement effects require an order");
    assert_eq!(replacement.deciding_player_id, 1);
    let replacement_id = replacement.replacement_options[0].application_id;
    assert!(matches!(
        &engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .continuation,
        tricerules_core::state::ResolutionContinuation::DamageReplacement { .. }
    ));
    assert_eq!(engine.state.objects[&target].damage, 0);
    let replacement_batch = engine
        .apply_scenario_command(1, &submit_resolution_choice(vec![replacement_id]))
        .expect("choose one replacement and resume the parked Program");
    assert_eq!(
        choice_count(&replacement_batch, ChoiceKind::ReplacementEffect),
        0
    );
    assert_eq!(
        choice_count(&replacement_batch, ChoiceKind::ResolutionBranch),
        1
    );
    assert_eq!(engine.state.objects[&target].damage, 0);
    let branch_choice = engine
        .apply_scenario_command(0, &select_branch())
        .expect("the resumed Program offers its paid branch once");
    assert_eq!(choice_count(&branch_choice, ChoiceKind::HandCards), 1);
    let chosen = *engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the paid branch parks for its hand-card cost")
        .presentation
        .candidates
        .iter()
        .find(|candidate| {
            *candidate != &fixture_card && engine.state.objects[candidate].card_id == "mountain"
        })
        .expect("an exact Mountain hand-card candidate is available");
    let drawn = *engine.state.players[0]
        .library
        .front()
        .expect("the Program has a card to draw");
    let payment = engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![chosen]))
        .expect("pay once and resume the child draw and lexical tail");
    assert_eq!(engine.state.objects[&chosen].zone, Zone::Library);
    assert_eq!(engine.state.players[0].library.back(), Some(&chosen));
    assert!(engine.state.players[0].hand.contains(&drawn));
    assert_eq!(
        engine.state.turn_history.current.player(0).cards_drawn,
        before_draws + 1
    );
    assert_eq!(engine.state.players[0].life, before_life + 1);
    assert_eq!(choice_count(&payment, ChoiceKind::HandCards), 0);
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    let terminal_events = [cast, resolution, replacement_batch, branch_choice, payment];
    assert_eq!(
        terminal_events
            .iter()
            .flat_map(|batch| &batch.events)
            .filter(|event| matches!(
                &event.ev,
                Some(Ev::StackResolved(resolved)) if resolved.object_id == fixture_card
            ))
            .count(),
        1,
        "the resumed Program completes its stack object once"
    );
}
