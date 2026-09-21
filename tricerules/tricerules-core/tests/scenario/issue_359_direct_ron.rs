//! Issue #359 — reviewed direct-RON Spree scenarios for Explosive Derailment and Unfortunate
//! Accident.
//!
//! Exact Scryfall records and `rulings_uri` were fetched 2026-09-20 against the pinned snapshot
//! identities `b23dc81d-01bb-4bf0-9932-5d32a6b22cf7` (Explosive Derailment) and
//! `011558f3-3c23-430e-a08b-8f91238e7971` (Unfortunate Accident). Both print `Spree (Choose one
//! or more additional costs.)` followed by two `+ {cost} - effect` bullets.
//!
//! CR 702.171a-e (spree), 601.2b/f-h (additional costs paid as part of the single total cost),
//! 700.2a/c/h (mode announcement and linked choices), 115.1 (targets), 608.2b (target
//! revalidation), 120.2b/120.3 (damage), 701.7 (destroy), 111.1-111.4 (tokens), and 400.3 (zone
//! identity) govern. The 2024-04-12 rulings confirm mode-local targeting, printed resolution
//! order, at-least-one-mode, and that a spell whose chosen-mode targets are all illegal does not
//! resolve (no effects at all).

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, CastCostGroupSelection, CastMethod, CastSpell, SelectedSpellMode,
};

fn spree_option(option_index: u32) -> CastCostGroupSelection {
    CastCostGroupSelection {
        group_index: 0,
        option_index,
        ..Default::default()
    }
}

fn cast_spree(
    hand_card_index: usize,
    modes: Vec<(u32, Vec<TargetRef>)>,
    options: Vec<u32>,
) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::CastSpell(CastSpell {
            cast_method: CastMethod::Normal as i32,
            source: Some(hand_cast_source(hand_card_index)),
            selected_modes: modes
                .into_iter()
                .map(|(mode_index, targets)| SelectedSpellMode {
                    mode_index,
                    targets,
                })
                .collect(),
            cast_cost_group_selections: options.into_iter().map(spree_option).collect(),
            ..Default::default()
        })),
    }
}

