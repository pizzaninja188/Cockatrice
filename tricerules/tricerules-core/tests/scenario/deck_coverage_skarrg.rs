//! Exact deck-corpus coverage for Skarrg, the Rage Pits.

use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::GameEngine;

const SKARRG: &str = "skarrg,_the_rage_pits";

fn engine(seed: u64) -> GameEngine {
    let mut engine = GameEngine::new(
        seed,
        &[0, 1],
        20,
        Some(vec![deck_with("island", &[]), deck_with("forest", &[])]),
        true,
    )
    .expect("new Skarrg engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn play_skarrg(engine: &mut GameEngine) -> u32 {
    inject_card_into_hand(engine, 0, SKARRG);
    let slot = hand_index_for_card(engine, 0, SKARRG);
    engine
        .apply_command(0, &play_land(slot))
        .expect("play Skarrg as a land");
    battlefield_object_for_card(engine, 0, SKARRG)
}

#[test]
fn skarrg_adds_colorless_immediately_and_cannot_reactivate_while_tapped() {
    let mut engine = engine(202_609_291);
    let skarrg = play_skarrg(&mut engine);
    assert!(!engine.state.objects[&skarrg].tapped);

    apply_ability(&mut engine, 0, skarrg, 0, vec![]).expect("tap Skarrg to add one colorless mana");

    assert_eq!(engine.state.players[0].mana_pool.colorless, 1);
    assert!(engine.state.objects[&skarrg].tapped);
    assert!(
        engine.state.stack.is_empty(),
        "mana ability resolves immediately"
    );

    let command_index = engine.state.command_index;
    assert!(apply_ability(&mut engine, 0, skarrg, 0, vec![]).is_err());
    assert_eq!(engine.state.command_index, command_index);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 1);
}

#[test]
fn skarrg_targets_an_opponents_creature_and_grants_pump_and_trample_until_cleanup() {
    let mut engine = engine(202_609_292);
    let skarrg = play_skarrg(&mut engine);
    let opponent_creature = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let opponent_land = inject_permanent_on_battlefield(&mut engine, 1, "forest");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            g: 1,
            ..Default::default()
        },
    );

    let invalid_target = apply_ability(&mut engine, 0, skarrg, 1, target_object(opponent_land));
    assert!(matches!(
        invalid_target,
        Err(tricerules_core::EngineError::Illegal(_))
    ));
    assert!(!engine.state.objects[&skarrg].tapped);
    assert_eq!(engine.state.players[0].mana_pool.red, 1);
    assert_eq!(engine.state.players[0].mana_pool.green, 1);

    apply_ability(&mut engine, 0, skarrg, 1, target_object(opponent_creature))
        .expect("pay red and green to target the opponent's creature");
    assert!(
        engine.state.objects[&skarrg].tapped,
        "tap is paid on activation"
    );
    assert_eq!(engine.state.players[0].mana_pool.red, 0);
    assert_eq!(engine.state.players[0].mana_pool.green, 0);
    assert_eq!(engine.effective_power(opponent_creature), Some(2));
    assert!(!engine.effective_has_keyword(opponent_creature, Keyword::Trample));

    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.effective_power(opponent_creature), Some(3));
    assert_eq!(engine.effective_toughness(opponent_creature), Some(3));
    assert!(engine.effective_has_keyword(opponent_creature, Keyword::Trample));

    end_active_turn(&mut engine, 0);
    assert_eq!(engine.effective_power(opponent_creature), Some(2));
    assert_eq!(engine.effective_toughness(opponent_creature), Some(2));
    assert!(!engine.effective_has_keyword(opponent_creature, Keyword::Trample));
}
