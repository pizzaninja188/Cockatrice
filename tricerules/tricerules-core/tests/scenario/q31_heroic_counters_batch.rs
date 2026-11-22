//! Actual-card scenarios for the queue-v31 heroic counter cohort.
//!
//! Expectations are printed Oracle values. CR 603.2/115.9b trigger heroic when a
//! spell is cast with this creature among its targets; CR 603.2c produces one
//! trigger per spell even if the same creature is named more than once; CR 122
//! places the printed number of +1/+1 counters.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::CounterKind;
use tricerules_core::Zone;

fn put_card(e: &mut GameEngine, player: i32, card_name: &str, card_id: &str) -> u32 {
    use tricerules_proto::ruled::v1::dev_command::Dev;
    use tricerules_proto::ruled::v1::{DevCommand, DevPutCardInZone, DevZone, RuledCommand};
    e.enable_dev_commands();
    e.apply_scenario_command(
        player,
        &RuledCommand {
            cmd: Some(tricerules_proto::ruled::v1::ruled_command::Cmd::DevCommand(
                DevCommand {
                    target_player_id: player,
                    dev: Some(Dev::PutCardInZone(DevPutCardInZone {
                        card_name: card_name.to_string(),
                        zone: DevZone::Battlefield as i32,
                        ready: true,
                    })),
                },
            )),
        },
    )
    .unwrap_or_else(|error| panic!("put {card_name}: {error:?}"));
    battlefield_object_for_card(e, player as usize, card_id)
}

struct Case {
    id: &'static str,
    name: &'static str,
    mana: ManaGift,
    body: (u32, u32),
    counters: u32,
    flying: bool,
}

fn cases() -> [Case; 6] {
    [
        Case {
            id: "lagonna-band_trailblazer",
            name: "Lagonna-Band Trailblazer",
            mana: ManaGift {
                w: 1,
                ..Default::default()
            },
            body: (0, 4),
            counters: 1,
            flying: false,
        },
        Case {
            id: "akroan_skyguard",
            name: "Akroan Skyguard",
            mana: ManaGift {
                w: 1,
                c: 1,
                ..Default::default()
            },
            body: (1, 1),
            counters: 1,
            flying: true,
        },
        Case {
            id: "wingsteed_rider",
            name: "Wingsteed Rider",
            mana: ManaGift {
                w: 2,
                c: 1,
                ..Default::default()
            },
            body: (2, 2),
            counters: 1,
            flying: true,
        },
        Case {
            id: "pheres-band_thunderhoof",
            name: "Pheres-Band Thunderhoof",
            mana: ManaGift {
                g: 1,
                c: 4,
                ..Default::default()
            },
            body: (3, 4),
            counters: 2,
            flying: false,
        },
        Case {
            id: "setessan_oathsworn",
            name: "Setessan Oathsworn",
            mana: ManaGift {
                g: 2,
                c: 1,
                ..Default::default()
            },
            body: (1, 1),
            counters: 2,
            flying: false,
        },
        Case {
            id: "staunch-hearted_warrior",
            name: "Staunch-Hearted Warrior",
            mana: ManaGift {
                g: 1,
                c: 3,
                ..Default::default()
            },
            body: (2, 2),
            counters: 2,
            flying: false,
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
    let hand = hand_index_for_card(e, caster as usize, "giant_growth");
    e.apply_scenario_command(caster, &cast_spell(hand, target_object(card)))
        .unwrap_or_else(|error| panic!("cast Giant Growth: {error:?}"));
}

#[test]
fn q31_heroic_canary_simplest() {
    let case = &cases()[0];
    let mut e = engine(31_060_001);
    let hero = put_card(&mut e, 0, case.name, case.id);
    give_mana(
        &mut e,
        0,
        ManaGift {
            g: 1,
            ..Default::default()
        },
    );
    cast_pump_at(&mut e, 0, hero);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(counter_count(&e, hero), 1);
    assert_eq!(e.effective_power(hero), Some(4));
    assert_eq!(e.effective_toughness(hero), Some(8));
}

#[test]
fn q31_heroic_canary_hardest() {
    let case = &cases()[3];
    let mut e = engine(31_060_002);
    let hero = put_card(&mut e, 0, case.name, case.id);
    give_mana(
        &mut e,
        0,
        ManaGift {
            g: 1,
            ..Default::default()
        },
    );
    cast_pump_at(&mut e, 0, hero);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(counter_count(&e, hero), 2);
    assert_eq!(e.effective_power(hero), Some(8));
    assert_eq!(e.effective_toughness(hero), Some(9));
}

#[test]
fn q31_heroic_cards_place_their_printed_counter_count() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(31_061_000 + index as u64);
        let hero = put_card(&mut e, 0, case.name, case.id);
        give_mana(
            &mut e,
            0,
            ManaGift {
                g: 1,
                ..Default::default()
            },
        );
        cast_pump_at(&mut e, 0, hero);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            counter_count(&e, hero),
            case.counters,
            "{}: printed counters",
            case.id
        );
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
fn q31_heroic_cards_keep_their_printed_flying_and_body() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(31_062_000 + index as u64);
        let hero = put_card(&mut e, 0, case.name, case.id);
        assert_eq!(e.effective_power(hero), Some(case.body.0), "{}", case.id);
        assert_eq!(
            e.effective_toughness(hero),
            Some(case.body.1),
            "{}",
            case.id
        );
        assert_eq!(
            e.effective_has_keyword(hero, tricerules_cards::Keyword::Flying),
            case.flying,
            "{}",
            case.id
        );
    }
}

#[test]
fn q31_heroic_does_not_trigger_for_a_spell_that_does_not_target_it() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(31_063_000 + index as u64);
        let hero = put_card(&mut e, 0, case.name, case.id);
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
fn q31_heroic_does_not_trigger_for_an_opponents_targeting_spell() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(31_064_000 + index as u64);
        let hero = put_card(&mut e, 0, case.name, case.id);
        give_mana(
            &mut e,
            1,
            ManaGift {
                g: 1,
                ..Default::default()
            },
        );
        // Active player 0 passes; player 1 then casts the targeting spell.
        e.apply_scenario_command(0, &pass()).expect("pass priority");
        cast_pump_at(&mut e, 1, hero);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(counter_count(&e, hero), 0, "{}", case.id);
        assert_eq!(
            e.effective_power(hero),
            Some(case.body.0 + 3),
            "{}: Giant Growth resolves without heroic",
            case.id
        );
    }
}

#[test]
fn q31_heroic_cards_enter_through_their_exact_mana_cost() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(31_065_000 + index as u64);
        inject_card_into_hand(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.mana));
        let slot = hand_index_for_card(&e, 0, case.id);
        e.apply_scenario_command(0, &cast_spell(slot, vec![]))
            .unwrap_or_else(|error| panic!("{}: {error:?}", case.id));
        resolve_entire_stack_two_player(&mut e);
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
            "{}",
            case.id
        );
        assert_eq!(
            e.state.objects[&battlefield_object_for_card(&e, 0, case.id)].zone,
            Zone::Battlefield
        );
    }
}
