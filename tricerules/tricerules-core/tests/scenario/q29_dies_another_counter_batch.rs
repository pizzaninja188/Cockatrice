//! Queue-v29 canaries: "Whenever another creature you control dies, put a +1/+1 counter on this
//! creature."
//!
//! CR 700.4, 603.6c/603.10a and 122.1. Expectations are printed Oracle values.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::CounterKind;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    mana: ManaGift,
    power: u32,
    toughness: u32,
}

fn case(id: &'static str) -> Case {
    match id {
        "unruly_mob" => Case {
            id,
            mana: ManaGift {
                w: 1,
                c: 1,
                ..Default::default()
            },
            power: 1,
            toughness: 1,
        },
        "gavony_unhallowed" => Case {
            id,
            mana: ManaGift {
                b: 1,
                c: 3,
                ..Default::default()
            },
            power: 2,
            toughness: 4,
        },
        "rot_shambler" => Case {
            id,
            mana: ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
            power: 1,
            toughness: 1,
        },
        _ => unreachable!(),
    }
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

fn cases() -> [Case; 3] {
    [
        case("unruly_mob"),
        case("rot_shambler"),
        case("gavony_unhallowed"),
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("plains", &[card, "murder"]),
        deck_with("forest", &["grizzly_bears"]),
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

fn cast_source(e: &mut GameEngine, case: &Case) -> u32 {
    give_mana(e, 0, gift(&case.mana));
    inject_card_into_hand(e, 0, case.id);
    let hand = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(hand, vec![]))
        .unwrap_or_else(|error| panic!("cast {}: {error:?}", case.id));
    resolve_entire_stack_two_player(e);
    battlefield_object_for_card(e, 0, case.id)
}

fn kill_own_creature(e: &mut GameEngine, victim: u32) {
    give_mana(
        e,
        0,
        ManaGift {
            b: 2,
            c: 1,
            ..Default::default()
        },
    );
    inject_card_into_hand(e, 0, "murder");
    let hand = hand_index_for_card(e, 0, "murder");
    e.apply_scenario_command(0, &cast_spell(hand, target_object(victim)))
        .expect("cast Murder on the other creature");
    resolve_entire_stack_two_player(e);
}

fn assert_canary(case: Case, seed: u64) {
    let mut e = engine(seed, case.id);
    let source = cast_source(&mut e, &case);
    let victim = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    kill_own_creature(&mut e, victim);
    assert_eq!(e.state.objects[&victim].zone, Zone::Graveyard);
    assert_eq!(
        e.state.objects[&source].counter_count(CounterKind::PlusOnePlusOne),
        1,
        "{}: one counter",
        case.id
    );
    assert_eq!(
        (e.effective_power(source), e.effective_toughness(source)),
        (Some(case.power + 1), Some(case.toughness + 1)),
        "{}: the counter modifies the printed body",
        case.id
    );
    assert!(e.state.stack.is_empty(), "{}", case.id);
}

#[test]
fn q29_dies_another_counter_canary_simplest() {
    assert_canary(case("unruly_mob"), 29_020_001);
}

#[test]
fn q29_dies_another_counter_canary_hardest() {
    assert_canary(case("gavony_unhallowed"), 29_020_002);
}

#[test]
fn q29_dies_another_counter_cards_cast_with_exact_cost_and_body() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(29_021_000 + index as u64, case.id);
        let source = cast_source(&mut e, &case);
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(case.power), Some(case.toughness)),
            "{}: printed body",
            case.id
        );
        assert_eq!(
            e.state.objects[&source].counter_count(CounterKind::PlusOnePlusOne),
            0,
            "{}: no counter without a death",
            case.id
        );
    }
}

#[test]
fn q29_dies_another_counter_cards_ignore_opposing_deaths() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(29_022_000 + index as u64, case.id);
        let source = cast_source(&mut e, &case);
        let opponent = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        kill_own_creature(&mut e, opponent);
        assert_eq!(
            e.state.objects[&source].counter_count(CounterKind::PlusOnePlusOne),
            0,
            "{}: only creatures you control count",
            case.id
        );
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(case.power), Some(case.toughness)),
            "{}: no pump",
            case.id
        );
    }
}

#[test]
fn q29_dies_another_counter_cards_count_each_other_death() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(29_023_000 + index as u64, case.id);
        let source = cast_source(&mut e, &case);
        let first = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        let second = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        kill_own_creature(&mut e, first);
        kill_own_creature(&mut e, second);
        assert_eq!(
            e.state.objects[&source].counter_count(CounterKind::PlusOnePlusOne),
            2,
            "{}: one trigger per other creature death",
            case.id
        );
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(case.power + 2), Some(case.toughness + 2)),
            "{}: two counters",
            case.id
        );
    }
}

#[test]
fn q29_dies_another_counter_cards_do_not_trigger_for_their_own_death() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(29_024_000 + index as u64, case.id);
        let source = cast_source(&mut e, &case);
        kill_own_creature(&mut e, source);
        assert_eq!(e.state.objects[&source].zone, Zone::Graveyard);
        assert!(
            e.state.pending_triggers.is_empty(),
            "{}: another-creature wording excludes the source",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn q29_dies_another_counter_simultaneous_death_does_not_save_the_source() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(29_025_000 + index as u64, case.id);
        let source = cast_source(&mut e, &case);
        let other = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        give_mana(
            &mut e,
            0,
            ManaGift {
                w: 2,
                c: 2,
                ..Default::default()
            },
        );
        inject_card_into_hand(&mut e, 0, "wrath_of_god");
        let hand = hand_index_for_card(&e, 0, "wrath_of_god");
        e.apply_scenario_command(0, &cast_spell(hand, vec![]))
            .expect("cast Wrath of God");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&source].zone,
            Zone::Graveyard,
            "{}",
            case.id
        );
        assert_eq!(e.state.objects[&other].zone, Zone::Graveyard, "{}", case.id);
        assert_eq!(
            e.state.objects[&source].counter_count(CounterKind::PlusOnePlusOne),
            0,
            "{}: the trigger cannot add a counter after the source died",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}
