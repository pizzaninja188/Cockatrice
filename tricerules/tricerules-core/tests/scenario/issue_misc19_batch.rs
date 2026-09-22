//! Reviewed direct-RON tutor, reanimation, and bounce scenarios: Rune-Scarred Demon, Starfield
//! Shepherd, Scampering Surveyor, Peerless Ropemaster, Ragamuffin Raptor, Defibrillating Current,
//! Unsparing Boltcaster and Emerge from the Cocoon.
//!
//! Exact Scryfall records and `rulings_uri` were fetched 2026-09-22 against the pinned snapshot.
//! Governance: CR 115.3 (the "up to one" bounce and reanimation targets), CR 119.3 (life gain),
//! CR 120.3 (damage), CR 400.7/608.2b (a returned card is a new object), CR 603.6a (entry trigger),
//! CR 608.2b (an illegal target fails the whole spell), CR 609.2 (the dealt-damage-this-turn
//! predicate), CR 701.23 (search), CR 702.9 (flying), and CR 702.185 (warp).

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

fn cast(e: &mut GameEngine, player: i32, card_id: &str, targets: Vec<TargetRef>) {
    inject_card_into_hand(e, player as usize, card_id);
    grant_pool(e, player as usize);
    let slot = hand_index_for_card(e, player as usize, card_id);
    semantic::accepted(e, player, &cast_spell(slot, targets));
    resolve_entire_stack_two_player(e);
}

/// Casts and passes priority until a resolution choice parks, then returns the candidate ids.
fn cast_park(e: &mut GameEngine, player: i32, card_id: &str, targets: Vec<TargetRef>) -> Vec<u32> {
    inject_card_into_hand(e, player as usize, card_id);
    grant_pool(e, player as usize);
    let slot = hand_index_for_card(e, player as usize, card_id);
    semantic::accepted(e, player, &cast_spell(slot, targets));
    for _ in 0..40 {
        if let Some(pending) = e.state.pending_resolution.as_ref() {
            return pending.presentation.candidates.clone();
        }
        if e.state.stack.is_empty() {
            break;
        }
        let priority = e.state.priority_player_id();
        semantic::accepted(e, priority, &pass());
    }
    panic!("{card_id} never parked a resolution choice");
}

fn battlefield_object(e: &GameEngine, player: usize, card_id: &str) -> u32 {
    *e.state.players[player]
        .battlefield
        .iter()
        .find(|id| e.state.objects[id].card_id == card_id)
        .unwrap_or_else(|| panic!("{card_id} on battlefield"))
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

fn target_graveyard_card(object_id: u32) -> Vec<TargetRef> {
    vec![TargetRef {
        object_id,
        group_index: 0,
        kind: TargetRefKind::Graveyard as i32,
        ..Default::default()
    }]
}

#[test]
fn issue_misc19_rune_scarred_demon_tutors_any_card() {
    let mut e = engine(819_001);
    let tutor = inject_library_card(&mut e, 0, "grizzly_bears");
    let candidates = cast_park(&mut e, 0, "rune-scarred_demon", vec![]);
    assert!(
        candidates.contains(&tutor),
        "an unrestricted tutor offers every card"
    );
    semantic::accepted(&mut e, 0, &submit_resolution_choice(vec![tutor]));
    assert_eq!(e.state.objects[&tutor].zone, Zone::Hand);
    let demon = battlefield_object(&e, 0, "rune-scarred_demon");
    assert!(e.effective_has_keyword(demon, Keyword::Flying));
}

#[test]
fn issue_misc19_starfield_shepherd_filters_its_search() {
    let mut e = engine(819_002);
    let plains = inject_library_card(&mut e, 0, "plains");
    let one_drop = inject_library_card(&mut e, 0, "faerie_miscreant");
    let forest = inject_library_card(&mut e, 0, "forest");
    let bear = inject_library_card(&mut e, 0, "grizzly_bears");
    let candidates = cast_park(&mut e, 0, "starfield_shepherd", vec![]);
    assert!(candidates.contains(&plains), "a basic Plains qualifies");
    assert!(
        candidates.contains(&one_drop),
        "a mana-value-1 creature qualifies"
    );
    assert!(!candidates.contains(&forest), "a non-Plains basic does not");
    assert!(
        !candidates.contains(&bear),
        "a mana-value-2 creature does not"
    );
    semantic::accepted(&mut e, 0, &submit_resolution_choice(vec![plains]));
    assert_eq!(e.state.objects[&plains].zone, Zone::Hand);
}

#[test]
fn issue_misc19_scampering_surveyor_ramps_a_basic_or_cave_tapped() {
    let mut e = engine(819_003);
    let forest = inject_library_card(&mut e, 0, "forest");
    let cave = inject_library_card(&mut e, 0, "promising_vein");
    let bear = inject_library_card(&mut e, 0, "grizzly_bears");
    let candidates = cast_park(&mut e, 0, "scampering_surveyor", vec![]);
    assert!(candidates.contains(&forest), "a basic land qualifies");
    assert!(candidates.contains(&cave), "a Cave land qualifies");
    assert!(!candidates.contains(&bear), "a creature does not");
    semantic::accepted(&mut e, 0, &submit_resolution_choice(vec![cave]));
    assert_eq!(e.state.objects[&cave].zone, Zone::Battlefield);
    assert!(
        e.state.objects[&cave].tapped,
        "the searched land enters tapped"
    );
}

#[test]
fn issue_misc19_peerless_ropemaster_bounces_only_a_tapped_creature() {
    let mut e = engine(819_004);
    let theirs = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    e.state.objects.get_mut(&theirs).unwrap().tapped = true;
    cast(&mut e, 0, "peerless_ropemaster", vec![]);
    assert_eq!(e.state.pending_triggers.len(), 1, "the entry trigger waits");
    semantic::accepted(&mut e, 0, &choose_trigger_target(theirs));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&theirs].zone, Zone::Hand);

    // An untapped creature is not a legal target.
    let mut untapped = engine(819_014);
    let fresh = inject_creature_on_battlefield(&mut untapped, 1, "grizzly_bears");
    cast(&mut untapped, 0, "peerless_ropemaster", vec![]);
    assert!(
        untapped
            .apply_command(0, &choose_trigger_target(fresh))
            .is_err(),
        "an untapped creature is not a legal target"
    );
}

