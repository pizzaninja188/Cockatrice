use super::helpers::*;
use tricerules_core::{GameEngine, Zone};
use tricerules_proto::ruled::v1::{
    permanent_moved::Destination, ruled_command::Cmd, ruled_event::Ev, ChoiceKind,
    ResolutionChoiceDecision, RuledCommand, RuledEventBatch, SubmitResolutionChoice,
};

fn select_branch() -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            decision: ResolutionChoiceDecision::SelectBranch as i32,
            selected_branch_index: 0,
            ..Default::default()
        })),
    }
}

fn resolve_top(engine: &mut GameEngine) -> RuledEventBatch {
    let mut batch = None;
    for _ in 0..engine.state.players.len() {
        batch = Some(
            engine
                .apply_command(engine.state.priority_player_id(), &pass())
                .expect("priority pass"),
        );
    }
    batch.expect("resolution batch")
}

fn setup(seed: u64) -> (GameEngine, u32) {
    let mut engine = GameEngine::new(
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

fn cast_and_choose_branch(engine: &mut GameEngine, target: u32) -> RuledEventBatch {
    let slot = hand_index_for_card(engine, 0, "manhole_missile");
    engine
        .apply_command(0, &cast_spell(slot, target_object(target)))
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
    engine.apply_command(0, &select_branch()).unwrap()
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
        .apply_command(0, &submit_resolution_choice(vec![chosen]))
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
        .apply_command(0, &submit_resolution_choice(vec![]))
        .unwrap();
    assert_eq!(declined.state.players[0].hand, hand_before);
    assert_eq!(declined.state.players[0].library, library_before);

    let (mut fizzled, target) = setup(240_003);
    let slot = hand_index_for_card(&fizzled, 0, "manhole_missile");
    fizzled
        .apply_command(0, &cast_spell(slot, target_object(target)))
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
        .apply_command(1, &submit_resolution_choice(vec![chosen]))
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
        .apply_command(0, &submit_resolution_choice(vec![chosen]))
        .is_err());
    assert_eq!(serde_json::to_value(&engine.state).unwrap(), before_stale);

    *engine
        .state
        .zone_change_generation
        .get_mut(&chosen)
        .unwrap() -= 1;
    let command = submit_resolution_choice(vec![chosen]);
    engine.apply_command(0, &command).unwrap();
    let after_payment = serde_json::to_value(&engine.state).unwrap();
    assert!(engine.apply_command(0, &command).is_err());
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
        .apply_command(0, &cast_spell(0, target_object(target)))
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
        .apply_command(0, &submit_resolution_choice(vec![chosen]))
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
            .apply_command(0, &cast_spell(slot, target_object(target)))
            .unwrap();
        let resolve = resolve_top(&mut engine);
        let branch = engine.apply_command(0, &select_branch()).unwrap();
        let chosen = engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .presentation
            .candidates[0];
        let payment = engine
            .apply_command(0, &submit_resolution_choice(vec![chosen]))
            .unwrap();
        (
            vec![cast, resolve, branch, payment],
            serde_json::to_value(&engine.state).unwrap(),
        )
    }

    assert_eq!(run(), run());
}
