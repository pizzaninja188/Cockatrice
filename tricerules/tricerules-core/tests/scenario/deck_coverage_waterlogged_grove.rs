//! Actual-card behavior for Waterlogged Grove, checked against its pinned Oracle and rulings.

use super::helpers::*;
use tricerules_core::{GameEngine, Zone};

type ManaPool = (u32, u32, u32, u32, u32, u32);

fn mana_pool(engine: &GameEngine) -> ManaPool {
    let pool = &engine.state.players[0].mana_pool;
    (
        pool.white,
        pool.blue,
        pool.black,
        pool.red,
        pool.green,
        pool.colorless,
    )
}

fn waterlogged_grove_engine(seed: u64) -> (GameEngine, u32) {
    let decks = Some(vec![
        deck_with("island", &["waterlogged_grove"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let grove = relocate_to_battlefield(&mut engine, 0, "waterlogged_grove", false);
    (engine, grove)
}

fn activate_mana_option(engine: &GameEngine, grove: u32, option: u32) -> RuledCommand {
    let mut command = activate_ability_for(engine, grove, 0, vec![]);
    let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
        unreachable!()
    };
    activation.mana_option_index = option;
    command
}

#[test]
fn waterlogged_grove_pays_one_life_and_immediately_adds_the_chosen_color() {
    for (option, expected_pool) in [(0, (0, 0, 0, 0, 1, 0)), (1, (0, 1, 0, 0, 0, 0))] {
        let (mut engine, grove) = waterlogged_grove_engine(202_609_270 + option as u64);

        engine
            .apply_command(0, &activate_mana_option(&engine, grove, option))
            .expect("activate Waterlogged Grove's mana ability");

        assert_eq!(engine.state.players[0].life, 19);
        assert_eq!(mana_pool(&engine), expected_pool);
        assert!(engine.state.objects[&grove].tapped);
        assert!(
            engine.state.stack.is_empty(),
            "mana abilities resolve immediately"
        );
    }
}

#[test]
fn waterlogged_grove_cannot_pay_its_mana_ability_life_cost_at_zero_life() {
    let (mut engine, grove) = waterlogged_grove_engine(202_609_272);
    engine.state.players[0].life = 0;
    let before_pool = mana_pool(&engine);

    engine
        .apply_command(0, &activate_mana_option(&engine, grove, 0))
        .expect_err("a player at zero life cannot pay one life");

    assert_eq!(engine.state.players[0].life, 0);
    assert_eq!(mana_pool(&engine), before_pool);
    assert!(!engine.state.objects[&grove].tapped);
    assert!(engine.state.stack.is_empty());
}

#[test]
fn waterlogged_grove_draws_after_its_source_is_sacrificed_as_an_activation_cost() {
    let (mut engine, grove) = waterlogged_grove_engine(202_609_273);
    let top_card = *engine.state.players[0]
        .library
        .front()
        .expect("library has a card to draw");
    let library_size = engine.state.players[0].library.len();
    let hand_size = engine.state.players[0].hand.len();
    let generation_before = engine
        .state
        .zone_change_generation
        .get(&grove)
        .copied()
        .unwrap_or_default();

    engine
        .apply_command(0, &activate_ability_for(&engine, grove, 1, vec![]))
        .expect_err("the draw ability requires one generic mana");
    assert_eq!(engine.state.objects[&grove].zone, Zone::Battlefield);
    assert!(!engine.state.objects[&grove].tapped);
    assert_eq!(engine.state.players[0].library.len(), library_size);
    assert_eq!(engine.state.players[0].hand.len(), hand_size);

    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    engine
        .apply_command(0, &activate_ability_for(&engine, grove, 1, vec![]))
        .expect("pay {1}, tap, and sacrifice Waterlogged Grove");

    assert_eq!(engine.state.objects[&grove].zone, Zone::Graveyard);
    assert!(engine.state.players[0].graveyard.contains(&grove));
    assert_eq!(
        engine
            .state
            .zone_change_generation
            .get(&grove)
            .copied()
            .unwrap_or_default(),
        generation_before + 1
    );
    assert_eq!(engine.state.players[0].library.len(), library_size);
    assert_eq!(engine.state.players[0].hand.len(), hand_size);
    assert_eq!(engine.state.stack.len(), 1);

    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.players[0].library.len(), library_size - 1);
    assert!(engine.state.players[0].hand.contains(&top_card));
    assert!(engine.state.stack.is_empty());
}
