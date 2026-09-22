//! Reviewed direct-RON pump/combat scenarios: Dauntless Veteran, Remnant Elemental, Skystinger,
//! Friendly Ghost, Nebula Dragon, Cavern Stomper, Hermitic Nautilus and Lion Heart.
//!
//! Exact Scryfall records and `rulings_uri` were fetched 2026-09-22 against the pinned snapshot.
//! Governance: CR 115.1a/120.3 (targeted damage), CR 301.5/702.6 (Equipment and equip),
//! CR 508.3 (attack trigger), CR 509.1b/509.3b (block trigger and evasion), CR 603.6a (entry
//! trigger), CR 611.2c/613.4c (until-end-of-turn P/T), CR 701.22 (scry), CR 702.9 (flying),
//! CR 702.17 (reach), and CR 702.20 (vigilance).

use super::helpers::*;
use tricerules_core::{TurnStep, Zone};
use tricerules_proto::ruled::v1::{
    BlockPair, ChooseTriggerTarget, RuledCommand, TargetRef, TargetRefKind,
};

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[]), deck_with("island", &[])]);
    let mut e = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut e);
    grant_pool(&mut e, 0);
    grant_pool(&mut e, 1);
    e
}

fn cast(
    e: &mut GameEngine,
    player: i32,
    card_id: &str,
    targets: Vec<TargetRef>,
) -> Option<tricerules_proto::ruled::v1::ResolutionChoiceRequired> {
    inject_card_into_hand(e, player as usize, card_id);
    grant_pool(e, player as usize);
    let slot = hand_index_for_card(e, player as usize, card_id);
    let batch = semantic::accepted(e, player, &cast_spell(slot, targets));
    if let Some(choice) = find_resolution_choice(&batch) {
        return Some(choice);
    }
    for _ in 0..40 {
        if e.state.stack.is_empty() || e.state.blocking_choice().is_some() {
            return None;
        }
        let priority = e.state.priority_player_id();
        let batch = semantic::accepted(e, priority, &pass());
        if let Some(choice) = find_resolution_choice(&batch) {
            return Some(choice);
        }
    }
    panic!("cast never settled");
}

fn choose_trigger_target(object_id: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(
            tricerules_proto::ruled::v1::ruled_command::Cmd::ChooseTriggerTarget(
                ChooseTriggerTarget {
                    decline: false,
                    selected_modes: Vec::new(),
                    targets: vec![TargetRef {
                        object_id,
                        group_index: 0,
                        kind: TargetRefKind::Permanent as i32,
                        ..Default::default()
                    }],
                },
            ),
        ),
    }
}

fn battlefield_object(e: &GameEngine, player: usize, card_id: &str) -> u32 {
    *e.state.players[player]
        .battlefield
        .iter()
        .find(|id| e.state.objects[id].card_id == card_id)
        .unwrap_or_else(|| panic!("{card_id} on battlefield"))
}

fn generation(e: &GameEngine, object_id: u32) -> u64 {
    e.state
        .zone_change_generation
        .get(&object_id)
        .copied()
        .unwrap_or(0)
}

fn activate_on(e: &GameEngine, object_id: u32, ability_index: u32) -> RuledCommand {
    activate_on_with_targets(e, object_id, ability_index, vec![])
}

fn activate_on_with_targets(
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

fn advance_main1_to_declare_attackers(e: &mut GameEngine) {
    e.apply_command(0, &primitive_yield())
        .expect("main phase to beginning of combat");
    e.apply_command(0, &pass()).expect("active player passes");
    e.apply_command(1, &pass()).expect("defender passes");
    assert_eq!(e.state.turn_step, TurnStep::DeclareAttackers);
}

fn pass_to_declare_blockers(e: &mut GameEngine) {
    e.apply_command(0, &pass()).expect("active player passes");
    e.apply_command(1, &pass()).expect("defender passes");
    assert_eq!(e.state.turn_step, TurnStep::DeclareBlockers);
}

#[test]
fn issue_misc15_dauntless_veteran_pumps_the_team_when_attacking() {
    let mut e = GameEngine::new(815_001, &[0, 1], 20, None, true).expect("engine");
    advance_to_main1_from_game_start(&mut e);
    let veteran = inject_creature_with_stats(&mut e, 0, "dauntless_veteran", 2, 2);
    let bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    advance_main1_to_declare_attackers(&mut e);
    e.apply_command(0, &declare_attackers(vec![veteran, bear]))
        .expect("declare attackers");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.effective_power(veteran), Some(3), "+1/+1 to the Veteran");
    assert_eq!(e.effective_toughness(veteran), Some(3));
    assert_eq!(e.effective_power(bear), Some(3), "+1/+1 to the Bear");
    assert_eq!(e.effective_toughness(bear), Some(3));
}

