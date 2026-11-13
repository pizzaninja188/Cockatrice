//! Actual-card scenarios for the reviewed entry target-pump family.
//!
//! Expectations are printed Oracle values. CR 603.6a stages the entry trigger, CR 115 makes the target chosen as the trigger is put on the stack, CR 611.2c/613.4c fix the pump until end of turn, and CR 514.2 ends it during cleanup.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::{TurnStep, Zone};
use tricerules_proto::ruled::v1::{ruled_command::Cmd, ChooseTriggerTarget, RuledCommand};

struct Case {
    id: &'static str,
    power: u32,
    toughness: u32,
    pump_power: i32,
    pump_toughness: i32,
}

fn cases() -> [Case; 7] {
    [
        Case {
            id: "guardian_of_pilgrims",
            power: 2,
            toughness: 2,
            pump_power: 1,
            pump_toughness: 1,
        },
        Case {
            id: "jadecraft_artisan",
            power: 3,
            toughness: 3,
            pump_power: 2,
            pump_toughness: 2,
        },
        Case {
            id: "kinsbaile_skirmisher",
            power: 2,
            toughness: 2,
            pump_power: 1,
            pump_toughness: 1,
        },
        Case {
            id: "rubblebelt_boar",
            power: 3,
            toughness: 3,
            pump_power: 2,
            pump_toughness: 0,
        },
        Case {
            id: "tenth_district_guard",
            power: 2,
            toughness: 2,
            pump_power: 0,
            pump_toughness: 1,
        },
        Case {
            id: "vulshok_heartstoker",
            power: 2,
            toughness: 2,
            pump_power: 2,
            pump_toughness: 0,
        },
        Case {
            id: "yevas_forcemage",
            power: 2,
            toughness: 2,
            pump_power: 2,
            pump_toughness: 2,
        },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("plains", &[card, "savannah_lions", "grizzly_bears"]),
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

fn apply_pump(e: &mut GameEngine, case: &Case, target: u32) -> u32 {
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

#[test]
fn etb_target_pump_batch_cards_pump_the_chosen_creature_by_the_printed_amount() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(11_500_000 + index as u64, case.id);
        let bears = move_ready_to_battlefield(&mut e, 0, "grizzly_bears");
        let other = move_ready_to_battlefield(&mut e, 0, "savannah_lions");
        apply_pump(&mut e, &case, bears);
        assert_eq!(
            (e.effective_power(bears), e.effective_toughness(bears)),
            (
                Some(2 + case.pump_power as u32),
                Some(2 + case.pump_toughness as u32)
            ),
            "{}: chosen creature gets the printed pump",
            case.id
        );
        assert_eq!(
            (e.effective_power(other), e.effective_toughness(other)),
            (Some(2), Some(1)),
            "{}: another creature is unchanged",
            case.id
        );
        assert_eq!(e.state.players[0].life, 20, "{}: no life change", case.id);
    }
}

#[test]
fn etb_target_pump_batch_cards_choose_the_target_when_the_trigger_is_put_on_the_stack() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(11_510_000 + index as u64, case.id);
        let opposing = move_ready_to_battlefield(&mut e, 1, "grizzly_bears");
        apply_pump(&mut e, &case, opposing);
        assert_eq!(
            (e.effective_power(opposing), e.effective_toughness(opposing)),
            (
                Some(2 + case.pump_power as u32),
                Some(2 + case.pump_toughness as u32)
            ),
            "{}: any creature, including an opponent's, is a legal target",
            case.id
        );

        let mut self_target = engine(11_520_000 + index as u64, case.id);
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
                Some(case.power + case.pump_power as u32),
                Some(case.toughness + case.pump_toughness as u32)
            ),
            "{}: the source may target itself",
            case.id
        );
    }
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
fn etb_target_pump_batch_cards_reject_a_declined_mandatory_target() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(11_515_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        assert_eq!(
            e.state.pending_triggers.len(),
            1,
            "{}: entry trigger is staged",
            case.id
        );
        assert!(
            e.apply_scenario_command(0, &decline_trigger_target())
                .is_err(),
            "{}: the mandatory target cannot be declined",
            case.id
        );
        assert_eq!(
            e.state.pending_triggers.len(),
            1,
            "{}: the rejected decline leaves the trigger pending",
            case.id
        );
        e.apply_scenario_command(0, &choose_trigger_target(target_object(source)))
            .expect("choose the mandatory target");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (
                Some(case.power + case.pump_power as u32),
                Some(case.toughness + case.pump_toughness as u32)
            ),
            "{}: the trigger still resolves after a rejected decline",
            case.id
        );
    }
}

#[test]
fn etb_target_pump_batch_cards_do_not_pump_a_target_that_left() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(11_517_000 + index as u64, case.id);
        let friendly = move_ready_to_battlefield(&mut e, 0, "grizzly_bears");
        let target = move_ready_to_battlefield(&mut e, 1, "grizzly_bears");
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        assert_eq!(e.state.pending_triggers.len(), 1, "{}", case.id);
        e.apply_scenario_command(0, &choose_trigger_target(target_object(target)))
            .expect("choose the departed target");
        move_to_graveyard(&mut e, 1, target);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&target].zone,
            Zone::Graveyard,
            "{}",
            case.id
        );
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(case.power), Some(case.toughness)),
            "{}: the source is not pumped when its target left",
            case.id
        );
        assert_eq!(
            (e.effective_power(friendly), e.effective_toughness(friendly)),
            (Some(2), Some(2)),
            "{}: no other creature is pumped",
            case.id
        );
    }
}

fn advance_to_next_upkeep(e: &mut GameEngine) {
    let starting_turn = e.state.turn_instance;
    for _ in 0..240 {
        if e.state.turn_instance > starting_turn && e.state.turn_step == TurnStep::Upkeep {
            return;
        }
        resolve_cleanup_discards_if_any(e);
        let priority = e.state.priority_player_id();
        e.apply_scenario_command(priority, &pass())
            .expect("advance to the next upkeep");
    }
    panic!("turn advancement stalled before the next upkeep");
}

#[test]
fn etb_target_pump_batch_cards_expire_at_end_of_turn() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(11_530_000 + index as u64, case.id);
        let bears = move_ready_to_battlefield(&mut e, 0, "grizzly_bears");
        apply_pump(&mut e, &case, bears);
        assert_eq!(
            e.effective_power(bears),
            Some(2 + case.pump_power as u32),
            "{}",
            case.id
        );
        advance_to_next_upkeep(&mut e);
        assert_eq!(
            e.effective_power(bears),
            Some(2),
            "{}: power expires",
            case.id
        );
        assert_eq!(
            e.effective_toughness(bears),
            Some(2),
            "{}: toughness expires",
            case.id
        );
    }
}
