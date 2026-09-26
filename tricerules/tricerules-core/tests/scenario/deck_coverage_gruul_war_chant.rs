//! Exact deck-corpus coverage for Gruul War Chant.
//!
//! Current Scryfall Oracle text was checked on 2026-09-26; its rulings endpoint has no entries.
//! CR 611.3 reevaluates static-ability effects continuously; CR 613.1f and 613.4c place the
//! menace grant and +1/+0 modification in layers 6 and 7c; CR 702.111 governs menace.

use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::{GameEngine, TurnStep, Zone};

const GRUUL_WAR_CHANT: &str = "gruul_war_chant";

fn engine(seed: u64) -> GameEngine {
    let mut engine = GameEngine::new(
        seed,
        &[0, 1],
        20,
        Some(vec![forest_only_deck(), forest_only_deck()]),
        true,
    )
    .expect("new Gruul War Chant engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn cast_war_chant(engine: &mut GameEngine) -> u32 {
    let object = inject_card_into_hand(engine, 0, GRUUL_WAR_CHANT);
    grant_pool(engine, 0);
    let slot = hand_index_for_card(engine, 0, GRUUL_WAR_CHANT);
    engine
        .apply_command(0, &cast_spell(slot, vec![]))
        .expect("cast Gruul War Chant");
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
fn gruul_war_chant_grants_attack_pump_and_menace_only_to_its_controllers_attackers() {
    let mut engine = engine(202_609_265);
    let chant = cast_war_chant(&mut engine);
    let attacker = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let bystander = inject_creature_on_battlefield(&mut engine, 0, "savannah_lions");
    let opposing_attacker = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let single_blocker = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");

    assert_eq!(engine.effective_power(attacker), Some(2));
    assert!(!engine.effective_has_keyword(attacker, Keyword::Menace));
    enter_declare_attackers(&mut engine, 0);
    engine
        .apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare the controlled creature as an attacker");

    assert_eq!(engine.effective_power(attacker), Some(3));
    assert!(engine.effective_has_keyword(attacker, Keyword::Menace));
    assert_eq!(engine.effective_power(bystander), Some(2));
    assert!(!engine.effective_has_keyword(bystander, Keyword::Menace));
    assert_eq!(engine.effective_power(opposing_attacker), Some(2));
    assert!(!engine.effective_has_keyword(opposing_attacker, Keyword::Menace));

    pass_round(&mut engine);
    assert_eq!(engine.state.turn_step, TurnStep::DeclareBlockers);
    let illegal_single_block = engine
        .apply_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: attacker,
                blocker_id: single_blocker,
            }]),
        )
        .expect_err("menace attacker cannot be blocked by one creature");
    assert_eq!(
        illegal_single_block.to_string(),
        "illegal command: Illegal blocks."
    );
    assert_eq!(engine.state.turn_step, TurnStep::DeclareBlockers);
    assert!(!engine.state.combat.as_ref().unwrap().blockers_declared);

    engine
        .apply_command(1, &declare_blockers(vec![]))
        .expect("declare no blockers after the illegal single-block attempt");
    pass_round(&mut engine);
    assert_eq!(engine.state.turn_step, TurnStep::CombatDamage);
    assert_eq!(engine.state.players[1].life, 17, "the 2/2 deals 3 damage");
    assert_eq!(engine.effective_power(attacker), Some(3));
    assert!(engine.effective_has_keyword(attacker, Keyword::Menace));

    advance_to_step(&mut engine, TurnStep::Main2);
    assert_eq!(engine.effective_power(attacker), Some(2));
    assert!(!engine.effective_has_keyword(attacker, Keyword::Menace));
    assert_eq!(engine.state.objects[&chant].zone, Zone::Battlefield);

    advance_to_next_turn(&mut engine);
    advance_to_player_main1(&mut engine, 1);
    enter_declare_attackers(&mut engine, 1);
    engine
        .apply_command(1, &declare_attackers(vec![opposing_attacker]))
        .expect("the opponent declares an attacker");
    assert_eq!(engine.effective_power(opposing_attacker), Some(2));
    assert!(!engine.effective_has_keyword(opposing_attacker, Keyword::Menace));

    pass_round(&mut engine);
    assert_eq!(engine.state.turn_step, TurnStep::DeclareBlockers);
    engine
        .apply_command(0, &declare_blockers(vec![]))
        .expect("declare no blockers against the opponent");
    pass_round(&mut engine);
    assert_eq!(engine.state.turn_step, TurnStep::CombatDamage);
    assert_eq!(
        engine.state.players[0].life, 18,
        "the opponent's 2/2 deals 2 damage"
    );
    advance_to_step(&mut engine, TurnStep::Main2);
}
