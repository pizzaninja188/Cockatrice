//! Actual-card scenarios for the queue-v46 reuse-only cohorts.
//!
//! Expectations are printed Oracle values. CR 603.6a governs enters-the-battlefield triggers;
//! CR 503.1 governs upkeep triggers; CR 118.12 governs optional payments; CR 701.21a governs
//! sacrifice; CR 107.3 governs X; CR 601.2d governs divided damage; CR 121 governs drawing;
//! CR 401.4 governs private looks; CR 611.2c/613.4c/514.2 govern pumps; CR 611.3/502.3 govern
//! skipped untap steps; CR 701.57 governs discover.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::GameEngine;
use tricerules_proto::ruled::v1::ruled_command::Cmd;
use tricerules_proto::ruled::v1::{
    cost_selection, ChoiceKind, ChooseTriggerTarget, CostObjectRef, CostSelection,
    CounterRemovalSelection, ResolutionChoiceDecision, RuledCommand, SubmitResolutionChoice,
    TargetRef,
};

fn base_engine(seed: u64) -> GameEngine {
    let decks = Some(vec![
        deck_with(
            "forest",
            &[
                "grizzly_bears",
                "white_knight",
                "air_elemental",
                "gray_ogre",
                "scathe_zombies",
            ],
        ),
        deck_with(
            "island",
            &[
                "grizzly_bears",
                "white_knight",
                "air_elemental",
                "gray_ogre",
                "scathe_zombies",
            ],
        ),
    ]);
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("engine");
    advance_to_main1_from_game_start(&mut e);
    e
}

fn seat_on_top(e: &mut GameEngine, player: usize, card_ids: &[&str]) -> Vec<u32> {
    let oids: Vec<u32> = card_ids
        .iter()
        .map(|card_id| inject_library_card(e, player, card_id))
        .collect();
    e.state.players[player]
        .library
        .retain(|oid| !oids.contains(oid));
    for &oid in oids.iter().rev() {
        e.state.players[player].library.push_front(oid);
    }
    oids
}

fn place_from_hand(e: &mut GameEngine, player: usize, card: &str) -> u32 {
    inject_card_into_hand(e, player, card);
    move_ready_to_battlefield(e, player, card)
}

fn untap(e: &mut GameEngine, object: u32) {
    e.state.objects.get_mut(&object).unwrap().tapped = false;
}

fn cast_named(
    e: &mut GameEngine,
    player: i32,
    card: &str,
    mana: ManaGift,
    targets: Vec<TargetRef>,
) {
    inject_card_into_hand(e, player as usize, card);
    give_mana(e, player, mana);
    let slot = hand_index_for_card(e, player as usize, card);
    e.apply_scenario_command(player, &cast_spell(slot, targets))
        .unwrap_or_else(|error| panic!("cast {card}: {error:?}"));
}

fn advance_to_active_player_upkeep(e: &mut GameEngine, player: i32) {
    for _ in 0..60 {
        let (actor, command) = match e.state.cleanup_discard_player {
            Some(cleanup_player) => {
                let player_index = e.state.player_idx(cleanup_player).expect("cleanup player");
                let excess = e.state.players[player_index].hand.len().saturating_sub(7);
                (
                    cleanup_player,
                    discard_cleanup_batch((0..excess as u32).collect()),
                )
            }
            None => (e.state.priority_player_id(), pass()),
        };
        e.apply_scenario_command(actor, &command)
            .expect("pass through turn");
        if e.state.active_player_id() == player
            && e.state.turn_step == tricerules_core::TurnStep::Upkeep
        {
            answer_trigger_order_in_engine_order(e);
            return;
        }
    }
    panic!("game did not reach player {player}'s upkeep");
}

fn choose_trigger_target(targets: Vec<TargetRef>) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: Vec::new(),
            targets,
        })),
    }
}

fn submit_branch(index: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            decision: ResolutionChoiceDecision::SelectBranch as i32,
            selected_branch_index: index,
            ..Default::default()
        })),
    }
}

// ── 09 flying twice pump ────────────────────────────────────────────────────

