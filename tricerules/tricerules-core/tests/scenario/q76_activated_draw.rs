//! Actual-card scenarios for the queue-v76 03-activated-draw cohort.
//!
//! Expectations are printed Oracle values. CR 602.2 makes the ability a stack-using activated
//! ability and pays its costs, CR 118.3 requires the resources to pay them fully, and CR 121.1 draws.
//! A tap cost can be paid only once while the permanent remains tapped.
use super::helpers::*;

struct Case {
    id: &'static str,
    mana: ManaGift,
    taps: bool,
    draw: usize,
    body: Option<(u32, u32)>,
}

const CASES: [Case; 6] = [
    Case {
        id: "archivist",
        mana: ManaGift {
            w: 0,
            u: 0,
            b: 0,
            r: 0,
            g: 0,
            c: 0,
        },
        taps: true,
        draw: 1,
        body: Some((1, 1)),
    },
    Case {
        id: "scepter_of_insight",
        mana: ManaGift {
            w: 0,
            u: 1,
            b: 0,
            r: 0,
            g: 0,
            c: 3,
        },
        taps: true,
        draw: 1,
        body: None,
    },
    Case {
        id: "treasure_trove",
        mana: ManaGift {
            w: 0,
            u: 2,
            b: 0,
            r: 0,
            g: 0,
            c: 2,
        },
        taps: false,
        draw: 1,
        body: None,
    },
    Case {
        id: "third_path_savant",
        mana: ManaGift {
            w: 0,
            u: 0,
            b: 0,
            r: 0,
            g: 0,
            c: 7,
        },
        taps: false,
        draw: 2,
        body: Some((2, 3)),
    },
    Case {
        id: "tower_of_fortunes",
        mana: ManaGift {
            w: 0,
            u: 0,
            b: 0,
            r: 0,
            g: 0,
            c: 8,
        },
        taps: true,
        draw: 4,
        body: None,
    },
    Case {
        id: "benalish_heralds",
        mana: ManaGift {
            w: 0,
            u: 1,
            b: 0,
            r: 0,
            g: 0,
            c: 3,
        },
        taps: true,
        draw: 1,
        body: Some((2, 4)),
    },
];

fn pool(e: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[0].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

fn hand_size(e: &GameEngine) -> usize {
    e.state.players[0].hand.len()
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

fn activate(e: &mut GameEngine, permanent: u32) {
    semantic::accepted(e, 0, &activate_ability_for(e, permanent, 0, vec![]));
}

#[test]
fn q76_activated_draw_canary_simplest_archivist_draws_for_tap_only() {
    let case = &CASES[0];
    let mut e = semantic::main_phase(76_300_001);
    let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
    assert_eq!(
        (e.effective_power(source), e.effective_toughness(source)),
        (Some(1), Some(1)),
        "printed body"
    );
    let before = hand_size(&e);

    activate(&mut e, source);
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "no mana cost");
    assert!(e.state.objects[&source].tapped, "the tap cost is paid");
    assert_eq!(hand_size(&e), before, "the draw waits for resolution");
    assert_eq!(e.state.stack.len(), 1);

    resolve_entire_stack_two_player(&mut e);
    assert_eq!(hand_size(&e), before + 1, "exactly one card");
    assert!(e.state.stack.is_empty());

    assert!(
        e.apply_scenario_command(0, &activate_ability_for(&e, source, 0, vec![]))
            .is_err(),
        "a tapped Archivist cannot pay the tap cost again"
    );
    assert_eq!(
        hand_size(&e),
        before + 1,
        "the rejected activation draws nothing"
    );
}

#[test]
fn q76_activated_draw_canary_hardest_tower_of_fortunes_draws_four_for_eight() {
    let case = &CASES[4];
    let mut e = semantic::main_phase(76_300_002);
    let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
    let before = hand_size(&e);
    give_mana(&mut e, 0, gift(&case.mana));
    activate(&mut e, source);
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "exact printed {{8}} cost");
    assert!(e.state.objects[&source].tapped);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(hand_size(&e), before + 4, "exactly four cards");
}

#[test]
fn q76_activated_draw_all_cards_use_their_printed_costs_and_counts() {
    for (index, case) in CASES.iter().enumerate() {
        let mut e = semantic::main_phase(76_301_000 + index as u64);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        if let Some((power, toughness)) = case.body {
            assert_eq!(
                (e.effective_power(source), e.effective_toughness(source)),
                (Some(power), Some(toughness)),
                "{}: printed body",
                case.id
            );
        }
        let before = hand_size(&e);
        give_mana(&mut e, 0, gift(&case.mana));
        activate(&mut e, source);
        assert_eq!(
            pool(&e),
            (0, 0, 0, 0, 0, 0),
            "{}: exact printed cost",
            case.id
        );
        assert_eq!(
            e.state.objects[&source].tapped, case.taps,
            "{}: tap cost matches the printed cost",
            case.id
        );
        assert_eq!(hand_size(&e), before, "{}: draw is on resolution", case.id);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            hand_size(&e),
            before + case.draw,
            "{}: exactly the printed draw count",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}: stack is empty", case.id);
    }
}

#[test]
fn q76_activated_draw_rejects_underfunded_activations_atomically() {
    for (index, case) in CASES.iter().enumerate() {
        if case.id == "archivist" {
            continue; // Archivist has no mana component to underfund.
        }
        assert!(
            tricerules_cards::registry::global().get(case.id).is_some(),
            "{}: the actual card must be registered before a rejection proves anything",
            case.id
        );
        let mut e = semantic::main_phase(76_302_000 + index as u64);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        let before = hand_size(&e);
        let underfunded = ManaGift {
            w: case.mana.w.saturating_sub(1),
            u: case.mana.u.saturating_sub(1),
            b: case.mana.b.saturating_sub(1),
            r: case.mana.r.saturating_sub(1),
            g: case.mana.g.saturating_sub(1),
            c: case.mana.c.saturating_sub(1),
        };
        give_mana(&mut e, 0, underfunded);
        let funded_pool = pool(&e);
        assert!(
            e.apply_scenario_command(0, &activate_ability_for(&e, source, 0, vec![]))
                .is_err(),
            "{}: an underfunded activation is rejected",
            case.id
        );
        assert_eq!(
            pool(&e),
            funded_pool,
            "{}: the rejected activation spends no mana",
            case.id
        );
        assert!(
            !e.state.objects[&source].tapped,
            "{}: no tap cost was paid",
            case.id
        );
        assert_eq!(hand_size(&e), before, "{}: no card was drawn", case.id);
        assert!(
            e.state.stack.is_empty(),
            "{}: nothing was put on the stack",
            case.id
        );
    }
}

#[test]
fn q76_activated_draw_repeatable_abilities_do_not_tap() {
    for (index, case) in CASES
        .iter()
        .enumerate()
        .filter(|(_, case)| !case.taps && case.id != "archivist")
    {
        let mut e = semantic::main_phase(76_303_000 + index as u64);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        let before = hand_size(&e);
        for _ in 0..2 {
            give_mana(&mut e, 0, gift(&case.mana));
            activate(&mut e, source);
            resolve_entire_stack_two_player(&mut e);
        }
        assert!(!e.state.objects[&source].tapped, "{}: never taps", case.id);
        assert_eq!(
            hand_size(&e),
            before + case.draw * 2,
            "{}: the ability can be activated again",
            case.id
        );
    }
}
