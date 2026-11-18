//! Actual-card scenarios for the queue-v33 creature mana-ability cohort.
//!
//! Expectations are printed Oracle values. Each activated ability is a mana
//! ability (CR 605.3a-b), pays its printed tap or mana cost (CR 118.3), and
//! produces the printed color (CR 107.4).
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;

type Pool = (u32, u32, u32, u32, u32, u32);

struct Case {
    id: &'static str,
    cost: ManaGift,
    expected: Pool,
    taps: bool,
}

const CASES: [Case; 4] = [
    Case {
        id: "agent_of_stromgald",
        cost: ManaGift {
            w: 0,
            u: 0,
            b: 0,
            r: 1,
            g: 0,
            c: 0,
        },
        expected: (0, 0, 1, 0, 0, 0),
        taps: false,
    },
    Case {
        id: "bog_initiate",
        cost: ManaGift {
            w: 0,
            u: 0,
            b: 0,
            r: 0,
            g: 0,
            c: 1,
        },
        expected: (0, 0, 1, 0, 0, 0),
        taps: false,
    },
    Case {
        id: "princess_lucrezia",
        cost: ManaGift {
            w: 0,
            u: 0,
            b: 0,
            r: 0,
            g: 0,
            c: 0,
        },
        expected: (0, 1, 0, 0, 0, 0),
        taps: true,
    },
    Case {
        id: "riven_turnbull",
        cost: ManaGift {
            w: 0,
            u: 0,
            b: 0,
            r: 0,
            g: 0,
            c: 0,
        },
        expected: (0, 0, 1, 0, 0, 0),
        taps: true,
    },
];

fn pool(e: &GameEngine, player: usize) -> Pool {
    let p = &e.state.players[player].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
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

fn gift(m: &ManaGift) -> ManaGift {
    ManaGift {
        w: m.w,
        u: m.u,
        b: m.b,
        r: m.r,
        g: m.g,
        c: m.c,
    }
}

#[test]
fn q33_creature_mana_abilities_canary_simplest() {
    let case = &CASES[0];
    let mut e = engine(33_070_001, case.id);
    let source = move_ready_to_battlefield(&mut e, 0, case.id);
    give_mana(&mut e, 0, gift(&case.cost));
    apply_ability(&mut e, 0, source, 0, vec![]).expect("activate the mana ability");
    assert_eq!(pool(&e, 0), case.expected);
}

#[test]
fn q33_creature_mana_abilities_canary_hardest() {
    let case = &CASES[3];
    let mut e = engine(33_070_002, case.id);
    let source = move_ready_to_battlefield(&mut e, 0, case.id);
    apply_ability(&mut e, 0, source, 0, vec![]).expect("tap for black");
    assert!(e.state.objects[&source].tapped);
    assert_eq!(pool(&e, 0), case.expected);
}

#[test]
fn q33_creature_mana_abilities_cards_pay_the_printed_cost_and_produce_the_printed_color() {
    for (index, case) in CASES.iter().enumerate() {
        let mut e = engine(33_071_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.cost));
        apply_ability(&mut e, 0, source, 0, vec![])
            .unwrap_or_else(|error| panic!("{}: {error:?}", case.id));
        assert_eq!(pool(&e, 0), case.expected, "{}", case.id);
        if case.taps {
            assert!(e.state.objects[&source].tapped, "{}: tap cost", case.id);
            assert!(
                apply_ability(&mut e, 0, source, 0, vec![]).is_err(),
                "{}: the tap cost prevents a second activation",
                case.id
            );
        } else {
            assert!(!e.state.objects[&source].tapped, "{}: no tap cost", case.id);
        }
    }
}

#[test]
fn q33_creature_mana_abilities_cards_reject_foreign_controllers() {
    for (index, case) in CASES.iter().enumerate() {
        let mut e = engine(33_072_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.cost));
        e.apply_scenario_command(0, &pass())
            .expect("controller passes");
        assert!(
            apply_ability(&mut e, 1, source, 0, vec![]).is_err(),
            "{}: another player cannot activate it",
            case.id
        );
        assert_eq!(
            pool(&e, 0),
            (
                case.cost.w,
                case.cost.u,
                case.cost.b,
                case.cost.r,
                case.cost.g,
                case.cost.c
            ),
            "{}: the rejected activation spends nothing",
            case.id
        );
    }
}
