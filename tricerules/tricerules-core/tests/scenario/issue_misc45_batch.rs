//! Actual-card semantics for five pinned Standard spells.
//! Oracle and rulings checked 2026-09-22; CR 601.2f, 608.2b-c, 701.16, 701.23.

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::ruled_event::Ev;
use tricerules_proto::ruled::v1::ChoiceKind;

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[]), deck_with("island", &[])]);
    let mut e = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut e);
    grant_pool(&mut e, 0);
    e
}

fn cast_regular(e: &mut GameEngine, card: &str, targets: Vec<TargetRef>) {
    inject_card_into_hand(e, 0, card);
    grant_pool(e, 0);
    let slot = hand_index_for_card(e, 0, card);
    semantic::accepted(e, 0, &cast_spell(slot, targets));
}

#[test]
fn issue_misc45_roadside_blowout_reduces_for_mana_value_one_and_bounces_then_draws() {
    let mut e = engine(845_001);
    let cheap = inject_creature_on_battlefield(&mut e, 1, "gingerbrute");
    let free = inject_creature_on_battlefield(&mut e, 1, "ornithopter");
    let vehicle = inject_permanent_on_battlefield(&mut e, 1, "careening_mine_cart");
    let own_creature = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let own_vehicle = inject_permanent_on_battlefield(&mut e, 0, "careening_mine_cart");
    inject_card_into_hand(&mut e, 0, "roadside_blowout");
    let slot = hand_index_for_card(&e, 0, "roadside_blowout");
    let published = &e.initial_response_batch().legal_by_player[&0].valid_targets_by_hand_slot
        [&((slot as u32) << 8)];
    let reduction = &published.targeted_cost_reduction_applications[0];
    assert_eq!(reduction.generic_mana, 2);
    assert!(reduction
        .qualifying_targets
        .iter()
        .any(|candidate| candidate.object_id == cheap));
    assert!(!reduction
        .qualifying_targets
        .iter()
        .any(|candidate| candidate.object_id == free));
    assert!(!reduction
        .qualifying_targets
        .iter()
        .any(|candidate| candidate.object_id == vehicle));
    let legal_targets = &published.groups[0].valid_permanent_ids;
    assert!(legal_targets.contains(&cheap));
    assert!(legal_targets.contains(&vehicle));
    assert!(!legal_targets.contains(&own_creature));
    assert!(!legal_targets.contains(&own_vehicle));
    let before = e.state.turn_history.current.player(0).cards_drawn;
    semantic::accepted(&mut e, 0, &cast_spell(slot, target_object(cheap)));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&cheap].zone, Zone::Hand);
    assert_eq!(
        e.state.turn_history.current.player(0).cards_drawn,
        before + 1
    );

    let mut vehicle_case = engine(845_002);
    let vehicle = inject_permanent_on_battlefield(&mut vehicle_case, 1, "careening_mine_cart");
    cast_regular(
        &mut vehicle_case,
        "roadside_blowout",
        target_object(vehicle),
    );
    resolve_entire_stack_two_player(&mut vehicle_case);
    assert_eq!(vehicle_case.state.objects[&vehicle].zone, Zone::Hand);
}

fn search_two(card: &str, subtype_card: &str, seed: u64) {
    let mut e = engine(seed);
    let basic = inject_library_card(&mut e, 0, "forest");
    let typed = inject_library_card(&mut e, 0, subtype_card);
    let illegal = inject_library_card(&mut e, 0, "grizzly_bears");
    cast_regular(&mut e, card, vec![]);
    e.apply_command(0, &pass()).expect("caster pass");
    let batch = e.apply_command(1, &pass()).expect("resolve to search");
    let choice = find_resolution_choice(&batch).expect("library search choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibrarySearch);
    assert_eq!((choice.min, choice.max), (0, 2));
    assert!(choice.candidate_object_ids.contains(&basic));
    assert!(choice.candidate_object_ids.contains(&typed));
    assert!(!choice.candidate_object_ids.contains(&illegal));
    let completion = e
        .apply_command(0, &submit_resolution_choice(vec![basic, typed]))
        .expect("choose two lands");
    assert!(completion.events.iter().any(
        |event| matches!(&event.ev, Some(Ev::Log(log)) if log.text == "P0 shuffles their library.")
    ));
    for id in [basic, typed] {
        assert_eq!(e.state.objects[&id].zone, Zone::Battlefield);
        assert!(e.state.objects[&id].tapped);
    }
}

#[test]
fn issue_misc45_map_and_route_search_basic_or_named_land_types_tapped() {
    search_two("map_the_frontier", "eroded_canyon", 845_010);
    search_two("circuitous_route", "azorius_guildgate", 845_011);
}

#[test]
fn issue_misc45_grow_extra_arms_reduces_for_spider_and_pumps() {
    let mut e = engine(845_020);
    let spider = inject_creature_on_battlefield(&mut e, 0, "giant_spider");
    let bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    inject_card_into_hand(&mut e, 0, "grow_extra_arms");
    let slot = hand_index_for_card(&e, 0, "grow_extra_arms");
    let published = &e.initial_response_batch().legal_by_player[&0].valid_targets_by_hand_slot
        [&((slot as u32) << 8)];
    let reduction = &published.targeted_cost_reduction_applications[0];
    assert_eq!(reduction.generic_mana, 1);
    assert!(reduction
        .qualifying_targets
        .iter()
        .any(|candidate| candidate.object_id == spider));
    assert!(!reduction
        .qualifying_targets
        .iter()
        .any(|candidate| candidate.object_id == bear));
    semantic::accepted(&mut e, 0, &cast_spell(slot, target_object(spider)));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.effective_power(spider), Some(6));
    assert_eq!(e.effective_toughness(spider), Some(6));
}

#[test]
fn issue_misc45_auspicious_arrival_pumps_and_investigates() {
    let mut e = engine(845_030);
    let target = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    cast_regular(&mut e, "auspicious_arrival", target_object(target));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.effective_power(target), Some(4));
    assert_eq!(e.effective_toughness(target), Some(4));
    assert_eq!(battlefield_token_oids(&e, 0, "clue").len(), 1);
}

#[test]
fn issue_misc45_auspicious_arrival_all_illegal_target_fizzles_before_investigating() {
    let mut fizzled = engine(845_031);
    let target = inject_creature_on_battlefield(&mut fizzled, 0, "grizzly_bears");
    cast_regular(&mut fizzled, "auspicious_arrival", target_object(target));
    cast_regular(&mut fizzled, "unsummon", target_object(target));
    resolve_entire_stack_two_player(&mut fizzled);
    assert!(battlefield_token_oids(&fizzled, 0, "clue").is_empty());
    assert_eq!(fizzled.state.objects[&target].zone, Zone::Hand);
}
