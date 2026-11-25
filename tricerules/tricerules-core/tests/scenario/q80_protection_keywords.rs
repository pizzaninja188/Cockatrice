//! Actual-card scenarios for the queue-v80 02-protection-keywords cohort.
//!
//! Expectations are printed Oracle values. CR 702.9a-b (flying), 702.7 (first strike),
//! 702.15 (lifelink), 702.19 (trample), 702.10 (haste), 702.20 (vigilance), and 702.16b-f
//! (protection's five effects) govern. The 2004-10-04 ruling defines "protection from each
//! color" as five separate color protections.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::primitives::ProtectionQuality;
use tricerules_cards::{Color, Keyword};
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    mana: &'static str,
    power: u32,
    toughness: u32,
    keywords: &'static [Keyword],
    protections: &'static [Color],
}

const CASES: [Case; 5] = [
    Case {
        id: "iridescent_angel",
        mana: "{5}{W}{U}",
        power: 4,
        toughness: 4,
        keywords: &[Keyword::Flying],
        protections: &[
            Color::White,
            Color::Blue,
            Color::Black,
            Color::Red,
            Color::Green,
        ],
    },
    Case {
        id: "oversoul_of_dusk",
        mana: "{G/W}{G/W}{G/W}{G/W}{G/W}",
        power: 5,
        toughness: 5,
        keywords: &[],
        protections: &[Color::Blue, Color::Black, Color::Red],
    },
    Case {
        id: "sabertooth_nishoba",
        mana: "{4}{G}{W}",
        power: 5,
        toughness: 5,
        keywords: &[Keyword::Trample],
        protections: &[Color::Blue, Color::Red],
    },
    Case {
        id: "sphinx_of_the_steel_wind",
        mana: "{5}{W}{U}{B}",
        power: 6,
        toughness: 6,
        keywords: &[
            Keyword::Flying,
            Keyword::FirstStrike,
            Keyword::Vigilance,
            Keyword::Lifelink,
        ],
        protections: &[Color::Red, Color::Green],
    },
    Case {
        id: "akroma,_angel_of_wrath",
        mana: "{5}{W}{W}{W}",
        power: 6,
        toughness: 6,
        keywords: &[
            Keyword::Flying,
            Keyword::FirstStrike,
            Keyword::Vigilance,
            Keyword::Trample,
            Keyword::Haste,
        ],
        protections: &[Color::Black, Color::Red],
    },
];

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("plains", &[]), deck_with("island", &[])]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn gift_for(cost: &str) -> ManaGift {
    let mut m = ManaGift::default();
    for symbol in cost.split('}') {
        let Some(inner) = symbol.split('{').nth(1) else {
            continue;
        };
        for color in inner.chars() {
            match color {
                'W' => m.w += 1,
                'U' => m.u += 1,
                'B' => m.b += 1,
                'R' => m.r += 1,
                'G' => m.g += 1,
                _ => {}
            }
        }
        if let Ok(count) = inner.parse::<u32>() {
            m.c += count;
        }
    }
    m
}

fn cast_card(engine: &mut GameEngine, case: &Case) -> u32 {
    let object = inject_card_into_hand(engine, 0, case.id);
    give_mana(engine, 0, gift_for(case.mana));
    let slot = hand_index_for_card(engine, 0, case.id);
    engine
        .apply_scenario_command(0, &cast_spell(slot, vec![]))
        .unwrap_or_else(|error| panic!("cast {}: {error:?}", case.id));
    resolve_entire_stack_two_player(engine);
    assert_eq!(
        engine.state.objects[&object].zone,
        Zone::Battlefield,
        "{} resolved",
        case.id
    );
    object
}

fn protected_target_spell(color: Color) -> (&'static str, ManaGift) {
    match color {
        Color::White => (
            "swords_to_plowshares",
            ManaGift {
                w: 1,
                ..Default::default()
            },
        ),
        Color::Blue => (
            "unsummon",
            ManaGift {
                u: 1,
                ..Default::default()
            },
        ),
        Color::Black => (
            "murder",
            ManaGift {
                b: 2,
                c: 1,
                ..Default::default()
            },
        ),
        Color::Red => (
            "lightning_bolt",
            ManaGift {
                r: 1,
                ..Default::default()
            },
        ),
        Color::Green => (
            "giant_growth",
            ManaGift {
                g: 1,
                ..Default::default()
            },
        ),
    }
}

