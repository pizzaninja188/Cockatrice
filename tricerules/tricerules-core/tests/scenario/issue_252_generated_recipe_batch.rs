//! Issue #252 — generated Standard triggers and five-color mana reuse established primitives.
//!
//! Oracle and rulings checked 2026-09-11. CR 603.6c and 603.10 preserve death-trigger LKI;
//! CR 608.2c preserves printed instruction order; CR 701.25 defines surveil; and CR 605.1a
//! makes the targetless tap abilities mana abilities.

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{ruled_command::Cmd, ChoiceKind};

fn seat_on_top(engine: &mut GameEngine, player: usize, card_ids: &[&str]) -> Vec<u32> {
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

fn resolve_top_stack(engine: &mut GameEngine) -> RuledEventBatch {
    let first = engine.state.priority_player_id();
    let second = if first == engine.state.players[0].id {
        engine.state.players[1].id
    } else {
        engine.state.players[0].id
    };
    engine.apply_command(first, &pass()).expect("first pass");
    engine
        .apply_command(second, &pass())
        .expect("second pass resolves stack item")
}

#[test]
fn generated_synthoids_surveils_two_with_private_ordered_library_choice() {
    let decks = Some(vec![
        deck_with("island", &["a.i.m._synthoids"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(252_001, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let top = seat_on_top(&mut engine, 0, &["storm_crow", "grizzly_bears"]);

    move_ready_to_battlefield(&mut engine, 0, "a.i.m._synthoids");
    let resolution = resolve_top_stack(&mut engine);
    let choice = find_resolution_choice(&resolution).expect("Surveil 2 choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibraryLook);
    assert_eq!(choice.deciding_player_id, 0);
    assert_eq!((choice.min, choice.max), (0, 2));
    assert!(choice.ordered);
    assert_eq!(choice.candidate_object_ids, top);

    let ordering = engine
        .apply_command(0, &submit_resolution_choice(vec![]))
        .expect("keep both cards on top");
    let order_choice = find_resolution_choice(&ordering).expect("top-order choice");
    assert_eq!((order_choice.min, order_choice.max), (2, 2));
    engine
        .apply_command(0, &submit_resolution_choice(vec![top[0], top[1]]))
        .expect("order retained cards");
    assert_eq!(
        engine.state.players[0]
            .library
            .iter()
            .take(2)
            .copied()
            .collect::<Vec<_>>(),
        vec![top[1], top[0]]
    );
}

#[test]
fn generated_buzz_bots_death_lki_draws_for_its_last_controller() {
    let mut engine = GameEngine::new(252_002, &[0, 1], 20, None, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let bots = inject_creature_under_foreign_control(&mut engine, 0, 1, "buzz_bots");
    let p0_hand = engine.state.players[0].hand.len();
    let p1_hand = engine.state.players[1].hand.len();

    engine
        .state
        .objects
        .get_mut(&bots)
        .expect("Buzz Bots")
        .damage = 1;
    engine.apply_command(0, &pass()).expect("death SBA");
    assert_eq!(engine.state.objects[&bots].zone, Zone::Graveyard);
    assert!(engine
        .state
        .stack
        .last()
        .is_some_and(|item| item.is_triggered && item.source_permanent_id == Some(bots)));

    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.players[0].hand.len(), p0_hand);
    assert_eq!(engine.state.players[1].hand.len(), p1_hand + 1);
}

#[test]
fn generated_vampire_spawn_uses_each_opponent_then_controller_life_effects() {
    let decks = Some(vec![
        deck_with("swamp", &["vampire_spawn"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(252_003, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);

    move_ready_to_battlefield(&mut engine, 0, "vampire_spawn");
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.players[0].life, 22);
    assert_eq!(engine.state.players[1].life, 18);
}

#[test]
fn generated_sanctifier_excludes_itself_opponents_and_noncreatures() {
    let decks = Some(vec![
        deck_with(
            "plains",
            &["hinterland_sanctifier", "grizzly_bears", "short_sword"],
        ),
        deck_with("forest", &["grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(252_004, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);

    move_ready_to_battlefield(&mut engine, 0, "hinterland_sanctifier");
    assert!(
        engine.state.stack.is_empty(),
        "source excludes its own entry"
    );

    move_ready_to_battlefield(&mut engine, 1, "grizzly_bears");
    move_ready_to_battlefield(&mut engine, 0, "short_sword");
    assert!(
        engine.state.stack.is_empty(),
        "wrong controller and type do not trigger"
    );

    move_ready_to_battlefield(&mut engine, 0, "grizzly_bears");
    assert_eq!(engine.state.stack.len(), 1);
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.players[0].life, 21);
}

#[test]
fn generated_druid_publishes_and_produces_each_colored_mana_option() {
    for (option, expected) in [
        (0, [1, 0, 0, 0, 0]),
        (1, [0, 1, 0, 0, 0]),
        (2, [0, 0, 1, 0, 0]),
        (3, [0, 0, 0, 1, 0]),
        (4, [0, 0, 0, 0, 1]),
    ] {
        let mut engine =
            GameEngine::new(252_100 + option as u64, &[0, 1], 20, None, true).expect("engine");
        advance_to_main1_from_game_start(&mut engine);
        let druid = inject_creature_on_battlefield(&mut engine, 0, "great_forest_druid");
        let mut command = activate_ability_for(&engine, druid, 0, vec![]);
        let Some(Cmd::ActivateAbility(ability)) = command.cmd.as_mut() else {
            unreachable!()
        };
        ability.mana_option_index = option;

        engine
            .apply_command(0, &command)
            .expect("activate chosen mana option");
        let pool = &engine.state.players[0].mana_pool;
        assert_eq!(
            [pool.white, pool.blue, pool.black, pool.red, pool.green],
            expected
        );
        assert_eq!(pool.colorless, 0);
        assert!(engine.state.objects[&druid].tapped);
        assert!(
            engine.state.stack.is_empty(),
            "mana ability resolves immediately"
        );
    }

    let mut engine = GameEngine::new(252_200, &[0, 1], 20, None, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let druid = inject_creature_on_battlefield(&mut engine, 0, "great_forest_druid");
    let mut invalid = activate_ability_for(&engine, druid, 0, vec![]);
    let Some(Cmd::ActivateAbility(ability)) = invalid.cmd.as_mut() else {
        unreachable!()
    };
    ability.mana_option_index = 5;
    engine
        .apply_command(0, &invalid)
        .expect_err("out-of-range mana option");
    assert!(!engine.state.objects[&druid].tapped);
}
