//! Reviewed direct-RON end-step conditional scenarios: Sami, Ship's Engineer, Frontline War-Rager,
//! Dawnstrike Vanguard, Stalactite Stalker, Ruin-Lurker Bat and Starlit Soothsayer.
//!
//! Exact Scryfall records and `rulings_uri` were fetched 2026-09-22 against the pinned snapshot.
//! Governance: CR 513.1/603.2b (the beginning-of-end-step trigger), CR 603.4/700.11 (intervening-if
//! conditions and Descend), CR 119 (life-change history), CR 122.1 (+1/+1 counters), CR 701.21
//! (sacrifice cost), CR 701.22 (scry), CR 701.25 (surveil), CR 702.15 (lifelink), and CR 702.110
//! (menace).

use super::helpers::*;
use tricerules_cards::CounterKind;
use tricerules_core::{TurnStep, Zone};
use tricerules_proto::ruled::v1::{RuledCommand, TargetRef};

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("island", &[]), deck_with("forest", &[])]);
    let mut e = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut e);
    grant_pool(&mut e, 0);
    grant_pool(&mut e, 1);
    e
}

fn tap(e: &mut GameEngine, object_id: u32) {
    e.state.objects.get_mut(&object_id).expect("object").tapped = true;
}

fn generation(e: &GameEngine, object_id: u32) -> u64 {
    e.state
        .zone_change_generation
        .get(&object_id)
        .copied()
        .unwrap_or(0)
}

fn activate_on(
    e: &GameEngine,
    object_id: u32,
    ability_index: u32,
    targets: Vec<TargetRef>,
) -> RuledCommand {
    let mut command = activate_ability_with_costs(object_id, ability_index, targets, vec![]);
    let Some(tricerules_proto::ruled::v1::ruled_command::Cmd::ActivateAbility(activation)) =
        command.cmd.as_mut()
    else {
        unreachable!()
    };
    activation.expected_zone_change_generation = generation(e, object_id);
    command
}

fn kill_with_murder(e: &mut GameEngine, source: u32) {
    inject_card_into_hand(e, 0, "murder");
    let slot = hand_index_for_card(e, 0, "murder");
    semantic::accepted(e, 0, &cast_spell(slot, target_object(source)));
    e.apply_command(0, &pass()).expect("p0 pass");
    e.apply_command(1, &pass()).expect("murder resolves");
}

fn advance_to_end_step(e: &mut GameEngine, active: i32) {
    e.apply_command(active, &primitive_yield())
        .expect("main 1 to beginning of combat");
    e.apply_command(active, &primitive_yield())
        .expect("beginning of combat advance");
    if e.state.turn_step == TurnStep::DeclareAttackers {
        e.apply_command(active, &primitive_yield())
            .expect("declare no attackers");
    }
    e.apply_command(active, &primitive_yield())
        .expect("end combat to main 2");
    e.apply_command(active, &primitive_yield())
        .expect("main 2 to end step");
    assert_eq!(e.state.turn_step, TurnStep::EndStep);
}

/// Resolves the stack until it empties or a resolution choice (scry/surveil) parks.
fn resolve_until_parked(e: &mut GameEngine) {
    for _ in 0..40 {
        if e.state.pending_resolution.is_some() || e.state.stack.is_empty() {
            return;
        }
        answer_trigger_order_in_engine_order(e);
        let priority = e.state.priority_player_id();
        semantic::accepted(e, priority, &pass());
    }
    panic!("stack never settled");
}

#[test]
fn issue_misc21_sami_makes_a_robot_with_two_tapped_creatures() {
    let mut e = engine(821_001);
    inject_permanent_on_battlefield(&mut e, 0, "sami,_ships_engineer");
    let a = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let b = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    tap(&mut e, a);
    tap(&mut e, b);
    advance_to_end_step(&mut e, 0);
    resolve_until_parked(&mut e);
    let robots = battlefield_token_oids(&e, 0, "robot_c_2_2");
    assert_eq!(robots.len(), 1, "the end-step trigger creates a Robot");
    assert!(
        e.state.objects[&robots[0]].tapped,
        "the Robot enters tapped"
    );

    // Fewer than two tapped creatures suppresses the intervening-if.
    let mut few = engine(821_011);
    inject_permanent_on_battlefield(&mut few, 0, "sami,_ships_engineer");
    let only = inject_creature_on_battlefield(&mut few, 0, "grizzly_bears");
    tap(&mut few, only);
    advance_to_end_step(&mut few, 0);
    resolve_until_parked(&mut few);
    assert!(battlefield_token_oids(&few, 0, "robot_c_2_2").is_empty());
}

