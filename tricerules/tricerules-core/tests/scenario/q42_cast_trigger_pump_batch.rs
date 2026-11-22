//! Actual-card scenarios for the queue-v42 instant-or-sorcery cast-trigger pump cohort.
//!
//! Expectations are printed Oracle values. CR 603.2 makes the cast trigger resolve before the
//! spell; CR 611.2c/613.4c apply the until-end-of-turn pump. Only the source controller's casts
//! qualify ("whenever you cast"), and a creature spell does not satisfy the printed filter.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::primitives::Keyword;

struct Case {
    id: &'static str,
    body: (u32, u32),
    pump_power: i32,
    pump_toughness: i32,
    keyword: Option<Keyword>,
    line: u16,
}

fn cases() -> [Case; 5] {
    [
        Case {
            id: "mercurial_geists",
            body: (1, 3),
            pump_power: 3,
            pump_toughness: 0,
            keyword: Some(Keyword::Flying),
            line: 2,
        },
        Case {
            id: "fire_urchin",
            body: (1, 3),
            pump_power: 1,
            pump_toughness: 0,
            keyword: Some(Keyword::Trample),
            line: 2,
        },
        Case {
            id: "festival_crasher",
            body: (1, 3),
            pump_power: 2,
            pump_toughness: 0,
            keyword: None,
            line: 1,
        },
        Case {
            id: "wee_dragonauts",
            body: (1, 3),
            pump_power: 2,
            pump_toughness: 0,
            keyword: Some(Keyword::Flying),
            line: 2,
        },
        Case {
            id: "kiln_fiend",
            body: (1, 2),
            pump_power: 3,
            pump_toughness: 0,
            keyword: None,
            line: 1,
        },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with(
            "mountain",
            &[card, "lightning_bolt", "divination", "grizzly_bears"],
        ),
        deck_with("island", &["lightning_bolt"]),
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

fn cast_named(
    e: &mut GameEngine,
    player: usize,
    card: &str,
    targets: Vec<tricerules_proto::ruled::v1::TargetRef>,
) {
    ensure_in_hand(e, player, card);
    grant_pool(e, player);
    let slot = hand_index_for_card(e, player, card);
    e.apply_scenario_command(player as i32, &cast_spell(slot, targets))
        .unwrap_or_else(|error| panic!("cast {card}: {error}"));
    resolve_entire_stack_two_player(e);
}

#[test]
fn q42_cast_trigger_pump_canary_simplest_pumps_on_an_instant() {
    let case = cases()
        .into_iter()
        .find(|case| case.id == "festival_crasher")
        .expect("Festival Crasher case");
    let mut e = engine(42_040_001, case.id);
    let source = move_ready_to_battlefield(&mut e, 0, case.id);
    cast_named(&mut e, 0, "lightning_bolt", target_player(1));
    assert_eq!(
        e.effective_power(source),
        Some(case.body.0 + case.pump_power as u32),
        "the printed pump applies on an instant"
    );
    assert_eq!(
        e.effective_toughness(source),
        Some(case.body.1 + case.pump_toughness as u32)
    );
}

#[test]
fn q42_cast_trigger_pump_canary_hardest_pumps_on_a_sorcery() {
    let case = cases()
        .into_iter()
        .find(|case| case.id == "mercurial_geists")
        .expect("Mercurial Geists case");
    let mut e = engine(42_040_002, case.id);
    let source = move_ready_to_battlefield(&mut e, 0, case.id);
    assert!(e.effective_has_keyword(source, Keyword::Flying));
    cast_named(&mut e, 0, "divination", vec![]);
    assert_eq!(
        e.effective_power(source),
        Some(case.body.0 + 3),
        "the printed +3/+0 applies on a sorcery"
    );
    assert_eq!(
        e.effective_toughness(source),
        Some(case.body.1),
        "toughness is unchanged"
    );
}

#[test]
fn q42_cast_trigger_pump_cards_apply_their_printed_pump() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(42_041_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        cast_named(&mut e, 0, "lightning_bolt", target_player(1));
        assert_eq!(
            e.effective_power(source),
            Some(case.body.0 + case.pump_power as u32),
            "{}: printed pump power",
            case.id
        );
        assert_eq!(
            e.effective_toughness(source),
            Some(case.body.1 + case.pump_toughness as u32),
            "{}: printed pump toughness",
            case.id
        );
        cast_named(&mut e, 0, "divination", vec![]);
        assert_eq!(
            e.effective_power(source),
            Some(case.body.0 + case.pump_power as u32 * 2),
            "{}: the trigger applies once per qualifying cast",
            case.id
        );
        match case.keyword {
            Some(keyword) => {
                assert!(e.effective_has_keyword(source, keyword), "{}", case.id)
            }
            None => assert!(
                !e.effective_has_keyword(source, Keyword::Flying),
                "{}: no printed flying",
                case.id
            ),
        }
    }
}

#[test]
fn q42_cast_trigger_pump_cards_ignore_creature_casts_and_opponents() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(42_042_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        cast_named(&mut e, 0, "grizzly_bears", vec![]);
        assert_eq!(
            e.effective_power(source),
            Some(case.body.0),
            "{}: a creature cast does not trigger",
            case.id
        );
        e.apply_scenario_command(0, &pass())
            .expect("pass priority to the opponent");
        cast_named(&mut e, 1, "lightning_bolt", target_player(0));
        assert_eq!(
            e.effective_power(source),
            Some(case.body.0),
            "{}: an opponent's instant does not trigger",
            case.id
        );
    }
}

#[test]
fn q42_cast_trigger_pump_cards_have_the_printed_presentation_line() {
    for case in cases() {
        let definition = tricerules_cards::registry::global()
            .get(case.id)
            .unwrap_or_else(|| panic!("missing {}", case.id));
        let face = definition.primary_face();
        assert_eq!(face.triggered_abilities.len(), 1, "{}", case.id);
        assert_eq!(
            face.triggered_abilities[0].presentation,
            tricerules_cards::AbilityPresentation::OracleLines(vec![case.line]),
            "{}",
            case.id
        );
    }
}
