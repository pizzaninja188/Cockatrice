//! Actual-card scenarios for the queue-v38 creature-and-player sweep cohort.
//!
//! Expectations are printed Oracle values. CR 120.2/120.3 deal damage simultaneously to each
//! creature and each player; CR 119.3 makes every player lose the printed life; CR 704.5g sends
//! creatures with lethal damage to the graveyard; CR 514.2 clears marked damage at cleanup.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;

struct Case {
    id: &'static str,
    mana: ManaGift,
    amount: u32,
}

fn cases() -> [Case; 6] {
    [
        Case {
            id: "dakmor_plague",
            mana: ManaGift {
                b: 2,
                c: 3,
                ..Default::default()
            },
            amount: 3,
        },
        Case {
            id: "dry_spell",
            mana: ManaGift {
                b: 1,
                c: 1,
                ..Default::default()
            },
            amount: 1,
        },
        Case {
            id: "famine",
            mana: ManaGift {
                b: 2,
                c: 3,
                ..Default::default()
            },
            amount: 3,
        },
        Case {
            id: "fire_tempest",
            mana: ManaGift {
                r: 2,
                c: 5,
                ..Default::default()
            },
            amount: 6,
        },
        Case {
            id: "rain_of_embers",
            mana: ManaGift {
                r: 1,
                c: 1,
                ..Default::default()
            },
            amount: 1,
        },
        Case {
            id: "steam_blast",
            mana: ManaGift {
                r: 1,
                c: 2,
                ..Default::default()
            },
            amount: 2,
        },
    ]
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

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[]), deck_with("forest", &[])]);
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

fn cast_sweep(e: &mut GameEngine, case: &Case) {
    inject_card_into_hand(e, 0, case.id);
    give_mana(e, 0, gift(&case.mana));
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .unwrap_or_else(|error| panic!("cast {}: {error:?}", case.id));
    assert_eq!(
        {
            let p = &e.state.players[0].mana_pool;
            (p.white, p.blue, p.black, p.red, p.green, p.colorless)
        },
        (0, 0, 0, 0, 0, 0),
        "{}: exact cost",
        case.id
    );
    resolve_entire_stack_two_player(e);
}

#[test]
fn q38_creature_player_sweep_canary_simplest() {
    let case = &cases()[1];
    let mut e = engine(38_010_001);
    let own = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
    let theirs = inject_creature_with_stats(&mut e, 1, "savannah_lions", 2, 1);
    cast_sweep(&mut e, case);
    assert_eq!(
        e.state.objects[&own].damage, 1,
        "own creature takes the printed 1"
    );
    assert_eq!(
        e.state.objects[&theirs].zone,
        tricerules_core::Zone::Graveyard
    );
    assert_eq!(
        e.state.players[0].life, 19,
        "the controller also loses 1 life"
    );
    assert_eq!(e.state.players[1].life, 19);
}

#[test]
fn q38_creature_player_sweep_canary_hardest() {
    let case = &cases()[3];
    let mut e = engine(38_010_002);
    let survivor = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 7, 7);
    let victim = inject_creature_with_stats(&mut e, 1, "savannah_lions", 5, 5);
    cast_sweep(&mut e, case);
    assert_eq!(
        e.state.objects[&survivor].zone,
        tricerules_core::Zone::Battlefield
    );
    assert_eq!(e.state.objects[&survivor].damage, 6);
    assert_eq!(
        e.state.objects[&victim].zone,
        tricerules_core::Zone::Graveyard
    );
    assert_eq!(e.state.players[0].life, 14, "six damage to each player");
    assert_eq!(e.state.players[1].life, 14);
}

#[test]
fn q38_creature_player_sweep_cards_deal_the_printed_amount_to_each_creature_and_player() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(38_011_000 + index as u64);
        let own = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 7, 7);
        let theirs = inject_creature_with_stats(&mut e, 1, "savannah_lions", 7, 7);
        let tiny = inject_creature_with_stats(&mut e, 0, "savannah_lions", 0, 1);
        cast_sweep(&mut e, case);
        assert_eq!(
            e.state.objects[&own].damage, case.amount,
            "{}: own creature",
            case.id
        );
        assert_eq!(
            e.state.objects[&theirs].damage, case.amount,
            "{}: opponent creature",
            case.id
        );
        assert_eq!(
            e.state.objects[&tiny].zone,
            tricerules_core::Zone::Graveyard,
            "{}: lethal damage",
            case.id
        );
        assert_eq!(
            e.state.players[0].life,
            20 - case.amount as i32,
            "{}: controller loses the printed life",
            case.id
        );
        assert_eq!(
            e.state.players[1].life,
            20 - case.amount as i32,
            "{}: opponent loses the printed life",
            case.id
        );
    }
}

#[test]
fn q38_creature_player_sweep_cards_mark_damage_until_cleanup_without_targets() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(38_012_000 + index as u64);
        let own = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 7, 7);
        cast_sweep(&mut e, case);
        assert_eq!(e.state.objects[&own].damage, case.amount, "{}", case.id);
        end_active_turn(&mut e, 0);
        assert_eq!(
            e.state.objects[&own].damage, 0,
            "{}: marked damage clears at cleanup",
            case.id
        );
    }
}