#[test]
fn issue_misc21_frontline_war_rager_counts_itself_up() {
    let mut e = engine(821_002);
    let rager = inject_creature_with_stats(&mut e, 0, "frontline_war-rager", 2, 3);
    let a = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let b = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    tap(&mut e, a);
    tap(&mut e, b);
    advance_to_end_step(&mut e, 0);
    resolve_until_parked(&mut e);
    assert_eq!(
        e.state.objects[&rager].counter_count(CounterKind::PlusOnePlusOne),
        1
    );
}

#[test]
fn issue_misc21_dawnstrike_vanguard_pumps_each_other_creature() {
    let mut e = engine(821_003);
    let vanguard = inject_creature_with_stats(&mut e, 0, "dawnstrike_vanguard", 4, 5);
    let a = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let b = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    tap(&mut e, a);
    tap(&mut e, b);
    assert!(e.effective_has_keyword(vanguard, tricerules_cards::Keyword::Lifelink));
    advance_to_end_step(&mut e, 0);
    resolve_until_parked(&mut e);
    assert_eq!(
        e.state.objects[&a].counter_count(CounterKind::PlusOnePlusOne),
        1
    );
    assert_eq!(
        e.state.objects[&b].counter_count(CounterKind::PlusOnePlusOne),
        1
    );
    assert_eq!(
        e.state.objects[&vanguard].counter_count(CounterKind::PlusOnePlusOne),
        0,
        "other than this creature"
    );
}

#[test]
fn issue_misc21_stalactite_stalker_descends_and_scales_its_activation() {
    let mut e = engine(821_004);
    let stalker = inject_creature_with_stats(&mut e, 0, "stalactite_stalker", 1, 1);
    let fodder = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    kill_with_murder(&mut e, fodder);
    advance_to_end_step(&mut e, 0);
    resolve_until_parked(&mut e);
    assert_eq!(
        e.state.objects[&stalker].counter_count(CounterKind::PlusOnePlusOne),
        1,
        "descend satisfied the intervening-if"
    );

    // {2}{B}, Sacrifice this creature: target creature gets -X/-X, X = the Stalker's power.
    let mut act = engine(821_014);
    let source = inject_creature_with_stats(&mut act, 0, "stalactite_stalker", 3, 1);
    let target = inject_creature_with_stats(&mut act, 1, "grizzly_bears", 5, 5);
    grant_pool(&mut act, 0);
    let command = activate_on(&act, source, 0, target_object(target));
    semantic::accepted(&mut act, 0, &command);
    resolve_entire_stack_two_player(&mut act);
    assert_eq!(act.state.objects[&source].zone, Zone::Graveyard);
    assert_eq!(act.effective_power(target), Some(2), "5 - 3");
    assert_eq!(act.effective_toughness(target), Some(2));
}

#[test]
fn issue_misc21_ruin_lurker_bat_scries_when_descended() {
    let mut e = engine(821_005);
    let bat = inject_creature_on_battlefield(&mut e, 0, "ruin-lurker_bat");
    assert!(e.effective_has_keyword(bat, tricerules_cards::Keyword::Flying));
    assert!(e.effective_has_keyword(bat, tricerules_cards::Keyword::Lifelink));
    let fodder = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    kill_with_murder(&mut e, fodder);
    advance_to_end_step(&mut e, 0);
    resolve_until_parked(&mut e);
    assert!(
        e.state.pending_resolution.is_some(),
        "the scry 1 parks a library-top choice"
    );
    semantic::accepted(&mut e, 0, &submit_resolution_choice(vec![]));
    assert!(e.state.pending_resolution.is_none());
}

#[test]
fn issue_misc21_starlit_soothsayer_surveils_after_a_life_change() {
    let mut e = engine(821_006);
    let soothsayer = inject_creature_on_battlefield(&mut e, 0, "starlit_soothsayer");
    assert!(e.effective_has_keyword(soothsayer, tricerules_cards::Keyword::Flying));
    // A life change to the controller this turn satisfies the intervening-if.
    inject_card_into_hand(&mut e, 0, "lightning_bolt");
    grant_pool(&mut e, 0);
    let slot = hand_index_for_card(&e, 0, "lightning_bolt");
    semantic::accepted(&mut e, 0, &cast_spell(slot, target_player(0)));
    resolve_entire_stack_two_player(&mut e);
    advance_to_end_step(&mut e, 0);
    resolve_until_parked(&mut e);
    assert!(
        e.state.pending_resolution.is_some(),
        "the surveil 1 parks a library choice"
    );
    semantic::accepted(&mut e, 0, &submit_resolution_choice(vec![]));
}
