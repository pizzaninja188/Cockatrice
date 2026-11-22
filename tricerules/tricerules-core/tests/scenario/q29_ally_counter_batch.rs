//! Queue-v29 canaries: "Whenever this creature or another Ally you control enters, you may put a
//! +1/+1 counter on this creature."
//!
//! CR 603.6a, 603.5 and 122.1. Expectations are printed Oracle values.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::CounterKind;
use tricerules_proto::ruled::v1::ResolutionChoiceDecision;

struct Case {
    id: &'static str,
    mana: ManaGift,
    power: u32,
    toughness: u32,
}

fn case(id: &'static str) -> Case {
    match id {
        "hada_freeblade" => Case {
            id,
            mana: ManaGift {
                w: 1,
                ..Default::default()
            },
            power: 0,
            toughness: 1,
        },
        "nimana_sell-sword" => Case {
            id,
            mana: ManaGift {
                b: 1,
                c: 3,
                ..Default::default()
            },
            power: 2,
            toughness: 2,
        },
        "oran-rief_survivalist" => Case {
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
        case("hada_freeblade"),
        case("oran-rief_survivalist"),
        case("nimana_sell-sword"),
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("forest", &[card, "coralhelm_guide", "grizzly_bears"]),
        deck_with("island", &["coralhelm_guide", "grizzly_bears"]),
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

fn assert_canary(case: Case, seed: u64) {
    let mut e = engine(seed, case.id);
    let source = cast_source(&mut e, &case);
    assert_eq!(
        e.state.objects[&source].counter_count(CounterKind::PlusOnePlusOne),
        0,
        "{}: the trigger is still awaiting its optional choice",
        case.id
    );
    answer_optional_triggered_ability_choice(&mut e, ResolutionChoiceDecision::SelectBranch);
    assert_eq!(
        e.state.objects[&source].counter_count(CounterKind::PlusOnePlusOne),
        1,
        "{}: the accepted trigger adds one counter",
        case.id
    );
    assert_eq!(
        (e.effective_power(source), e.effective_toughness(source)),
        (Some(case.power + 1), Some(case.toughness + 1)),
        "{}: printed body plus the counter",
        case.id
    );
}

#[test]
fn q29_ally_counter_canary_simplest() {
    assert_canary(case("hada_freeblade"), 29_060_001);
}

#[test]
fn q29_ally_counter_canary_hardest() {
    assert_canary(case("nimana_sell-sword"), 29_060_002);
}

#[test]
fn q29_ally_counter_cards_cast_with_exact_cost_and_decline() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(29_061_000 + index as u64, case.id);
        let source = cast_source(&mut e, &case);
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(case.power), Some(case.toughness)),
            "{}: the creature entered with its printed body",
            case.id
        );
        answer_optional_triggered_ability_choice(&mut e, ResolutionChoiceDecision::Decline);
        assert_eq!(
            e.state.objects[&source].counter_count(CounterKind::PlusOnePlusOne),
            0,
            "{}: declining adds no counter",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn q29_ally_counter_cards_trigger_for_another_ally_you_control() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(29_062_000 + index as u64, case.id);
        let source = cast_source(&mut e, &case);
        answer_optional_triggered_ability_choice(&mut e, ResolutionChoiceDecision::Decline);
        move_ready_to_battlefield(&mut e, 0, "coralhelm_guide");
        resolve_entire_stack_two_player(&mut e);
        assert!(e.state.pending_resolution.is_some(), "{}", case.id);
        answer_optional_triggered_ability_choice(&mut e, ResolutionChoiceDecision::SelectBranch);
        assert_eq!(
            e.state.objects[&source].counter_count(CounterKind::PlusOnePlusOne),
            1,
            "{}: another Ally entering triggers the counter",
            case.id
        );
    }
}

#[test]
fn q29_ally_counter_cards_ignore_non_allies_and_opposing_allies() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(29_063_000 + index as u64, case.id);
        let source = cast_source(&mut e, &case);
        answer_optional_triggered_ability_choice(&mut e, ResolutionChoiceDecision::Decline);
        move_ready_to_battlefield(&mut e, 0, "grizzly_bears");
        resolve_entire_stack_two_player(&mut e);
        assert!(
            e.state.pending_resolution.is_none(),
            "{}: a non-Ally does not trigger the ability",
            case.id
        );
        move_ready_to_battlefield(&mut e, 1, "coralhelm_guide");
        resolve_entire_stack_two_player(&mut e);
        assert!(
            e.state.pending_resolution.is_none(),
            "{}: an opponent's Ally does not trigger the ability",
            case.id
        );
        assert_eq!(
            e.state.objects[&source].counter_count(CounterKind::PlusOnePlusOne),
            0,
            "{}: no counters",
            case.id
        );
    }
}
