//! Exact deck-corpus coverage for Berserkers' Onslaught.
//!
//! Scryfall Oracle and rulings were checked on 2026-09-26; its rulings endpoint has no entries.
//! CR 613.1f places the granted ability in layer 6, and CR 702.4a-b governs double strike and
//! its two combat-damage steps.

use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::{GameEngine, TurnStep, Zone};

const BERSERKERS_ONSLAUGHT: &str = "berserkers_onslaught";

fn engine(seed: u64) -> GameEngine {
    let mut engine = GameEngine::new(
        seed,
        &[0, 1],
        20,
        Some(vec![forest_only_deck(), forest_only_deck()]),
        true,
    )
    .expect("new Berserkers' Onslaught engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn cast_onslaught(engine: &mut GameEngine) -> u32 {
    let object = inject_card_into_hand(engine, 0, BERSERKERS_ONSLAUGHT);
    grant_pool(engine, 0);
    let slot = hand_index_for_card(engine, 0, BERSERKERS_ONSLAUGHT);
    engine
        .apply_command(0, &cast_spell(slot, vec![]))
        .expect("cast Berserkers' Onslaught");
    resolve_entire_stack_two_player(engine);
    assert_eq!(engine.state.objects[&object].zone, Zone::Battlefield);
    object
}

fn pass_once(engine: &mut GameEngine) {
    let actor = engine.state.priority_player_id();
    engine
        .apply_command(actor, &pass())
        .expect("pass priority toward the requested step");
}

fn pass_round(engine: &mut GameEngine) {
    pass_once(engine);
    pass_once(engine);
}

fn advance_to_step(engine: &mut GameEngine, wanted: TurnStep) {
    for _ in 0..40 {
        if engine.state.turn_step == wanted {
            return;
        }
        pass_once(engine);
    }
    panic!("game did not reach {wanted:?}");
}

fn enter_declare_attackers(engine: &mut GameEngine, active_player: i32) {
    engine
        .apply_command(active_player, &primitive_yield())
        .expect("yield from main phase to beginning of combat");
    advance_to_step(engine, TurnStep::DeclareAttackers);
}

fn advance_to_next_turn(engine: &mut GameEngine) {
    let starting_turn = engine.state.turn;
    for _ in 0..80 {
        if engine.state.turn > starting_turn {
            return;
        }
        if let Some(player) = engine.state.cleanup_discard_player {
            let player_index = engine.state.player_idx(player).expect("cleanup player");
            let excess = engine.state.players[player_index].hand.len() - 7;
            engine
                .apply_command(player, &discard_cleanup_batch((0..excess as u32).collect()))
                .expect("discard down to hand size during cleanup");
        } else {
            pass_once(engine);
        }
    }
    panic!("game did not advance to the next turn");
}

fn advance_to_player_main1(engine: &mut GameEngine, player: i32) {
    for _ in 0..80 {
        if engine.state.turn_step == TurnStep::Main1 && engine.state.active_player_id() == player {
            return;
        }
        if let Some(player) = engine.state.cleanup_discard_player {
            let player_index = engine.state.player_idx(player).expect("cleanup player");
            let excess = engine.state.players[player_index].hand.len() - 7;
            engine
                .apply_command(player, &discard_cleanup_batch((0..excess as u32).collect()))
                .expect("discard down to hand size during cleanup");
        } else {
            pass_once(engine);
        }
    }
    panic!("game did not reach player {player}'s main phase");
}

#[test]
fn onslaught_grants_double_strike_only_to_its_controllers_attackers_for_both_damage_steps() {
    let mut engine = engine(202_609_264);
    cast_onslaught(&mut engine);

    let attacker = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let bystander = inject_creature_on_battlefield(&mut engine, 0, "savannah_lions");
    let opposing_attacker = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");

    assert!(!engine.effective_has_keyword(attacker, Keyword::DoubleStrike));
    enter_declare_attackers(&mut engine, 0);
    engine
        .apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare one attacker");

    assert!(
        engine.effective_has_keyword(attacker, Keyword::DoubleStrike),
        "your attacking creature gains double strike"
    );
    assert!(
        !engine.effective_has_keyword(bystander, Keyword::DoubleStrike),
        "your nonattacking creature does not gain double strike"
    );
    assert!(
        !engine.effective_has_keyword(opposing_attacker, Keyword::DoubleStrike),
        "the opponent's creature does not gain double strike"
    );

    pass_round(&mut engine);
    assert_eq!(engine.state.turn_step, TurnStep::DeclareBlockers);
    engine
        .apply_command(1, &declare_blockers(vec![]))
        .expect("declare no blockers");
    pass_round(&mut engine);

    assert_eq!(engine.state.turn_step, TurnStep::FirstStrikeDamage);
    assert_eq!(engine.state.players[1].life, 18);
    assert!(
        engine.effective_has_keyword(attacker, Keyword::DoubleStrike),
        "the attacker remains double striking after first-strike damage"
    );
    pass_round(&mut engine);
    assert_eq!(engine.state.turn_step, TurnStep::CombatDamage);
    assert_eq!(
        engine.state.players[1].life, 16,
        "the 2/2 attacker deals damage in both combat-damage steps"
    );

    advance_to_step(&mut engine, TurnStep::Main2);
    assert!(
        !engine.effective_has_keyword(attacker, Keyword::DoubleStrike),
        "the continuous grant ends when the creature is no longer attacking"
    );

    advance_to_next_turn(&mut engine);
    advance_to_player_main1(&mut engine, 1);
    enter_declare_attackers(&mut engine, 1);
    engine
        .apply_command(1, &declare_attackers(vec![opposing_attacker]))
        .expect("the opponent declares an attacker");
    assert!(
        !engine.effective_has_keyword(opposing_attacker, Keyword::DoubleStrike),
        "the static ability follows its controller rather than the active player"
    );
}
