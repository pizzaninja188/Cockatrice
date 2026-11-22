//! Actual-card scenarios for the queue-v26 entry pump-and-flying cohort.
//!
//! Expectations are printed Oracle values. CR 603.6a stages the entry trigger;
//! CR 115.1 makes the target chosen as the trigger is put on the stack; CR 611.2a
//! fixes the duration until end of turn; CR 613.1f grants flying and CR 613.4c
//! applies the pump; CR 514.2 ends both during cleanup.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::{TurnStep, Zone};
use tricerules_proto::ruled::v1::{ruled_command::Cmd, ChooseTriggerTarget, RuledCommand};

struct Case {
    id: &'static str,
    power: u32,
    toughness: u32,
    pump_power: u32,
    pump_toughness: u32,
}

const CASES: [Case; 3] = [
    Case {
        id: "welkin_guide",
        power: 2,
        toughness: 2,
        pump_power: 2,
        pump_toughness: 2,
    },
    Case {
        id: "arborea_pegasus",
        power: 2,
        toughness: 3,
        pump_power: 1,
        pump_toughness: 1,
    },
    Case {
        id: "battleflight_eagle",
        power: 2,
        toughness: 2,
        pump_power: 2,
        pump_toughness: 2,
    },
];

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("plains", &[card, "grizzly_bears", "savannah_lions"]),
        deck_with("forest", &["grizzly_bears"]),
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

fn choose_trigger_target(targets: Vec<tricerules_proto::ruled::v1::TargetRef>) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: Vec::new(),
            targets,
        })),
    }
}

fn decline_trigger_target() -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: true,
            selected_modes: Vec::new(),
            targets: Vec::new(),
        })),
    }
}

fn advance_to_next_own_main1(e: &mut GameEngine) {
    let starting_turn = e.state.turn_instance;
    for _ in 0..320 {
        if e.state.turn_instance > starting_turn
            && e.state.active_player_id() == 0
            && e.state.turn_step == TurnStep::Main1
            && e.state.priority_player_id() == 0
        {
            return;
        }
        resolve_cleanup_discards_if_any(e);
        let priority = e.state.priority_player_id();
        e.apply_scenario_command(priority, &pass())
            .expect("advance to the next own main phase");
    }
    panic!("turn advancement stalled before the next own main phase");
}

fn apply_trigger(e: &mut GameEngine, case: &Case, target: u32) -> u32 {
    let source = move_ready_to_battlefield(e, 0, case.id);
    assert_eq!(
        e.state.pending_triggers.len(),
        1,
        "{}: the entry trigger is staged",
        case.id
    );
    e.apply_scenario_command(0, &choose_trigger_target(target_object(target)))
        .expect("choose the trigger target");
    resolve_entire_stack_two_player(e);
    source
}

fn move_to_graveyard(e: &mut GameEngine, player: usize, object: u32) {
    e.state.players[player]
        .battlefield
        .retain(|oid| *oid != object);
    e.state.players[player].graveyard.push(object);
    e.state.objects.get_mut(&object).expect("target").zone = Zone::Graveyard;
    *e.state.zone_change_generation.entry(object).or_default() += 1;
}

#[test]
fn etb_pump_flying_batch_cards_pump_and_grant_flying_to_the_chosen_creature() {
    for (index, case) in CASES.iter().enumerate() {
        let mut e = engine(26_080_000 + index as u64, case.id);
        let bears = move_ready_to_battlefield(&mut e, 0, "grizzly_bears");
        let other = move_ready_to_battlefield(&mut e, 0, "savannah_lions");
        let opposing = move_ready_to_battlefield(&mut e, 1, "grizzly_bears");
        let source = apply_trigger(&mut e, case, bears);

        assert_eq!(
            (e.effective_power(bears), e.effective_toughness(bears)),
            (Some(2 + case.pump_power), Some(2 + case.pump_toughness)),
            "{}: chosen creature gets the printed pump",
            case.id
        );
        assert!(
            e.effective_has_keyword(bears, Keyword::Flying),
            "{}: chosen creature gains flying",
            case.id
        );
        assert_eq!(
            (e.effective_power(other), e.effective_toughness(other)),
            (Some(2), Some(1)),
            "{}: another creature is unchanged",
            case.id
        );
        assert!(
            !e.effective_has_keyword(other, Keyword::Flying),
            "{}",
            case.id
        );
        assert_eq!(
            (e.effective_power(opposing), e.effective_toughness(opposing)),
            (Some(2), Some(2)),
            "{}: an opponent's creature is unchanged when not chosen",
            case.id
        );
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(case.power), Some(case.toughness)),
            "{}: the source keeps its printed body",
            case.id
        );
        assert!(
            e.effective_has_keyword(source, Keyword::Flying),
            "{}: the source has printed flying",
            case.id
        );
    }
}

