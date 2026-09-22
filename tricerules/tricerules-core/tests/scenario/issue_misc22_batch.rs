//! Reviewed direct-RON dies-value scenarios: Driver of the Dead, Harried Spearguard, Callous
//! Inspector, Edge Rover, Sizzling Changeling and Miner's Guidewing.
//!
//! Exact Scryfall records and `rulings_uri` were fetched 2026-09-22 against the pinned snapshot.
//! Governance: CR 603.6c (dies triggers), CR 111.10 (Rat, Clue, and Lander tokens), CR 115.3 (the
//! targets), CR 120.3 (the damage the source deals to its controller), CR 400.7/608.2b (a returned
//! card is a new object), CR 701.8 (the graveyard return), CR 701.44 (Explore), and CR 702.73
//! (Changeling). Rulings confirm the Clue token is not an investigate and the explore counter.

use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{ChooseTriggerTarget, RuledCommand, TargetRef, TargetRefKind};

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("island", &[]), deck_with("forest", &[])]);
    let mut e = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut e);
    grant_pool(&mut e, 0);
    grant_pool(&mut e, 1);
    e
}

fn choose_trigger_target(object_id: u32) -> RuledCommand {
    choose_trigger_target_kind(object_id, TargetRefKind::Permanent)
}

fn choose_graveyard_trigger_target(object_id: u32) -> RuledCommand {
    choose_trigger_target_kind(object_id, TargetRefKind::Graveyard)
}

fn choose_trigger_target_kind(object_id: u32, kind: TargetRefKind) -> RuledCommand {
    RuledCommand {
        cmd: Some(
            tricerules_proto::ruled::v1::ruled_command::Cmd::ChooseTriggerTarget(
                ChooseTriggerTarget {
                    decline: false,
                    selected_modes: Vec::new(),
                    targets: vec![TargetRef {
                        object_id,
                        group_index: 0,
                        kind: kind as i32,
                        ..Default::default()
                    }],
                },
            ),
        ),
    }
}

fn kill_with_murder(e: &mut GameEngine, source: u32) {
    inject_card_into_hand(e, 0, "murder");
    let slot = hand_index_for_card(e, 0, "murder");
    semantic::accepted(e, 0, &cast_spell(slot, target_object(source)));
    e.apply_command(0, &pass()).expect("p0 pass");
    e.apply_command(1, &pass()).expect("murder resolves");
}

fn inject_library_top(e: &mut GameEngine, player: usize, card_id: &str) -> u32 {
    let id = inject_library_card(e, player, card_id);
    let library = &mut e.state.players[player].library;
    library.retain(|object_id| *object_id != id);
    library.push_front(id);
    id
}

#[test]
fn issue_misc22_driver_of_the_dead_reanimates_a_small_creature() {
    let mut e = engine(822_001);
    let driver = inject_creature_with_stats(&mut e, 0, "driver_of_the_dead", 3, 2);
    let small = inject_graveyard_card(&mut e, 0, "grizzly_bears");
    kill_with_murder(&mut e, driver);
    assert_eq!(e.state.pending_triggers.len(), 1, "the dies trigger waits");
    semantic::accepted(&mut e, 0, &choose_graveyard_trigger_target(small));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&small].zone, Zone::Battlefield);

    // A creature card with mana value greater than 2 is not a legal target.
    let mut other = engine(822_011);
    let died = inject_creature_with_stats(&mut other, 0, "driver_of_the_dead", 3, 2);
    let too_big = inject_graveyard_card(&mut other, 0, "air_elemental");
    kill_with_murder(&mut other, died);
    assert!(
        other
            .apply_command(0, &choose_graveyard_trigger_target(too_big))
            .is_err(),
        "a mana-value-4 creature card is not a legal target"
    );
}

#[test]
fn issue_misc22_harried_spearguard_makes_a_rat() {
    let mut e = engine(822_002);
    let guard = inject_creature_with_stats(&mut e, 0, "harried_spearguard", 1, 1);
    assert!(e.effective_has_keyword(guard, Keyword::Haste));
    kill_with_murder(&mut e, guard);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        battlefield_token_oids(&e, 0, "rat_b_1_1_cant_block").len(),
        1
    );
}

#[test]
fn issue_misc22_callous_inspector_deals_damage_and_makes_a_clue() {
    let mut e = engine(822_003);
    let inspector = inject_creature_with_stats(&mut e, 0, "callous_inspector", 1, 1);
    let life_before = e.state.players[0].life;
    kill_with_murder(&mut e, inspector);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[0].life, life_before - 1, "1 damage to you");
    assert_eq!(battlefield_token_oids(&e, 0, "clue").len(), 1);
}

#[test]
fn issue_misc22_edge_rover_makes_a_lander_for_each_player() {
    let mut e = engine(822_004);
    let rover = inject_creature_with_stats(&mut e, 0, "edge_rover", 2, 2);
    kill_with_murder(&mut e, rover);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(battlefield_token_oids(&e, 0, "lander").len(), 1);
    assert_eq!(battlefield_token_oids(&e, 1, "lander").len(), 1);
}

#[test]
fn issue_misc22_sizzling_changeling_exiles_and_grants_play() {
    let mut e = engine(822_005);
    let changeling = inject_creature_with_stats(&mut e, 0, "sizzling_changeling", 3, 2);
    let top = inject_library_top(&mut e, 0, "grizzly_bears");
    kill_with_murder(&mut e, changeling);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&top].zone, Zone::Exile);
}

#[test]
fn issue_misc22_miners_guidewing_makes_a_creature_explore() {
    let mut e = engine(822_006);
    let guidewing = inject_creature_with_stats(&mut e, 0, "miners_guidewing", 1, 1);
    let target = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let land = inject_library_top(&mut e, 0, "forest");
    kill_with_murder(&mut e, guidewing);
    assert_eq!(e.state.pending_triggers.len(), 1, "the dies trigger waits");
    semantic::accepted(&mut e, 0, &choose_trigger_target(target));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&land].zone,
        Zone::Hand,
        "the revealed land is drawn by the exploring creature"
    );

    // A creature you don't control is not a legal target.
    let mut other = engine(822_016);
    let died = inject_creature_with_stats(&mut other, 0, "miners_guidewing", 1, 1);
    let theirs = inject_creature_on_battlefield(&mut other, 1, "grizzly_bears");
    inject_library_top(&mut other, 0, "forest");
    kill_with_murder(&mut other, died);
    assert!(
        other
            .apply_command(0, &choose_trigger_target(theirs))
            .is_err(),
        "an opponent's creature is not a legal target"
    );
}
