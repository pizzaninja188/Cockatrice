//! Scenario tests for CR 701.19 Regenerate mechanic.
//!
//! Regenerate creates a replacement effect: the next time the creature would be destroyed
//! this turn, instead tap it, remove it from combat, and clear all damage from it.
//! Regeneration does NOT save a creature from toughness-0 (CR 704.5f — not a "destroy").
//! Wrath of God's "can't be regenerated" bypasses shields (CR 701.19c).

use crate::helpers::*;
use tricerules_cards::primitives::{ContinuousEffectKind, EffectDuration};
use tricerules_core::state::{AffectedScope, ContinuousEffect};

fn regen_engine() -> GameEngine {
    let decks = Some(vec![
        deck_with("forest", &["cudgel_troll", "forest"]),
        deck_with("swamp", &["drudge_skeletons"]),
    ]);
    let mut e = GameEngine::new(42, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    e
}

/// Activate `{G}: Regenerate` on Cudgel Troll and verify the shield is placed.
#[test]
fn regen_shield_placed_by_ability() {
    let mut e = regen_engine();
    let troll = relocate_to_battlefield(&mut e, 0, "cudgel_troll", false);
    give_mana(
        &mut e,
        0,
        ManaGift {
            g: 1,
            ..Default::default()
        },
    );
    e.apply_command(0, &activate_ability(troll, 0, vec![]))
        .expect("regenerate ability activates");
    // Resolve: both players pass.
    e.apply_command(0, &pass()).expect("caster pass");
    e.apply_command(1, &pass()).expect("other pass");
    assert_eq!(
        e.state.objects.get(&troll).map(|o| o.regeneration_shields),
        Some(1),
        "troll must have 1 regeneration shield after ability resolves"
    );
    assert_eq!(
        e.state.objects.get(&troll).map(|o| o.zone),
        Some(tricerules_core::Zone::Battlefield),
    );
    assert_eq!(
        e.state.objects.get(&troll).map(|o| o.tapped),
        Some(false),
        "placing a shield must not tap the creature"
    );
    assert_eq!(
        zone_view_rules_annotation_labels(&mut e, 0, troll),
        vec!["Regeneration shield"],
        "the active regeneration shield is visible on the permanent"
    );
}

/// Shield consumed on lethal damage: creature survives tapped, damage cleared.
#[test]
fn regen_shield_consumed_on_lethal_damage_sba() {
    let mut e = regen_engine();
    let troll = relocate_to_battlefield(&mut e, 0, "cudgel_troll", false);
    // Place shield and mark lethal damage manually; a pass triggers SBAs.
    if let Some(o) = e.state.objects.get_mut(&troll) {
        o.regeneration_shields = 1;
        o.damage = 3; // toughness = 3 → lethal
    }
    assert_eq!(
        zone_view_rules_annotation_labels(&mut e, 0, troll),
        vec!["Regeneration shield"]
    );
    e.apply_command(0, &pass()).expect("pass to trigger sba");
    e.apply_command(1, &pass()).expect("second pass");
    assert_eq!(
        e.state.objects.get(&troll).map(|o| o.zone),
        Some(tricerules_core::Zone::Battlefield),
        "troll with regen shield must survive lethal damage"
    );
    assert_eq!(
        e.state.objects.get(&troll).map(|o| o.regeneration_shields),
        Some(0),
        "shield must be consumed"
    );
    assert_eq!(
        e.state.objects.get(&troll).map(|o| o.tapped),
        Some(true),
        "regenerated creature must be tapped"
    );
    assert_eq!(
        e.state.objects.get(&troll).map(|o| o.damage),
        Some(0),
        "regenerated creature's damage must be cleared"
    );
    assert!(zone_view_rules_annotation_labels(&mut e, 0, troll).is_empty());
}

/// Without a shield, the creature dies to lethal damage normally.
#[test]
fn no_shield_creature_dies_to_lethal_damage() {
    let mut e = regen_engine();
    let skeletons = relocate_to_battlefield(&mut e, 1, "drudge_skeletons", false);
    if let Some(o) = e.state.objects.get_mut(&skeletons) {
        o.damage = 1; // toughness = 1 → lethal
    }
    e.apply_command(0, &pass()).expect("pass to trigger sba");
    e.apply_command(1, &pass()).expect("second pass");
    assert_eq!(
        e.state.objects.get(&skeletons).map(|o| o.zone),
        Some(tricerules_core::Zone::Graveyard),
        "creature without shield must die to lethal damage"
    );
}

/// Second lethal hit in the same turn — shield consumed on first, creature dies on second.
#[test]
fn shield_not_present_on_second_lethal() {
    let mut e = regen_engine();
    let troll = relocate_to_battlefield(&mut e, 0, "cudgel_troll", false);
    if let Some(o) = e.state.objects.get_mut(&troll) {
        o.regeneration_shields = 1;
        o.damage = 3;
    }
    // First SBA sweep: shield consumed.
    e.apply_command(0, &pass()).expect("pass");
    e.apply_command(1, &pass()).expect("second pass");
    assert_eq!(
        e.state.objects.get(&troll).map(|o| o.zone),
        Some(tricerules_core::Zone::Battlefield),
        "troll must survive first lethal hit"
    );
    // Second lethal hit same turn (no shield left).
    if let Some(o) = e.state.objects.get_mut(&troll) {
        o.damage = 3;
    }
    e.apply_command(0, &pass()).expect("pass");
    e.apply_command(1, &pass()).expect("second pass");
    assert_eq!(
        e.state.objects.get(&troll).map(|o| o.zone),
        Some(tricerules_core::Zone::Graveyard),
        "troll must die on second lethal hit — shield is gone"
    );
}

/// Wrath of God's `prevent_regeneration` bypasses shields (CR 701.19c).
#[test]
fn wrath_bypasses_regen_shields() {
    let decks = Some(vec![
        deck_with("plains", &["wrath_of_god"]),
        deck_with("swamp", &["drudge_skeletons"]),
    ]);
    let mut e = GameEngine::new(43, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let skeletons = relocate_to_battlefield(&mut e, 1, "drudge_skeletons", false);
    if let Some(o) = e.state.objects.get_mut(&skeletons) {
        o.regeneration_shields = 1;
    }
    ensure_in_hand(&mut e, 0, "wrath_of_god");
    give_mana(
        &mut e,
        0,
        ManaGift {
            w: 4,
            ..Default::default()
        },
    );
    let wrath_idx = hand_index_for_card(&e, 0, "wrath_of_god");
    e.apply_command(0, &cast_spell(wrath_idx, vec![]))
        .expect("cast wrath");
    e.apply_command(0, &pass()).expect("caster pass");
    e.apply_command(1, &pass()).expect("other pass");
    assert_eq!(
        e.state.objects.get(&skeletons).map(|o| o.zone),
        Some(tricerules_core::Zone::Graveyard),
        "Wrath of God must destroy despite regen shield"
    );
}

/// Toughness-0 SBA (CR 704.5f) is NOT a destroy and must not check regeneration shields.
#[test]
fn toughness_zero_bypasses_regen_shield() {
    let mut e = regen_engine();
    let troll = relocate_to_battlefield(&mut e, 0, "cudgel_troll", false);
    if let Some(o) = e.state.objects.get_mut(&troll) {
        o.regeneration_shields = 1;
    }
    // Apply a -4 toughness continuous effect (base 3 → effective 0).
    e.state.continuous_effects.push(ContinuousEffect {
        source_id: None,
        affected: AffectedScope::Single(troll),
        kind: ContinuousEffectKind::PtModify {
            delta_power: 0,
            delta_toughness: -4,
        },
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: 0,
    });
    e.apply_command(0, &pass()).expect("pass to trigger sba");
    e.apply_command(1, &pass()).expect("second pass");
    assert_eq!(
        e.state.objects.get(&troll).map(|o| o.zone),
        Some(tricerules_core::Zone::Graveyard),
        "toughness-0 must put creature in graveyard even with a regen shield"
    );
}

/// Regeneration shields expire at cleanup (CR 701.19a "this turn").
#[test]
fn regen_shields_expire_at_cleanup() {
    let mut e = regen_engine();
    let troll = relocate_to_battlefield(&mut e, 0, "cudgel_troll", false);
    if let Some(o) = e.state.objects.get_mut(&troll) {
        o.regeneration_shields = 2;
    }
    assert_eq!(
        zone_view_rules_annotation_labels(&mut e, 0, troll),
        vec!["2 regeneration shields"]
    );
    end_active_turn(&mut e, 0);
    assert_eq!(
        e.state.objects.get(&troll).map(|o| o.regeneration_shields),
        Some(0),
        "regeneration shields must expire at cleanup"
    );
    assert!(zone_view_rules_annotation_labels(&mut e, 0, troll).is_empty());
}