#[test]
fn etb_pump_flying_batch_cards_can_target_the_source_or_an_opponents_creature() {
    for (index, case) in CASES.iter().enumerate() {
        let mut self_target = engine(26_081_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut self_target, 0, case.id);
        assert_eq!(self_target.state.pending_triggers.len(), 1, "{}", case.id);
        self_target
            .apply_scenario_command(0, &choose_trigger_target(target_object(source)))
            .expect("target the source itself");
        resolve_entire_stack_two_player(&mut self_target);
        assert_eq!(
            (
                self_target.effective_power(source),
                self_target.effective_toughness(source)
            ),
            (
                Some(case.power + case.pump_power),
                Some(case.toughness + case.pump_toughness)
            ),
            "{}: the source may target itself",
            case.id
        );

        let mut opposing_target = engine(26_082_000 + index as u64, case.id);
        let opposing = move_ready_to_battlefield(&mut opposing_target, 1, "grizzly_bears");
        apply_trigger(&mut opposing_target, case, opposing);
        assert_eq!(
            (
                opposing_target.effective_power(opposing),
                opposing_target.effective_toughness(opposing)
            ),
            (Some(2 + case.pump_power), Some(2 + case.pump_toughness)),
            "{}: any creature is a legal target",
            case.id
        );
        assert!(
            opposing_target.effective_has_keyword(opposing, Keyword::Flying),
            "{}: an opponent's creature can gain flying",
            case.id
        );
    }
}

#[test]
fn etb_pump_flying_batch_cards_reject_a_declined_mandatory_target() {
    for (index, case) in CASES.iter().enumerate() {
        let mut e = engine(26_083_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        assert_eq!(e.state.pending_triggers.len(), 1, "{}", case.id);
        e.apply_scenario_command(0, &decline_trigger_target())
            .expect_err("the mandatory target cannot be declined");
        assert_eq!(e.state.pending_triggers.len(), 1, "{}", case.id);
        e.apply_scenario_command(0, &choose_trigger_target(target_object(source)))
            .expect("the trigger remains answerable");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.effective_power(source),
            Some(case.power + case.pump_power),
            "{}",
            case.id
        );
    }
}

#[test]
fn etb_pump_flying_batch_cards_end_the_pump_and_flying_at_cleanup() {
    for (index, case) in CASES.iter().enumerate() {
        let mut e = engine(26_084_000 + index as u64, case.id);
        let bears = move_ready_to_battlefield(&mut e, 0, "grizzly_bears");
        apply_trigger(&mut e, case, bears);
        assert!(
            e.effective_has_keyword(bears, Keyword::Flying),
            "{}",
            case.id
        );
        advance_to_next_own_main1(&mut e);
        assert_eq!(
            (e.effective_power(bears), e.effective_toughness(bears)),
            (Some(2), Some(2)),
            "{}: the pump ended",
            case.id
        );
        assert!(
            !e.effective_has_keyword(bears, Keyword::Flying),
            "{}: the flying grant ended",
            case.id
        );
    }
}

#[test]
fn etb_pump_flying_batch_cards_apply_the_pump_only_when_the_trigger_resolves() {
    for (index, case) in CASES.iter().enumerate() {
        let mut e = engine(26_085_000 + index as u64, case.id);
        let bears = move_ready_to_battlefield(&mut e, 0, "grizzly_bears");
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        assert_eq!(e.state.pending_triggers.len(), 1, "{}", case.id);
        e.apply_scenario_command(0, &choose_trigger_target(target_object(bears)))
            .expect("choose the trigger target");
        assert_eq!(
            (e.effective_power(bears), e.effective_toughness(bears)),
            (Some(2), Some(2)),
            "{}: staging the trigger does not pump",
            case.id
        );
        assert!(
            !e.effective_has_keyword(bears, Keyword::Flying),
            "{}: staging the trigger does not grant flying",
            case.id
        );
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            (e.effective_power(bears), e.effective_toughness(bears)),
            (Some(2 + case.pump_power), Some(2 + case.pump_toughness)),
            "{}: resolution applies the pump",
            case.id
        );
        assert!(
            e.effective_has_keyword(bears, Keyword::Flying),
            "{}: resolution grants flying",
            case.id
        );
        assert_eq!(e.effective_power(source), Some(case.power), "{}", case.id);
    }
}

#[test]
fn etb_pump_flying_batch_cards_fizzle_when_the_only_target_leaves() {
    for (index, case) in CASES.iter().enumerate() {
        let mut e = engine(26_086_000 + index as u64, case.id);
        let bears = move_ready_to_battlefield(&mut e, 0, "grizzly_bears");
        let bystander = move_ready_to_battlefield(&mut e, 0, "savannah_lions");
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        assert_eq!(e.state.pending_triggers.len(), 1, "{}", case.id);
        e.apply_scenario_command(0, &choose_trigger_target(target_object(bears)))
            .expect("choose the trigger target");
        move_to_graveyard(&mut e, 0, bears);
        resolve_entire_stack_two_player(&mut e);

        assert!(e.state.stack.is_empty(), "{}", case.id);
        assert!(e.state.pending_triggers.is_empty(), "{}", case.id);
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(case.power), Some(case.toughness)),
            "{}: the source is not pumped by its own fizzled trigger",
            case.id
        );
        assert!(
            e.effective_has_keyword(source, Keyword::Flying),
            "{}: only the printed flying remains",
            case.id
        );
        assert_eq!(
            (
                e.effective_power(bystander),
                e.effective_toughness(bystander)
            ),
            (Some(2), Some(1)),
            "{}: an unrelated creature is untouched",
            case.id
        );
        assert!(
            !e.effective_has_keyword(bystander, Keyword::Flying),
            "{}: no flying was granted to an unrelated creature",
            case.id
        );
    }
}
