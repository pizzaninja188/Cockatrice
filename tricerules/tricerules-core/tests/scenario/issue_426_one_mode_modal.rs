//! Issue #426 — the ten retained one-mode-away `Choose one —` modal identities.
//!
//! Oracle and the current Comprehensive Rules were checked 2026-09-19 against the pinned
//! Scryfall snapshot. CR 700.2 (mode announcement), CR 120 (damage), CR 701.26 (untap),
//! CR 122 (counters), CR 702.12b (indestructible), CR 701.44 (explore), CR 613.1d (type
//! addition), CR 701.9/701.20 (discard and public reveal), and CR 608.2c-f (printed instruction
//! order) govern the exercised behavior.

use super::helpers::*;
use tricerules_cards::{CounterKind, Keyword};
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::ChoiceKind;

fn modal_engine(seed: u64) -> GameEngine {
    // Explicit land-only decks keep the default grizzly_bears/library fixtures out of the
    // opening hand so injected fixture names stay unique.
    let decks = Some(vec![deck_with("mountain", &[]), deck_with("mountain", &[])]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn prepare_spell(engine: &mut GameEngine, card_id: &str) -> usize {
    inject_card_into_hand(engine, 0, card_id);
    grant_pool(engine, 0);
    hand_index_for_card(engine, 0, card_id)
}

fn seat_on_top(engine: &mut GameEngine, player: usize, card_ids: &[&str]) -> Vec<u32> {
    let object_ids = card_ids
        .iter()
        .map(|card_id| inject_library_card(engine, player, card_id))
        .collect::<Vec<_>>();
    engine.state.players[player]
        .library
        .retain(|object_id| !object_ids.contains(object_id));
    for object_id in object_ids.iter().rev() {
        engine.state.players[player].library.push_front(*object_id);
    }
    object_ids
}

fn cast_murder(engine: &mut GameEngine, target: u32) {
    inject_card_into_hand(engine, 0, "murder");
    grant_pool(engine, 0);
    let slot = hand_index_for_card(engine, 0, "murder");
    engine
        .apply_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast Murder");
    resolve_entire_stack_two_player(engine);
}

fn jace_with_loyalty(engine: &mut GameEngine, player: usize, loyalty: u32) -> u32 {
    let jace = inject_permanent_on_battlefield(engine, player, "jace_beleren");
    engine
        .state
        .objects
        .get_mut(&jace)
        .expect("planeswalker object")
        .counters
        .insert(CounterKind::Loyalty, loyalty);
    jace
}

#[test]
fn cerebral_confiscation_reveals_and_discards_a_chosen_nonland() {
    let mut engine = modal_engine(426_001);
    let cleared: Vec<_> = engine.state.players[1].hand.drain(..).collect();
    engine.state.players[1].library.extend(cleared);
    let nonland = inject_card_into_hand(&mut engine, 1, "grizzly_bears");
    let land = inject_card_into_hand(&mut engine, 1, "mountain");
    let slot = prepare_spell(&mut engine, "cerebral_confiscation");

    engine
        .apply_command(0, &cast_modal_spell(slot, vec![(1, target_player(1))]))
        .expect("cast the reveal mode");
    engine.apply_command(0, &pass()).expect("caster passes");
    let parked = engine
        .apply_command(1, &pass())
        .expect("opponent passes and the spell parks for the choice");

    let choice = find_resolution_choice(&parked).expect("opponent-hand choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::OpponentHand);
    assert_eq!(choice.deciding_player_id, 0, "the caster chooses");
    assert_eq!((choice.min, choice.max), (1, 1));
    assert_eq!(
        choice
            .public_reveal
            .as_ref()
            .map(|reveal| reveal.zone_owner_player_id),
        Some(1),
        "CR 701.20: the revealed hand window is public"
    );
    let land_index = choice
        .candidate_object_ids
        .iter()
        .position(|object_id| *object_id == land)
        .expect("land remains visible");
    assert!(
        !choice.candidate_selectable[land_index],
        "the nonland filter publishes the land as visible but ineligible"
    );

    engine
        .apply_command(0, &submit_resolution_choice(vec![nonland]))
        .expect("choose the nonland card");
    assert_eq!(engine.state.objects[&nonland].zone, Zone::Graveyard);
    assert!(engine.state.players[1].hand.contains(&land));
}

#[test]
fn cerebral_confiscation_discard_two_mode_uses_the_affected_player_choice() {
    let mut engine = modal_engine(426_002);
    let cleared: Vec<_> = engine.state.players[1].hand.drain(..).collect();
    engine.state.players[1].library.extend(cleared);
    let first = inject_card_into_hand(&mut engine, 1, "grizzly_bears");
    let second = inject_card_into_hand(&mut engine, 1, "storm_crow");
    let kept = inject_card_into_hand(&mut engine, 1, "forest");
    let slot = prepare_spell(&mut engine, "cerebral_confiscation");

    engine
        .apply_command(0, &cast_modal_spell(slot, vec![(0, target_player(1))]))
        .expect("cast the discard-two mode");
    engine.apply_command(0, &pass()).expect("caster passes");
    let parked = engine
        .apply_command(1, &pass())
        .expect("the affected player receives the choice");
    let choice = find_resolution_choice(&parked).expect("discard choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::HandCards);
    assert_eq!(choice.deciding_player_id, 1, "the affected player chooses");
    engine
        .apply_command(1, &submit_resolution_choice(vec![first, second]))
        .expect("the affected player discards two cards");

    assert_eq!(engine.state.objects[&first].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&second].zone, Zone::Graveyard);
    assert!(engine.state.players[1].hand.contains(&kept));
}

#[test]
fn collision_course_counts_creatures_and_vehicles_you_control() {
    let mut engine = modal_engine(426_010);
    let target = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    engine.state.objects.get_mut(&target).unwrap().toughness = Some(5);
    inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    inject_creature_on_battlefield(&mut engine, 0, "storm_crow");
    inject_permanent_on_battlefield(&mut engine, 0, "cultivators_caravan");
    let slot = prepare_spell(&mut engine, "collision_course");

    engine
        .apply_command(0, &cast_modal_spell(slot, vec![(0, target_object(target))]))
        .expect("cast the creature and/or Vehicle count damage");
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(
        engine.state.objects[&target].damage, 3,
        "two creatures plus one Vehicle count three permanents"
    );
    assert_eq!(engine.state.objects[&target].zone, Zone::Battlefield);
}

#[test]
fn coordinated_maneuver_damages_a_planeswalker_with_the_creature_count() {
    let mut engine = modal_engine(426_011);
    let jace = jace_with_loyalty(&mut engine, 1, 5);
    inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    inject_creature_on_battlefield(&mut engine, 0, "storm_crow");
    let slot = prepare_spell(&mut engine, "coordinated_maneuver");

    engine
        .apply_command(0, &cast_modal_spell(slot, vec![(0, target_object(jace))]))
        .expect("the creature-or-planeswalker target is legal");
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(
        engine.state.objects[&jace]
            .counters
            .get(&CounterKind::Loyalty),
        Some(&3),
        "two controlled creatures deal two loyalty damage"
    );
    assert_eq!(engine.state.objects[&jace].zone, Zone::Battlefield);
}

#[test]
fn crash_and_burn_six_damage_destroys_a_planeswalker() {
    let mut engine = modal_engine(426_020);
    let jace = jace_with_loyalty(&mut engine, 1, 5);
    let carved = inject_permanent_on_battlefield(&mut engine, 1, "short_sword");
    let slot = prepare_spell(&mut engine, "crash_and_burn");

    engine
        .apply_command(0, &cast_modal_spell(slot, vec![(1, target_object(jace))]))
        .expect("six damage to a planeswalker");
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.objects[&jace].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&carved].zone, Zone::Battlefield);

    let mut vehicle_engine = modal_engine(426_021);
    let vehicle = inject_permanent_on_battlefield(&mut vehicle_engine, 1, "cultivators_caravan");
    let slot = prepare_spell(&mut vehicle_engine, "crash_and_burn");
    vehicle_engine
        .apply_command(
            0,
            &cast_modal_spell(slot, vec![(0, target_object(vehicle))]),
        )
        .expect("the paired Vehicle mode destroys the Vehicle");
    resolve_entire_stack_two_player(&mut vehicle_engine);
    assert_eq!(vehicle_engine.state.objects[&vehicle].zone, Zone::Graveyard);
}

#[test]
fn frontline_rush_pump_scales_with_controlled_creatures() {
    let mut engine = modal_engine(426_030);
    let target = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    inject_creature_on_battlefield(&mut engine, 0, "storm_crow");
    let slot = prepare_spell(&mut engine, "frontline_rush");

    engine
        .apply_command(0, &cast_modal_spell(slot, vec![(1, target_object(target))]))
        .expect("cast the X pump mode");
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.effective_power(target), Some(4));
    assert_eq!(engine.effective_toughness(target), Some(4));
}

