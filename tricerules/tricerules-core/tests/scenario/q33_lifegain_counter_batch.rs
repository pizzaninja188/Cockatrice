//! Actual-card scenarios for the queue-v33 life-gain counter cohort.
//!
//! Expectations are printed Oracle values. Revitalize is a shared, already
//! reviewed life-gain source; CR 119.3 emits the life-gain event and CR 122.1
//! puts one +1/+1 counter on the source for each event.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::CounterKind;

fn plus_one_counters(e: &GameEngine, id: u32) -> u32 {
    e.state.objects[&id].counter_count(CounterKind::PlusOnePlusOne)
}

struct Case {
    id: &'static str,
    power: u32,
    toughness: u32,
    keyword: Option<tricerules_cards::Keyword>,
}

const CASES: [Case; 4] = [
    Case {
        id: "blood_researcher",
        power: 2,
        toughness: 2,
        keyword: Some(tricerules_cards::Keyword::Menace),
    },
    Case {
        id: "bloodbond_vampire",
        power: 3,
        toughness: 3,
        keyword: None,
    },
    Case {
        id: "celestial_unicorn",
        power: 3,
        toughness: 2,
        keyword: None,
    },
    Case {
        id: "scion_of_the_swarm",
        power: 3,
        toughness: 3,
        keyword: Some(tricerules_cards::Keyword::Flying),
    },
];

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("plains", &[card, "revitalize", "revitalize"]),
        deck_with("plains", &["revitalize"]),
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

fn gain_three(e: &mut GameEngine, player: usize, card: &str) {
    ensure_in_hand(e, player, card);
    give_mana(
        e,
        player as i32,
        ManaGift {
            w: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(e, player, card);
    e.apply_scenario_command(player as i32, &cast_spell(slot, vec![]))
        .unwrap_or_else(|error| panic!("cast {card}: {error:?}"));
    resolve_entire_stack_two_player(e);
}

#[test]
fn q33_lifegain_counter_canary_simplest() {
    let case = &CASES[1];
    let mut e = engine(33_020_001, case.id);
    let source = move_ready_to_battlefield(&mut e, 0, case.id);
    gain_three(&mut e, 0, "revitalize");
    assert_eq!(plus_one_counters(&e, source), 1);
}

#[test]
fn q33_lifegain_counter_canary_hardest() {
    let case = &CASES[0];
    let mut e = engine(33_020_002, case.id);
    let source = move_ready_to_battlefield(&mut e, 0, case.id);
    assert!(e.effective_has_keyword(source, tricerules_cards::Keyword::Menace));
    gain_three(&mut e, 0, "revitalize");
    assert_eq!(plus_one_counters(&e, source), 1);
    assert_eq!(e.effective_power(source), Some(3));
}

#[test]
fn q33_lifegain_counter_cards_count_only_their_controllers_life_gain() {
    for (index, case) in CASES.iter().enumerate() {
        let mut e = engine(33_021_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        gain_three(&mut e, 0, "revitalize");
        assert_eq!(plus_one_counters(&e, source), 1, "{}", case.id);
        gain_three(&mut e, 0, "revitalize");
        assert_eq!(
            plus_one_counters(&e, source),
            2,
            "{}: each life-gain event adds one counter",
            case.id
        );

        e.apply_scenario_command(0, &pass()).expect("pass priority");
        gain_three(&mut e, 1, "revitalize");
        assert_eq!(
            e.state.players[1].life, 23,
            "{}: opponent gained life",
            case.id
        );
        assert_eq!(
            plus_one_counters(&e, source),
            2,
            "{}: an opponent's life gain does not trigger it",
            case.id
        );
    }
}

#[test]
fn q33_lifegain_counter_cards_keep_printed_keywords_and_bodies() {
    for (index, case) in CASES.iter().enumerate() {
        let mut e = engine(33_022_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(case.power), Some(case.toughness)),
            "{}: printed body",
            case.id
        );
        match case.keyword {
            Some(keyword) => assert!(
                e.effective_has_keyword(source, keyword),
                "{}: printed keyword",
                case.id
            ),
            None => assert!(!e.effective_has_keyword(source, tricerules_cards::Keyword::Menace)),
        }
    }
}
