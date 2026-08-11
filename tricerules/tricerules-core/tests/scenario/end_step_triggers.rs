//! CR 513.1-2 / 603.4 beginning-of-end-step triggers and their intervening-if conditions.

use crate::helpers::*;
use tricerules_cards::CounterKind;
use tricerules_proto::ruled::v1::RuledEventBatch;

fn end_step_engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[]), deck_with("island", &[])]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn advance_to_end_step(engine: &mut GameEngine, active: i32) {
    engine
        .apply_command(active, &primitive_yield())
        .expect("main 1 to beginning of combat");
    engine
        .apply_command(active, &primitive_yield())
        .expect("beginning of combat advance");
    if engine.state.turn_step == tricerules_core::TurnStep::DeclareAttackers {
        engine
            .apply_command(active, &primitive_yield())
            .expect("declare no attackers");
    }
    engine
        .apply_command(active, &primitive_yield())
        .expect("end combat to main 2");
    engine
        .apply_command(active, &primitive_yield())
        .expect("main 2 to end step");
    assert_eq!(engine.state.turn_step, tricerules_core::TurnStep::EndStep);
}

fn resolve_stack_collecting_batches(engine: &mut GameEngine) -> Vec<RuledEventBatch> {
    let mut batches = Vec::new();
    while !engine.state.stack.is_empty() {
        answer_trigger_order_in_engine_order(engine);
        let first = engine.state.priority_player_id();
        let second = if first == engine.state.players[0].id {
            engine.state.players[1].id
        } else {
            engine.state.players[0].id
        };
        batches.push(
            engine
                .apply_command(first, &pass())
                .expect("first priority pass"),
        );
        batches.push(
            engine
                .apply_command(second, &pass())
                .expect("second priority pass"),
        );
    }
    batches
}

#[test]
fn sabertooth_mauler_triggers_once_after_one_or_more_creature_deaths() {
    let mut engine = end_step_engine(6001);
    let mauler = inject_creature_on_battlefield(&mut engine, 0, "sabertooth_mauler");
    engine
        .state
        .objects
        .get_mut(&mauler)
        .expect("mauler")
        .tapped = true;
    engine.state.turn_history.current.creatures_died = 2;

    advance_to_end_step(&mut engine, 0);

    assert_eq!(
        engine.state.stack.len(),
        1,
        "one trigger, not one per death"
    );
    assert!(
        engine.state.objects[&mauler].tapped,
        "effect has not resolved"
    );
    assert_eq!(
        engine.state.objects[&mauler].counter_count(CounterKind::PlusOnePlusOne),
        0
    );

    let batches = resolve_stack_collecting_batches(&mut engine);
    assert_eq!(
        engine.state.objects[&mauler].counter_count(CounterKind::PlusOnePlusOne),
        1
    );
    assert!(!engine.state.objects[&mauler].tapped, "the Mauler untaps");
    assert!(
        batches.iter().any(|batch| batch.events.iter().any(|event| {
            matches!(
                event.ev.as_ref(),
                Some(Ev::PermanentsUntapped(untapped)) if untapped.object_ids == [mauler]
            )
        })),
        "the physical untap edge is emitted"
    );
}

