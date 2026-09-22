//! Reviewed direct-RON cost/spell/artifact scenarios: Gigastorm Titan, Fate of the Sun-Cryst,
//! Splinter's Technique, Vayne's Treachery, Barrels of Blasting Jelly, Fountainport Bell,
//! S.H.I.E.L.D. Helicarrier and Debris Beetle.
//!
//! Exact Scryfall records and `rulings_uri` were fetched 2026-09-22 against the pinned snapshot.
//! Governance: CR 118.7a/601.2f (cost reductions), CR 119.3/120 (life), CR 301.7/111.10 (Vehicle
//! and Soldier token), CR 603.6a (entry trigger), CR 701.8 (destroy), CR 701.21/118.12a
//! (sacrifice cost), CR 701.23 (search), CR 702.33 (kicker), CR 702.122 (crew), and CR 702.190
//! (sneak).

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    CastCostGroupSelection, ChoiceKind, CostObjectRef, CostObjectRefs, RuledCommand, TargetRef,
};

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("island", &[]), deck_with("forest", &[])]);
    let mut e = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut e);
    grant_pool(&mut e, 0);
    grant_pool(&mut e, 1);
    e
}

fn clear_pool(e: &mut GameEngine, player: usize) {
    e.state.players[player].mana_pool = Default::default();
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

#[test]
fn issue_misc16_gigastorm_titan_costs_less_after_another_spell() {
    let mut e = engine(816_001);
    // Without a prior spell, {1}{U} cannot pay {4}{U}.
    clear_pool(&mut e, 0);
    inject_card_into_hand(&mut e, 0, "gigastorm_titan");
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 0, "gigastorm_titan");
    assert!(
        e.apply_command(0, &cast_spell(slot, vec![])).is_err(),
        "no prior spell means no reduction"
    );

    // After casting an instant this turn the {3} reduction applies.
    let mut reduced = engine(816_011);
    cast(&mut reduced, 0, "lightning_bolt", target_player(1));
    clear_pool(&mut reduced, 0);
    inject_card_into_hand(&mut reduced, 0, "gigastorm_titan");
    give_mana(
        &mut reduced,
        0,
        ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&reduced, 0, "gigastorm_titan");
    semantic::accepted(&mut reduced, 0, &cast_spell(slot, vec![]));
    resolve_entire_stack_two_player(&mut reduced);
    assert!(
        reduced.state.players.iter().any(|p| p
            .battlefield
            .iter()
            .any(|id| reduced.state.objects[id].card_id == "gigastorm_titan")),
        "the reduced cost was payable"
    );
}