fn protection_qualities(case: &Case) -> Vec<ProtectionQuality> {
    case.protections
        .iter()
        .copied()
        .map(ProtectionQuality::Color)
        .collect()
}

#[test]
fn q80_protection_canary_simplest_iridescent_angel_has_each_color_protection() {
    let mut e = engine(80_200_001);
    let case = &CASES[0];
    let angel = cast_card(&mut e, case);
    assert_eq!(e.effective_power(angel), Some(4));
    assert_eq!(e.effective_toughness(angel), Some(4));
    let characteristics = e.characteristics(angel).expect("characteristics");
    assert_eq!(characteristics.protections, protection_qualities(case));
    assert!(characteristics.has_keyword(Keyword::Flying));

    inject_card_into_hand(&mut e, 0, "murder");
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 2,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 0, "murder");
    assert!(
        e.apply_scenario_command(0, &cast_spell(slot, target_object(angel)))
            .is_err(),
        "a black spell cannot target Iridescent Angel"
    );
    assert!(e.state.stack.is_empty());
    assert_eq!(e.state.objects[&angel].zone, Zone::Battlefield);
}

#[test]
fn q80_protection_canary_hardest_akroma_keeps_keywords_and_selective_protection() {
    let mut e = engine(80_200_002);
    let case = &CASES[4];
    let akroma = cast_card(&mut e, case);
    let characteristics = e.characteristics(akroma).expect("characteristics");
    assert_eq!(
        (characteristics.power, characteristics.toughness),
        (Some(6), Some(6))
    );
    for keyword in case.keywords {
        assert!(
            characteristics.has_keyword(*keyword),
            "Akroma has {keyword:?}"
        );
    }
    assert_eq!(characteristics.protections, protection_qualities(case));

    inject_card_into_hand(&mut e, 0, "lightning_bolt");
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 0, "lightning_bolt");
    assert!(
        e.apply_scenario_command(0, &cast_spell(slot, target_object(akroma)))
            .is_err(),
        "a red spell cannot target Akroma"
    );
    assert!(e.state.stack.is_empty());

    inject_card_into_hand(&mut e, 0, "swords_to_plowshares");
    give_mana(
        &mut e,
        0,
        ManaGift {
            w: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 0, "swords_to_plowshares");
    e.apply_scenario_command(0, &cast_spell(slot, target_object(akroma)))
        .expect("white is not protected");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&akroma].zone, Zone::Exile);
}

#[test]
fn q80_protection_cards_all_cast_with_exact_printed_characteristics() {
    for (index, case) in CASES.iter().enumerate() {
        let mut e = engine(80_201_000 + index as u64);
        let object = cast_card(&mut e, case);
        assert_eq!(
            e.effective_power(object),
            Some(case.power),
            "{} power",
            case.id
        );
        assert_eq!(
            e.effective_toughness(object),
            Some(case.toughness),
            "{} toughness",
            case.id
        );
        let characteristics = e.characteristics(object).expect("characteristics");
        assert_eq!(
            characteristics.protections,
            protection_qualities(case),
            "{} protections",
            case.id
        );
        for keyword in case.keywords {
            assert!(
                characteristics.has_keyword(*keyword),
                "{} has {keyword:?}",
                case.id
            );
        }
    }
}

#[test]
fn q80_protection_cards_reject_targeting_by_each_protected_color() {
    for (index, case) in CASES.iter().enumerate() {
        for (offset, color) in case.protections.iter().enumerate() {
            let mut e = engine(80_202_000 + (index * 10 + offset) as u64);
            let object = cast_card(&mut e, case);
            let (spell, mana) = protected_target_spell(*color);
            inject_card_into_hand(&mut e, 0, spell);
            give_mana(&mut e, 0, mana);
            let slot = hand_index_for_card(&e, 0, spell);
            assert!(
                e.apply_scenario_command(0, &cast_spell(slot, target_object(object)))
                    .is_err(),
                "{}: {spell} of protected color {color:?} cannot target it",
                case.id
            );
            assert!(e.state.stack.is_empty(), "{}: {spell}", case.id);
            assert_eq!(e.state.objects[&object].zone, Zone::Battlefield);
        }
    }
}