#[test]
fn q46_flying_twice_pump_canary_simplest_pumps_once() {
    let mut e = base_engine(46_090_001);
    let imp = inject_permanent_on_battlefield(&mut e, 0, "pit_imp");
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            ..Default::default()
        },
    );
    e.apply_scenario_command(0, &activate_ability_for(&e, imp, 0, vec![]))
        .expect("activate Pit Imp");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.effective_power(imp), Some(1));
    assert_eq!(e.effective_toughness(imp), Some(1));
    assert!(e.effective_has_keyword(imp, tricerules_cards::primitives::Keyword::Flying));
}

#[test]
fn q46_flying_twice_pump_canary_hardest_allows_exactly_two_activations() {
    let mut e = base_engine(46_090_002);
    let imp = inject_permanent_on_battlefield(&mut e, 0, "pit_imp");
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 3,
            ..Default::default()
        },
    );
    for _ in 0..2 {
        e.apply_scenario_command(0, &activate_ability_for(&e, imp, 0, vec![]))
            .expect("activate within the printed limit");
        resolve_entire_stack_two_player(&mut e);
    }
    assert_eq!(e.effective_power(imp), Some(2), "two +1/+0 activations");
    assert!(
        e.apply_scenario_command(0, &activate_ability_for(&e, imp, 0, vec![]))
            .is_err(),
        "the third activation is illegal"
    );
}

#[test]
fn q46_flying_twice_pump_cards_share_the_printed_shape() {
    for (index, id) in ["pit_imp", "phyrexian_battleflies", "vampire_bats"]
        .into_iter()
        .enumerate()
    {
        let mut e = base_engine(46_091_000 + index as u64);
        let source = inject_permanent_on_battlefield(&mut e, 0, id);
        give_mana(
            &mut e,
            0,
            ManaGift {
                b: 2,
                ..Default::default()
            },
        );
        e.apply_scenario_command(0, &activate_ability_for(&e, source, 0, vec![]))
            .expect("first activation");
        resolve_entire_stack_two_player(&mut e);
        e.apply_scenario_command(0, &activate_ability_for(&e, source, 0, vec![]))
            .expect("second activation");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.effective_power(source), Some(2), "{id}");
        assert_eq!(e.effective_toughness(source), Some(1), "{id}");
    }
}

// ── 06 damage divided two ───────────────────────────────────────────────────

fn divided_targets(
    first: u32,
    second: u32,
    first_damage: u32,
    second_damage: u32,
) -> Vec<TargetRef> {
    vec![
        TargetRef {
            object_id: first,
            damage_amount: first_damage,
            ..Default::default()
        },
        TargetRef {
            object_id: second,
            damage_amount: second_damage,
            ..Default::default()
        },
    ]
}

#[test]
fn q46_damage_divided_canary_simplest_two_to_one_target() {
    let mut e = base_engine(46_060_001);
    let bear = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
    cast_named(
        &mut e,
        0,
        "forked_bolt",
        ManaGift {
            r: 1,
            ..Default::default()
        },
        vec![TargetRef {
            object_id: bear,
            damage_amount: 2,
            ..Default::default()
        }],
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&bear].zone,
        tricerules_core::Zone::Graveyard
    );
}

#[test]
fn q46_damage_divided_canary_hardest_splits_between_two_targets() {
    let mut e = base_engine(46_060_002);
    let first = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
    let second = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
    cast_named(
        &mut e,
        0,
        "twin_bolt",
        ManaGift {
            r: 1,
            c: 1,
            ..Default::default()
        },
        divided_targets(first, second, 1, 1),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&first].damage, 1);
    assert_eq!(e.state.objects[&second].damage, 1);
    assert_eq!(
        e.state.objects[&first].zone,
        tricerules_core::Zone::Battlefield
    );
    assert_eq!(
        e.state.objects[&second].zone,
        tricerules_core::Zone::Battlefield
    );
}

