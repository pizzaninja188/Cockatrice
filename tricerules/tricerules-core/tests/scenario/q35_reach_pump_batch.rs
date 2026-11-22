//! Actual-card scenarios for the queue-v35 reach pump cohort.
//!
//! Expectations are printed Oracle values. CR 611.2a/514.2 give both the pump
//! and the reach grant their until-end-of-turn duration; CR 702.17 makes reach
//! the keyword that allows blocking a creature with flying.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::Keyword;

struct Case {
    id: &'static str,
    pump: (i32, i32),
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "shape_the_sands",
            pump: (0, 5),
        },
        Case {
            id: "silk_net",
            pump: (1, 1),
        },
        Case {
            id: "snare_the_skies",
            pump: (1, 1),
        },
    ]
}

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[]), deck_with("mountain", &[])]);
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

fn cast_at(e: &mut GameEngine, case: &Case, target: u32) {
    inject_card_into_hand(e, 0, case.id);
    give_mana(
        e,
        0,
        ManaGift {
            g: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
        .unwrap_or_else(|error| panic!("cast {}: {error:?}", case.id));
    let p = &e.state.players[0].mana_pool;
    assert_eq!(
        (p.white, p.blue, p.black, p.red, p.green, p.colorless),
        (0, 0, 0, 0, 0, 0),
        "{}: exact printed cost",
        case.id
    );
    resolve_entire_stack_two_player(e);
}

#[test]
fn q35_reach_pump_canary_simplest() {
    let case = &cases()[1];
    let mut e = engine(35_070_001);
    let target = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 5, 5);
    cast_at(&mut e, case, target);
    assert_eq!(
        (e.effective_power(target), e.effective_toughness(target)),
        (Some(6), Some(6)),
        "Silk Net gives +1/+1"
    );
    assert!(
        e.effective_has_keyword(target, Keyword::Reach),
        "Silk Net grants reach"
    );
}

#[test]
fn q35_reach_pump_canary_hardest() {
    let case = &cases()[0];
    let mut e = engine(35_070_002);
    let target = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 5, 5);
    cast_at(&mut e, case, target);
    assert_eq!(
        (e.effective_power(target), e.effective_toughness(target)),
        (Some(5), Some(10)),
        "Shape the Sands gives +0/+5"
    );
    assert!(
        e.effective_has_keyword(target, Keyword::Reach),
        "Shape the Sands grants reach"
    );
}

#[test]
fn q35_reach_pump_cards_apply_the_printed_pump_and_reach() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(35_071_000 + index as u64);
        let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 5, 5);
        let bystander = inject_creature_with_stats(&mut e, 0, "savannah_lions", 2, 2);
        cast_at(&mut e, case, target);
        assert_eq!(
            (e.effective_power(target), e.effective_toughness(target)),
            (
                Some((5 + case.pump.0) as u32),
                Some((5 + case.pump.1) as u32)
            ),
            "{}: printed pump",
            case.id
        );
        assert!(
            e.effective_has_keyword(target, Keyword::Reach),
            "{}: the chosen creature gains reach",
            case.id
        );
        assert!(
            !e.effective_has_keyword(bystander, Keyword::Reach),
            "{}: only the chosen creature is affected",
            case.id
        );
        end_active_turn(&mut e, 0);
        assert_eq!(
            (e.effective_power(target), e.effective_toughness(target)),
            (Some(5), Some(5)),
            "{}: the pump ends with the turn",
            case.id
        );
        assert!(
            !e.effective_has_keyword(target, Keyword::Reach),
            "{}: reach ends with the turn",
            case.id
        );
    }
}

#[test]
fn q35_reach_pump_cards_reject_noncreature_targets() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(35_072_000 + index as u64);
        let land = inject_permanent_on_battlefield(&mut e, 1, "forest");
        inject_card_into_hand(&mut e, 0, case.id);
        give_mana(
            &mut e,
            0,
            ManaGift {
                g: 1,
                ..Default::default()
            },
        );
        let slot = hand_index_for_card(&e, 0, case.id);
        assert!(
            e.apply_scenario_command(0, &cast_spell(slot, target_object(land)))
                .is_err(),
            "{}: a land is not a legal creature target",
            case.id
        );
        let p = &e.state.players[0].mana_pool;
        assert_eq!(
            (p.white, p.blue, p.black, p.red, p.green, p.colorless),
            (0, 0, 0, 0, 1, 0),
            "{}: the rejected cast leaves mana unspent",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}: nothing was cast", case.id);
    }
}
