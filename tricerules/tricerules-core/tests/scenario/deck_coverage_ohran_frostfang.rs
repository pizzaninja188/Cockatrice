//! Exact deck-corpus coverage for Ohran Frostfang.
//!
//! WotC's Commander 2019 release notes and ruling were checked against the current card text.
//! CR 510.2-510.3a govern simultaneous combat damage and trigger placement; CR 603.2 and 603.2c
//! cover each qualifying damage event, CR 613.1f places the granted keyword in layer 6, CR 702.2
//! defines deathtouch, and CR 113.7a keeps a triggered ability independent of its source.

use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::{GameEngine, TurnStep, Zone};

const OHRAN_FROSTFANG: &str = "ohran_frostfang";

fn engine(seed: u64) -> GameEngine {
    let mut engine = GameEngine::new(
        seed,
        &[0, 1],
        20,
        Some(vec![forest_only_deck(), forest_only_deck()]),
        true,
    )
    .expect("new Ohran Frostfang engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn cast_ohran_frostfang(engine: &mut GameEngine) -> u32 {
    let object = inject_card_into_hand(engine, 0, OHRAN_FROSTFANG);
    grant_pool(engine, 0);
    let slot = hand_index_for_card(engine, 0, OHRAN_FROSTFANG);
    engine
        .apply_command(0, &cast_spell(slot, vec![]))
        .expect("cast Ohran Frostfang");
    resolve_entire_stack_two_player(engine);
    assert_eq!(engine.state.objects[&object].zone, Zone::Battlefield);

    // The scenario starts from main phase 1 and primes the creature as an existing attacker.
    engine
        .state
        .objects
        .get_mut(&object)
        .unwrap()
        .summoning_sick = false;
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
fn attacking_creatures_you_control_gain_deathtouch_and_any_damage_kills_a_blocker() {
    let mut engine = engine(202_609_266);
    cast_ohran_frostfang(&mut engine);
    let attacker = inject_creature_with_stats(&mut engine, 0, "savannah_lions", 1, 2);
    let bystander = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let opponent_creature = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let blocker = inject_creature_with_stats(&mut engine, 1, "hill_giant", 4, 4);

    assert!(!engine.effective_has_keyword(attacker, Keyword::Deathtouch));
    assert!(!engine.effective_has_keyword(bystander, Keyword::Deathtouch));
    assert!(!engine.effective_has_keyword(opponent_creature, Keyword::Deathtouch));

    enter_declare_attackers(&mut engine, 0);
    engine
        .apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare one controlled attacker");
    assert!(engine.effective_has_keyword(attacker, Keyword::Deathtouch));
    assert!(!engine.effective_has_keyword(bystander, Keyword::Deathtouch));
    assert!(!engine.effective_has_keyword(opponent_creature, Keyword::Deathtouch));

    pass_round(&mut engine);
    assert_eq!(engine.state.turn_step, TurnStep::DeclareBlockers);
    engine
        .apply_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: attacker,
                blocker_id: blocker,
            }]),
        )
        .expect("block the attacking creature");
    pass_round(&mut engine);

    assert_eq!(engine.state.objects[&blocker].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.objects[&attacker].zone,
        Zone::Graveyard,
        "the 1/2 receives lethal damage from its blocker"
    );
    assert!(
        engine.state.stack.is_empty(),
        "damage to a creature draws no card"
    );
}

#[test]
fn draws_once_per_friendly_combat_damager_but_not_for_damage_to_a_creature() {
    let mut engine = engine(202_609_267);
    cast_ohran_frostfang(&mut engine);
    let first_unblocked = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let second_unblocked = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let blocked_attacker = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let blocker = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");

    enter_declare_attackers(&mut engine, 0);
    engine
        .apply_command(
            0,
            &declare_attackers(vec![first_unblocked, second_unblocked, blocked_attacker]),
        )
        .expect("declare three controlled attackers");
    pass_round(&mut engine);
    engine
        .apply_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: blocked_attacker,
                blocker_id: blocker,
            }]),
        )
        .expect("block one attacker so it damages only a creature");
    pass_round(&mut engine);

    assert_eq!(engine.state.players[1].life, 16);
    assert_eq!(engine.state.objects[&blocker].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.objects[&blocked_attacker].zone,
        Zone::Graveyard
    );
    assert_eq!(
        engine
            .state
            .pending_trigger_order
            .as_ref()
            .expect("two separate player-damage triggers require ordering")
            .candidates
            .len(),
        2,
        "only the two creatures that dealt damage to the player trigger draws"
    );

    let hand_before = engine.state.players[0].hand.len();
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.players[0].hand.len(), hand_before + 2);
}

#[test]
fn ignores_noncombat_creature_damage_and_opponent_combat_damage() {
    let mut engine = engine(202_609_268);
    cast_ohran_frostfang(&mut engine);
    let pinger = inject_creature_on_battlefield(&mut engine, 0, "prodigal_sorcerer");

    let hand_before_noncombat = engine.state.players[0].hand.len();
    engine
        .apply_command(0, &activate_ability(pinger, 0, target_player(1)))
        .expect("activate Prodigal Sorcerer to deal noncombat damage to a player");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.players[1].life, 19);
    assert_eq!(engine.state.players[0].hand.len(), hand_before_noncombat);
    assert!(engine.state.stack.is_empty());

    advance_to_next_turn(&mut engine);
    advance_to_player_main1(&mut engine, 1);
    let hand_before_opponent_combat = engine.state.players[0].hand.len();
    let opponent_attacker = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    enter_declare_attackers(&mut engine, 1);
    engine
        .apply_command(1, &declare_attackers(vec![opponent_attacker]))
        .expect("the opponent declares an attacker");
    assert!(!engine.effective_has_keyword(opponent_attacker, Keyword::Deathtouch));
    pass_round(&mut engine);
    engine
        .apply_command(0, &declare_blockers(vec![]))
        .expect("declare no blockers against the opponent");
    pass_round(&mut engine);

    assert_eq!(engine.state.players[0].life, 18);
    assert_eq!(
        engine.state.players[0].hand.len(),
        hand_before_opponent_combat
    );
    assert!(engine.state.pending_trigger_order.is_none());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn qualifying_draw_triggers_survive_simultaneous_lethal_damage_to_ohran() {
    let mut engine = engine(202_609_269);
    let ohran = cast_ohran_frostfang(&mut engine);
    let first_unblocked = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let second_unblocked = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let lethal_blocker = inject_creature_with_stats(&mut engine, 1, "hill_giant", 6, 6);

    enter_declare_attackers(&mut engine, 0);
    engine
        .apply_command(
            0,
            &declare_attackers(vec![ohran, first_unblocked, second_unblocked]),
        )
        .expect("attack with Ohran Frostfang and two other controlled creatures");
    assert!(engine.effective_has_keyword(ohran, Keyword::Deathtouch));
    pass_round(&mut engine);
    engine
        .apply_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: ohran,
                blocker_id: lethal_blocker,
            }]),
        )
        .expect("block Ohran with a creature that deals lethal damage back");
    pass_round(&mut engine);

    assert_eq!(engine.state.objects[&ohran].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&lethal_blocker].zone, Zone::Graveyard);
    assert_eq!(engine.state.players[1].life, 16);
    assert_eq!(
        engine
            .state
            .pending_trigger_order
            .as_ref()
            .expect("the two simultaneous qualifying triggers remain to be ordered")
            .candidates
            .len(),
        2,
        "both controlled creatures trigger even though Ohran died in the same damage event"
    );

    let hand_before = engine.state.players[0].hand.len();
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.players[0].hand.len(), hand_before + 2);
}