#[test]
fn q46_damage_divided_stale_target_still_damages_the_legal_one() {
    let mut e = base_engine(46_061_001);
    let first = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
    let second = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
    cast_named(
        &mut e,
        0,
        "chandras_pyrohelix",
        ManaGift {
            r: 1,
            c: 1,
            ..Default::default()
        },
        divided_targets(first, second, 1, 1),
    );
    e.apply_scenario_command(0, &pass()).expect("p0 passes");
    inject_card_into_hand(&mut e, 1, "murder");
    give_mana(
        &mut e,
        1,
        ManaGift {
            b: 2,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 1, "murder");
    e.apply_scenario_command(1, &cast_spell(slot, target_object(first)))
        .expect("respond with Murder");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&first].zone,
        tricerules_core::Zone::Graveyard
    );
    assert_eq!(
        e.state.objects[&second].damage, 1,
        "the remaining legal target takes its allocated damage"
    );
}

// ── 05 target player draws ──────────────────────────────────────────────────

fn library_len(e: &GameEngine, player: usize) -> usize {
    e.state.players[player].library.len()
}

#[test]
fn q46_target_player_draw_canary_simplest_fixed_two() {
    let mut e = base_engine(46_050_001);
    let before_hand = e.state.players[0].hand.len();
    let before_library = library_len(&e, 0);
    cast_named(
        &mut e,
        0,
        "inspiration",
        ManaGift {
            u: 1,
            c: 3,
            ..Default::default()
        },
        target_object(0),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[0].hand.len(), before_hand + 2);
    assert_eq!(library_len(&e, 0), before_library - 2);
}

#[test]
fn q46_target_player_draw_canary_hardest_x_draws_the_announced_amount() {
    let mut e = base_engine(46_050_002);
    let before_hand = e.state.players[0].hand.len();
    inject_card_into_hand(&mut e, 0, "stroke_of_genius");
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 3,
            c: 4,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 0, "stroke_of_genius");
    e.apply_scenario_command(0, &cast_spell_x(slot, target_object(0), 3))
        .expect("cast Stroke of Genius X=3");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[0].hand.len(), before_hand + 3);
}

#[test]
fn q46_target_player_draw_cards_use_their_printed_amounts() {
    for (index, (id, mana, amount)) in [
        (
            "inspiration",
            ManaGift {
                u: 1,
                c: 3,
                ..Default::default()
            },
            2usize,
        ),
        (
            "opportunity",
            ManaGift {
                u: 2,
                c: 4,
                ..Default::default()
            },
            4,
        ),
        (
            "overflowing_insight",
            ManaGift {
                u: 3,
                c: 4,
                ..Default::default()
            },
            7,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let mut e = base_engine(46_051_000 + index as u64);
        let before_hand = e.state.players[0].hand.len();
        cast_named(&mut e, 0, id, mana, target_object(0));
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.state.players[0].hand.len(), before_hand + amount, "{id}");
    }
}

// ── 04 impulse look ─────────────────────────────────────────────────────────

fn settle_look(e: &mut GameEngine, selected: Vec<u32>, expected_order: Vec<u32>) {
    let batch = e
        .apply_scenario_command(0, &submit_resolution_choice(selected))
        .expect("select the looked-at card");
    if let Some(choice) = find_resolution_choice(&batch) {
        if choice.choice_kind() == ChoiceKind::LibraryLook {
            e.apply_scenario_command(0, &submit_resolution_choice(expected_order))
                .expect("order the rest on the bottom");
        }
    }
}

#[test]
fn q46_impulse_look_canary_simplest_anticipate() {
    let mut e = base_engine(46_040_001);
    let top = seat_on_top(&mut e, 0, &["forest", "island", "mountain"]);
    let before_hand = e.state.players[0].hand.len();
    cast_named(
        &mut e,
        0,
        "anticipate",
        ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        },
        vec![],
    );
    let batch = pass_both_players_with_batch(&mut e);
    let choice = find_resolution_choice(&batch).expect("look choice");
    assert_eq!((choice.min, choice.max), (1, 1));
    settle_look(&mut e, vec![top[1]], vec![top[0], top[2]]);
    assert_eq!(e.state.players[0].hand.len(), before_hand + 1);
    assert!(e.state.players[0].hand.contains(&top[1]));
    assert_eq!(e.state.players[0].library.back(), Some(&top[2]));
}

