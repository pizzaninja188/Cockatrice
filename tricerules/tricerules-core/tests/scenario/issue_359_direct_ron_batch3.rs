//! Batch 48 of the issue #359 Spree cohort: Caught in the Crossfire.
//!
//! Exact pinned Scryfall card data and all 15 rulings were checked on 2026-09-22 for Oracle ID
//! `d262f3e1-b5bc-4dab-86c2-cd89c2419e54`. The Outlaw ruling names Assassin, Mercenary, Pirate,
//! Rogue, and Warlock; Changeling is every creature type and must match that union once.
//! Current CR 702.172a defines Spree; CR 601.2b/f-h governs its additional costs; CR 608.2c
//! preserves instruction order; CR 120.2b and 120.3e govern spell damage to creatures.
//!
//! The engine owns one untargeted `DamageAll` batch per selected mode. Outlaw and non-outlaw are
//! disjoint filters, and both modes together still deal only two damage to each creature.

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

fn spree_engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("mountain", &[]), deck_with("forest", &[])]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    grant_pool(&mut engine, 0);
    grant_pool(&mut engine, 1);
    engine
}

fn prepare(engine: &mut GameEngine) -> (usize, u32) {
    let source = inject_card_into_hand(engine, 0, "caught_in_the_crossfire");
    (
        hand_index_for_card(engine, 0, "caught_in_the_crossfire"),
        source,
    )
}

fn outlaw_cohort(engine: &mut GameEngine) -> Vec<u32> {
    [
        (0, "cackling_slasher"),
        (1, "rough_rhino_cavalry"),
        (0, "gorehorn_raider"),
        (1, "marauding_sphinx"),
        (0, "chaos_spewer"),
        // CR 702.73a: the Changeling matches all five subtype alternatives, but only once.
        (1, "barkform_harvester"),
    ]
    .into_iter()
    .map(|(player, card)| inject_creature_with_stats(engine, player, card, 3, 8))
    .collect()
}

#[test]
fn issue_359_caught_in_the_crossfire_modes_partition_all_creatures_and_charge_selected_costs() {
    // Outlaw mode: all five subtypes match, including Changeling; ordinary creatures and
    // noncreature permanents are untouched. The list spans both players.
    let mut outlaws = spree_engine(359_060);
    let outlaw_objects = outlaw_cohort(&mut outlaws);
    let ordinary = inject_creature_with_stats(&mut outlaws, 0, "grizzly_bears", 2, 8);
    let artifact = inject_permanent_on_battlefield(&mut outlaws, 1, "explosive_apparatus");
    let (slot, source) = prepare(&mut outlaws);
    semantic::accepted(
        &mut outlaws,
        0,
        &cast_spree(slot, vec![(0, vec![])], vec![0]),
    );
    let stack = outlaws.state.stack.last().expect("Spree spell");
    assert_eq!(stack.chosen_modes.len(), 1);
    assert_eq!(stack.chosen_modes[0].mode_id.as_str(), "outlaws");
    assert_eq!(stack.cast_cost_receipts.len(), 1);
    assert_eq!(
        outlaws.state.players[0].mana_pool.red, 7,
        "base double-red cost"
    );
    assert_eq!(
        outlaws.state.players[0].mana_pool.colorless, 8,
        "one generic mode cost"
    );
    semantic::complete(&mut outlaws, 8, |_| None).require_exercised();
    for object in outlaw_objects {
        assert_eq!(outlaws.state.objects[&object].damage, 2);
        assert_eq!(outlaws.state.objects[&object].zone, Zone::Battlefield);
    }
    assert_eq!(outlaws.state.objects[&ordinary].damage, 0);
    assert_eq!(outlaws.state.objects[&artifact].damage, 0);
    assert_eq!(outlaws.state.objects[&source].zone, Zone::Graveyard);

    // Non-outlaw mode: the Changeling and all five single-subtype outlaws are excluded.
    let mut non_outlaws = spree_engine(359_061);
    let outlaw_objects = outlaw_cohort(&mut non_outlaws);
    let ordinary = inject_creature_with_stats(&mut non_outlaws, 1, "grizzly_bears", 2, 8);
    let (slot, source) = prepare(&mut non_outlaws);
    semantic::accepted(
        &mut non_outlaws,
        0,
        &cast_spree(slot, vec![(1, vec![])], vec![1]),
    );
    let stack = non_outlaws.state.stack.last().expect("Spree spell");
    assert_eq!(stack.chosen_modes[0].mode_id.as_str(), "non_outlaws");
    assert_eq!(stack.cast_cost_receipts.len(), 1);
    assert_eq!(non_outlaws.state.players[0].mana_pool.red, 7);
    assert_eq!(non_outlaws.state.players[0].mana_pool.colorless, 8);
    semantic::complete(&mut non_outlaws, 8, |_| None).require_exercised();
    for object in outlaw_objects {
        assert_eq!(non_outlaws.state.objects[&object].damage, 0);
    }
    assert_eq!(non_outlaws.state.objects[&ordinary].damage, 2);
    assert_eq!(non_outlaws.state.objects[&source].zone, Zone::Graveyard);

    // Both modes: reverse announcement order is normalized to printed order and their two
    // independent additional costs are both paid. Each creature still receives two damage.
    let mut both = spree_engine(359_062);
    let changeling = inject_creature_with_stats(&mut both, 1, "barkform_harvester", 3, 8);
    let ordinary = inject_creature_with_stats(&mut both, 0, "grizzly_bears", 2, 8);
    let (slot, source) = prepare(&mut both);
    semantic::accepted(
        &mut both,
        0,
        &cast_spree(slot, vec![(1, vec![]), (0, vec![])], vec![1, 0]),
    );
    let stack = both.state.stack.last().expect("Spree spell");
    assert_eq!(
        stack
            .chosen_modes
            .iter()
            .map(|mode| mode.mode_id.as_str())
            .collect::<Vec<_>>(),
        ["outlaws", "non_outlaws"]
    );
    assert_eq!(stack.cast_cost_receipts.len(), 2);
    assert_eq!(both.state.players[0].mana_pool.red, 7);
    assert_eq!(both.state.players[0].mana_pool.colorless, 7);
    semantic::complete(&mut both, 8, |_| None).require_exercised();
    assert_eq!(both.state.objects[&changeling].damage, 2);
    assert_eq!(both.state.objects[&ordinary].damage, 2);
    assert_eq!(both.state.objects[&source].zone, Zone::Graveyard);
}