#[test]
fn keep_out_damages_only_tapped_creatures() {
    let mut engine = modal_engine(426_040);
    let target = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let slot = prepare_spell(&mut engine, "keep_out");

    assert!(
        engine
            .apply_command(0, &cast_modal_spell(slot, vec![(0, target_object(target))]))
            .is_err(),
        "an untapped creature is not a legal target"
    );
    engine.state.objects.get_mut(&target).unwrap().tapped = true;
    engine
        .apply_command(0, &cast_modal_spell(slot, vec![(0, target_object(target))]))
        .expect("the tapped creature is a legal target");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&target].zone, Zone::Graveyard);

    let mut enchantment_engine = modal_engine(426_041);
    let anthem = inject_permanent_on_battlefield(&mut enchantment_engine, 1, "glorious_anthem");
    let slot = prepare_spell(&mut enchantment_engine, "keep_out");
    enchantment_engine
        .apply_command(0, &cast_modal_spell(slot, vec![(1, target_object(anthem))]))
        .expect("the paired enchantment mode is legal");
    resolve_entire_stack_two_player(&mut enchantment_engine);
    assert_eq!(
        enchantment_engine.state.objects[&anthem].zone,
        Zone::Graveyard
    );
}

#[test]
fn moment_of_valor_untaps_pumps_and_grants_indestructible() {
    let mut engine = modal_engine(426_050);
    let target = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    engine.state.objects.get_mut(&target).unwrap().tapped = true;
    let slot = prepare_spell(&mut engine, "moment_of_valor");

    engine
        .apply_command(0, &cast_modal_spell(slot, vec![(0, target_object(target))]))
        .expect("cast the untap, pump, and indestructible mode");
    resolve_entire_stack_two_player(&mut engine);

    assert!(!engine.state.objects[&target].tapped);
    assert_eq!(engine.effective_power(target), Some(3));
    assert_eq!(engine.effective_toughness(target), Some(2));
    assert!(engine.effective_has_keyword(target, Keyword::Indestructible));

    cast_murder(&mut engine, target);
    assert_eq!(
        engine.state.objects[&target].zone,
        Zone::Battlefield,
        "CR 702.12b: destroy cannot remove the indestructible target"
    );
}

