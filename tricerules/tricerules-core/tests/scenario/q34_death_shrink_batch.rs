//! Actual-card scenarios for the queue-v34 dies-shrink cohort.
//!
//! Expectations are printed Oracle values. CR 700.4/603.6c trigger on leaving
//! the battlefield for a graveyard; CR 115.1 makes the chosen creature the only
//! target; CR 611.2a/514.2 and 613.4c give the -N/-N its duration and layer.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{ruled_command::Cmd, ChooseTriggerTarget, RuledCommand};

struct Case {
    id: &'static str,
    amount: i32,
    power: u32,
    toughness: u32,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "disease_carriers",
            amount: -2,
            power: 2,
            toughness: 2,
        },
        Case {
            id: "festering_goblin",
            amount: -1,
            power: 1,
            toughness: 1,
        },
        Case {
            id: "hornet_harasser",
            amount: -2,
            power: 2,
            toughness: 2,
        },
    ]
}

fn choose_trigger_target(targets: Vec<tricerules_proto::ruled::v1::TargetRef>) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: Vec::new(),
            targets,
        })),
    }
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("swamp", &[card, "murder", "swords_to_plowshares"]),
        deck_with("forest", &["grizzly_bears", "savannah_lions"]),
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

fn kill_source(e: &mut GameEngine, source: u32) {
    inject_card_into_hand(e, 0, "murder");
    give_mana(
        e,
        0,
        ManaGift {
            b: 2,
            c: 1,
            ..Default::default()
        },
    );
    let murder = hand_index_for_card(e, 0, "murder");
    e.apply_scenario_command(0, &cast_spell(murder, target_object(source)))
        .expect("destroy the source");
    e.apply_scenario_command(0, &pass()).expect("p0 pass");
    e.apply_scenario_command(1, &pass())
        .expect("Murder resolves");
}

#[test]
fn q34_death_shrink_canary_simplest() {
    let case = &cases()[1];
    let mut e = engine(34_040_001, case.id);
    let source = move_ready_to_battlefield(&mut e, 0, case.id);
    let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
    kill_source(&mut e, source);
    assert_eq!(e.state.objects[&source].zone, Zone::Graveyard);
    assert_eq!(e.state.pending_triggers.len(), 1);
    e.apply_scenario_command(0, &choose_trigger_target(target_object(target)))
        .expect("target the opponent's creature");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        (e.effective_power(target), e.effective_toughness(target)),
        (Some(1), Some(1)),
        "Festering Goblin gives -1/-1"
    );
}

#[test]
fn q34_death_shrink_canary_hardest() {
    let case = &cases()[2];
    let mut e = engine(34_040_002, case.id);
    let source = move_ready_to_battlefield(&mut e, 0, case.id);
    let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 4, 4);
    kill_source(&mut e, source);
    e.apply_scenario_command(0, &choose_trigger_target(target_object(target)))
        .expect("target the opponent's creature");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        (e.effective_power(target), e.effective_toughness(target)),
        (Some(2), Some(2)),
        "Hornet Harasser gives -2/-2"
    );
}

#[test]
fn q34_death_shrink_cards_apply_their_printed_shrink_to_any_creature() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(34_041_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(case.power), Some(case.toughness)),
            "{}: printed body",
            case.id
        );
        let own = inject_creature_with_stats(&mut e, 0, "savannah_lions", 4, 4);
        let opposing = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 4, 4);
        kill_source(&mut e, source);
        e.apply_scenario_command(0, &choose_trigger_target(target_object(own)))
            .expect("the controller's own creature is legal");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            (e.effective_power(own), e.effective_toughness(own)),
            (
                Some((4 + case.amount) as u32),
                Some((4 + case.amount) as u32)
            ),
            "{}: own creature",
            case.id
        );
        assert_eq!(
            (e.effective_power(opposing), e.effective_toughness(opposing)),
            (Some(4), Some(4)),
            "{}: the other creature is unaffected",
            case.id
        );
    }
}

#[test]
fn q34_death_shrink_cards_reject_noncreature_targets() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(34_042_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        let land = inject_permanent_on_battlefield(&mut e, 1, "forest");
        kill_source(&mut e, source);
        assert!(
            e.apply_scenario_command(0, &choose_trigger_target(target_object(land)))
                .is_err(),
            "{}: a land is not a legal creature target",
            case.id
        );
    }
}

#[test]
fn q34_death_shrink_cards_do_not_trigger_on_exile() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(34_043_000 + index as u64, case.id);
        let exiled = move_ready_to_battlefield(&mut e, 0, case.id);
        let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 4, 4);
        inject_card_into_hand(&mut e, 0, "swords_to_plowshares");
        give_mana(
            &mut e,
            0,
            ManaGift {
                w: 1,
                ..Default::default()
            },
        );
        let swords = hand_index_for_card(&e, 0, "swords_to_plowshares");
        e.apply_scenario_command(0, &cast_spell(swords, target_object(exiled)))
            .expect("exile the source");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.state.objects[&exiled].zone, Zone::Exile, "{}", case.id);
        assert!(
            e.state.pending_triggers.is_empty(),
            "{}: leaving for exile is not dying",
            case.id
        );
        assert_eq!(
            (e.effective_power(target), e.effective_toughness(target)),
            (Some(4), Some(4)),
            "{}: the unrelated creature is unchanged",
            case.id
        );
    }
}
