//! Reviewed direct-RON mixed-permanent scenarios: Thirst for Identity, Thousand Moons Infantry,
//! Henchbots, Well-Worn Spatula, Spotcycle Scouter, Agna Qel'a, Nutrient Block and Scene of the
//! Crime.
//!
//! Exact Scryfall records and `rulings_uri` were fetched 2026-09-21 against the pinned snapshot.
//! Every expectation is the reviewed printed Oracle behavior. Governance: CR 121.1 (draw),
//! CR 701.8 (discard), CR 502.3 (untap step), CR 603.6a (entry trigger), CR 610.3/608.2b (exile
//! until source leaves), CR 301.5/702.6 (Equipment and equip), CR 701.22 (scry), CR 702.122
//! (crew), CR 614.1c/d (enters tapped), CR 702.12b (indestructible), and CR 700.4 ("dies").

use super::helpers::*;
use tricerules_core::{TurnStep, Zone};
use tricerules_proto::ruled::v1::{
    cost_selection::Selection, ChoiceKind, ChooseTriggerTarget, CostObjectRef, CostObjectRefs,
    CostSelection, ResolutionChoiceRequired, RuledCommand, TargetRef, TargetRefKind,
};

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("swamp", &[]), deck_with("island", &[])]);
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

/// Drives passes until the engine parks on a resolution choice.
fn drive_to_choice(e: &mut GameEngine) -> ResolutionChoiceRequired {
    for _ in 0..40 {
        answer_trigger_order_in_engine_order(e);
        let player = e.state.priority_player_id();
        let batch = semantic::accepted(e, player, &pass());
        if let Some(choice) = find_resolution_choice(&batch) {
            return choice;
        }
    }
    panic!("resolution never parked on a choice");
}

fn object_cost_selection(cost_index: u32, e: &GameEngine, objects: &[u32]) -> CostSelection {
    CostSelection {
        cost_index,
        selection: Some(Selection::BattlefieldObjects(CostObjectRefs {
            objects: objects
                .iter()
                .map(|object_id| CostObjectRef {
                    object_id: *object_id,
                    zone_change_generation: generation(e, *object_id),
                })
                .collect(),
        })),
    }
}

/// Activates an ability on a permanent that entered through a real cast/land play, binding the
/// source's current zone-change generation so the engine does not reject it as stale.
fn activate_on(
    e: &GameEngine,
    object_id: u32,
    ability_index: u32,
    targets: Vec<TargetRef>,
    cost_selections: Vec<CostSelection>,
) -> RuledCommand {
    let mut command =
        activate_ability_with_costs(object_id, ability_index, targets, cost_selections);
    let Some(tricerules_proto::ruled::v1::ruled_command::Cmd::ActivateAbility(activation)) =
        command.cmd.as_mut()
    else {
        unreachable!()
    };
    activation.expected_zone_change_generation = generation(e, object_id);
    command
}

#[test]
fn issue_misc11_thirst_for_identity_draws_three_then_discards_a_creature() {
    let mut e = engine(811_001);
    let creature = inject_card_into_hand(&mut e, 0, "grizzly_bears");
    let land = inject_card_into_hand(&mut e, 0, "island");
    inject_card_into_hand(&mut e, 0, "thirst_for_identity");
    let hand_before = e.state.players[0].hand.len();
    let library_top: Vec<u32> = e.state.players[0].library.iter().take(3).copied().collect();
    let slot = hand_index_for_card(&e, 0, "thirst_for_identity");
    semantic::accepted(&mut e, 0, &cast_spell(slot, vec![]));
    let choice = drive_to_choice(&mut e);
    assert_eq!(choice.choice_kind(), ChoiceKind::HandCards);
    // The spell leaves hand and three cards are drawn before the discard choice.
    assert_eq!(e.state.players[0].hand.len(), hand_before - 1 + 3);
    assert!(library_top
        .iter()
        .all(|oid| e.state.players[0].hand.contains(oid)));

    // An empty selection cannot satisfy the mandatory two-card alternative.
    assert!(e
        .apply_command(0, &submit_resolution_choice(vec![]))
        .is_err());

    // Discarding the single creature satisfies "unless you discard a creature card".
    semantic::accepted(&mut e, 0, &submit_resolution_choice(vec![creature]));
    assert_eq!(e.state.objects[&creature].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&land].zone, Zone::Hand);
    assert!(e.state.pending_resolution.is_none());
}