#[test]
fn q46_impulse_look_canary_hardest_stock_up_puts_two() {
    let mut e = base_engine(46_040_002);
    let top = seat_on_top(
        &mut e,
        0,
        &["forest", "island", "mountain", "swamp", "plains"],
    );
    cast_named(
        &mut e,
        0,
        "stock_up",
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
        vec![],
    );
    let batch = pass_both_players_with_batch(&mut e);
    let choice = find_resolution_choice(&batch).expect("look choice");
    assert_eq!((choice.min, choice.max), (2, 2));
    settle_look(&mut e, vec![top[0], top[3]], vec![top[1], top[2], top[4]]);
    assert!(e.state.players[0].hand.contains(&top[0]));
    assert!(e.state.players[0].hand.contains(&top[3]));
    assert_eq!(e.state.players[0].library.back(), Some(&top[4]));
}

#[test]
fn q46_impulse_look_impulse_looks_at_four() {
    let mut e = base_engine(46_041_001);
    let top = seat_on_top(
        &mut e,
        0,
        &["forest", "island", "mountain", "swamp", "plains"],
    );
    cast_named(
        &mut e,
        0,
        "impulse",
        ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        },
        vec![],
    );
    let batch = pass_both_players_with_batch(&mut e);
    let choice = find_resolution_choice(&batch).expect("look choice");
    assert_eq!(choice.candidate_object_ids, top[..4].to_vec());
    settle_look(&mut e, vec![top[2]], vec![top[0], top[1], top[3]]);
    assert!(e.state.players[0].hand.contains(&top[2]));
    assert!(!e.state.players[0].hand.contains(&top[4]));
}

// ── 02 archway commons ──────────────────────────────────────────────────────

#[test]
fn q46_archway_canary_simplest_pays_and_keeps_the_land() {
    let mut e = base_engine(46_020_001);
    let land = place_from_hand(&mut e, 0, "archway_commons");
    let batch = pass_both_players_with_batch(&mut e);
    let choice = find_resolution_choice(&batch).expect("pay-or-sacrifice");
    assert_eq!(choice.choice_kind(), ChoiceKind::ResolutionBranch);
    e.apply_scenario_command(0, &submit_branch(0))
        .expect("select the {1} payment");
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    submit_mana_resolution_decision(&mut e, 0, ResolutionChoiceDecision::PayMana).expect("pay {1}");
    assert_eq!(
        e.state.players[0].mana_pool.colorless, 0,
        "the printed {{1}} is paid"
    );
    assert_eq!(
        e.state.objects[&land].zone,
        tricerules_core::Zone::Battlefield
    );
}

#[test]
fn q46_archway_canary_hardest_declines_and_sacrifices() {
    let mut e = base_engine(46_020_002);
    let land = place_from_hand(&mut e, 0, "rupture_spire");
    pass_both_players_with_batch(&mut e);
    e.apply_scenario_command(
        0,
        &submit_resolution_decision(ResolutionChoiceDecision::Decline),
    )
    .expect("decline the payment");
    assert_eq!(
        e.state.objects[&land].zone,
        tricerules_core::Zone::Graveyard
    );
}

#[test]
fn q46_archway_cards_enter_tapped_and_add_any_color() {
    for (index, id) in [
        "archway_commons",
        "rupture_spire",
        "transguild_promenade",
        "gateway_plaza",
    ]
    .into_iter()
    .enumerate()
    {
        let mut e = base_engine(46_021_000 + index as u64);
        let land = place_from_hand(&mut e, 0, id);
        assert!(e.state.objects[&land].tapped, "{id}: enters tapped");
        untap(&mut e, land);
        e.apply_scenario_command(0, &activate_ability_for(&e, land, 0, vec![]))
            .expect("mana ability");
        resolve_entire_stack_two_player(&mut e);
        let pool = &e.state.players[0].mana_pool;
        assert_eq!(
            pool.white + pool.blue + pool.black + pool.red + pool.green,
            1,
            "{id}: exactly one colored mana"
        );
    }
}

// ── 03 hidden cave discover ─────────────────────────────────────────────────

