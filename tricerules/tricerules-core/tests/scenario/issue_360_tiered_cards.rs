//! Issue #360 — registered Tiered card behavior through authoritative commands.
//!
//! The selected FF cards exercise CR 702.183a (one linked mode/additional cost), 601.2f-h
//! (additional costs), 700.2 (mode announcement), 608.2b (target revalidation), 400.3
//! (owner/library movement), 613.4 (base P/T), 613.4c (power/toughness modifiers), and
//! 603.2/611.2c (granted death trigger).

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, CastCostGroupSelection, CastMethod, CastSpell, ResolutionChoiceDecision,
    SelectedSpellMode, SubmitResolutionChoice,
};

fn engine(seed: u64, p0: &[&str], p1: &[&str]) -> GameEngine {
    let decks = Some(vec![deck_with("forest", p0), deck_with("forest", p1)]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    grant_pool(&mut engine, 0);
    grant_pool(&mut engine, 1);
    engine
}

fn cast_tier(
    card_slot: usize,
    mode_index: u32,
    option_index: u32,
    targets: Vec<TargetRef>,
) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::CastSpell(CastSpell {
            cast_method: CastMethod::Normal as i32,
            source: Some(hand_cast_source(card_slot)),
            selected_modes: vec![SelectedSpellMode {
                mode_index,
                targets,
            }],
            cast_cost_group_selections: vec![CastCostGroupSelection {
                group_index: 0,
                option_index,
                ..Default::default()
            }],
            ..Default::default()
        })),
    }
}

fn select_branch(index: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            decision: ResolutionChoiceDecision::SelectBranch as i32,
            selected_branch_index: index,
            ..Default::default()
        })),
    }
}

#[test]
fn fire_magic_binds_firaga_cost_and_damage() {
    let mut e = engine(360_001, &["fire_magic"], &[]);
    let creature = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 3, 3);
    ensure_card_in_hand(&mut e, 0, "fire_magic");
    let slot = hand_index_for_card(&e, 0, "fire_magic");

    semantic::accepted(&mut e, 0, &cast_tier(slot, 2, 2, vec![]));
    semantic::complete(&mut e, 8, |_| None).require_exercised();
    assert_eq!(e.state.objects[&creature].zone, Zone::Graveyard);
}

#[test]
fn ice_magic_blizzara_requires_the_creature_owner_to_choose_placement() {
    let mut e = engine(360_002, &["ice_magic"], &["grizzly_bears"]);
    let bear = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    ensure_card_in_hand(&mut e, 0, "ice_magic");
    let slot = hand_index_for_card(&e, 0, "ice_magic");

    semantic::accepted(&mut e, 0, &cast_tier(slot, 1, 1, target_object(bear)));
    e.apply_command(0, &pass()).expect("caster passes priority");
    let parked = e
        .apply_command(1, &pass())
        .expect("spell resolves to owner choice");
    let choice = find_resolution_choice(&parked).expect("top or bottom choice");
    assert_eq!(choice.deciding_player_id, 1);
    assert_eq!(choice.resolution_branches[0].label, "Top");
    assert_eq!(choice.resolution_branches[1].label, "Bottom");
    assert!(e.apply_command(0, &select_branch(1)).is_err());
    e.apply_command(1, &select_branch(1))
        .expect("owner chooses bottom");
    assert_eq!(e.state.objects[&bear].zone, Zone::Library);
    assert_eq!(e.state.players[1].library.back(), Some(&bear));
}

#[test]
fn restoration_magic_cura_gains_life_and_curaga_protects_your_creatures() {
    let mut cura = engine(360_003, &["restoration_magic"], &[]);
    let creature = inject_creature_on_battlefield(&mut cura, 0, "grizzly_bears");
    ensure_card_in_hand(&mut cura, 0, "restoration_magic");
    let slot = hand_index_for_card(&cura, 0, "restoration_magic");
    semantic::accepted(
        &mut cura,
        0,
        &cast_tier(slot, 1, 1, target_object(creature)),
    );
    semantic::complete(&mut cura, 8, |_| None).require_exercised();
    assert_eq!(cura.state.players[0].life, 23);
    assert!(cura.effective_has_keyword(creature, tricerules_cards::Keyword::Hexproof));
    assert!(cura.effective_has_keyword(creature, tricerules_cards::Keyword::Indestructible));

    let mut curaga = engine(360_004, &["restoration_magic"], &[]);
    let creature = inject_creature_on_battlefield(&mut curaga, 0, "grizzly_bears");
    ensure_card_in_hand(&mut curaga, 0, "restoration_magic");
    let slot = hand_index_for_card(&curaga, 0, "restoration_magic");
    semantic::accepted(&mut curaga, 0, &cast_tier(slot, 2, 2, vec![]));
    semantic::complete(&mut curaga, 8, |_| None).require_exercised();
    assert_eq!(curaga.state.players[0].life, 26);
    assert!(curaga.effective_has_keyword(creature, tricerules_cards::Keyword::Hexproof));
    assert!(curaga.effective_has_keyword(creature, tricerules_cards::Keyword::Indestructible));
}

