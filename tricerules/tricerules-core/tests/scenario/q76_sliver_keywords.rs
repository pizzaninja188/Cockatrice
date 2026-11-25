//! Actual-card scenarios for the queue-v76 01-sliver-keyword-anthems cohort.
//!
//! Expectations are printed Oracle values. CR 604.1/611.3a make the grant continuous,
//! CR 613.1f applies it in layer 6, and CR 109.2 makes "Sliver" describe every Sliver
//! creature on the battlefield, including the source. Pinned 2013-07-01 sliver rulings
//! confirm the grants are cumulative but redundant for single-instance keywords.
use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    keyword: Keyword,
    power: u32,
    toughness: u32,
}

const CASES: [Case; 9] = [
    Case {
        id: "winged_sliver",
        keyword: Keyword::Flying,
        power: 1,
        toughness: 1,
    },
    Case {
        id: "heart_sliver",
        keyword: Keyword::Haste,
        power: 1,
        toughness: 1,
    },
    Case {
        id: "reflex_sliver",
        keyword: Keyword::Haste,
        power: 2,
        toughness: 2,
    },
    Case {
        id: "battering_sliver",
        keyword: Keyword::Trample,
        power: 4,
        toughness: 4,
    },
    Case {
        id: "horned_sliver",
        keyword: Keyword::Trample,
        power: 2,
        toughness: 2,
    },
    Case {
        id: "spitting_sliver",
        keyword: Keyword::FirstStrike,
        power: 3,
        toughness: 3,
    },
    Case {
        id: "talon_sliver",
        keyword: Keyword::FirstStrike,
        power: 1,
        toughness: 1,
    },
    Case {
        id: "synchronous_sliver",
        keyword: Keyword::Vigilance,
        power: 3,
        toughness: 3,
    },
    Case {
        id: "fury_sliver",
        keyword: Keyword::DoubleStrike,
        power: 3,
        toughness: 3,
    },
];

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("forest", &[card, card, "sliver_construct"]),
        deck_with("island", &["sliver_construct"]),
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

fn move_to_graveyard(e: &mut GameEngine, player: usize, object: u32) {
    e.state.players[player]
        .battlefield
        .retain(|oid| *oid != object);
    e.state.players[player].graveyard.push(object);
    e.state.objects.get_mut(&object).expect("source").zone = Zone::Graveyard;
    *e.state.zone_change_generation.entry(object).or_default() += 1;
}

#[test]
fn q76_sliver_keywords_canary_simplest_talon_sliver_grants_first_strike() {
    let mut e = engine(76_100_001, "talon_sliver");
    let source = move_ready_to_battlefield(&mut e, 0, "talon_sliver");
    let friend = move_ready_to_battlefield(&mut e, 0, "sliver_construct");
    let opposing = move_ready_to_battlefield(&mut e, 1, "sliver_construct");
    let bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");

    assert_eq!(
        (e.effective_power(source), e.effective_toughness(source)),
        (Some(1), Some(1)),
        "Talon Sliver keeps its printed body"
    );
    for sliver in [source, friend, opposing] {
        assert!(
            e.effective_has_keyword(sliver, Keyword::FirstStrike),
            "every Sliver gains first strike, including the source and opponents' Slivers"
        );
    }
    assert!(
        !e.effective_has_keyword(bear, Keyword::FirstStrike),
        "a non-Sliver does not gain the keyword"
    );
}

#[test]
fn q76_sliver_keywords_canary_hardest_fury_sliver_grants_double_strike() {
    let mut e = engine(76_100_002, "fury_sliver");
    let source = move_ready_to_battlefield(&mut e, 0, "fury_sliver");
    let friend = move_ready_to_battlefield(&mut e, 0, "sliver_construct");
    let bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");

    assert_eq!(
        (e.effective_power(source), e.effective_toughness(source)),
        (Some(3), Some(3))
    );
    assert!(e.effective_has_keyword(source, Keyword::DoubleStrike));
    assert!(e.effective_has_keyword(friend, Keyword::DoubleStrike));
    assert!(!e.effective_has_keyword(bear, Keyword::DoubleStrike));
}

#[test]
fn q76_sliver_keywords_all_cards_grant_their_printed_keyword_to_every_sliver() {
    for (index, case) in CASES.iter().enumerate() {
        let mut e = engine(76_101_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        let friend = move_ready_to_battlefield(&mut e, 0, "sliver_construct");
        let opposing = move_ready_to_battlefield(&mut e, 1, "sliver_construct");
        let bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");

        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(case.power), Some(case.toughness)),
            "{}: printed body",
            case.id
        );
        for sliver in [source, friend, opposing] {
            assert!(
                e.effective_has_keyword(sliver, case.keyword),
                "{}: every Sliver gains {:?}",
                case.id,
                case.keyword
            );
        }
        assert!(
            !e.effective_has_keyword(bear, case.keyword),
            "{}: a non-Sliver is unaffected",
            case.id
        );
    }
}

#[test]
fn q76_sliver_keywords_apply_to_later_slivers_and_end_when_the_source_leaves() {
    for (index, case) in CASES.iter().enumerate() {
        let mut e = engine(76_102_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        let late = inject_creature_on_battlefield(&mut e, 0, "sliver_construct");
        assert!(
            e.effective_has_keyword(late, case.keyword),
            "{}: a Sliver that enters later is affected",
            case.id
        );
        move_to_graveyard(&mut e, 0, source);
        assert!(
            !e.effective_has_keyword(late, case.keyword),
            "{}: the grant ends when the source leaves the battlefield",
            case.id
        );
    }
}
