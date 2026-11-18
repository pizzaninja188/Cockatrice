//! Actual-card scenarios for the queue-v33 enters-tapped dual-land remnants.
//!
//! Expectations are printed Oracle values. CR 614.1d/614.12 applies the entry
//! replacement; each targetless mana ability (CR 605.3a-b) taps for one of its
//! printed colors (CR 107.4).
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_proto::ruled::v1::ruled_command::Cmd;

type Pool = (u32, u32, u32, u32, u32, u32);

struct Case {
    id: &'static str,
    options: &'static [char],
}

const CASES: [Case; 4] = [
    Case {
        id: "crumbling_necropolis",
        options: &['u', 'b', 'r'],
    },
    Case {
        id: "highland_lake",
        options: &['u', 'r'],
    },
    Case {
        id: "jungle_shrine",
        options: &['r', 'g', 'w'],
    },
    Case {
        id: "stone_quarry",
        options: &['r', 'w'],
    },
];

fn pool(e: &GameEngine, player: usize) -> Pool {
    let p = &e.state.players[player].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

fn expected(color: char) -> Pool {
    match color {
        'w' => (1, 0, 0, 0, 0, 0),
        'u' => (0, 1, 0, 0, 0, 0),
        'b' => (0, 0, 1, 0, 0, 0),
        'r' => (0, 0, 0, 1, 0, 0),
        'g' => (0, 0, 0, 0, 1, 0),
        _ => unreachable!("printed color"),
    }
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[card]), deck_with("forest", &[])]);
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

fn activate_option(
    e: &mut GameEngine,
    source: u32,
    option: u32,
) -> Result<(), tricerules_core::EngineError> {
    let mut command = activate_ability_for(e, source, 0, vec![]);
    let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
        unreachable!()
    };
    activation.mana_option_index = option;
    e.apply_scenario_command(0, &command).map(|_| ())
}

#[test]
fn q33_tapland_remnants_canary_simplest() {
    let case = &CASES[3];
    let mut e = engine(33_080_001, case.id);
    inject_card_into_hand(&mut e, 0, case.id);
    let slot = hand_index_for_card(&e, 0, case.id);
    e.apply_scenario_command(0, &play_land(slot))
        .expect("play land");
    let source = battlefield_object_for_card(&e, 0, case.id);
    assert!(e.state.objects[&source].tapped, "the land enters tapped");
    e.state.objects.get_mut(&source).unwrap().tapped = false;
    activate_option(&mut e, source, 0).expect("tap for red");
    assert!(e.state.objects[&source].tapped);
    assert_eq!(pool(&e, 0), expected('r'));
}

#[test]
fn q33_tapland_remnants_canary_hardest() {
    let case = &CASES[0];
    let mut e = engine(33_080_002, case.id);
    inject_card_into_hand(&mut e, 0, case.id);
    let slot = hand_index_for_card(&e, 0, case.id);
    e.apply_scenario_command(0, &play_land(slot))
        .expect("play land");
    let source = battlefield_object_for_card(&e, 0, case.id);
    assert!(e.state.objects[&source].tapped);
    e.state.objects.get_mut(&source).unwrap().tapped = false;
    activate_option(&mut e, source, 2).expect("tap for red");
    assert_eq!(pool(&e, 0), expected('r'));
}

#[test]
fn q33_tapland_remnants_cards_offer_each_printed_mana_option_and_enter_tapped() {
    for (case_index, case) in CASES.iter().enumerate() {
        for (option, color) in case.options.iter().enumerate() {
            let mut e = engine(33_081_000 + (case_index * 10 + option) as u64, case.id);
            inject_card_into_hand(&mut e, 0, case.id);
            let slot = hand_index_for_card(&e, 0, case.id);
            e.apply_scenario_command(0, &play_land(slot))
                .expect("play land");
            let source = battlefield_object_for_card(&e, 0, case.id);
            assert!(
                e.state.objects[&source].tapped,
                "{}: enters tapped",
                case.id
            );
            e.state.objects.get_mut(&source).unwrap().tapped = false;
            activate_option(&mut e, source, option as u32)
                .unwrap_or_else(|error| panic!("{} option {option}: {error:?}", case.id));
            assert_eq!(pool(&e, 0), expected(*color), "{} option {option}", case.id);
            assert!(e.state.objects[&source].tapped, "{}: tap cost", case.id);
        }
    }
}