#[test]
fn over_the_edge_explores_twice_and_keeps_its_destroy_mode() {
    let mut engine = modal_engine(426_060);
    let explorer = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    seat_on_top(&mut engine, 0, &["forest", "island"]);
    let hand_before = engine.state.players[0].hand.len();
    let slot = prepare_spell(&mut engine, "over_the_edge");

    engine
        .apply_command(
            0,
            &cast_modal_spell(slot, vec![(1, target_object(explorer))]),
        )
        .expect("cast the double-explore mode");
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(
        engine.state.players[0].hand.len(),
        hand_before + 2,
        "both explores reveal and move a land"
    );
    assert!(engine.state.objects[&explorer].counters.is_empty());
    assert_eq!(engine.state.objects[&explorer].zone, Zone::Battlefield);

    let mut destroy_engine = modal_engine(426_061);
    let anthem = inject_permanent_on_battlefield(&mut destroy_engine, 1, "glorious_anthem");
    let slot = prepare_spell(&mut destroy_engine, "over_the_edge");
    destroy_engine
        .apply_command(0, &cast_modal_spell(slot, vec![(0, target_object(anthem))]))
        .expect("the paired destroy mode is legal");
    resolve_entire_stack_two_player(&mut destroy_engine);
    assert_eq!(destroy_engine.state.objects[&anthem].zone, Zone::Graveyard);
}

#[test]
fn spectacular_tactics_counters_and_grants_hexproof() {
    let mut engine = modal_engine(426_070);
    let target = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let slot = prepare_spell(&mut engine, "spectacular_tactics");

    engine
        .apply_command(0, &cast_modal_spell(slot, vec![(0, target_object(target))]))
        .expect("cast the counter and hexproof mode");
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(
        engine.state.objects[&target]
            .counters
            .get(&CounterKind::PlusOnePlusOne),
        Some(&1)
    );
    assert!(engine.effective_has_keyword(target, Keyword::Hexproof));

    inject_card_into_hand(&mut engine, 1, "abrade");
    grant_pool(&mut engine, 1);
    let slot = hand_index_for_card(&engine, 1, "abrade");
    engine.apply_command(0, &pass()).expect("pass priority");
    assert!(
        engine
            .apply_command(1, &cast_modal_spell(slot, vec![(0, target_object(target))]))
            .is_err(),
        "CR 702.11b: an opponent cannot target the hexproof creature"
    );
}

#[test]
fn stone_by_sunlight_adds_artifact_and_grants_indestructible() {
    let mut engine = modal_engine(426_080);
    let target = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let slot = prepare_spell(&mut engine, "stone_by_sunlight");

    engine
        .apply_command(0, &cast_modal_spell(slot, vec![(1, target_object(target))]))
        .expect("cast the artifact type addition mode");
    resolve_entire_stack_two_player(&mut engine);

    let characteristics = engine.characteristics(target).expect("characteristics");
    assert!(characteristics.has_type("Artifact"));
    assert!(characteristics.is_creature());
    assert!(engine.effective_has_keyword(target, Keyword::Indestructible));

    cast_murder(&mut engine, target);
    assert_eq!(
        engine.state.objects[&target].zone,
        Zone::Battlefield,
        "CR 702.12b: the indestructible artifact survives destroy"
    );
}
