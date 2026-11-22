//! Actual-card scenarios for the reviewed "dies, deal N damage to any target"
//! cohort.
//!
//! Expectations are printed Oracle values. CR 700.4 makes dies mean leaving the
//! battlefield for the graveyard; CR 603.6c triggers from that event; CR 115.1
//! makes any target a target chosen when the trigger is put on the stack; CR
//! 608.2b leaves a departed target undamaged.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{ruled_command::Cmd, ChooseTriggerTarget, RuledCommand};

struct Case {
    id: &'static str,
    damage: u32,
}

fn cases() -> [Case; 4] {
    [
        Case {
            id: "footlight_fiend",
            damage: 1,
        },
        Case {
            id: "mudbutton_torchrunner",
            damage: 3,
        },
        Case {
            id: "perilous_myr",
            damage: 2,
        },
        Case {
            id: "pyre_spawn",
            damage: 3,
        },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with(
            "swamp",
            &[card, "murder", "unsummon", "swords_to_plowshares"],
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

#[test]
fn dies_damage_any_target_batch_cards_deal_their_printed_damage_to_players() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(10_700_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        kill_source(&mut e, source);
        assert_eq!(
            e.state.objects[&source].zone,
            Zone::Graveyard,
            "{}",
            case.id
        );
        assert_eq!(e.state.pending_triggers.len(), 1, "{}", case.id);
        e.apply_scenario_command(0, &choose_trigger_target(target_player(1)))
            .expect("choose the opposing player");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.players[1].life,
            20 - i32::try_from(case.damage).expect("small printed damage"),
            "{}: exact printed damage to the player",
            case.id
        );
        assert_eq!(e.state.players[0].life, 20, "{}", case.id);
    }
}

#[test]
fn dies_damage_any_target_batch_cards_can_target_creatures() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(10_710_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        let bear = move_ready_to_battlefield(&mut e, 1, "grizzly_bears");
        kill_source(&mut e, source);
        assert_eq!(e.state.pending_triggers.len(), 1, "{}", case.id);
        e.apply_scenario_command(0, &choose_trigger_target(target_object(bear)))
            .expect("choose the opposing creature");
        resolve_entire_stack_two_player(&mut e);
        if case.damage >= 2 {
            assert_eq!(
                e.state.objects[&bear].zone,
                Zone::Graveyard,
                "{}: lethal damage destroys the 2/2",
                case.id
            );
        } else {
            assert_eq!(
                e.state.objects[&bear].damage, case.damage,
                "{}: exact nonlethal damage",
                case.id
            );
        }
        assert_eq!(e.state.players[1].life, 20, "{}", case.id);
    }
}

#[test]
fn dies_damage_any_target_batch_cards_do_not_damage_a_departed_target() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(10_720_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        let bear = move_ready_to_battlefield(&mut e, 1, "grizzly_bears");
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
        assert_eq!(
            e.state.objects[&bear].zone,
            Zone::Hand,
            "{}: the target left the battlefield",
            case.id
        );
        assert_eq!(
            e.state.objects[&bear].damage, 0,
            "{}: the stale trigger deals no damage",
            case.id
        );
        assert_eq!(e.state.players[1].life, 20, "{}", case.id);
    }
}

#[test]
fn dies_damage_any_target_batch_cards_do_not_trigger_when_exiled() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(10_730_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
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
        assert_eq!(e.state.players[1].life, 20, "{}", case.id);
    }
}