/// A game with both pools refilled so a scenario never starves for mana. The spell under test is
/// injected into hand rather than dealt, so the registered object used for zone assertions is the
/// exact object that gets cast.
fn spree_engine(seed: u64) -> GameEngine {
    let decks = Some(vec![
        deck_with("mountain", &[]),
        deck_with("forest", &["grizzly_bears", "explosive_apparatus"]),
    ]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    grant_pool(&mut engine, 0);
    grant_pool(&mut engine, 1);
    engine
}

fn prepare(engine: &mut GameEngine, card: &str) -> (usize, u32) {
    let source = inject_card_into_hand(engine, 0, card);
    let slot = hand_index_for_card(engine, 0, card);
    (slot, source)
}

#[test]
fn issue_359_explosive_derailment_single_modes_charge_only_the_selected_additional_cost() {
    // Mode 0 alone: four damage to a five-toughness creature, artifact untouched.
    let mut damage = spree_engine(359_001);
    let creature = inject_creature_with_stats(&mut damage, 1, "grizzly_bears", 2, 5);
    let artifact = inject_permanent_on_battlefield(&mut damage, 1, "explosive_apparatus");
    let (slot, source) = prepare(&mut damage, "explosive_derailment");
    semantic::accepted(
        &mut damage,
        0,
        &cast_spree(slot, vec![(0, target_object(creature))], vec![0]),
    );
    let stack = damage.state.stack.last().expect("spell on the stack");
    assert_eq!(stack.chosen_modes.len(), 1);
    assert_eq!(stack.chosen_modes[0].mode_id.as_str(), "damage_creature");
    assert_eq!(stack.cast_cost_receipts.len(), 1);
    // Base {R} plus only the selected additional {2}: red 9 -> 8, colorless 9 -> 7.
    assert_eq!(damage.state.players[0].mana_pool.red, 8);
    assert_eq!(damage.state.players[0].mana_pool.colorless, 7);
    semantic::complete(&mut damage, 8, |_| None).require_exercised();
    assert_eq!(
        damage.state.objects[&creature].damage, 4,
        "exactly four damage"
    );
    assert_eq!(damage.state.objects[&creature].zone, Zone::Battlefield);
    assert_eq!(damage.state.objects[&artifact].zone, Zone::Battlefield);
    assert_eq!(damage.state.objects[&source].zone, Zone::Graveyard);

    // Mode 1 alone: destroy the artifact, leave the creature untouched.
    let mut destroy = spree_engine(359_002);
    let creature = inject_creature_with_stats(&mut destroy, 1, "grizzly_bears", 2, 5);
    let artifact = inject_permanent_on_battlefield(&mut destroy, 1, "explosive_apparatus");
    let (slot, source) = prepare(&mut destroy, "explosive_derailment");
    semantic::accepted(
        &mut destroy,
        0,
        &cast_spree(slot, vec![(1, target_object(artifact))], vec![1]),
    );
    let stack = destroy.state.stack.last().expect("spell on the stack");
    assert_eq!(stack.chosen_modes[0].mode_id.as_str(), "destroy_artifact");
    assert_eq!(stack.cast_cost_receipts.len(), 1);
    assert_eq!(destroy.state.players[0].mana_pool.red, 8);
    assert_eq!(destroy.state.players[0].mana_pool.colorless, 7);
    semantic::complete(&mut destroy, 8, |_| None).require_exercised();
    assert_eq!(destroy.state.objects[&artifact].zone, Zone::Graveyard);
    assert_eq!(destroy.state.objects[&creature].zone, Zone::Battlefield);
    assert_eq!(destroy.state.objects[&creature].damage, 0);
    assert_eq!(destroy.state.objects[&source].zone, Zone::Graveyard);
}

#[test]
fn issue_359_explosive_derailment_both_modes_resolve_in_printed_order_and_charge_both_costs() {
    let mut engine = spree_engine(359_003);
    let creature = inject_creature_with_stats(&mut engine, 1, "grizzly_bears", 2, 5);
    let artifact = inject_permanent_on_battlefield(&mut engine, 1, "explosive_apparatus");
    let (slot, source) = prepare(&mut engine, "explosive_derailment");

    // Announce the artifact mode first, then the creature mode. Printed order must win: the
    // engine sorts selected modes by printed index before publishing the chosen-mode cohort.
    semantic::accepted(
        &mut engine,
        0,
        &cast_spree(
            slot,
            vec![(1, target_object(artifact)), (0, target_object(creature))],
            vec![1, 0],
        ),
    );
    let stack = engine.state.stack.last().expect("spell on the stack");
    assert_eq!(stack.chosen_modes.len(), 2);
    assert_eq!(stack.chosen_modes[0].mode_id.as_str(), "damage_creature");
    assert_eq!(stack.chosen_modes[1].mode_id.as_str(), "destroy_artifact");
    assert_eq!(stack.cast_cost_receipts.len(), 2);
    // Base {R} plus both additional {2} costs: red 9 -> 8, colorless 9 -> 5.
    assert_eq!(engine.state.players[0].mana_pool.red, 8);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 5);

    semantic::complete(&mut engine, 8, |_| None).require_exercised();
    assert_eq!(engine.state.objects[&creature].damage, 4);
    assert_eq!(engine.state.objects[&artifact].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
}

#[test]
fn issue_359_spree_rejects_no_mode_unfunded_and_mismatched_cost_atomically() {
    let card = "explosive_derailment";

    // Zero modes violates the printed "choose one or more".
    let mut none = spree_engine(359_010);
    let creature = inject_creature_with_stats(&mut none, 1, "grizzly_bears", 2, 5);
    let (slot, source) = prepare(&mut none, card);
    let before = none.state.command_index;
    none.apply_command(0, &cast_spree(slot, vec![], vec![]))
        .expect_err("spree needs at least one mode");
    assert_eq!(none.state.command_index, before);
    assert_eq!(none.state.objects[&source].zone, Zone::Hand);

    // An announced mode without its linked additional cost is illegal.
    let before = none.state.command_index;
    none.apply_command(
        0,
        &cast_spree(slot, vec![(0, target_object(creature))], vec![]),
    )
    .expect_err("a selected mode requires its linked cost");
    assert_eq!(none.state.command_index, before);

    // A selected mode paired with the other mode's cost is illegal.
    let before = none.state.command_index;
    none.apply_command(
        0,
        &cast_spree(slot, vec![(0, target_object(creature))], vec![1]),
    )
    .expect_err("selected modes and linked cast costs must match exactly");
    assert_eq!(none.state.command_index, before);
    assert_eq!(none.state.objects[&source].zone, Zone::Hand);
    assert!(none.state.stack.is_empty());

    // Fund only the printed base {R}; the announced additional {2} is unpayable.
    let mut poor = spree_engine(359_011);
    let creature = inject_creature_with_stats(&mut poor, 1, "grizzly_bears", 2, 5);
    let (slot, _) = prepare(&mut poor, card);
    {
        let pool = &mut poor.state.players[0].mana_pool;
        pool.white = 0;
        pool.blue = 0;
        pool.black = 0;
        pool.red = 1;
        pool.green = 0;
        pool.colorless = 1;
    }
    let before = poor.state.command_index;
    poor.apply_command(
        0,
        &cast_spree(slot, vec![(0, target_object(creature))], vec![0]),
    )
    .expect_err("the announced additional cost must be payable at cast time");
    assert_eq!(poor.state.command_index, before);
    assert_eq!(poor.state.players[0].mana_pool.red, 1);
    assert_eq!(poor.state.players[0].mana_pool.colorless, 1);
    assert_eq!(poor.state.objects[&creature].damage, 0);
    assert!(poor.state.stack.is_empty());

    // Positive control: the same cast is accepted once both costs are funded.
    grant_pool(&mut poor, 0);
    semantic::accepted(
        &mut poor,
        0,
        &cast_spree(slot, vec![(0, target_object(creature))], vec![0]),
    );
    semantic::complete(&mut poor, 8, |_| None).require_exercised();
    assert_eq!(poor.state.objects[&creature].damage, 4);
}

#[test]
fn issue_359_spree_mode_local_illegal_targets_are_rejected() {
    let mut derailment = spree_engine(359_020);
    let land = inject_permanent_on_battlefield(&mut derailment, 1, "forest");
    let creature = inject_creature_with_stats(&mut derailment, 1, "grizzly_bears", 2, 5);
    let (slot, source) = prepare(&mut derailment, "explosive_derailment");

    let before = derailment.state.command_index;
    derailment
        .apply_command(
            0,
            &cast_spree(slot, vec![(0, target_object(land))], vec![0]),
        )
        .expect_err("the damage mode cannot target a land");
    assert_eq!(derailment.state.command_index, before);

    let before = derailment.state.command_index;
    derailment
        .apply_command(
            0,
            &cast_spree(slot, vec![(1, target_object(creature))], vec![1]),
        )
        .expect_err("the destroy mode cannot target a creature");
    assert_eq!(derailment.state.command_index, before);
    assert_eq!(derailment.state.objects[&source].zone, Zone::Hand);
    assert!(derailment.state.stack.is_empty());

    let mut accident = spree_engine(359_021);
    let land = inject_permanent_on_battlefield(&mut accident, 1, "forest");
    let (slot, _) = prepare(&mut accident, "unfortunate_accident");
    let before = accident.state.command_index;
    accident
        .apply_command(
            0,
            &cast_spree(slot, vec![(0, target_object(land))], vec![0]),
        )
        .expect_err("the destroy mode cannot target a land");
    assert_eq!(accident.state.command_index, before);
    assert!(accident.state.stack.is_empty());
}

#[test]
fn issue_359_one_illegal_mode_target_still_resolves_the_other_mode() {
    // Both modes chosen; the damage target leaves before resolution. The artifact mode's target
    // remains legal, so the spell resolves and destroys the artifact (2024-04-12 ruling).
    let mut engine = spree_engine(359_030);
    let creature = inject_creature_with_stats(&mut engine, 1, "grizzly_bears", 2, 5);
    let artifact = inject_permanent_on_battlefield(&mut engine, 1, "explosive_apparatus");
    let (slot, source) = prepare(&mut engine, "explosive_derailment");
    semantic::accepted(
        &mut engine,
        0,
        &cast_spree(
            slot,
            vec![(0, target_object(creature)), (1, target_object(artifact))],
            vec![0, 1],
        ),
    );

    inject_card_into_hand(&mut engine, 0, "unsummon");
    let unsummon = hand_index_for_card(&engine, 0, "unsummon");
    semantic::accepted(
        &mut engine,
        0,
        &cast_spell(unsummon, target_object(creature)),
    );
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.objects[&creature].zone, Zone::Hand);
    assert_eq!(engine.state.objects[&artifact].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
}

#[test]
fn issue_359_complete_target_invalidation_fizzles_the_whole_spree_spell() {
    // Unfortunate Accident's token mode has no target, but the destroy mode's target does. When
    // the only chosen target is illegal on resolution, the entire spell fails to resolve, so no
    // Mercenary is created (2024-04-12 ruling; CR 608.2b).
    let mut engine = spree_engine(359_040);
    let creature = inject_creature_with_stats(&mut engine, 1, "grizzly_bears", 2, 5);
    let (slot, source) = prepare(&mut engine, "unfortunate_accident");
    semantic::accepted(
        &mut engine,
        0,
        &cast_spree(
            slot,
            vec![(0, target_object(creature)), (1, vec![])],
            vec![0, 1],
        ),
    );
    assert!(battlefield_token_oids(&engine, 0, "mercenary_r_1_1").is_empty());

    inject_card_into_hand(&mut engine, 0, "unsummon");
    let unsummon = hand_index_for_card(&engine, 0, "unsummon");
    semantic::accepted(
        &mut engine,
        0,
        &cast_spell(unsummon, target_object(creature)),
    );
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.objects[&creature].zone, Zone::Hand);
    assert!(
        battlefield_token_oids(&engine, 0, "mercenary_r_1_1").is_empty(),
        "a fully invalidated spree spell creates no token"
    );
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
    // The selected additional costs were still paid at cast time (CR 601.2h).
    assert_eq!(engine.state.players[0].mana_pool.black, 7);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 6);
}

