//! Actual-card scenarios for the queue-v33 entry team-counter cohort.
//!
//! Expectations are printed Oracle values. CR 603.6a triggers on the source's
//! entry; CR 122.1 places one +1/+1 counter on each other creature controlled
//! by that player. The source and opponents are excluded.

use super::helpers::*;
use tricerules_cards::CounterKind;

fn plus_one_counters(e: &GameEngine, id: u32) -> u32 {
    e.state.objects[&id].counter_count(CounterKind::PlusOnePlusOne)
}

struct Case {
    id: &'static str,
    keyword: Option<tricerules_cards::Keyword>,
}

const CASES: [Case; 4] = [
    Case {
        id: "katara,_heroic_healer",
        keyword: Some(tricerules_cards::Keyword::Lifelink),
    },
    Case {
        id: "ridgescale_tusker",
        keyword: None,
    },
    Case {
        id: "supply_runners",
        keyword: None,
    },
    Case {
        id: "the_falcon,_sam_wilson",
        keyword: Some(tricerules_cards::Keyword::Flying),
    },
];

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![deck_with("plains", &[card]), deck_with("forest", &[])]);
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

#[test]
fn q33_etb_team_counter_canary_simplest() {
    let case = &CASES[2];
    let mut e = engine(33_040_001, case.id);
    let friendly = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let other = inject_creature_on_battlefield(&mut e, 0, "savannah_lions");
    let opponent = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let source = move_ready_to_battlefield(&mut e, 0, case.id);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(plus_one_counters(&e, friendly), 1);
    assert_eq!(plus_one_counters(&e, other), 1);
    assert_eq!(plus_one_counters(&e, source), 0);
    assert_eq!(plus_one_counters(&e, opponent), 0);
}

#[test]
fn q33_etb_team_counter_canary_hardest() {
    let case = &CASES[0];
    let mut e = engine(33_040_002, case.id);
    let friendly = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let source = move_ready_to_battlefield(&mut e, 0, case.id);
    assert!(e.effective_has_keyword(source, tricerules_cards::Keyword::Lifelink));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(plus_one_counters(&e, friendly), 1);
    assert_eq!(e.effective_power(friendly), Some(3));
}

#[test]
fn q33_etb_team_counter_cards_apply_exactly_one_counter_to_each_other_friendly_creature() {
    for (index, case) in CASES.iter().enumerate() {
        let mut e = engine(33_041_000 + index as u64, case.id);
        let first = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        let second = inject_creature_on_battlefield(&mut e, 0, "savannah_lions");
        let opponent = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        if let Some(keyword) = case.keyword {
            assert!(
                e.effective_has_keyword(source, keyword),
                "{}: printed keyword",
                case.id
            );
        }
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(plus_one_counters(&e, first), 1, "{}", case.id);
        assert_eq!(plus_one_counters(&e, second), 1, "{}", case.id);
        assert_eq!(plus_one_counters(&e, source), 0, "{}", case.id);
        assert_eq!(plus_one_counters(&e, opponent), 0, "{}", case.id);
        assert_eq!(e.effective_power(first), Some(3), "{}", case.id);
        assert_eq!(e.effective_toughness(first), Some(3), "{}", case.id);
    }
}