#[test]
fn q46_hidden_caves_canary_simplest_mana_ability() {
    let mut e = base_engine(46_030_001);
    let land = place_from_hand(&mut e, 0, "hidden_nursery");
    untap(&mut e, land);
    e.apply_scenario_command(0, &activate_ability_for(&e, land, 0, vec![]))
        .expect("add {G}");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[0].mana_pool.green, 1);
}

#[test]
fn q46_hidden_caves_canary_hardest_discovers_and_sacrifices() {
    let mut e = base_engine(46_030_002);
    let land = place_from_hand(&mut e, 0, "hidden_nursery");
    untap(&mut e, land);
    let top = seat_on_top(&mut e, 0, &["forest", "grizzly_bears"]);
    give_mana(
        &mut e,
        0,
        ManaGift {
            g: 1,
            c: 4,
            ..Default::default()
        },
    );
    e.apply_scenario_command(0, &activate_ability_for(&e, land, 1, vec![]))
        .expect("activate discover");
    pass_both_players_with_batch(&mut e);
    e.apply_scenario_command(
        0,
        &submit_resolution_decision(ResolutionChoiceDecision::Decline),
    )
    .expect("put the discovered card into hand");
    assert_eq!(
        e.state.objects[&land].zone,
        tricerules_core::Zone::Graveyard
    );
    assert_eq!(
        e.state.objects[&top[1]].zone,
        tricerules_core::Zone::Hand,
        "the discovered nonland card goes to hand when not cast"
    );
    assert_eq!(
        e.state.players[0].library.back(),
        Some(&top[0]),
        "the other looked-at card goes to the bottom"
    );
}

#[test]
fn q46_hidden_caves_cards_add_their_printed_color() {
    for (index, (id, field)) in [
        ("hidden_nursery", "green"),
        ("hidden_cataract", "blue"),
        ("hidden_volcano", "red"),
        ("hidden_courtyard", "white"),
        ("hidden_necropolis", "black"),
    ]
    .into_iter()
    .enumerate()
    {
        let mut e = base_engine(46_031_000 + index as u64);
        let land = place_from_hand(&mut e, 0, id);
        untap(&mut e, land);
        e.apply_scenario_command(0, &activate_ability_for(&e, land, 0, vec![]))
            .expect("mana ability");
        resolve_entire_stack_two_player(&mut e);
        let pool = &e.state.players[0].mana_pool;
        let value = match field {
            "white" => pool.white,
            "blue" => pool.blue,
            "black" => pool.black,
            "red" => pool.red,
            _ => pool.green,
        };
        assert_eq!(value, 1, "{id}");
    }
}

// ── 07 enters-tap lock ──────────────────────────────────────────────────────

#[test]
fn q46_etb_tap_lock_canary_simplest_locks_the_target() {
    let mut e = base_engine(46_070_001);
    let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
    let mangler = place_from_hand(&mut e, 0, "stitched_mangler");
    e.apply_scenario_command(0, &choose_trigger_target(target_object(target)))
        .expect("choose the target");
    resolve_entire_stack_two_player(&mut e);
    assert!(e.state.objects[&target].tapped);
    assert!(
        e.state.objects[&mangler].tapped,
        "Stitched Mangler enters tapped"
    );
    advance_to_active_player_upkeep(&mut e, 1);
    assert!(
        e.state.objects[&target].tapped,
        "the target skips its controller's next untap step"
    );
}

#[test]
fn q46_etb_tap_lock_canary_hardest_flying_body_and_lock() {
    let mut e = base_engine(46_070_002);
    let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
    let dragon = place_from_hand(&mut e, 0, "white_dragon");
    assert!(e.effective_has_keyword(dragon, tricerules_cards::primitives::Keyword::Flying));
    e.apply_scenario_command(0, &choose_trigger_target(target_object(target)))
        .expect("choose the target");
    resolve_entire_stack_two_player(&mut e);
    assert!(e.state.objects[&target].tapped);
    assert_eq!(e.effective_power(dragon), Some(4));
    assert_eq!(e.effective_toughness(dragon), Some(4));
}