#[test]
fn twinblade_assassins_draws_once_when_a_creature_died() {
    let decks = Some(vec![
        deck_with("swamp", &["murder"]),
        deck_with("forest", &["grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(6002, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut engine);
    inject_creature_on_battlefield(&mut engine, 0, "twinblade_assassins");
    let bear = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", false);
    ensure_in_hand(&mut engine, 0, "murder");
    grant_pool(&mut engine, 0);
    let murder = hand_index_for_card(&engine, 0, "murder");
    engine
        .apply_command(0, &cast_spell(murder, targets_with_damage(vec![(bear, 0)])))
        .expect("cast Murder");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.turn_history.current.creatures_died, 1);
    let library_before = engine.state.players[0].library.len();

    advance_to_end_step(&mut engine, 0);
    assert_eq!(engine.state.stack.len(), 1);
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.players[0].library.len(), library_before - 1);
}

#[test]
fn end_step_condition_and_controller_scope_suppress_triggers() {
    let mut no_death = end_step_engine(6003);
    inject_creature_on_battlefield(&mut no_death, 0, "sabertooth_mauler");
    advance_to_end_step(&mut no_death, 0);
    assert!(
        no_death.state.stack.is_empty(),
        "no creature died this turn"
    );

    let mut opponent_step = end_step_engine(6004);
    inject_creature_on_battlefield(&mut opponent_step, 1, "twinblade_assassins");
    opponent_step.state.turn_history.current.creatures_died = 1;
    advance_to_end_step(&mut opponent_step, 0);
    assert!(
        opponent_step.state.stack.is_empty(),
        "your-end-step trigger stays silent on an opponent's end step"
    );
}

#[test]
fn end_step_trigger_observes_only_sources_present_when_the_step_begins() {
    let mut late = end_step_engine(6005);
    inject_creature_on_battlefield(&mut late, 0, "sabertooth_mauler");
    advance_to_end_step(&mut late, 0);
    late.state.turn_history.current.creatures_died = 1;
    assert!(
        late.state.stack.is_empty(),
        "a death after step start is not retroactive"
    );

    let mut late_source = end_step_engine(6009);
    late_source.state.turn_history.current.creatures_died = 1;
    advance_to_end_step(&mut late_source, 0);
    inject_creature_on_battlefield(&mut late_source, 0, "sabertooth_mauler");
    assert!(
        late_source.state.stack.is_empty(),
        "a late source does not trigger"
    );

    let mut timely = end_step_engine(6006);
    timely.state.turn_history.current.creatures_died = 1;
    inject_creature_on_battlefield(&mut timely, 0, "sabertooth_mauler");
    advance_to_end_step(&mut timely, 0);
    assert_eq!(
        timely.state.stack.len(),
        1,
        "a source present at step start triggers"
    );
}

#[test]
fn end_step_condition_is_checked_again_on_resolution() {
    let mut engine = end_step_engine(6010);
    inject_creature_on_battlefield(&mut engine, 0, "twinblade_assassins");
    engine.state.turn_history.current.creatures_died = 1;
    let library_before = engine.state.players[0].library.len();
    advance_to_end_step(&mut engine, 0);
    assert_eq!(engine.state.stack.len(), 1);

    // The real death count is monotonic within a turn. Mutating the public test state proves the
    // generic CR 603.4 resolution check is nevertheless performed through `condition_holds`.
    engine.state.turn_history.current.creatures_died = 0;
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.players[0].library.len(), library_before);
}

#[test]
fn simultaneous_end_step_triggers_use_the_normal_ordering_flow() {
    let mut engine = end_step_engine(6007);
    inject_creature_on_battlefield(&mut engine, 0, "sabertooth_mauler");
    inject_creature_on_battlefield(&mut engine, 0, "twinblade_assassins");
    engine.state.turn_history.current.creatures_died = 1;

    advance_to_end_step(&mut engine, 0);

    let pending = engine
        .state
        .pending_trigger_order
        .as_ref()
        .expect("the controller orders two simultaneous triggers");
    assert_eq!(pending.deciding_player, 0);
    assert_eq!(pending.candidates.len(), 2);
    assert!(
        engine.state.stack.is_empty(),
        "ordering precedes stack placement"
    );
}

#[test]
fn source_that_changes_object_generation_gets_neither_mauler_effect() {
    let decks = Some(vec![
        deck_with("forest", &["sabertooth_mauler"]),
        deck_with("island", &["boomerang"]),
    ]);
    let mut engine = GameEngine::new(6008, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut engine);
    let mauler = relocate_to_battlefield(&mut engine, 0, "sabertooth_mauler", true);
    ensure_in_hand(&mut engine, 1, "boomerang");
    engine.state.turn_history.current.creatures_died = 1;
    advance_to_end_step(&mut engine, 0);
    assert_eq!(engine.state.stack.len(), 1);

    engine
        .apply_command(0, &pass())
        .expect("active player passes");
    grant_pool(&mut engine, 1);
    let boomerang = hand_index_for_card(&engine, 1, "boomerang");
    engine
        .apply_command(
            1,
            &cast_spell(boomerang, targets_with_damage(vec![(mauler, 0)])),
        )
        .expect("bounce Mauler in response");
    pass_both_players(&mut engine);
    assert_eq!(
        engine.state.objects[&mauler].zone,
        tricerules_core::Zone::Hand
    );

    let hand_pos = engine.state.players[0]
        .hand
        .iter()
        .position(|oid| *oid == mauler)
        .expect("Mauler returned to hand");
    engine.state.players[0].hand.remove(hand_pos);
    engine.state.players[0].battlefield.push(mauler);
    let object = engine
        .state
        .objects
        .get_mut(&mauler)
        .expect("Mauler object");
    object.zone = tricerules_core::Zone::Battlefield;
    object.tapped = true;

    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&mauler].counter_count(CounterKind::PlusOnePlusOne),
        0,
        "the old trigger cannot affect the new object"
    );
    assert!(
        engine.state.objects[&mauler].tapped,
        "untap also checks identity"
    );
}
