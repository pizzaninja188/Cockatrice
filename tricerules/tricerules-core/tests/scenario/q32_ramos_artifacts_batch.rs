//! Actual-card scenarios for the queue-v32 Ramos artifact cycle.
//!
//! Expectations are printed Oracle values. CR 605.1a/605.3b make both mana
//! abilities resolve immediately; CR 118.3/701.21a pay the sacrifice cost. The
//! pinned ruling confirms the sacrifice ability may be activated while tapped.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    color: char,
}

const CASES: [Case; 5] = [
    Case {
        id: "eye_of_ramos",
        color: 'u',
    },
    Case {
        id: "heart_of_ramos",
        color: 'r',
    },
    Case {
        id: "horn_of_ramos",
        color: 'g',
    },
    Case {
        id: "skull_of_ramos",
        color: 'b',
    },
    Case {
        id: "tooth_of_ramos",
        color: 'w',
    },
];

type ManaPool = (u32, u32, u32, u32, u32, u32);

fn pool(e: &GameEngine, player: usize) -> ManaPool {
    let p = &e.state.players[player].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

fn expected(color: char, count: u32) -> ManaPool {
    let mut value = match color {
        'w' => (count, 0, 0, 0, 0, 0),
        'u' => (0, count, 0, 0, 0, 0),
        'b' => (0, 0, count, 0, 0, 0),
        'r' => (0, 0, 0, count, 0, 0),
        'g' => (0, 0, 0, 0, count, 0),
        _ => unreachable!("printed color"),
    };
    value.5 = 0;
    value
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

fn cast_ramos(e: &mut GameEngine, case: &Case) -> u32 {
    give_mana(
        e,
        0,
        ManaGift {
            c: 3,
            ..Default::default()
        },
    );
    inject_card_into_hand(e, 0, case.id);
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .unwrap_or_else(|error| panic!("cast {}: {error:?}", case.id));
    resolve_entire_stack_two_player(e);
    let source = battlefield_object_for_card(e, 0, case.id);
    assert_eq!(
        e.state.objects[&source].zone,
        Zone::Battlefield,
        "{}",
        case.id
    );
    assert_eq!(pool(e, 0), (0, 0, 0, 0, 0, 0), "{}: exact cost", case.id);
    source
}

#[test]
fn q32_ramos_artifacts_canary_simplest() {
    let case = &CASES[0];
    let mut e = engine(32_020_001);
    let source = cast_ramos(&mut e, case);
    apply_ability(&mut e, 0, source, 0, vec![]).expect("tap for blue");
    assert!(e.state.objects[&source].tapped);
    assert_eq!(pool(&e, 0), expected(case.color, 1));
    apply_ability(&mut e, 0, source, 1, vec![]).expect("sacrifice while tapped");
    assert_eq!(e.state.objects[&source].zone, Zone::Graveyard);
    assert_eq!(pool(&e, 0), expected(case.color, 2));
    assert!(e.state.stack.is_empty());
}

#[test]
fn q32_ramos_artifacts_canary_hardest() {
    let case = &CASES[4];
    let mut e = engine(32_020_002);
    let source = cast_ramos(&mut e, case);
    apply_ability(&mut e, 0, source, 0, vec![]).expect("tap for white");
    apply_ability(&mut e, 0, source, 1, vec![]).expect("sacrifice while tapped");
    assert_eq!(e.state.objects[&source].zone, Zone::Graveyard);
    assert_eq!(pool(&e, 0), expected(case.color, 2));
}

#[test]
fn q32_ramos_artifacts_cards_produce_twice_and_sacrifice_while_tapped() {
    for (index, case) in CASES.iter().enumerate() {
        let mut e = engine(32_021_000 + index as u64);
        let source = cast_ramos(&mut e, case);
        apply_ability(&mut e, 0, source, 0, vec![])
            .unwrap_or_else(|error| panic!("{}: tap mana: {error:?}", case.id));
        assert_eq!(pool(&e, 0), expected(case.color, 1), "{}", case.id);
        assert!(e.state.objects[&source].tapped, "{}", case.id);
        apply_ability(&mut e, 0, source, 1, vec![])
            .unwrap_or_else(|error| panic!("{}: sacrifice mana: {error:?}", case.id));
        assert_eq!(
            e.state.objects[&source].zone,
            Zone::Graveyard,
            "{}",
            case.id
        );
        assert_eq!(pool(&e, 0), expected(case.color, 2), "{}", case.id);
        assert!(
            apply_ability(&mut e, 0, source, 1, vec![]).is_err(),
            "{}: a sacrificed source cannot be activated again",
            case.id
        );
    }
}

#[test]
fn q32_ramos_artifacts_cards_sacrifice_without_tapping_first() {
    for (index, case) in CASES.iter().enumerate() {
        let mut e = engine(32_022_000 + index as u64);
        let source = cast_ramos(&mut e, case);
        assert!(!e.state.objects[&source].tapped, "{}", case.id);
        apply_ability(&mut e, 0, source, 1, vec![]).expect("sacrifice the untapped artifact");
        assert_eq!(
            e.state.objects[&source].zone,
            Zone::Graveyard,
            "{}",
            case.id
        );
        assert_eq!(pool(&e, 0), expected(case.color, 1), "{}", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn q32_ramos_artifacts_cards_reject_a_foreign_controller() {
    for (index, case) in CASES.iter().enumerate() {
        let mut e = engine(32_023_000 + index as u64);
        let source = cast_ramos(&mut e, case);
        e.apply_scenario_command(0, &pass())
            .expect("controller passes");
        assert!(
            apply_ability(&mut e, 1, source, 0, vec![]).is_err(),
            "{}: another player cannot activate it",
            case.id
        );
        assert!(!e.state.objects[&source].tapped, "{}", case.id);
        assert_eq!(pool(&e, 1), (0, 0, 0, 0, 0, 0), "{}", case.id);
    }
}