#[test]
fn issue_misc11_thousand_moons_infantry_untaps_on_another_players_untap_step() {
    let decks = Some(vec![
        deck_with("plains", &["thousand_moons_infantry"]),
        deck_with("island", &[]),
    ]);
    let mut e = GameEngine::new(811_002, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut e);
    let infantry = relocate_to_battlefield(&mut e, 0, "thousand_moons_infantry", true);
    assert!(e.state.objects[&infantry].tapped);

    e.state.turn_step = TurnStep::EndStep;
    e.state.active_player_idx = 0;
    e.state.priority_idx = 0;
    e.state.passes_since_stack_change = 0;
    let _ = e.initial_response_batch();
    semantic::accepted(&mut e, 0, &primitive_yield());

    assert!(
        !e.state.objects[&infantry].tapped,
        "the Infantry untaps during the opponent's untap step (CR 502.3)"
    );
}

#[test]
fn issue_misc11_henchbots_exiles_a_tapped_creature_until_it_leaves() {
    let mut e = engine(811_003);
    let their_creature = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    e.state.objects.get_mut(&their_creature).unwrap().tapped = true;
    // An untapped creature is not a legal target for the entry trigger.
    let untapped = inject_creature_on_battlefield(&mut e, 1, "storm_crow");
    cast(&mut e, 0, "henchbots", vec![]);
    assert_eq!(e.state.pending_triggers.len(), 1, "the entry trigger waits");
    assert!(
        e.apply_command(0, &choose_trigger_target(untapped))
            .is_err(),
        "an untapped creature is not a legal target"
    );
    semantic::accepted(&mut e, 0, &choose_trigger_target(their_creature));
    resolve_entire_stack_two_player(&mut e);
    let henchbots = battlefield_object(&e, 0, "henchbots");
    assert_eq!(e.state.objects[&their_creature].zone, Zone::Exile);

    // When the source leaves, the exiled card returns under its owner's control.
    cast(&mut e, 0, "murder", target_object(henchbots));
    assert_eq!(e.state.objects[&henchbots].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&their_creature].zone, Zone::Battlefield);
    assert_eq!(e.state.objects[&their_creature].controller, 1);
}

#[test]
fn issue_misc11_well_worn_spatula_gains_life_and_equips() {
    let mut e = engine(811_004);
    let life_before = e.state.players[0].life;
    cast(&mut e, 0, "well-worn_spatula", vec![]);
    assert_eq!(e.state.players[0].life, life_before + 2, "ETB gain 2 life");

    let spatula = battlefield_object(&e, 0, "well-worn_spatula");
    let bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let base_power = e.effective_power(bear).expect("power");
    let base_toughness = e.effective_toughness(bear).expect("toughness");
    let equip_cmd = activate_on(&e, spatula, 0, target_object(bear), vec![]);
    semantic::accepted(&mut e, 0, &equip_cmd);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.effective_power(bear), Some(base_power + 1));
    assert_eq!(e.effective_toughness(bear), Some(base_toughness + 1));

    // Equip only targets a creature its controller controls.
    let their_bear = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let illegal_equip = activate_on(&e, spatula, 0, target_object(their_bear), vec![]);
    assert!(
        e.apply_command(0, &illegal_equip).is_err(),
        "an opponent's creature is not a legal equip target"
    );
}

#[test]
fn issue_misc11_spotcycle_scouter_scries_then_crews() {
    let mut e = engine(811_005);
    let library_top: Vec<u32> = e.state.players[0].library.iter().take(2).copied().collect();
    inject_card_into_hand(&mut e, 0, "spotcycle_scouter");
    grant_pool(&mut e, 0);
    let slot = hand_index_for_card(&e, 0, "spotcycle_scouter");
    semantic::accepted(&mut e, 0, &cast_spell(slot, vec![]));
    let choice = drive_to_choice(&mut e);
    assert_eq!(choice.choice_kind(), ChoiceKind::LibraryTop);
    assert_eq!(choice.min, 0);
    assert!(choice
        .candidate_object_ids
        .iter()
        .all(|oid| library_top.contains(oid)));
    // Bottom both scried cards; the private top ordering is the only remaining step.
    semantic::accepted(
        &mut e,
        0,
        &submit_resolution_choice(vec![library_top[1], library_top[0]]),
    );
    assert!(e.state.pending_resolution.is_none());
    let remaining: Vec<u32> = e.state.players[0].library.iter().take(2).copied().collect();
    assert!(
        remaining.iter().all(|oid| !library_top.contains(oid)),
        "both scried cards left the top of the library"
    );

    // Crew 1: tap one creature with power 1 or more to animate the Vehicle for the turn.
    let scouter = battlefield_object(&e, 0, "spotcycle_scouter");
    let pilot = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    assert!(!e.characteristics(scouter).unwrap().is_creature());
    let crew_cost = object_cost_selection(0, &e, &[pilot]);
    let crew_cmd = activate_on(&e, scouter, 0, vec![], vec![crew_cost]);
    semantic::accepted(&mut e, 0, &crew_cmd);
    resolve_entire_stack_two_player(&mut e);
    assert!(e.state.objects[&pilot].tapped, "the pilot is tapped");
    let crewed = e.characteristics(scouter).unwrap();
    assert!(
        crewed.is_creature(),
        "the Vehicle becomes an artifact creature"
    );
    assert_eq!((crewed.power, crewed.toughness), (Some(3), Some(2)));
}

