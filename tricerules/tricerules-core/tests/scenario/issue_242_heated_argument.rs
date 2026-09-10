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
    batch.expect("two-player resolution batch")
}

fn setup(seed: u64, with_graveyard_card: bool) -> (GameEngine, u32, Option<u32>) {
    let mut engine = GameEngine::new(
        seed,
        &[0, 1],
        20,
        Some(vec![
            deck_with("mountain", &["heated_argument"]),
            forest_only_deck(),
        ]),
        true,
    )
    .expect("Heated Argument is registered");
    advance_to_main1_from_game_start(&mut engine);
    ensure_card_in_hand(&mut engine, 0, "heated_argument");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            c: 4,
            ..Default::default()
        },
    );
    let target = inject_creature_with_stats(&mut engine, 1, "indomitable_ancients", 2, 10);
    let graveyard = with_graveyard_card.then(|| inject_graveyard_card(&mut engine, 0, "mountain"));
    (engine, target, graveyard)
}

fn cast_and_park(engine: &mut GameEngine, target: u32) -> RuledEventBatch {
    let slot = hand_index_for_card(engine, 0, "heated_argument");
    engine
        .apply_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast Heated Argument");
    let batch = resolve_top(engine);
    assert_eq!(engine.state.objects[&target].damage, 6);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .expect("optional exile branch")
            .presentation
            .choice_kind,
        ChoiceKind::ResolutionBranch
    );
    batch
}

#[test]
fn issue_242_payment_exiles_exact_card_and_damages_current_controller() {
    let (mut engine, target, graveyard) = setup(242_001, true);
    let graveyard = graveyard.unwrap();
    cast_and_park(&mut engine, target);
    engine.apply_command(0, &select_branch()).unwrap();
    let pending = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(pending.presentation.choice_kind, ChoiceKind::GraveyardCards);
    assert_eq!(pending.presentation.candidates, vec![graveyard]);

    engine.state.players[1]
        .battlefield
        .retain(|oid| *oid != target);
    engine.state.players[0].battlefield.push(target);
    let target_object = engine.state.objects.get_mut(&target).unwrap();
    target_object.base_controller = 0;
    target_object.controller = 0;

    let batch = engine
        .apply_command(0, &submit_resolution_choice(vec![graveyard]))
        .expect("pay by exiling the selected incarnation");
    assert_eq!(engine.state.objects[&graveyard].zone, Zone::Exile);
    assert_eq!(engine.state.players[0].life, 18);
    assert_eq!(engine.state.players[1].life, 20);
    assert!(batch.events.iter().any(|event| matches!(
        &event.ev,
        Some(Ev::PermanentMoved(moved))
            if moved.object_id == graveyard
                && moved.destination == Destination::Exile as i32
    )));
}

#[test]
fn issue_242_decline_and_empty_graveyard_skip_the_contingent_damage() {
    let (mut declined, target, graveyard) = setup(242_002, true);
    cast_and_park(&mut declined, target);
    declined
        .apply_command(
            0,
            &submit_resolution_decision(ResolutionChoiceDecision::Decline),
        )
        .unwrap();
    assert_eq!(
        declined.state.objects[&graveyard.unwrap()].zone,
        Zone::Graveyard
    );
    assert_eq!(declined.state.players[1].life, 20);

    let (mut empty, target, _) = setup(242_003, false);
    let slot = hand_index_for_card(&empty, 0, "heated_argument");
    empty
        .apply_command(0, &cast_spell(slot, target_object(target)))
        .unwrap();
    resolve_top(&mut empty);
    assert!(empty.state.pending_resolution.is_none());
    assert_eq!(empty.state.objects[&target].damage, 6);
    assert_eq!(empty.state.players[1].life, 20);
}

#[test]
fn issue_242_invalid_graveyard_selections_are_atomic() {
    for case in 0..4 {
        let (mut engine, target, graveyard) = setup(242_010 + case, true);
        let graveyard = graveyard.unwrap();
        let opposing = inject_graveyard_card(&mut engine, 1, "forest");
        cast_and_park(&mut engine, target);
        engine.apply_command(0, &select_branch()).unwrap();
        let chosen = match case {
            0 => vec![opposing],
            1 => vec![graveyard, graveyard],
            2 => {
                *engine
                    .state
                    .zone_change_generation
                    .entry(graveyard)
                    .or_default() += 1;
                vec![graveyard]
            }
            3 => {
                engine.state.objects.get_mut(&graveyard).unwrap().zone = Zone::Hand;
                engine.state.players[0]
                    .graveyard
                    .retain(|oid| *oid != graveyard);
                engine.state.players[0].hand.push(graveyard);
                vec![graveyard]
            }
            _ => unreachable!(),
        };
        let before = serde_json::to_value(&engine.state).unwrap();
        assert!(engine
            .apply_command(0, &submit_resolution_choice(chosen))
            .is_err());
        assert_eq!(serde_json::to_value(&engine.state).unwrap(), before);
        assert_eq!(engine.state.players[1].life, 20);
    }
}

#[test]
fn issue_242_illegal_sole_target_fizzles_before_damage_or_payment() {
    let (mut engine, target, graveyard) = setup(242_020, true);
    let slot = hand_index_for_card(&engine, 0, "heated_argument");
    engine
        .apply_command(0, &cast_spell(slot, target_object(target)))
        .unwrap();
    engine.state.players[1]
        .battlefield
        .retain(|oid| *oid != target);
    engine.state.players[1].graveyard.push(target);
    engine.state.objects.get_mut(&target).unwrap().zone = Zone::Graveyard;
    *engine
        .state
        .zone_change_generation
        .entry(target)
        .or_default() += 1;
    resolve_top(&mut engine);
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.players[1].life, 20);
    assert_eq!(
        engine.state.objects[&graveyard.unwrap()].zone,
        Zone::Graveyard
    );
}

#[test]
fn issue_242_accepted_commands_replay_identically() {
    fn run() -> (Vec<RuledEventBatch>, serde_json::Value) {
        let (mut engine, target, graveyard) = setup(242_030, true);
        let graveyard = graveyard.unwrap();
        let slot = hand_index_for_card(&engine, 0, "heated_argument");
        let cast = engine
            .apply_command(0, &cast_spell(slot, target_object(target)))
            .unwrap();
        let resolve = resolve_top(&mut engine);
        let branch = engine.apply_command(0, &select_branch()).unwrap();
        let payment = engine
            .apply_command(0, &submit_resolution_choice(vec![graveyard]))
            .unwrap();
        (
            vec![cast, resolve, branch, payment],
            serde_json::to_value(&engine.state).unwrap(),
        )
    }

    assert_eq!(run(), run());
}
