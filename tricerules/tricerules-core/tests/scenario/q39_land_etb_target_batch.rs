//! Actual-card scenarios for the queue-v39 land entry-target cohort.
//!
//! Expectations are printed Oracle values. CR 603.6a puts the entry trigger on the stack; CR 611.2a/
//! 514.2 ends the targeted pump or grant at cleanup; CR 613.4c applies P/T; CR 613.1f grants a
//! keyword; CR 701.26 pays the tap cost; and CR 106.1 makes the mana ability immediate.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{ChooseTriggerTarget, RuledCommand, TargetRef, TargetRefKind};

struct Case {
    id: &'static str,
    power: i32,
    toughness: i32,
    keywords: &'static [Keyword],
    color: (u32, u32, u32, u32, u32, u32),
}

fn cases() -> [Case; 5] {
    [
        Case {
            id: "sandstone_bridge",
            power: 1,
            toughness: 1,
            keywords: &[Keyword::Vigilance],
            color: (1, 0, 0, 0, 0, 0),
        },
        Case {
            id: "teetering_peaks",
            power: 2,
            toughness: 0,
            keywords: &[],
            color: (0, 0, 0, 1, 0, 0),
        },
        Case {
            id: "turntimber_grove",
            power: 1,
            toughness: 1,
            keywords: &[],
            color: (0, 0, 0, 0, 1, 0),
        },
        Case {
            id: "looming_spires",
            power: 1,
            toughness: 1,
            keywords: &[Keyword::FirstStrike],
            color: (0, 0, 0, 1, 0, 0),
        },
        Case {
            id: "soaring_seacliff",
            power: 0,
            toughness: 0,
            keywords: &[Keyword::Flying],
            color: (0, 1, 0, 0, 0, 0),
        },
    ]
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

fn pool(e: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[0].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[]), deck_with("island", &[])]);
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

fn play_targeted_land(e: &mut GameEngine, case: &Case, target: u32) -> u32 {
    inject_card_into_hand(e, 0, case.id);
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &play_land(slot))
        .unwrap_or_else(|error| panic!("{}: play land: {error:?}", case.id));
    let land = battlefield_object_for_card(e, 0, case.id);
    assert!(e.state.objects[&land].tapped, "{}: enters tapped", case.id);
    assert_eq!(
        e.state.pending_triggers.len(),
        1,
        "{}: entry trigger waits",
        case.id
    );
    semantic::accepted(e, 0, &choose_trigger_target(target));
    resolve_entire_stack_two_player(e);
    land
}

fn activate_mana(e: &mut GameEngine, land: u32, case: &Case) {
    e.state.objects.get_mut(&land).expect("land").tapped = false;
    e.apply_scenario_command(0, &activate_ability_for(e, land, 0, vec![]))
        .unwrap_or_else(|error| panic!("{}: mana ability: {error:?}", case.id));
    assert_eq!(pool(e), case.color, "{}: exact printed mana", case.id);
    assert!(e.state.objects[&land].tapped, "{}: tap cost paid", case.id);
    assert!(
        e.state.stack.is_empty(),
        "{}: mana ability resolves immediately",
        case.id
    );
}

#[test]
fn q39_land_etb_target_canary_simplest() {
    let case = &cases()[2];
    let mut e = engine(39_030_001);
    let target = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
    let land = play_targeted_land(&mut e, case, target);
    assert_eq!(
        (e.effective_power(target), e.effective_toughness(target)),
        (Some(3), Some(3))
    );
    activate_mana(&mut e, land, case);
}

#[test]
fn q39_land_etb_target_canary_hardest() {
    let case = &cases()[0];
    let mut e = engine(39_030_002);
    let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
    let land = play_targeted_land(&mut e, case, target);
    assert_eq!(
        (e.effective_power(target), e.effective_toughness(target)),
        (Some(3), Some(3))
    );
    assert!(e.effective_has_keyword(target, Keyword::Vigilance));
    activate_mana(&mut e, land, case);
}

#[test]
fn q39_land_etb_target_cards_apply_their_printed_pump_and_keyword() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(39_031_000 + index as u64);
        let target = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
        let bystander = inject_creature_with_stats(&mut e, 0, "savannah_lions", 2, 2);
        let land = play_targeted_land(&mut e, case, target);
        assert_eq!(
            (e.effective_power(target), e.effective_toughness(target)),
            (
                Some((2 + case.power) as u32),
                Some((2 + case.toughness) as u32)
            ),
            "{}: exact printed pump",
            case.id
        );
        for keyword in case.keywords {
            assert!(
                e.effective_has_keyword(target, *keyword),
                "{}: {:?}",
                case.id,
                keyword
            );
            assert!(
                !e.effective_has_keyword(bystander, *keyword),
                "{}: bystander {:?}",
                case.id,
                keyword
            );
        }
        assert_eq!(
            (
                e.effective_power(bystander),
                e.effective_toughness(bystander)
            ),
            (Some(2), Some(2)),
            "{}: only the chosen creature is affected",
            case.id
        );
        activate_mana(&mut e, land, case);
    }
}

#[test]
fn q39_land_etb_target_trigger_does_not_go_on_the_stack_without_a_creature() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(39_032_000 + index as u64);
        inject_card_into_hand(&mut e, 0, case.id);
        let slot = hand_index_for_card(&e, 0, case.id);
        e.apply_scenario_command(0, &play_land(slot))
            .unwrap_or_else(|error| panic!("{}: play land: {error:?}", case.id));
        let land = battlefield_object_for_card(&e, 0, case.id);
        assert!(e.state.objects[&land].tapped, "{}", case.id);
        assert!(
            e.state.pending_triggers.is_empty(),
            "{}: no legal target",
            case.id
        );
        assert!(
            e.state.stack.is_empty(),
            "{}: no trigger on the stack",
            case.id
        );
    }
}

#[test]
fn q39_land_etb_target_cards_fizzle_when_the_chosen_creature_leaves() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(39_033_000 + index as u64);
        let target = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
        inject_card_into_hand(&mut e, 0, case.id);
        let slot = hand_index_for_card(&e, 0, case.id);
        e.apply_scenario_command(0, &play_land(slot))
            .expect("play land");
        semantic::accepted(&mut e, 0, &choose_trigger_target(target));
        e.state.players[0]
            .battlefield
            .retain(|object| *object != target);
        e.state.players[0].graveyard.push(target);
        e.state.objects.get_mut(&target).expect("target").zone = Zone::Graveyard;
        *e.state.zone_change_generation.entry(target).or_default() += 1;
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&target].zone,
            Zone::Graveyard,
            "{}",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}
