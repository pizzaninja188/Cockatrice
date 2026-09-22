//! Four pinned Standard spells reuse established Spree and player-target effects.
//! CR 702.172, 601.2f-h, 115, 608.2b, 613.4, 701.21, and 701.25 apply.
use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, CastCostGroupSelection, CastMethod, CastSpell, SelectedSpellMode,
};

fn engine(seed: u64) -> GameEngine {
    let mut e = GameEngine::new(seed, &[0, 1], 20, None, true).expect("engine");
    advance_to_main1_from_game_start(&mut e);
    grant_pool(&mut e, 0);
    e
}

fn cast_spree(e: &mut GameEngine, card: &str, modes: Vec<(u32, Vec<TargetRef>)>, costs: Vec<u32>) {
    inject_card_into_hand(e, 0, card);
    let slot = hand_index_for_card(e, 0, card);
    let command = RuledCommand {
        cmd: Some(Cmd::CastSpell(CastSpell {
            cast_method: CastMethod::Normal as i32,
            source: Some(hand_cast_source(slot)),
            selected_modes: modes
                .into_iter()
                .map(|(mode_index, targets)| SelectedSpellMode {
                    mode_index,
                    targets,
                })
                .collect(),
            cast_cost_group_selections: costs
                .into_iter()
                .map(|option_index| CastCostGroupSelection {
                    group_index: 0,
                    option_index,
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        })),
    };
    semantic::accepted(e, 0, &command);
}

fn finish(e: &mut GameEngine) {
    semantic::complete(e, 12, |_| None).require_exercised();
}

#[test]
fn issue_misc46_requisition_raid_modes_and_player_scope() {
    let mut e = engine(846_001);
    let counter_target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let own = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let artifact = inject_permanent_on_battlefield(&mut e, 1, "explosive_apparatus");
    let enchantment = inject_permanent_on_battlefield(&mut e, 1, "phyrexian_arena");
    cast_spree(
        &mut e,
        "requisition_raid",
        vec![
            (0, target_object(artifact)),
            (1, target_object(enchantment)),
            (2, target_player(1)),
        ],
        vec![0, 1, 2],
    );
    finish(&mut e);
    assert_eq!(e.state.objects[&artifact].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&enchantment].zone, Zone::Graveyard);
    assert_eq!(
        e.state.objects[&counter_target]
            .counter_count(tricerules_cards::CounterKind::PlusOnePlusOne),
        1
    );
    assert_eq!(
        e.state.objects[&own].counter_count(tricerules_cards::CounterKind::PlusOnePlusOne),
        0
    );
}

#[test]
fn issue_misc46_rustler_rampage_modes_and_player_scope() {
    let mut e = engine(846_002);
    let opposing = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let chosen = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    e.state.objects.get_mut(&opposing).unwrap().tapped = true;
    e.state.objects.get_mut(&chosen).unwrap().tapped = true;
    cast_spree(
        &mut e,
        "rustler_rampage",
        vec![(0, target_player(1)), (1, target_object(chosen))],
        vec![0, 1],
    );
    finish(&mut e);
    assert!(!e.state.objects[&opposing].tapped);
    assert!(e.state.objects[&chosen].tapped);
    assert!(e.effective_has_keyword(chosen, Keyword::DoubleStrike));
}

#[test]
fn issue_misc46_how_to_start_a_riot_separates_targets() {
    let mut e = engine(846_003);
    let keyworded = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let mass = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let untouched = inject_creature_on_battlefield(&mut e, 0, "giant_spider");
    inject_card_into_hand(&mut e, 0, "how_to_start_a_riot");
    let slot = hand_index_for_card(&e, 0, "how_to_start_a_riot");
    let mut targets: Vec<_> = target_object(keyworded)
        .into_iter()
        .chain(target_player(1))
        .collect();
    targets[1].group_index = 1;
    semantic::accepted(&mut e, 0, &cast_spell(slot, targets));
    finish(&mut e);
    assert!(e.effective_has_keyword(keyworded, Keyword::Menace));
    assert_eq!(e.effective_power(mass), Some(4));
    assert_eq!(e.effective_power(untouched), Some(2));
}

#[test]
fn issue_misc46_neutralize_the_guards_scopes_debuff() {
    let mut e = engine(846_004);
    let opposing = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let own = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    inject_library_card(&mut e, 0, "forest");
    inject_library_card(&mut e, 0, "island");
    inject_card_into_hand(&mut e, 0, "neutralize_the_guards");
    let slot = hand_index_for_card(&e, 0, "neutralize_the_guards");
    semantic::accepted(&mut e, 0, &cast_spell(slot, target_player(1)));
    e.apply_command(e.state.priority_player_id(), &pass())
        .expect("first pass");
    let parked = e
        .apply_command(e.state.priority_player_id(), &pass())
        .expect("resolve into Surveil");
    let choice = find_resolution_choice(&parked).expect("Surveil choice");
    e.apply_command(
        0,
        &submit_resolution_choice(choice.candidate_object_ids.clone()),
    )
    .expect("complete Surveil");
    assert_eq!(choice.candidate_object_ids.len(), 2);
    assert!(choice
        .candidate_object_ids
        .iter()
        .all(|oid| e.state.objects[oid].zone == Zone::Graveyard));
    assert_eq!(e.effective_power(opposing), Some(1));
    assert_eq!(e.effective_toughness(opposing), Some(1));
    assert_eq!(e.effective_power(own), Some(2));
}
