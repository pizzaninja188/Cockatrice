//! Actual-card scenarios for the queue-v42 landfall +2/+2 creature cohort.
//!
//! Expectations are printed Oracle values. CR 603.6a makes a land entering under the
//! controller's control trigger landfall; CR 611.2c/613.4c apply the pump to the source only;
//! CR 514.2 ends it at cleanup. Hagra Crocodile's printed "can't block" is a CR 509.1b
//! combat restriction.
use super::helpers::*;
use tricerules_cards::primitives::Keyword;

struct Case {
    id: &'static str,
    body: (u32, u32),
    keyword: Option<Keyword>,
    cant_block: bool,
}

fn cases() -> [Case; 5] {
    [
        Case {
            id: "wave-wing_elemental",
            body: (3, 4),
            keyword: Some(Keyword::Flying),
            cant_block: false,
        },
        Case {
            id: "hagra_crocodile",
            body: (3, 1),
            keyword: None,
            cant_block: true,
        },
        Case {
            id: "windrider_eel",
            body: (2, 2),
            keyword: Some(Keyword::Flying),
            cant_block: false,
        },
        Case {
            id: "ondu_greathorn",
            body: (2, 3),
            keyword: Some(Keyword::FirstStrike),
            cant_block: false,
        },
        Case {
            id: "plated_geopede",
            body: (1, 1),
            keyword: Some(Keyword::FirstStrike),
            cant_block: false,
        },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[card]), deck_with("forest", &[])]);
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
fn q42_landfall_pump_canary_simplest_pumps_on_a_land_entry() {
    let case = cases()
        .into_iter()
        .find(|case| case.id == "wave-wing_elemental")
        .expect("Wave-Wing Elemental case");
    let mut e = engine(42_030_001, case.id);
    let source = move_ready_to_battlefield(&mut e, 0, case.id);
    assert!(e.effective_has_keyword(source, Keyword::Flying));
    assert_eq!(e.effective_power(source), Some(3));
    let land = move_ready_to_battlefield(&mut e, 0, "forest");
    resolve_entire_stack_two_player(&mut e);
    assert!(e.state.players[0].battlefield.contains(&land));
    assert_eq!(e.effective_power(source), Some(5), "printed +2/+2");
    assert_eq!(e.effective_toughness(source), Some(6), "printed +2/+2");
}

#[test]
fn q42_landfall_pump_canary_hardest_cannot_block_and_still_pumps() {
    let case = cases()
        .into_iter()
        .find(|case| case.id == "hagra_crocodile")
        .expect("Hagra Crocodile case");
    let mut e = engine(42_030_002, case.id);
    let source = move_ready_to_battlefield(&mut e, 0, case.id);
    assert_eq!(
        zone_view_rules_annotation_labels(&mut e, 0, source),
        vec!["Can't block"],
        "the printed restriction is published"
    );
    move_ready_to_battlefield(&mut e, 0, "forest");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.effective_power(source), Some(5));
    assert_eq!(e.effective_toughness(source), Some(3));
}

#[test]
fn q42_landfall_pump_cards_apply_the_printed_effect_to_the_source_only() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(42_031_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        let bystander = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(case.body.0), Some(case.body.1)),
            "{}: printed body",
            case.id
        );
        move_ready_to_battlefield(&mut e, 0, "forest");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.effective_power(source),
            Some(case.body.0 + 2),
            "{}: landfall pumps power",
            case.id
        );
        assert_eq!(
            e.effective_toughness(source),
            Some(case.body.1 + 2),
            "{}: landfall pumps toughness",
            case.id
        );
        assert_eq!(
            (
                e.effective_power(bystander),
                e.effective_toughness(bystander)
            ),
            (Some(2), Some(2)),
            "{}: the pump is source-only",
            case.id
        );
        if let Some(keyword) = case.keyword {
            assert!(
                e.effective_has_keyword(source, keyword),
                "{}: printed keyword",
                case.id
            );
        }
        if case.cant_block {
            assert_eq!(
                zone_view_rules_annotation_labels(&mut e, 0, source),
                vec!["Can't block"],
                "{}",
                case.id
            );
        }
    }
}

#[test]
fn q42_landfall_pump_cards_scope_the_trigger_to_the_controller() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(42_032_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        move_ready_to_battlefield(&mut e, 1, "forest");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(case.body.0), Some(case.body.1)),
            "{}: an opponent's land does not trigger landfall",
            case.id
        );
    }
}

#[test]
fn q42_landfall_pump_cards_trigger_once_per_land_entry() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(42_033_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        move_ready_to_battlefield(&mut e, 0, "forest");
        resolve_entire_stack_two_player(&mut e);
        move_ready_to_battlefield(&mut e, 0, "forest");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(case.body.0 + 4), Some(case.body.1 + 4)),
            "{}: two entries apply the pump twice",
            case.id
        );
    }
}