fn cast_to_choice(
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

fn cast(e: &mut GameEngine, player: i32, card_id: &str, targets: Vec<TargetRef>) {
    inject_card_into_hand(e, player as usize, card_id);
    grant_pool(e, player as usize);
    let slot = hand_index_for_card(e, player as usize, card_id);
    semantic::accepted(e, player, &cast_spell(slot, targets));
    resolve_entire_stack_two_player(e);
}

#[test]
fn issue_misc16_fate_of_the_sun_cryst_costs_less_on_a_tapped_target() {
    let mut e = engine(816_002);
    let tapped = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    e.state.objects.get_mut(&tapped).unwrap().tapped = true;
    clear_pool(&mut e, 0);
    inject_card_into_hand(&mut e, 0, "fate_of_the_sun-cryst");
    give_mana(
        &mut e,
        0,
        ManaGift {
            w: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 0, "fate_of_the_sun-cryst");
    semantic::accepted(&mut e, 0, &cast_spell(slot, target_object(tapped)));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&tapped].zone, Zone::Graveyard);

    // An untapped target gets no reduction, so the {2}{W} amount is insufficient.
    let mut full = engine(816_012);
    let untapped = inject_creature_on_battlefield(&mut full, 1, "grizzly_bears");
    clear_pool(&mut full, 0);
    inject_card_into_hand(&mut full, 0, "fate_of_the_sun-cryst");
    give_mana(
        &mut full,
        0,
        ManaGift {
            w: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&full, 0, "fate_of_the_sun-cryst");
    assert!(
        full.apply_command(0, &cast_spell(slot, target_object(untapped)))
            .is_err(),
        "an untapped target gets no reduction"
    );
}

#[test]
fn issue_misc16_splinters_technique_tutors_a_card() {
    let mut e = engine(816_003);
    let target = inject_library_card(&mut e, 0, "grizzly_bears");
    inject_card_into_hand(&mut e, 0, "splinters_technique");
    grant_pool(&mut e, 0);
    let slot = hand_index_for_card(&e, 0, "splinters_technique");
    semantic::accepted(&mut e, 0, &cast_spell(slot, vec![]));
    let mut found = false;
    for _ in 0..20 {
        if let Some(choice) = e.state.pending_resolution.as_ref() {
            if choice.presentation.candidates.contains(&target) {
                found = true;
                break;
            }
        }
        if e.state.stack.is_empty() && e.state.pending_resolution.is_none() {
            break;
        }
        let priority = e.state.priority_player_id();
        semantic::accepted(&mut e, priority, &pass());
    }
    assert!(found, "the tutor parks a library choice");
    semantic::accepted(&mut e, 0, &submit_resolution_choice(vec![target]));
    assert_eq!(e.state.objects[&target].zone, Zone::Hand);
}

#[test]
fn issue_misc16_vaynes_treachery_scales_with_kicker() {
    // Unkicked: -2/-2.
    let mut plain = engine(816_004);
    let bear = inject_creature_with_stats(&mut plain, 1, "grizzly_bears", 4, 4);
    inject_card_into_hand(&mut plain, 0, "vaynes_treachery");
    grant_pool(&mut plain, 0);
    let slot = hand_index_for_card(&plain, 0, "vaynes_treachery");
    semantic::accepted(&mut plain, 0, &cast_spell(slot, target_object(bear)));
    resolve_entire_stack_two_player(&mut plain);
    assert_eq!(plain.effective_power(bear), Some(2));
    assert_eq!(plain.effective_toughness(bear), Some(2));

    // Kicked: sacrifice a creature, then -6/-6.
    let mut kicked = engine(816_014);
    let target = inject_creature_with_stats(&mut kicked, 1, "grizzly_bears", 8, 8);
    let fodder = inject_creature_on_battlefield(&mut kicked, 0, "grizzly_bears");
    inject_card_into_hand(&mut kicked, 0, "vaynes_treachery");
    grant_pool(&mut kicked, 0);
    let slot = hand_index_for_card(&kicked, 0, "vaynes_treachery");
    let selection = CastCostGroupSelection {
        group_index: 0,
        option_index: 0,
        battlefield_objects: Some(CostObjectRefs {
            objects: vec![CostObjectRef {
                object_id: fodder,
                zone_change_generation: generation(&kicked, fodder),
            }],
        }),
        ..Default::default()
    };
    semantic::accepted(
        &mut kicked,
        0,
        &cast_spell_with_cast_cost_groups(slot, target_object(target), vec![selection]),
    );
    resolve_entire_stack_two_player(&mut kicked);
    assert_eq!(kicked.state.objects[&fodder].zone, Zone::Graveyard);
    assert_eq!(kicked.effective_power(target), Some(2));
    assert_eq!(kicked.effective_toughness(target), Some(2));
}

#[test]
fn issue_misc16_barrels_of_blasting_jelly_limits_mana_and_deals_damage() {
    let mut e = engine(816_005);
    cast(&mut e, 0, "barrels_of_blasting_jelly", vec![]);
    let barrels = battlefield_object(&e, 0, "barrels_of_blasting_jelly");

    let (before_colorless, before_colors) = {
        let p = &e.state.players[0].mana_pool;
        (p.colorless, p.white + p.blue + p.black + p.red + p.green)
    };
    let mana_cmd = activate_on(&e, barrels, 0, vec![]);
    semantic::accepted(&mut e, 0, &mana_cmd);
    let (after_colorless, after_colors) = {
        let p = &e.state.players[0].mana_pool;
        (p.colorless, p.white + p.blue + p.black + p.red + p.green)
    };
    assert_eq!(
        after_colorless,
        before_colorless - 1,
        "the {{1}} cost is paid"
    );
    assert_eq!(
        after_colors,
        before_colors + 1,
        "one mana of any color is produced"
    );
    let second_mana = activate_on(&e, barrels, 0, vec![]);
    assert!(
        e.apply_command(0, &second_mana).is_err(),
        "the mana ability is limited to once each turn"
    );

    let their_creature = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 6, 6);
    let sac = activate_on(&e, barrels, 1, target_object(their_creature));
    semantic::accepted(&mut e, 0, &sac);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&barrels].zone,
        Zone::Graveyard,
        "sacrificed"
    );
    assert_eq!(e.state.objects[&their_creature].damage, 5);
}

#[test]
fn issue_misc16_fountainport_bell_searches_and_sacrifices_to_draw() {
    let mut e = engine(816_006);
    let basic = inject_library_card(&mut e, 0, "forest");
    let choice = cast_to_choice(&mut e, 0, "fountainport_bell", vec![])
        .expect("the optional search branch parks");
    assert_eq!(choice.choice_kind(), ChoiceKind::ResolutionBranch);
    semantic::accepted(&mut e, 0, &select_branch(0));
    for _ in 0..20 {
        if let Some(choice) = e.state.pending_resolution.as_ref() {
            if choice.presentation.candidates.contains(&basic) {
                break;
            }
        }
        let priority = e.state.priority_player_id();
        semantic::accepted(&mut e, priority, &pass());
    }
    semantic::accepted(&mut e, 0, &submit_resolution_choice(vec![basic]));
    assert_eq!(
        e.state.players[0].library.front(),
        Some(&basic),
        "the searched basic land goes on top"
    );

    let bell = battlefield_object(&e, 0, "fountainport_bell");
    let hand_before = e.state.players[0].hand.len();
    let bell_cmd = activate_on(&e, bell, 0, vec![]);
    semantic::accepted(&mut e, 0, &bell_cmd);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&bell].zone, Zone::Graveyard);
    assert_eq!(e.state.players[0].hand.len(), hand_before + 1);
}

fn select_branch(index: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(
            tricerules_proto::ruled::v1::ruled_command::Cmd::SubmitResolutionChoice(
                tricerules_proto::ruled::v1::SubmitResolutionChoice {
                    decision: tricerules_proto::ruled::v1::ResolutionChoiceDecision::SelectBranch
                        as i32,
                    selected_branch_index: index,
                    ..Default::default()
                },
            ),
        ),
    }
}

#[test]
fn issue_misc16_shield_helicarrier_makes_two_soldiers() {
    let mut e = engine(816_007);
    cast(&mut e, 0, "s.h.i.e.l.d._helicarrier", vec![]);
    assert_eq!(battlefield_token_oids(&e, 0, "soldier_w_1_1").len(), 2);
}

#[test]
fn issue_misc16_debris_beetle_drains_on_entry() {
    let mut e = engine(816_008);
    let life_before = e.state.players[0].life;
    let their_life = e.state.players[1].life;
    cast(&mut e, 0, "debris_beetle", vec![]);
    assert_eq!(e.state.players[0].life, life_before + 3);
    assert_eq!(e.state.players[1].life, their_life - 3);
}