#[test]
fn q46_etb_tap_lock_cards_share_the_lock_shape() {
    for (index, id) in ["stitched_mangler", "chillbringer", "white_dragon"]
        .into_iter()
        .enumerate()
    {
        let mut e = base_engine(46_071_000 + index as u64);
        let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
        place_from_hand(&mut e, 0, id);
        e.apply_scenario_command(0, &choose_trigger_target(target_object(target)))
            .expect("choose the target");
        resolve_entire_stack_two_player(&mut e);
        assert!(e.state.objects[&target].tapped, "{id}");
    }
}

// ── 08 graveyard-name draw ──────────────────────────────────────────────────

#[test]
fn q46_graveyard_name_draw_canary_simplest_zero_in_graveyard() {
    let mut e = base_engine(46_080_001);
    let before = e.state.players[0].hand.len();
    cast_named(
        &mut e,
        0,
        "take_inventory",
        ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        },
        vec![],
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[0].hand.len(), before + 1);
}

#[test]
fn q46_graveyard_name_draw_canary_hardest_counts_all_graveyards() {
    let mut e = base_engine(46_080_002);
    inject_graveyard_card(&mut e, 0, "accumulated_knowledge");
    inject_graveyard_card(&mut e, 0, "accumulated_knowledge");
    inject_graveyard_card(&mut e, 1, "accumulated_knowledge");
    let before = e.state.players[0].hand.len();
    cast_named(
        &mut e,
        0,
        "accumulated_knowledge",
        ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        },
        vec![],
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.players[0].hand.len(),
        before + 4,
        "one draw plus three named cards in all graveyards"
    );
}

#[test]
fn q46_graveyard_name_draw_cards_scope_the_count_to_the_printed_graveyard() {
    for (index, id) in ["take_inventory", "frantic_inventory"]
        .into_iter()
        .enumerate()
    {
        let mut e = base_engine(46_081_000 + index as u64);
        inject_graveyard_card(&mut e, 0, id);
        inject_graveyard_card(&mut e, 1, id);
        let before = e.state.players[0].hand.len();
        cast_named(
            &mut e,
            0,
            id,
            ManaGift {
                u: 1,
                c: 1,
                ..Default::default()
            },
            vec![],
        );
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.players[0].hand.len(),
            before + 1 + 1,
            "{id}: only the controller's graveyard counts"
        );
    }
}

// ── 01 quest-counter expeditions ────────────────────────────────────────────

fn activate_quest(e: &mut GameEngine, source: u32, targets: Vec<TargetRef>) {
    let legal = e.initial_response_batch();
    let ability_key = u64::from(source) << 32;
    let choices = &legal.legal_by_player[&0].cost_choices_by_ability[&ability_key];
    let choice = choices
        .choices
        .iter()
        .find(|choice| choice.kind() == tricerules_proto::ruled::v1::CostChoiceKind::RemoveCounters)
        .expect("quest-counter cost is published");
    let removal = choice
        .counter_removal
        .as_ref()
        .expect("counter-removal options");
    let source_ref = removal.source.as_ref().expect("source counter payment");
    let costs = vec![CostSelection {
        cost_index: choice.cost_index,
        selection: Some(cost_selection::Selection::CounterRemoval(
            CounterRemovalSelection {
                source: Some(CostObjectRef {
                    object_id: source_ref.object_id,
                    zone_change_generation: source_ref.zone_change_generation,
                }),
                option_id: removal.options[0].option_id,
            },
        )),
    }];
    e.apply_scenario_command(0, &activate_ability_with_costs(source, 0, targets, costs))
        .expect("activate the quest expedition");
}

fn add_quest_counters(e: &mut GameEngine, object: u32, count: u32) {
    e.state
        .objects
        .get_mut(&object)
        .unwrap()
        .counters
        .insert(tricerules_cards::primitives::CounterKind::Quest, count);
}

#[test]
fn q46_quest_cycle_canary_simplest_gains_eight() {
    let mut e = base_engine(46_010_001);
    let source = inject_permanent_on_battlefield(&mut e, 0, "sunspring_expedition");
    add_quest_counters(&mut e, source, 3);
    let before = e.state.players[0].life;
    activate_quest(&mut e, source, vec![]);
    let _ = &e;
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[0].life, before + 8);
    assert_eq!(
        e.state.objects[&source].zone,
        tricerules_core::Zone::Graveyard
    );
}

