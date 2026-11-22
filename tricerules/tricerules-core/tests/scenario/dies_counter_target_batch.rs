//! Actual-card scenarios for the queue-v22 "dies, put a +1/+1 counter on target
//! creature you control" cohort.
//!
//! Expectations are printed Oracle values. CR 700.4 makes dies mean leaving the
//! battlefield for the graveyard; CR 603.6c triggers from that event; CR 115.1
//! makes the chosen creature a target; 608.2b removes a trigger whose target left.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::CounterKind;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{ruled_command::Cmd, ChooseTriggerTarget, RuledCommand};

struct Case {
    id: &'static str,
}

fn cases() -> [Case; 5] {
    [
        Case {
            id: "spinal_centipede",
        },
        Case {
            id: "guul_draz_mucklord",
        },
        Case {
            id: "sparring_construct",
        },
        Case {
            id: "lawless_broker",
        },
        Case {
            id: "goblin_assault_team",
        },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with(
            "swamp",
            &[
                card,
                "murder",
                "unsummon",
                "swords_to_plowshares",
                "grizzly_bears",
            ],
        ),
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

fn engine_for_player_one(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("swamp", &["murder", "grizzly_bears"]),
        deck_with("forest", &[card, "grizzly_bears"]),
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
    ensure_in_hand(e, 0, "murder");
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
    resolve_entire_stack_two_player(e);
}

fn assert_counter_on(e: &GameEngine, target: u32, case: &Case) {
    assert_eq!(
        e.state.objects[&target].counters[&CounterKind::PlusOnePlusOne],
        1,
        "{}: exactly one +1/+1 counter",
        case.id
    );
}

#[test]
fn dies_counter_target_batch_canary_cards_place_one_counter_on_a_chosen_creature() {
    for case in cases()
        .into_iter()
        .filter(|case| matches!(case.id, "sparring_construct" | "goblin_assault_team"))
    {
        let mut e = engine(10_900_000, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        let bear = move_ready_to_battlefield(&mut e, 0, "grizzly_bears");
        kill_source(&mut e, source);
        assert_eq!(
            e.state.objects[&source].zone,
            Zone::Graveyard,
            "{}",
            case.id
        );
        assert_eq!(e.state.pending_triggers.len(), 1, "{}", case.id);
        e.apply_scenario_command(0, &choose_trigger_target(target_object(bear)))
            .expect("choose a creature you control");
        resolve_entire_stack_two_player(&mut e);
        assert_counter_on(&e, bear, &case);
        assert_eq!(
            (e.effective_power(bear), e.effective_toughness(bear)),
            (Some(2 + 1), Some(2 + 1)),
            "{}: the counter grows the chosen creature",
            case.id
        );
    }
}

#[test]
fn dies_counter_target_batch_cards_place_exactly_one_counter() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(10_910_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        let bear = move_ready_to_battlefield(&mut e, 0, "grizzly_bears");
        kill_source(&mut e, source);
        e.apply_scenario_command(0, &choose_trigger_target(target_object(bear)))
            .expect("choose the creature");
        resolve_entire_stack_two_player(&mut e);
        assert_counter_on(&e, bear, &case);
        assert_eq!(
            (e.effective_power(bear), e.effective_toughness(bear)),
            (Some(3), Some(3)),
            "{}",
            case.id
        );
        assert_eq!(
            e.state.objects[&source].counters.len(),
            0,
            "{}: the dead source receives nothing",
            case.id
        );
    }
}

#[test]
fn dies_counter_target_batch_cards_do_not_trigger_when_exiled() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(10_920_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        let bear = move_ready_to_battlefield(&mut e, 0, "grizzly_bears");
        ensure_in_hand(&mut e, 0, "swords_to_plowshares");
        give_mana(
            &mut e,
            0,
            ManaGift {
                w: 1,
                ..Default::default()
            },
        );
        let swords = hand_index_for_card(&e, 0, "swords_to_plowshares");
        e.apply_scenario_command(0, &cast_spell(swords, target_object(source)))
            .expect("exile the source");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.state.objects[&source].zone, Zone::Exile, "{}", case.id);
        assert!(
            e.state.pending_triggers.is_empty(),
            "{}: leaving for exile is not dying",
            case.id
        );
        assert_eq!(e.state.objects[&bear].counters.len(), 0, "{}", case.id);
    }
}

#[test]
fn dies_counter_target_batch_cards_without_a_legal_target_do_not_prompt() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(10_930_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        kill_source(&mut e, source);
        assert_eq!(
            e.state.objects[&source].zone,
            Zone::Graveyard,
            "{}",
            case.id
        );
        assert!(
            e.state.pending_triggers.is_empty(),
            "{}: the only creature you control died, so the trigger has no legal target",
            case.id
        );
    }
}

#[test]
fn dies_counter_target_batch_cards_do_not_counter_a_departed_target() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(10_940_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        let bear = move_ready_to_battlefield(&mut e, 0, "grizzly_bears");
        kill_source(&mut e, source);
        e.apply_scenario_command(0, &choose_trigger_target(target_object(bear)))
            .expect("choose the creature");
        ensure_in_hand(&mut e, 0, "unsummon");
        give_mana(
            &mut e,
            0,
            ManaGift {
                u: 1,
                ..Default::default()
            },
        );
        let unsummon = hand_index_for_card(&e, 0, "unsummon");
        e.apply_scenario_command(0, &cast_spell(unsummon, target_object(bear)))
            .expect("return the chosen creature in response");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.state.objects[&bear].zone, Zone::Hand, "{}", case.id);
        assert_eq!(
            e.state.objects[&bear].counters.len(),
            0,
            "{}: the stale trigger places no counter",
            case.id
        );
        assert_eq!(e.state.players[0].life, 20, "{}", case.id);
    }
}

#[test]
fn dies_counter_target_batch_cards_scope_the_target_to_their_controller() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine_for_player_one(10_950_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 1, case.id);
        let own_bear = move_ready_to_battlefield(&mut e, 1, "grizzly_bears");
        let other_bear = move_ready_to_battlefield(&mut e, 0, "grizzly_bears");
        kill_source(&mut e, source);
        assert_eq!(
            e.state.objects[&source].zone,
            Zone::Graveyard,
            "{}",
            case.id
        );
        assert_eq!(e.state.pending_triggers.len(), 1, "{}", case.id);
        let command_index = e.state.command_index;
        assert!(
            e.apply_scenario_command(1, &choose_trigger_target(target_object(other_bear)))
                .is_err(),
            "{}: an opponent's creature is not a legal target",
            case.id
        );
        assert_eq!(e.state.command_index, command_index, "{}", case.id);
        e.apply_scenario_command(1, &choose_trigger_target(target_object(own_bear)))
            .expect("choose a creature its controller controls");
        resolve_entire_stack_two_player(&mut e);
        assert_counter_on(&e, own_bear, &case);
        assert_eq!(
            e.state.objects[&other_bear].counters.len(),
            0,
            "{}: the opponent's creature receives nothing",
            case.id
        );
    }
}