#[test]
fn issue_359_unfortunate_accident_modes_destroy_and_create_the_registered_mercenary() {
    // Mode 0 alone: destroy the creature; no token; total {2}{B}{B}.
    let mut destroy = spree_engine(359_050);
    let creature = inject_creature_with_stats(&mut destroy, 1, "grizzly_bears", 2, 5);
    let (slot, source) = prepare(&mut destroy, "unfortunate_accident");
    semantic::accepted(
        &mut destroy,
        0,
        &cast_spree(slot, vec![(0, target_object(creature))], vec![0]),
    );
    assert_eq!(destroy.state.players[0].mana_pool.black, 7);
    assert_eq!(destroy.state.players[0].mana_pool.colorless, 7);
    semantic::complete(&mut destroy, 8, |_| None).require_exercised();
    assert_eq!(destroy.state.objects[&creature].zone, Zone::Graveyard);
    assert!(battlefield_token_oids(&destroy, 0, "mercenary_r_1_1").is_empty());
    assert_eq!(destroy.state.objects[&source].zone, Zone::Graveyard);

    // Mode 1 alone: create exactly one registered Mercenary for its controller; total {1}{B}.
    let mut token = spree_engine(359_051);
    let creature = inject_creature_with_stats(&mut token, 1, "grizzly_bears", 2, 5);
    let (slot, source) = prepare(&mut token, "unfortunate_accident");
    semantic::accepted(&mut token, 0, &cast_spree(slot, vec![(1, vec![])], vec![1]));
    assert_eq!(token.state.players[0].mana_pool.black, 8);
    assert_eq!(token.state.players[0].mana_pool.colorless, 8);
    semantic::complete(&mut token, 8, |_| None).require_exercised();
    let mercenaries = battlefield_token_oids(&token, 0, "mercenary_r_1_1");
    let [mercenary] = mercenaries.as_slice() else {
        panic!("exactly one Mercenary token");
    };
    assert_eq!(token.state.objects[mercenary].owner, 0);
    assert_eq!(token.state.objects[mercenary].controller, 0);
    assert_eq!(token.effective_power(*mercenary), Some(1));
    assert_eq!(token.effective_toughness(*mercenary), Some(1));
    assert!(battlefield_token_oids(&token, 1, "mercenary_r_1_1").is_empty());
    assert_eq!(token.state.objects[&creature].zone, Zone::Battlefield);
    assert_eq!(token.state.objects[&creature].damage, 0);
    assert_eq!(token.state.objects[&source].zone, Zone::Graveyard);

    // Both modes together: destroy and create; total {3}{B}{B}.
    let mut both = spree_engine(359_052);
    let creature = inject_creature_with_stats(&mut both, 1, "grizzly_bears", 2, 5);
    let (slot, _) = prepare(&mut both, "unfortunate_accident");
    semantic::accepted(
        &mut both,
        0,
        &cast_spree(
            slot,
            vec![(0, target_object(creature)), (1, vec![])],
            vec![0, 1],
        ),
    );
    assert_eq!(both.state.players[0].mana_pool.black, 7);
    assert_eq!(both.state.players[0].mana_pool.colorless, 6);
    semantic::complete(&mut both, 8, |_| None).require_exercised();
    assert_eq!(both.state.objects[&creature].zone, Zone::Graveyard);
    assert_eq!(battlefield_token_oids(&both, 0, "mercenary_r_1_1").len(), 1);
}
