//! Queue-v29 canaries: basic Snow lands with an intrinsic colored mana ability.
//!
//! CR 205.4g, 305.6/305.8, 605.1a/605.3b and 106.1a. Expectations are printed Oracle values.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    mana: (u32, u32, u32, u32, u32, u32),
}

fn case(id: &'static str) -> Case {
    match id {
        "snow-covered_plains" => Case {
            id,
            mana: (1, 0, 0, 0, 0, 0),
        },
        "snow-covered_forest" => Case {
            id,
            mana: (0, 0, 0, 0, 1, 0),
        },
        "snow-covered_island" => Case {
            id,
            mana: (0, 1, 0, 0, 0, 0),
        },
        "snow-covered_swamp" => Case {
            id,
            mana: (0, 0, 1, 0, 0, 0),
        },
        "snow-covered_mountain" => Case {
            id,
            mana: (0, 0, 0, 1, 0, 0),
        },
        _ => unreachable!(),
    }
}

fn cases() -> [Case; 5] {
    [
        case("snow-covered_plains"),
        case("snow-covered_island"),
        case("snow-covered_swamp"),
        case("snow-covered_mountain"),
        case("snow-covered_forest"),
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[card]), deck_with("island", &[])]);
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

fn pool(e: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[0].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

fn assert_canary(case: Case, seed: u64) {
    let mut e = engine(seed, case.id);
    inject_card_into_hand(&mut e, 0, case.id);
    let hand = hand_index_for_card(&e, 0, case.id);
    e.apply_scenario_command(0, &play_land(hand))
        .expect("play the snow basic");
    let land = battlefield_object_for_card(&e, 0, case.id);
    assert_eq!(e.state.objects[&land].zone, Zone::Battlefield);
    assert!(
        !e.state.objects[&land].tapped,
        "{}: a basic land enters untapped",
        case.id
    );
    e.apply_scenario_command(0, &activate_ability_for(&e, land, 0, vec![]))
        .expect("tap for one colored mana");
    assert_eq!(pool(&e), case.mana, "{}: exact colored mana", case.id);
    assert!(e.state.objects[&land].tapped, "{}", case.id);
    assert!(
        e.state.stack.is_empty(),
        "{}: the mana ability resolves immediately",
        case.id
    );
    assert!(
        e.apply_scenario_command(0, &activate_ability_for(&e, land, 0, vec![]))
            .is_err(),
        "{}: a tapped land cannot produce again",
        case.id
    );
}

#[test]
fn q29_snow_basics_canary_simplest() {
    assert_canary(case("snow-covered_plains"), 29_050_001);
}

#[test]
fn q29_snow_basics_canary_hardest() {
    assert_canary(case("snow-covered_forest"), 29_050_002);
}

#[test]
fn q29_snow_basics_cards_produce_exactly_one_colored_mana_and_no_snow_mana() {
    for (index, case) in cases().into_iter().enumerate() {
        assert_canary(case, 29_051_000 + index as u64);
    }
}
