//! Issue #455 — the ETB opponent-pump family through the command boundary.
//!
//! Every expectation is the reviewed printed Oracle behavior, not a copy of generator output.
//! Exact Scryfall records and `rulings_uri` responses were fetched 2026-09-20 against pinned
//! snapshot `27bf3214-1271-490b-bdfe-c0be6c23d02e`; none returned a ruling. Governing CR concepts:
//! CR 603.6a (entry triggers), CR 115.1 (targets and controller scope), CR 608.2b (target
//! revalidation), CR 611.2c/613.4c (until-end-of-turn P/T modifiers), and CR 704.5f (lethal
//! toughness state-based action).

use super::helpers::*;
use tricerules_core::{GameEngine, Zone};

fn main1(seed: u64) -> GameEngine {
    semantic::main_phase(seed)
}

fn choose_trigger_object(object_id: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: Vec::new(),
            targets: target_object(object_id),
        })),
    }
}

fn choose_trigger_player(player: i32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: Vec::new(),
            targets: target_player(player),
        })),
    }
}

/// Cast the reviewed creature and resolve its entry trigger against `target`, returning the source
/// object id. The creature spell is cast with no targets; the ETB target is chosen at CR 603.3d as
/// the trigger is put on the stack.
fn resolve_etb_against(engine: &mut GameEngine, card: &str, target: u32) -> u32 {
    let source = inject_card_into_hand(engine, 0, card);
    let slot = hand_index_for_card(engine, 0, card);
    grant_pool(engine, 0);
    semantic::accepted(engine, 0, &cast_spell(slot, vec![]));
    for _ in 0..engine.state.players.len() {
        let player = engine.state.priority_player_id();
        semantic::accepted(engine, player, &pass());
    }
    assert_eq!(
        engine.state.objects[&source].zone,
        Zone::Battlefield,
        "{card} must enter before its trigger"
    );
    assert_eq!(
        engine.state.pending_triggers.len(),
        1,
        "{card} must park one entry-trigger target"
    );
    semantic::accepted(engine, 0, &choose_trigger_object(target));
    semantic::complete(engine, 24, |_| None).require_exercised();
    semantic::assert_main_priority(engine, 0);
    source
}

#[test]
fn issue_455_etb_pump_applies_the_exact_delta_and_expires() {
    for (index, (card, target_power, target_toughness, delta_power, delta_toughness)) in [
        ("burrog_befuddler", 3, 3, -1, 0),
        ("sinister_cryologist", 5, 5, -3, 0),
    ]
    .into_iter()
    .enumerate()
    {
        let mut engine = main1(455_100 + index as u64);
        let target = inject_creature_with_stats(
            &mut engine,
            1,
            "grizzly_bears",
            target_power,
            target_toughness,
        );
        let source = resolve_etb_against(&mut engine, card, target);
        assert_eq!(
            engine.effective_power(target),
            Some((target_power as i32 + delta_power) as u32),
            "{card} power delta"
        );
        assert_eq!(
            engine.effective_toughness(target),
            Some((target_toughness as i32 + delta_toughness) as u32),
            "{card} toughness delta"
        );
        assert_eq!(engine.state.objects[&target].zone, Zone::Battlefield);
        assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);

        // CR 611.2c: the pump lasts only until end of turn.
        end_active_turn(&mut engine, 0);
        assert_eq!(
            engine.effective_power(target),
            Some(target_power),
            "{card} pump must expire"
        );
        assert_eq!(
            engine.effective_toughness(target),
            Some(target_toughness),
            "{card} pump must expire"
        );
        assert_eq!(engine.state.objects[&target].zone, Zone::Battlefield);
    }
}

#[test]
fn issue_455_ambush_gigapede_minus_two_two_kills_a_two_toughness_creature() {
    let mut engine = main1(455_200);
    let target = inject_creature_with_stats(&mut engine, 1, "grizzly_bears", 2, 2);
    resolve_etb_against(&mut engine, "ambush_gigapede", target);
    // CR 704.5f: zero toughness is lethal before any priority is handed back.
    assert_eq!(engine.state.objects[&target].zone, Zone::Graveyard);
    assert!(engine.state.players[1].graveyard.contains(&target));
}

#[test]
fn issue_455_etb_pump_rejects_non_opponent_targets() {
    let mut engine = main1(455_300);
    let own = inject_creature_with_stats(&mut engine, 0, "grizzly_bears", 3, 3);
    let opposing = inject_creature_with_stats(&mut engine, 1, "grizzly_bears", 3, 3);
    let source = inject_card_into_hand(&mut engine, 0, "burrog_befuddler");
    let slot = hand_index_for_card(&engine, 0, "burrog_befuddler");
    grant_pool(&mut engine, 0);
    semantic::accepted(&mut engine, 0, &cast_spell(slot, vec![]));
    for _ in 0..engine.state.players.len() {
        let player = engine.state.priority_player_id();
        semantic::accepted(&mut engine, player, &pass());
    }
    assert_eq!(engine.state.pending_triggers.len(), 1);

    // A creature the caster controls and an opponent player are both outside the target filter.
    for illegal in [choose_trigger_object(own), choose_trigger_player(1)] {
        let before = engine.state.command_index;
        engine
            .apply_command(0, &illegal)
            .expect_err("the entry trigger only accepts a creature an opponent controls");
        assert_eq!(engine.state.command_index, before);
        assert_eq!(
            engine.state.pending_triggers.len(),
            1,
            "a rejected target must restore the parked trigger"
        );
    }
    assert_eq!(engine.effective_power(own), Some(3));
    assert_eq!(engine.effective_power(opposing), Some(3));

    semantic::accepted(&mut engine, 0, &choose_trigger_object(opposing));
    semantic::complete(&mut engine, 24, |_| None).require_exercised();
    assert_eq!(engine.effective_power(own), Some(3));
    assert_eq!(engine.effective_power(opposing), Some(2));
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
}
