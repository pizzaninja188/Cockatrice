//! Actual-card scenarios for the queue-v32 Diamond artifact cycle.
//!
//! Expectations are printed Oracle values. CR 614.1d/614.12 make the
//! enters-tapped clause a replacement effect; CR 605.1a/605.3b resolve the
//! mana ability immediately; CR 118.3 requires the tap cost.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    color: char,
}

const CASES: [Case; 5] = [
    Case {
        id: "charcoal_diamond",
        color: 'b',
    },
    Case {
        id: "fire_diamond",
        color: 'r',
    },
    Case {
        id: "marble_diamond",
        color: 'w',
    },
    Case {
        id: "moss_diamond",
        color: 'g',
    },
    Case {
        id: "sky_diamond",
        color: 'u',
    },
];

type ManaPool = (u32, u32, u32, u32, u32, u32);

fn pool(e: &GameEngine, player: usize) -> ManaPool {
    let p = &e.state.players[player].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

fn expected(color: char) -> ManaPool {
    match color {
        'w' => (1, 0, 0, 0, 0, 0),
        'u' => (0, 1, 0, 0, 0, 0),
        'b' => (0, 0, 1, 0, 0, 0),
        'r' => (0, 0, 0, 1, 0, 0),
        'g' => (0, 0, 0, 0, 1, 0),
        _ => unreachable!("printed color"),
    }
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

fn cast_diamond(e: &mut GameEngine, case: &Case) -> u32 {
    give_mana(
        e,
        0,
        ManaGift {
            c: 2,
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
    source
}

#[test]
fn q32_diamond_artifacts_canary_simplest() {
    let case = &CASES[0];
    let mut e = engine(32_010_001);
    let source = cast_diamond(&mut e, case);
    assert!(e.state.objects[&source].tapped, "enters tapped");
    assert_eq!(
        pool(&e, 0),
        (0, 0, 0, 0, 0, 0),
        "the two generic mana was spent"
    );
    assert!(
        apply_ability(&mut e, 0, source, 0, vec![]).is_err(),
        "a tapped artifact cannot pay the tap cost"
    );
    e.state.objects.get_mut(&source).unwrap().tapped = false;
    apply_ability(&mut e, 0, source, 0, vec![]).expect("tap for black");
    assert_eq!(pool(&e, 0), expected(case.color));
    assert!(e.state.objects[&source].tapped);
    assert!(e.state.stack.is_empty());
}

#[test]
fn q32_diamond_artifacts_canary_hardest() {
    let case = &CASES[4];
    let mut e = engine(32_010_002);
    let source = cast_diamond(&mut e, case);
    assert!(e.state.objects[&source].tapped, "enters tapped");
    e.state.objects.get_mut(&source).unwrap().tapped = false;
    apply_ability(&mut e, 0, source, 0, vec![]).expect("tap for blue");
    assert_eq!(pool(&e, 0), expected(case.color));
    assert!(e.state.stack.is_empty());
}

#[test]
fn q32_diamond_artifacts_cards_enter_tapped_and_produce_the_printed_color() {
    for (index, case) in CASES.iter().enumerate() {
        let mut e = engine(32_011_000 + index as u64);
        let source = cast_diamond(&mut e, case);
        assert!(
            e.state.objects[&source].tapped,
            "{}: enters tapped",
            case.id
        );
        assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 0), "{}: exact cost", case.id);
        assert!(
            apply_ability(&mut e, 0, source, 0, vec![]).is_err(),
            "{}: tapped rejection",
            case.id
        );
        e.state.objects.get_mut(&source).unwrap().tapped = false;
        apply_ability(&mut e, 0, source, 0, vec![])
            .unwrap_or_else(|error| panic!("{}: mana: {error:?}", case.id));
        assert_eq!(pool(&e, 0), expected(case.color), "{}", case.id);
        assert!(e.state.objects[&source].tapped, "{}", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn q32_diamond_artifacts_cards_reject_a_second_activation_and_a_foreign_controller() {
    for (index, case) in CASES.iter().enumerate() {
        let mut e = engine(32_012_000 + index as u64);
        let source = cast_diamond(&mut e, case);
        e.state.objects.get_mut(&source).unwrap().tapped = false;
        apply_ability(&mut e, 0, source, 0, vec![]).expect("first activation");
        assert!(
            apply_ability(&mut e, 0, source, 0, vec![]).is_err(),
            "{}: tapping is a real cost",
            case.id
        );
        assert_eq!(pool(&e, 0), expected(case.color), "{}", case.id);

        let mut foreign = engine(32_013_000 + index as u64);
        let foreign_source = cast_diamond(&mut foreign, case);
        foreign
            .state
            .objects
            .get_mut(&foreign_source)
            .unwrap()
            .tapped = false;
        foreign
            .apply_scenario_command(0, &pass())
            .expect("controller passes");
        assert!(
            apply_ability(&mut foreign, 1, foreign_source, 0, vec![]).is_err(),
            "{}: another player cannot activate the artifact",
            case.id
        );
        assert_eq!(pool(&foreign, 1), (0, 0, 0, 0, 0, 0), "{}", case.id);
        assert!(
            !foreign.state.objects[&foreign_source].tapped,
            "{}",
            case.id
        );
    }
}