#[test]
fn issue_misc11_agna_qela_enters_tapped_and_loots() {
    // Without a basic land, the land enters tapped.
    let mut tapped = engine(811_006);
    inject_card_into_hand(&mut tapped, 0, "agna_qela");
    let slot = hand_index_for_card(&tapped, 0, "agna_qela");
    semantic::accepted(&mut tapped, 0, &play_land(slot));
    let agna = battlefield_object(&tapped, 0, "agna_qela");
    assert!(
        tapped.state.objects[&agna].tapped,
        "no basic land -> tapped"
    );

    // With a basic land controlled, it enters untapped.
    let mut untapped = engine(811_016);
    inject_permanent_on_battlefield(&mut untapped, 0, "island");
    inject_card_into_hand(&mut untapped, 0, "agna_qela");
    let slot = hand_index_for_card(&untapped, 0, "agna_qela");
    semantic::accepted(&mut untapped, 0, &play_land(slot));
    let agna = battlefield_object(&untapped, 0, "agna_qela");
    assert!(
        !untapped.state.objects[&agna].tapped,
        "a controlled basic land keeps it untapped"
    );

    // {2}{U}, {T}: draw a card, then discard a card.
    grant_pool(&mut untapped, 0);
    let hand_before = untapped.state.players[0].hand.len();
    let loot_cmd = activate_on(&untapped, agna, 1, vec![], vec![]);
    semantic::accepted(&mut untapped, 0, &loot_cmd);
    let choice = drive_to_choice(&mut untapped);
    assert_eq!(choice.choice_kind(), ChoiceKind::HandCards);
    assert_eq!(untapped.state.players[0].hand.len(), hand_before + 1);
    let discarded = *choice
        .candidate_object_ids
        .first()
        .expect("a hand card to discard");
    semantic::accepted(&mut untapped, 0, &submit_resolution_choice(vec![discarded]));
    assert_eq!(untapped.state.objects[&discarded].zone, Zone::Graveyard);
    assert!(untapped.state.pending_resolution.is_none());
}

#[test]
fn issue_misc11_nutrient_block_sacrifices_for_life_and_draws() {
    let mut e = engine(811_007);
    cast(&mut e, 0, "nutrient_block", vec![]);
    let block = battlefield_object(&e, 0, "nutrient_block");
    let life_before = e.state.players[0].life;
    let hand_before = e.state.players[0].hand.len();
    let sac_cmd = activate_on(&e, block, 0, vec![], vec![]);
    semantic::accepted(&mut e, 0, &sac_cmd);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[0].life, life_before + 3, "sac for 3 life");
    assert_eq!(e.state.objects[&block].zone, Zone::Graveyard);
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before + 1,
        "the dies trigger draws a card"
    );
}

#[test]
fn issue_misc11_scene_of_the_crime_enters_tapped_and_sacrifices_to_draw() {
    let mut e = engine(811_008);
    inject_card_into_hand(&mut e, 0, "scene_of_the_crime");
    let slot = hand_index_for_card(&e, 0, "scene_of_the_crime");
    semantic::accepted(&mut e, 0, &play_land(slot));
    let scene = battlefield_object(&e, 0, "scene_of_the_crime");
    assert!(
        e.state.objects[&scene].tapped,
        "Scene of the Crime enters tapped"
    );

    // While tapped it cannot pay {T}, but {2}, Sacrifice this land is still payable.
    let hand_before = e.state.players[0].hand.len();
    let sac_scene = activate_on(&e, scene, 2, vec![], vec![]);
    semantic::accepted(&mut e, 0, &sac_scene);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[0].hand.len(), hand_before + 1);
    assert_eq!(e.state.objects[&scene].zone, Zone::Graveyard);
}