#[test]
fn restoration_magic_cura_does_not_gain_life_when_its_target_becomes_illegal() {
    let mut e = engine(360_008, &["restoration_magic"], &["murder"]);
    let creature = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    ensure_card_in_hand(&mut e, 0, "restoration_magic");
    ensure_card_in_hand(&mut e, 1, "murder");
    let cura_slot = hand_index_for_card(&e, 0, "restoration_magic");
    semantic::accepted(
        &mut e,
        0,
        &cast_tier(cura_slot, 1, 1, target_object(creature)),
    );
    e.apply_command(0, &pass()).expect("Cura caster passes");

    let murder_slot = hand_index_for_card(&e, 1, "murder");
    semantic::accepted(&mut e, 1, &cast_spell(murder_slot, target_object(creature)));
    e.apply_command(1, &pass()).expect("Murder caster passes");
    e.apply_command(0, &pass()).expect("Murder resolves");
    assert_eq!(e.state.objects[&creature].zone, Zone::Graveyard);

    e.apply_command(0, &pass())
        .expect("Cura caster passes again");
    semantic::accepted(&mut e, 1, &pass());
    assert_eq!(e.state.players[0].life, 20);
}

#[test]
fn tifas_limit_break_scales_asymmetric_current_power_and_toughness() {
    for (seed, option, expected_power, expected_toughness) in
        [(360_005, 1, 4, 10), (360_006, 2, 6, 15)]
    {
        let mut e = engine(seed, &["tifas_limit_break"], &[]);
        let creature = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 5);
        ensure_card_in_hand(&mut e, 0, "tifas_limit_break");
        let slot = hand_index_for_card(&e, 0, "tifas_limit_break");
        semantic::accepted(
            &mut e,
            0,
            &cast_tier(slot, option, option, target_object(creature)),
        );
        semantic::complete(&mut e, 8, |_| None).require_exercised();
        assert_eq!(e.effective_power(creature), Some(expected_power));
        assert_eq!(e.effective_toughness(creature), Some(expected_toughness));
    }
}

#[test]
fn vincents_limit_break_sets_base_stats_and_returns_the_creature_tapped() {
    let mut e = engine(
        360_007,
        &["vincents_limit_break", "thunder_magic", "thunder_magic"],
        &[],
    );
    let creature = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 5);
    ensure_card_in_hand(&mut e, 0, "vincents_limit_break");
    let slot = hand_index_for_card(&e, 0, "vincents_limit_break");
    semantic::accepted(&mut e, 0, &cast_tier(slot, 1, 1, target_object(creature)));
    semantic::complete(&mut e, 8, |_| None).require_exercised();
    assert_eq!(e.effective_power(creature), Some(5));
    assert_eq!(e.effective_toughness(creature), Some(2));

    ensure_card_in_hand(&mut e, 0, "thunder_magic");
    let slot = hand_index_for_card(&e, 0, "thunder_magic");
    semantic::accepted(&mut e, 0, &cast_tier(slot, 2, 2, target_object(creature)));
    semantic::complete(&mut e, 12, |_| None).require_exercised();
    assert_eq!(e.state.objects[&creature].zone, Zone::Battlefield);
    assert!(e.state.objects[&creature].tapped);
    assert_eq!(e.effective_power(creature), Some(2));
    assert_eq!(e.effective_toughness(creature), Some(2));

    let large_creature = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 5, 5);
    ensure_card_in_hand(&mut e, 0, "thunder_magic");
    let slot = hand_index_for_card(&e, 0, "thunder_magic");
    semantic::accepted(
        &mut e,
        0,
        &cast_tier(slot, 2, 2, target_object(large_creature)),
    );
    semantic::complete(&mut e, 8, |_| None).require_exercised();
    assert_eq!(e.state.objects[&large_creature].zone, Zone::Graveyard);
}
