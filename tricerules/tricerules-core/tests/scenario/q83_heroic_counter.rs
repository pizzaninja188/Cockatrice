//! Actual-card scenarios for the queue-v83 02-heroic-counter cohort.
//!
//! Every card has the Heroic ability "Whenever you cast a spell that targets this creature, put a
//! +1/+1 counter on this creature" plus printed keywords. Expectations are printed Oracle values.
//! CR 603.2/115.9b stage the trigger, CR 603.2c allows one trigger per spell, and CR 122.1 places
//! the printed counter. The pinned 2014-04-26 and 2013-09-15 rulings confirm the trigger resolves
//! before the targeting spell and does not fire for a copied spell or a changed target.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::{CounterKind, Keyword};

struct Case {
    id: &'static str,
    mana: ManaGift,
    body: (u32, u32),
    counters: u32,
    keywords: &'static [Keyword],
}

fn cases() -> [Case; 4] {
    [
        Case {
            id: "satyr_hoplite",
            mana: ManaGift {
                r: 1,
                ..Default::default()
            },
            body: (1, 1),
            counters: 1,
            keywords: &[],
        },
        Case {
            id: "war-wing_siren",
            mana: ManaGift {
                u: 1,
                c: 2,
                ..Default::default()
            },
            body: (1, 3),
            counters: 1,
            keywords: &[Keyword::Flying],
        },
        Case {
            id: "dawnbringer_charioteers",
            mana: ManaGift {
                w: 2,
                c: 2,
                ..Default::default()
            },
            body: (2, 4),
            counters: 1,
            keywords: &[Keyword::Flying, Keyword::Lifelink],
        },
        Case {
            id: "fabled_hero",
            mana: ManaGift {
                w: 2,
                c: 1,
                ..Default::default()
            },
            body: (2, 2),
            counters: 1,
            keywords: &[Keyword::DoubleStrike],
        },
    ]
}

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![
        deck_with("forest", &["giant_growth", "lightning_bolt"]),
        deck_with("forest", &["giant_growth"]),
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

fn counter_count(e: &GameEngine, card: u32) -> u32 {
    e.state.objects[&card]
        .counters
        .get(&CounterKind::PlusOnePlusOne)
        .copied()
        .unwrap_or(0)
}

fn cast_pump_at(e: &mut GameEngine, caster: i32, card: u32) {
    ensure_card_in_hand(e, caster as usize, "giant_growth");
    give_mana(
        e,
        caster,
        ManaGift {
            g: 1,
            ..Default::default()
        },
    );
    let hand = hand_index_for_card(e, caster as usize, "giant_growth");
    e.apply_scenario_command(caster, &cast_spell(hand, target_object(card)))
        .unwrap_or_else(|error| panic!("cast Giant Growth: {error:?}"));
}

fn cast_hero(e: &mut GameEngine, case: &Case) -> u32 {
    inject_card_into_hand(e, 0, case.id);
    give_mana(e, 0, gift(&case.mana));
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .unwrap_or_else(|error| panic!("{}: cast: {error:?}", case.id));
    resolve_entire_stack_two_player(e);
    let pool = &e.state.players[0].mana_pool;
    assert_eq!(
        (
            pool.white,
            pool.blue,
            pool.black,
            pool.red,
            pool.green,
            pool.colorless
        ),
        (0, 0, 0, 0, 0, 0),
        "{}: exact cast cost",
        case.id
    );
    battlefield_object_for_card(e, 0, case.id)
}

#[test]
fn q83_heroic_canary_simplest_satyr_hoplite() {
    let case = &cases()[0];
    let mut e = engine(83_020_001);
    let hero = cast_hero(&mut e, case);
    assert_eq!(
        (e.effective_power(hero), e.effective_toughness(hero)),
        (Some(1), Some(1))
    );
    cast_pump_at(&mut e, 0, hero);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(counter_count(&e, hero), 1);
    assert_eq!(e.effective_power(hero), Some(1 + 1 + 3));
    assert_eq!(e.effective_toughness(hero), Some(1 + 1 + 3));
}

#[test]
fn q83_heroic_canary_hardest_dawnbringer_charioteers() {
    let case = &cases()[2];
    let mut e = engine(83_021_001);
    let hero = cast_hero(&mut e, case);
    assert!(e.effective_has_keyword(hero, Keyword::Flying));
    assert!(e.effective_has_keyword(hero, Keyword::Lifelink));
    cast_pump_at(&mut e, 0, hero);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(counter_count(&e, hero), 1);
    assert_eq!(e.effective_power(hero), Some(2 + 1 + 3));
    assert_eq!(e.effective_toughness(hero), Some(4 + 1 + 3));
}

#[test]
fn q83_heroic_cards_place_their_printed_counter_and_keep_their_body() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(83_022_000 + index as u64);
        let hero = cast_hero(&mut e, &case);
        assert_eq!(
            (e.effective_power(hero), e.effective_toughness(hero)),
            (Some(case.body.0), Some(case.body.1)),
            "{}: printed body",
            case.id
        );
        for keyword in case.keywords {
            assert!(
                e.effective_has_keyword(hero, *keyword),
                "{}: printed {keyword:?}",
                case.id
            );
        }
        cast_pump_at(&mut e, 0, hero);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(counter_count(&e, hero), case.counters, "{}", case.id);
        assert_eq!(
            e.effective_power(hero),
            Some(case.body.0 + case.counters + 3),
            "{}: counter then Giant Growth",
            case.id
        );
        assert_eq!(
            e.effective_toughness(hero),
            Some(case.body.1 + case.counters + 3),
            "{}",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn q83_heroic_does_not_trigger_for_a_spell_that_does_not_target_it() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(83_023_000 + index as u64);
        let hero = cast_hero(&mut e, &case);
        give_mana(
            &mut e,
            0,
            ManaGift {
                r: 1,
                ..Default::default()
            },
        );
        ensure_card_in_hand(&mut e, 0, "lightning_bolt");
        let bolt = hand_index_for_card(&e, 0, "lightning_bolt");
        e.apply_scenario_command(0, &cast_spell(bolt, target_player(1)))
            .expect("cast at the opponent");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(counter_count(&e, hero), 0, "{}", case.id);
        assert_eq!(e.effective_power(hero), Some(case.body.0), "{}", case.id);
    }
}

#[test]
fn q83_heroic_does_not_trigger_for_an_opponents_targeting_spell() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(83_024_000 + index as u64);
        let hero = cast_hero(&mut e, &case);
        e.apply_scenario_command(0, &pass()).expect("pass priority");
        cast_pump_at(&mut e, 1, hero);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(counter_count(&e, hero), 0, "{}", case.id);
        assert_eq!(
            e.effective_power(hero),
            Some(case.body.0 + 3),
            "{}: Giant Growth still resolves",
            case.id
        );
    }
}