#[test]
fn q46_quest_cycle_canary_hardest_returns_two_cards() {
    let mut e = base_engine(46_010_002);
    let source = inject_permanent_on_battlefield(&mut e, 0, "soul_stair_expedition");
    let first = inject_graveyard_card(&mut e, 0, "grizzly_bears");
    let second = inject_graveyard_card(&mut e, 0, "white_knight");
    add_quest_counters(&mut e, source, 3);
    activate_quest(
        &mut e,
        source,
        [target_object(first), target_object(second)].concat(),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&first].zone, tricerules_core::Zone::Hand);
    assert_eq!(e.state.objects[&second].zone, tricerules_core::Zone::Hand);
    assert_eq!(
        e.state.objects[&source].zone,
        tricerules_core::Zone::Graveyard
    );
}

#[test]
fn q46_quest_cycle_cards_use_their_printed_payoffs() {
    // Ior Ruin Expedition: draw two.
    let mut e = base_engine(46_011_002);
    let source = inject_permanent_on_battlefield(&mut e, 0, "ior_ruin_expedition");
    add_quest_counters(&mut e, source, 3);
    let before = e.state.players[0].hand.len();
    activate_quest(&mut e, source, vec![]);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.players[0].hand.len(),
        before + 2,
        "Ior Ruin draws two"
    );

    // Khalni Heart Expedition: up to two basic lands enter tapped.
    let mut e = base_engine(46_011_003);
    let source = inject_permanent_on_battlefield(&mut e, 0, "khalni_heart_expedition");
    let basics = seat_on_top(&mut e, 0, &["forest", "island", "mountain"]);
    add_quest_counters(&mut e, source, 3);
    activate_quest(&mut e, source, vec![]);
    let batch = pass_both_players_with_batch(&mut e);
    let choice = find_resolution_choice(&batch).expect("library search");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibrarySearch);
    assert_eq!((choice.min, choice.max), (0, 2));
    e.apply_scenario_command(0, &submit_resolution_choice(vec![basics[0], basics[1]]))
        .expect("choose two basic lands");
    resolve_entire_stack_two_player(&mut e);
    for basic in [basics[0], basics[1]] {
        assert_eq!(
            e.state.objects[&basic].zone,
            tricerules_core::Zone::Battlefield,
            "both searched basics enter"
        );
        assert!(e.state.objects[&basic].tapped, "they enter tapped");
    }

    // Sunspring Expedition: gain eight life.
    let mut e = base_engine(46_011_004);
    let source = inject_permanent_on_battlefield(&mut e, 0, "sunspring_expedition");
    add_quest_counters(&mut e, source, 3);
    let before = e.state.players[0].life;
    activate_quest(&mut e, source, vec![]);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[0].life, before + 8, "Sunspring gains eight");

    // Soul Stair Expedition: up to two creature cards return to hand.
    let mut e = base_engine(46_011_005);
    let source = inject_permanent_on_battlefield(&mut e, 0, "soul_stair_expedition");
    let first = inject_graveyard_card(&mut e, 0, "grizzly_bears");
    let second = inject_graveyard_card(&mut e, 0, "white_knight");
    add_quest_counters(&mut e, source, 3);
    activate_quest(
        &mut e,
        source,
        [target_object(first), target_object(second)].concat(),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&first].zone, tricerules_core::Zone::Hand);
    assert_eq!(e.state.objects[&second].zone, tricerules_core::Zone::Hand);
}

#[test]
fn q46_quest_cycle_landfall_puts_a_quest_counter() {
    let mut e = base_engine(46_011_001);
    let source = inject_permanent_on_battlefield(&mut e, 0, "ior_ruin_expedition");
    move_ready_to_battlefield(&mut e, 0, "forest");
    pass_both_players_with_batch(&mut e);
    answer_optional_triggered_ability_choice(&mut e, ResolutionChoiceDecision::SelectBranch);
    assert_eq!(
        e.state.objects[&source]
            .counters
            .get(&tricerules_cards::primitives::CounterKind::Quest)
            .copied(),
        Some(1),
        "the landfall trigger may put one quest counter"
    );
}