#[test]
fn issue_misc19_ragamuffin_raptor_returns_a_creature_or_food_card() {
    let mut e = engine(819_005);
    let bear = inject_graveyard_card(&mut e, 0, "grizzly_bears");
    cast(&mut e, 0, "ragamuffin_raptor", vec![]);
    assert_eq!(e.state.pending_triggers.len(), 1, "the entry trigger waits");
    semantic::accepted(&mut e, 0, &choose_graveyard_trigger_target(bear));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&bear].zone, Zone::Hand);

    // A land card in the graveyard is not a legal target.
    let mut land = engine(819_015);
    let forest = inject_graveyard_card(&mut land, 0, "forest");
    cast(&mut land, 0, "ragamuffin_raptor", vec![]);
    assert!(
        land.apply_command(0, &choose_graveyard_trigger_target(forest))
            .is_err(),
        "a land card is not a legal target"
    );
}

#[test]
fn issue_misc19_defibrillating_current_damages_and_gains_life() {
    let mut e = engine(819_006);
    let theirs = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 6, 6);
    let life_before = e.state.players[0].life;
    cast(&mut e, 0, "defibrillating_current", target_object(theirs));
    assert_eq!(e.state.objects[&theirs].damage, 4);
    assert_eq!(e.state.players[0].life, life_before + 2);

    // A land is not a legal target.
    let mut land = engine(819_016);
    let forest = inject_permanent_on_battlefield(&mut land, 1, "forest");
    inject_card_into_hand(&mut land, 0, "defibrillating_current");
    grant_pool(&mut land, 0);
    let slot = hand_index_for_card(&land, 0, "defibrillating_current");
    assert!(
        land.apply_command(0, &cast_spell(slot, target_object(forest)))
            .is_err(),
        "a land is not a legal target"
    );
}

#[test]
fn issue_misc19_unsparing_boltcaster_needs_a_damaged_target() {
    let mut e = engine(819_007);
    let theirs = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 10, 10);
    cast(&mut e, 0, "lightning_bolt", target_object(theirs));
    assert_eq!(
        e.state.objects[&theirs].damage, 3,
        "the Bolt marks it this turn"
    );
    cast(&mut e, 0, "unsparing_boltcaster", vec![]);
    assert_eq!(e.state.pending_triggers.len(), 1, "the entry trigger waits");
    semantic::accepted(&mut e, 0, &choose_trigger_target(theirs));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&theirs].damage, 8, "3 + 5 total damage");

    // An undamaged creature is not a legal target.
    let mut fresh = engine(819_017);
    let untouched = inject_creature_with_stats(&mut fresh, 1, "grizzly_bears", 10, 10);
    cast(&mut fresh, 0, "unsparing_boltcaster", vec![]);
    assert!(
        fresh
            .apply_command(0, &choose_trigger_target(untouched))
            .is_err(),
        "an undamaged creature is not a legal target"
    );
}

#[test]
fn issue_misc19_emerge_from_the_cocoon_reanimates_and_gains_life() {
    let mut e = engine(819_008);
    let bear = inject_graveyard_card(&mut e, 0, "grizzly_bears");
    let life_before = e.state.players[0].life;
    cast(
        &mut e,
        0,
        "emerge_from_the_cocoon",
        target_graveyard_card(bear),
    );
    assert_eq!(e.state.objects[&bear].zone, Zone::Battlefield);
    assert_eq!(e.state.players[0].life, life_before + 3);
}
