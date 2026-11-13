//! Actual-card scenarios for the reviewed "dies, shrink an opponent's creature"
//! family.
//!
//! Expectations are printed Oracle values. CR 700.4 makes "dies" mean leaving
//! the battlefield for the graveyard; CR 603.6c triggers from that event. The
//! -1/-1 lasts until end of turn and may only target an opponent's creature.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{ruled_command::Cmd, ChooseTriggerTarget, RuledCommand};

struct Case {
    id: &'static str,
    power: u32,
    toughness: u32,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "grim_physician",
            power: 1,
            toughness: 1,
        },
        Case {
            id: "shambling_goblin",
            power: 1,
            toughness: 1,
        },
        Case {
            id: "splatter_goblin",
            power: 2,
            toughness: 1,
        },
    ]
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
fn dies_shrink_opponent_batch_cards_shrink_an_opponent_creature_when_they_die() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_230_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(case.power), Some(case.toughness)),
            "{}: printed body",
            case.id
        );
        let bear = move_ready_to_battlefield(&mut e, 1, "grizzly_bears");
        kill_source(&mut e, source);
        assert_eq!(
            e.state.objects[&source].zone,
            Zone::Graveyard,
            "{}: the source died",
            case.id
        );
        assert_eq!(
            e.state.pending_triggers.len(),
            1,
            "{}: the dies trigger is staged",
            case.id
        );
        e.apply_scenario_command(0, &choose_trigger_target(target_object(bear)))
            .expect("target the opponent's creature");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            (e.effective_power(bear), e.effective_toughness(bear)),
            (Some(1), Some(1)),
            "{}: the target gets -1/-1",
            case.id
        );
        assert!(
            e.state.players[1].battlefield.contains(&bear),
            "{}: the 2/2 survives at 1/1",
            case.id
        );
    }
}

#[test]
fn dies_shrink_opponent_batch_cards_cannot_target_their_controllers_creature() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_231_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        let own_creature = inject_creature_on_battlefield(&mut e, 0, "savannah_lions");
        let bear = move_ready_to_battlefield(&mut e, 1, "grizzly_bears");
        kill_source(&mut e, source);
        assert_eq!(e.state.pending_triggers.len(), 1, "{}", case.id);
        assert!(
            e.apply_scenario_command(0, &choose_trigger_target(target_object(own_creature)))
                .is_err(),
            "{}: the trigger may not target its controller's creature",
            case.id
        );
        e.apply_scenario_command(0, &choose_trigger_target(target_object(bear)))
            .expect("the opponent's creature is legal");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            (
                e.effective_power(own_creature),
                e.effective_toughness(own_creature)
            ),
            (Some(2), Some(2)),
            "{}: the rejected target is unchanged",
            case.id
        );
        assert_eq!(
            (e.effective_power(bear), e.effective_toughness(bear)),
            (Some(1), Some(1)),
            "{}: the legal target is shrunk",
            case.id
        );
    }
}

#[test]
fn dies_shrink_opponent_batch_cards_do_not_trigger_when_exiled() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_232_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        let bear = move_ready_to_battlefield(&mut e, 1, "grizzly_bears");
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
        assert_eq!(
            e.state.objects[&source].zone,
            Zone::Exile,
            "{}: the source was exiled, not destroyed",
            case.id
        );
        assert!(
            e.state.pending_triggers.is_empty(),
            "{}: leaving for exile is not dying",
            case.id
        );
        assert_eq!(
            (e.effective_power(bear), e.effective_toughness(bear)),
            (Some(2), Some(2)),
            "{}: the opponent's creature is unchanged",
            case.id
        );
    }
}
