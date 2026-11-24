//! Actual-card scenarios for the queue-v78 01-ritual-mana cohort.
//!
//! Expectations are printed Oracle values. CR 601.2 puts these spells on the stack; CR 106.1a
//! and 106.4 add exactly the printed mana on resolution; CR 500.4 empties unused mana at the end
//! of the step. The pinned 2010-08-15 Pyretic Ritual rulings confirm a ritual is not a mana
//! ability and can be responded to or countered.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    mana: ManaGift,
    expected: (u32, u32, u32, u32, u32, u32),
}

fn cases() -> [Case; 4] {
    [
        Case {
            id: "dark_ritual",
            mana: ManaGift {
                b: 1,
                ..Default::default()
            },
            expected: (0, 0, 3, 0, 0, 0),
        },
        Case {
            id: "pyretic_ritual",
            mana: ManaGift {
                r: 1,
                c: 1,
                ..Default::default()
            },
            expected: (0, 0, 0, 3, 0, 0),
        },
        Case {
            id: "seething_song",
            mana: ManaGift {
                r: 1,
                c: 2,
                ..Default::default()
            },
            expected: (0, 0, 0, 5, 0, 0),
        },
        Case {
            id: "channel_the_suns",
            mana: ManaGift {
                g: 1,
                c: 3,
                ..Default::default()
            },
            expected: (1, 1, 1, 1, 1, 0),
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

fn pool(e: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[0].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("swamp", &[]), deck_with("swamp", &[])]);
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

fn cast_ritual(e: &mut GameEngine, case: &Case) -> Result<u32, String> {
    inject_card_into_hand(e, 0, case.id);
    give_mana(e, 0, gift(&case.mana));
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .map(|_| e.state.stack.last().expect("spell").id)
        .map_err(|error| format!("{error:?}"))
}

#[test]
fn q78_ritual_mana_canary_simplest_dark_ritual_adds_three_black_on_resolution() {
    let case = &cases()[0];
    let mut e = engine(78_100_001);
    let spell = cast_ritual(&mut e, case).expect("cast Dark Ritual");
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "the printed {{B}} is spent");
    assert_eq!(e.state.stack.len(), 1, "the ritual uses the stack");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(pool(&e), case.expected, "exactly {{B}}{{B}}{{B}} is added");
    assert_eq!(e.state.objects[&spell].zone, Zone::Graveyard);
    assert!(e.state.stack.is_empty());
}

#[test]
fn q78_ritual_mana_canary_hardest_channel_the_suns_adds_one_of_each_color() {
    let case = &cases()[3];
    let mut e = engine(78_100_002);
    let spell = cast_ritual(&mut e, case).expect("cast Channel the Suns");
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "the printed cost is spent");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        pool(&e),
        case.expected,
        "exactly {{W}}{{U}}{{B}}{{R}}{{G}} is added"
    );
    assert_eq!(e.state.objects[&spell].zone, Zone::Graveyard);
}

#[test]
fn q78_ritual_mana_cards_add_only_their_printed_mana() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(78_101_000 + index as u64);
        let spell =
            cast_ritual(&mut e, &case).unwrap_or_else(|error| panic!("{}: {error}", case.id));
        assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "{}: cost paid", case.id);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(pool(&e), case.expected, "{}: printed output", case.id);
        assert_eq!(e.state.objects[&spell].zone, Zone::Graveyard, "{}", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn q78_ritual_mana_rejects_underfunded_casts_without_spending_mana() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(78_102_000 + index as u64);
        inject_card_into_hand(&mut e, 0, case.id);
        let underfunded = ManaGift {
            w: case.mana.w.saturating_sub(1),
            u: case.mana.u.saturating_sub(1),
            b: case.mana.b.saturating_sub(1),
            r: case.mana.r.saturating_sub(1),
            g: case.mana.g.saturating_sub(1),
            c: case.mana.c.saturating_sub(1),
        };
        give_mana(&mut e, 0, underfunded);
        let before = pool(&e);
        let hand_before = e.state.players[0].hand.len();
        let slot = hand_index_for_card(&e, 0, case.id);
        assert!(
            e.apply_scenario_command(0, &cast_spell(slot, vec![]))
                .is_err(),
            "{}: an underfunded cast is rejected",
            case.id
        );
        assert_eq!(pool(&e), before, "{}: no mana is spent", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);
        assert_eq!(
            e.state.players[0].hand.len(),
            hand_before,
            "{}: the card stays in hand",
            case.id
        );
    }
}

#[test]
fn q78_ritual_mana_empties_at_the_end_of_the_step() {
    let case = &cases()[0];
    let mut e = engine(78_103_000);
    cast_ritual(&mut e, case).expect("cast Dark Ritual");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(pool(&e), (0, 0, 3, 0, 0, 0), "mana is in the pool");
    e.apply_scenario_command(0, &primitive_yield())
        .expect("main1 to begin combat");
    assert_eq!(
        pool(&e),
        (0, 0, 0, 0, 0, 0),
        "CR 500.4 empties unused mana when the step ends"
    );
}

#[test]
fn q78_ritual_mana_pyretic_ritual_is_countered_and_adds_no_mana() {
    let case = &cases()[1];
    let mut e = engine(78_104_000);
    let ritual = cast_ritual(&mut e, case).expect("cast Pyretic Ritual");
    inject_card_into_hand(&mut e, 0, "counterspell");
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 0, "counterspell");
    e.apply_scenario_command(0, &cast_spell(slot, target_object(ritual)))
        .expect("counter the ritual");
    assert_eq!(e.state.stack.len(), 2, "both spells are on the stack");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        pool(&e),
        (0, 0, 0, 0, 0, 0),
        "the countered ritual adds no mana"
    );
    assert_eq!(e.state.objects[&ritual].zone, Zone::Graveyard);
    assert!(e.state.players[0].graveyard.contains(&ritual));
    assert!(e.state.stack.is_empty());
}
