//! Actual-card scenarios for the reviewed sacrifice-for-two-mana land family.
//!
//! Expectations are printed Oracle values. CR 614.1d and 614.12 make the
//! enters-tapped clause a replacement effect; CR 605.1a and 605.3a-b make the
//! targetless mana abilities resolve immediately without using the stack;
//! CR 118.3 and 701.21a govern sacrificing the land as a cost.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    single: (u32, u32, u32, u32, u32, u32),
    pair: (u32, u32, u32, u32, u32, u32),
}

fn cases() -> [Case; 10] {
    [
        Case {
            id: "ancient_spring",
            single: (0, 1, 0, 0, 0, 0),
            pair: (1, 0, 1, 0, 0, 0),
        },
        Case {
            id: "dwarven_ruins",
            single: (0, 0, 0, 1, 0, 0),
            pair: (0, 0, 0, 2, 0, 0),
        },
        Case {
            id: "ebon_stronghold",
            single: (0, 0, 1, 0, 0, 0),
            pair: (0, 0, 2, 0, 0, 0),
        },
        Case {
            id: "geothermal_crevice",
            single: (0, 0, 0, 1, 0, 0),
            pair: (0, 0, 1, 0, 1, 0),
        },
        Case {
            id: "havenwood_battleground",
            single: (0, 0, 0, 0, 1, 0),
            pair: (0, 0, 0, 0, 2, 0),
        },
        Case {
            id: "irrigation_ditch",
            single: (1, 0, 0, 0, 0, 0),
            pair: (0, 1, 0, 0, 1, 0),
        },
        Case {
            id: "ruins_of_trokair",
            single: (1, 0, 0, 0, 0, 0),
            pair: (2, 0, 0, 0, 0, 0),
        },
        Case {
            id: "sulfur_vent",
            single: (0, 0, 1, 0, 0, 0),
            pair: (0, 1, 0, 1, 0, 0),
        },
        Case {
            id: "svyelunite_temple",
            single: (0, 1, 0, 0, 0, 0),
            pair: (0, 2, 0, 0, 0, 0),
        },
        Case {
            id: "tinder_farm",
            single: (0, 0, 0, 0, 1, 0),
            pair: (1, 0, 0, 1, 0, 0),
        },
    ]
}

fn canaries() -> Vec<Case> {
    cases()
        .into_iter()
        .filter(|case| matches!(case.id, "dwarven_ruins" | "ancient_spring"))
        .collect()
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[card]), deck_with("island", &[])]);
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

fn pool(e: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[0].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

fn assert_untapped_mana_ability(case: &Case) {
    let mut e = engine(851_000, case.id);
    let land = inject_permanent_on_battlefield(&mut e, 0, case.id);
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "{}", case.id);
    apply_ability(&mut e, 0, land, 0, vec![]).expect("activate the single-color mana ability");
    assert_eq!(pool(&e), case.single, "{}: printed single color", case.id);
    assert!(e.state.objects[&land].tapped, "{}: tap is a cost", case.id);
    assert!(
        e.state.stack.is_empty(),
        "{}: mana abilities do not use the stack",
        case.id
    );
}

fn assert_sacrifice_ability(case: &Case) {
    let mut e = engine(852_000, case.id);
    let land = inject_permanent_on_battlefield(&mut e, 0, case.id);
    apply_ability(&mut e, 0, land, 1, vec![]).expect("activate the sacrifice mana ability");
    assert_eq!(pool(&e), case.pair, "{}: printed pair", case.id);
    assert_eq!(
        e.state.objects[&land].zone,
        Zone::Graveyard,
        "{}: the land is sacrificed as a cost",
        case.id
    );
    assert!(
        e.state.players[0].graveyard.contains(&land),
        "{}: the sacrificed land is in its owner's graveyard",
        case.id
    );
    assert!(e.state.stack.is_empty(), "{}", case.id);
    let before = pool(&e);
    assert!(
        apply_ability(&mut e, 0, land, 1, vec![]).is_err(),
        "{}: a sacrificed land cannot be activated again",
        case.id
    );
    assert_eq!(
        pool(&e),
        before,
        "{}: the rejected activation adds no mana",
        case.id
    );
}

#[test]
fn sac_lands_cc_batch_canary_cards_enter_tapped_and_tap_for_the_single_color() {
    for (index, case) in canaries().into_iter().enumerate() {
        let mut e = engine(850_000 + index as u64, case.id);
        inject_card_into_hand(&mut e, 0, case.id);
        let hand_index = hand_index_for_card(&e, 0, case.id);
        e.apply_scenario_command(0, &play_land(hand_index))
            .expect("play the land");
        let land = battlefield_object_for_card(&e, 0, case.id);
        assert!(e.state.objects[&land].tapped, "{}: enters tapped", case.id);
        assert!(
            apply_ability(&mut e, 0, land, 0, vec![]).is_err(),
            "{}: a tapped land cannot pay its tap cost",
            case.id
        );
        assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "{}", case.id);
    }
    for case in canaries() {
        assert_untapped_mana_ability(&case);
    }
}

#[test]
fn sac_lands_cc_batch_canary_cards_sacrifice_for_the_printed_pair() {
    for case in canaries() {
        assert_sacrifice_ability(&case);
    }
}

#[test]
fn sac_lands_cc_batch_cards_enter_tapped_and_reject_immediate_tap() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(853_000 + index as u64, case.id);
        inject_card_into_hand(&mut e, 0, case.id);
        let hand_index = hand_index_for_card(&e, 0, case.id);
        e.apply_scenario_command(0, &play_land(hand_index))
            .expect("play the land");
        let land = battlefield_object_for_card(&e, 0, case.id);
        assert_eq!(
            e.state.objects[&land].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
        assert!(e.state.objects[&land].tapped, "{}: enters tapped", case.id);
        assert!(
            apply_ability(&mut e, 0, land, 0, vec![]).is_err(),
            "{}: a tapped land cannot pay its tap cost",
            case.id
        );
        assert!(
            apply_ability(&mut e, 0, land, 1, vec![]).is_err(),
            "{}: the sacrifice ability also needs an untapped source",
            case.id
        );
        assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "{}", case.id);
        assert_eq!(
            e.state.objects[&land].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
    }
}

#[test]
fn sac_lands_cc_batch_cards_tap_for_exactly_the_printed_single_color() {
    for case in cases() {
        assert_untapped_mana_ability(&case);
    }
}

#[test]
fn sac_lands_cc_batch_cards_sacrifice_for_exactly_the_printed_pair() {
    for case in cases() {
        assert_sacrifice_ability(&case);
    }
}

#[test]
fn sac_lands_cc_batch_cards_do_not_sacrifice_when_the_tap_cost_is_unavailable() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(854_000 + index as u64, case.id);
        let land = inject_permanent_on_battlefield(&mut e, 0, case.id);
        apply_ability(&mut e, 0, land, 0, vec![])
            .expect("tap for the single color first, leaving the source tapped");
        let before = pool(&e);
        assert!(
            apply_ability(&mut e, 0, land, 1, vec![]).is_err(),
            "{}: the tapped source cannot pay the sacrifice ability's tap cost",
            case.id
        );
        assert_eq!(
            pool(&e),
            before,
            "{}: a rejected activation spends nothing",
            case.id
        );
        assert_eq!(
            e.state.objects[&land].zone,
            Zone::Battlefield,
            "{}: the land stays on the battlefield",
            case.id
        );
        assert!(e.state.objects[&land].tapped, "{}", case.id);
    }
}
