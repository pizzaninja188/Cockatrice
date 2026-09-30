//! Exact pinned-deck coverage for Greater Good.
//!
//! Oracle and rulings checked 2026-09-30. CR 602.2 and 701.21a govern activation and sacrificing
//! the creature as a cost; CR 608.2h uses its last known power for the draw count. CR 121.2 and
//! 701.9a govern drawing, then discarding three cards during the same resolution.

use super::helpers::*;
use tricerules_cards::CounterKind;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::ruled_command::Cmd;

const GREATER_GOOD: &str = "greater_good";

fn greater_good_engine(seed: u64) -> GameEngine {
    let mut engine = GameEngine::new(
        seed,
        &[0, 1],
        20,
        Some(vec![deck_with("island", &[]), deck_with("forest", &[])]),
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn activate_greater_good(engine: &GameEngine, source: u32, creature: u32) -> RuledCommand {
    let mut command = activate_ability_with_costs(
        source,
        0,
        vec![],
        vec![permanent_cost_selection(0, creature)],
    );
    let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
        unreachable!("constructed Greater Good activation")
    };
    activation.expected_zone_change_generation = engine
        .state
        .zone_change_generation
        .get(&source)
        .copied()
        .unwrap_or(0);
    command
}

fn resolve_top_ability(engine: &mut GameEngine) -> RuledEventBatch {
    let first = engine.state.priority_player_id();
    let second = if first == engine.state.players[0].id {
        engine.state.players[1].id
    } else {
        engine.state.players[0].id
    };
    semantic::accepted(engine, first, &pass());
    semantic::accepted(engine, second, &pass())
}

#[test]
fn greater_good_uses_sacrificed_creatures_last_known_power_then_discards_three() {
    let mut engine = greater_good_engine(20_260_930);
    let source = inject_permanent_on_battlefield(&mut engine, 0, GREATER_GOOD);
    let sacrificed = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    engine
        .state
        .objects
        .get_mut(&sacrificed)
        .unwrap()
        .add_counters(CounterKind::PlusOnePlusOne, 1, engine.state.command_index);
    assert_eq!(engine.effective_power(sacrificed), Some(3));

    let drawn = authoring_fixture::library_top(
        &mut engine,
        0,
        &["serra_angel", "hill_giant", "storm_crow"],
    );
    let hand_before = engine.state.players[0].hand.clone();
    let activation = activate_greater_good(&engine, source, sacrificed);
    semantic::accepted(&mut engine, 0, &activation);

    assert_eq!(engine.state.objects[&sacrificed].zone, Zone::Graveyard);
    assert_eq!(engine.state.stack.len(), 1);
    assert_eq!(engine.state.players[0].hand, hand_before);

    let choice_batch = resolve_top_ability(&mut engine);
    let choice = find_resolution_choice(&choice_batch).expect("mandatory three-card discard");
    assert_eq!(choice.deciding_player_id, 0);
    assert_eq!(choice.choice_kind(), ChoiceKind::HandCards);
    assert_eq!((choice.min, choice.max), (3, 3));
    assert!(drawn
        .iter()
        .all(|object_id| choice.candidate_object_ids.contains(object_id)));

    let discarded = vec![hand_before[0], drawn[0], drawn[1]];
    semantic::accepted(
        &mut engine,
        choice.deciding_player_id,
        &submit_resolution_choice(discarded.clone()),
    );
    assert!(discarded
        .iter()
        .all(|object_id| engine.state.objects[object_id].zone == Zone::Graveyard));
    assert_eq!(engine.state.objects[&drawn[2]].zone, Zone::Hand);
    assert_eq!(engine.state.players[0].hand.len(), hand_before.len());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn greater_good_rejects_an_opponents_creature_or_a_noncreature_as_its_cost() {
    let mut engine = greater_good_engine(20_260_931);
    let source = inject_permanent_on_battlefield(&mut engine, 0, GREATER_GOOD);
    let opponent_creature = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let own_land = inject_permanent_on_battlefield(&mut engine, 0, "island");

    for permanent in [opponent_creature, own_land] {
        let command = activate_greater_good(&engine, source, permanent);
        assert!(engine.apply_command(0, &command).is_err());
        assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
        assert_eq!(engine.state.objects[&permanent].zone, Zone::Battlefield);
        assert!(engine.state.stack.is_empty());
    }
}

#[test]
fn greater_good_with_zero_power_draws_zero_and_discards_a_smaller_hand() {
    let mut engine = GameEngine::new(
        20_260_932,
        &[0, 1],
        20,
        Some(vec![
            deck_with("island", &["ornithopter"]),
            deck_with("forest", &[]),
        ]),
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let source = inject_permanent_on_battlefield(&mut engine, 0, GREATER_GOOD);
    let sacrificed = move_ready_to_battlefield(&mut engine, 0, "ornithopter");
    assert_eq!(engine.effective_power(sacrificed), Some(0));

    while let Some(object_id) = engine.state.players[0].hand.pop() {
        engine.state.objects.get_mut(&object_id).unwrap().zone = Zone::Graveyard;
        engine.state.players[0].graveyard.push(object_id);
    }
    let only_card = inject_card_into_hand(&mut engine, 0, "grizzly_bears");
    let command = activate_greater_good(&engine, source, sacrificed);
    semantic::accepted(&mut engine, 0, &command);
    assert_eq!(engine.state.objects[&sacrificed].zone, Zone::Graveyard);

    let choice_batch = resolve_top_ability(&mut engine);
    let choice = find_resolution_choice(&choice_batch).expect("discard the available hand");
    assert_eq!((choice.min, choice.max), (1, 1));
    assert_eq!(choice.candidate_object_ids, vec![only_card]);
    semantic::accepted(
        &mut engine,
        choice.deciding_player_id,
        &submit_resolution_choice(vec![only_card]),
    );
    assert_eq!(engine.state.objects[&only_card].zone, Zone::Graveyard);
    assert!(engine.state.players[0].hand.is_empty());
    assert!(engine.state.stack.is_empty());
}