#[test]
fn issue_misc15_remnant_elemental_pumps_on_landfall() {
    let mut e = engine(815_002);
    let remnant = inject_creature_with_stats(&mut e, 0, "remnant_elemental", 0, 4);
    inject_card_into_hand(&mut e, 0, "forest");
    let slot = hand_index_for_card(&e, 0, "forest");
    semantic::accepted(&mut e, 0, &play_land(slot));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.effective_power(remnant), Some(2), "+2/+0 on landfall");
    assert_eq!(
        e.effective_toughness(remnant),
        Some(4),
        "toughness unchanged"
    );
}

#[test]
fn issue_misc15_skystinger_pumps_when_blocking_a_flyer() {
    let mut e = GameEngine::new(815_003, &[0, 1], 20, None, true).expect("engine");
    advance_to_main1_from_game_start(&mut e);
    let flyer = inject_creature_with_stats(&mut e, 0, "air_elemental", 4, 4);
    let skystinger = inject_creature_with_stats(&mut e, 1, "skystinger", 3, 3);
    advance_main1_to_declare_attackers(&mut e);
    e.apply_command(0, &declare_attackers(vec![flyer]))
        .expect("declare the flyer as an attacker");
    pass_to_declare_blockers(&mut e);
    semantic::accepted(
        &mut e,
        1,
        &declare_blockers(vec![BlockPair {
            attacker_id: flyer,
            blocker_id: skystinger,
        }]),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.effective_power(skystinger),
        Some(8),
        "Reach blocks the flyer and the trigger adds +5/+0"
    );
}

#[test]
fn issue_misc15_friendly_ghost_pumps_a_target_on_entry() {
    let mut e = engine(815_004);
    let bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    cast(&mut e, 0, "friendly_ghost", vec![]);
    assert_eq!(e.state.pending_triggers.len(), 1, "the entry trigger waits");
    semantic::accepted(&mut e, 0, &choose_trigger_target(bear));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.effective_power(bear), Some(4), "+2/+4");
    assert_eq!(e.effective_toughness(bear), Some(6));
}

#[test]
fn issue_misc15_nebula_dragon_deals_three_to_any_target() {
    let mut e = engine(815_005);
    let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    cast(&mut e, 0, "nebula_dragon", vec![]);
    assert_eq!(e.state.pending_triggers.len(), 1, "the entry trigger waits");
    semantic::accepted(&mut e, 0, &choose_trigger_target(target));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&target].zone,
        Zone::Graveyard,
        "3 damage kills a 2/2"
    );
}

#[test]
fn issue_misc15_cavern_stomper_scries_and_shrinks_its_blockers() {
    let mut e = engine(815_006);
    let stomper = inject_creature_with_stats(&mut e, 0, "cavern_stomper", 7, 7);
    let weak = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let strong = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 3, 3);

    // The evasion restriction is a main-phase activated ability.
    let stomper_cmd = activate_on(&e, stomper, 0);
    semantic::accepted(&mut e, 0, &stomper_cmd);
    resolve_entire_stack_two_player(&mut e);

    advance_main1_to_declare_attackers(&mut e);
    e.apply_command(0, &declare_attackers(vec![stomper]))
        .expect("declare Stomper as an attacker");
    pass_to_declare_blockers(&mut e);
    let pairs = e.initial_response_batch().legal_by_player[&1]
        .legal_block_pairs
        .clone();
    assert!(
        pairs
            .iter()
            .any(|pair| pair.attacker_id == stomper && pair.blocker_id == strong),
        "a power-3 creature can block"
    );
    assert!(
        !pairs.iter().any(|pair| pair.blocker_id == weak),
        "a power-2-or-less creature cannot block"
    );
}

#[test]
fn issue_misc15_hermitic_nautilus_pumps_itself() {
    let mut e = engine(815_007);
    let nautilus = inject_creature_with_stats(&mut e, 0, "hermitic_nautilus", 1, 4);
    let nautilus_cmd = activate_on(&e, nautilus, 0);
    semantic::accepted(&mut e, 0, &nautilus_cmd);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.effective_power(nautilus), Some(4), "+3");
    assert_eq!(e.effective_toughness(nautilus), Some(1), "-3 from 4");
}

#[test]
fn issue_misc15_lion_heart_deals_damage_and_equips() {
    let mut e = engine(815_008);
    let target = inject_creature_with_stats(&mut e, 1, "air_elemental", 3, 3);
    cast(&mut e, 0, "lion_heart", vec![]);
    assert_eq!(e.state.pending_triggers.len(), 1, "the entry trigger waits");
    semantic::accepted(&mut e, 0, &choose_trigger_target(target));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&target].damage, 2, "2 damage to the target");

    let lion = battlefield_object(&e, 0, "lion_heart");
    let bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let lion_cmd = activate_on_with_targets(&e, lion, 0, target_object(bear));
    semantic::accepted(&mut e, 0, &lion_cmd);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.effective_power(bear), Some(4), "+2/+1");
    assert_eq!(e.effective_toughness(bear), Some(3));
}
