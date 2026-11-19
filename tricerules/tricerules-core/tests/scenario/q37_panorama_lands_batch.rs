//! Actual-card scenarios for the queue-v37 Panorama land cohort.
//!
//! Expectations are printed Oracle values. CR 305.6/605.1a make the {T} mana ability
//! immediate; CR 602.1 pays all costs on activation; CR 701.23b/e allows a hidden-zone
//! search to find no card and requires the shuffle; CR 701.24a requires the found
//! basic land to enter tapped; CR 614.1c applies that entry replacement.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::ChoiceKind;

struct Case {
    id: &'static str,
    allowed: [&'static str; 3],
    disallowed: &'static str,
}

fn cases() -> [Case; 5] {
    [
        Case {
            id: "bant_panorama",
            allowed: ["forest", "plains", "island"],
            disallowed: "swamp",
        },
        Case {
            id: "esper_panorama",
            allowed: ["plains", "island", "swamp"],
            disallowed: "mountain",
        },
        Case {
            id: "grixis_panorama",
            allowed: ["island", "swamp", "mountain"],
            disallowed: "forest",
        },
        Case {
            id: "jund_panorama",
            allowed: ["swamp", "mountain", "forest"],
            disallowed: "plains",
        },
        Case {
            id: "naya_panorama",
            allowed: ["mountain", "forest", "plains"],
            disallowed: "island",
        },
    ]
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

fn activate_search(e: &mut GameEngine, land: u32) -> Vec<u32> {
    give_mana(
        e,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    assert_eq!(pool(e).5, 1, "one colorless mana available");
    e.apply_scenario_command(0, &activate_ability_for(e, land, 1, vec![]))
        .expect("activate the search ability");
    assert_eq!(e.state.objects[&land].zone, Zone::Graveyard);
    assert_eq!(pool(e).5, 0, "exact printed {{1}} paid");
    let batch = resolve_entire_stack_two_player_for_choice(e);
    let choice = find_resolution_choice(&batch).expect("basic-land search choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibrarySearch);
    assert_eq!((choice.min, choice.max), (0, 1));
    choice.candidate_object_ids.clone()
}

fn resolve_entire_stack_two_player_for_choice(e: &mut GameEngine) -> RuledEventBatch {
    let first = e.state.priority_player_id();
    let second = if first == e.state.players[0].id {
        e.state.players[1].id
    } else {
        e.state.players[0].id
    };
    e.apply_scenario_command(first, &pass())
        .expect("first pass");
    e.apply_scenario_command(second, &pass())
        .expect("second pass resolves the search")
}

#[test]
fn q37_panorama_lands_canary_simplest() {
    let case = &cases()[0];
    let mut e = engine(37_010_001, cases()[0].id);
    let land = move_ready_to_battlefield(&mut e, 0, case.id);
    e.apply_scenario_command(0, &activate_ability_for(&e, land, 0, vec![]))
        .expect("tap for colorless");
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 1));
    assert!(e.state.objects[&land].tapped);
    assert!(
        e.state.stack.is_empty(),
        "mana abilities do not use the stack"
    );

    let mut search = engine(37_010_002, cases()[0].id);
    let land = move_ready_to_battlefield(&mut search, 0, case.id);
    let allowed = inject_library_card(&mut search, 0, case.allowed[0]);
    let wrong = inject_library_card(&mut search, 0, case.disallowed);
    let candidates = activate_search(&mut search, land);
    assert!(
        candidates.contains(&allowed),
        "the allowed basic is offered"
    );
    assert!(!candidates.contains(&wrong));
    search
        .apply_scenario_command(0, &submit_resolution_choice(vec![allowed]))
        .expect("choose the allowed basic");
    assert_eq!(search.state.objects[&allowed].zone, Zone::Battlefield);
    assert!(search.state.objects[&allowed].tapped);
    assert_eq!(search.state.objects[&wrong].zone, Zone::Library);
}

#[test]
fn q37_panorama_lands_canary_hardest() {
    let case = &cases()[4];
    let mut e = engine(37_010_003, cases()[4].id);
    let land = move_ready_to_battlefield(&mut e, 0, case.id);
    let mountain = inject_library_card(&mut e, 0, "mountain");
    let island = inject_library_card(&mut e, 0, case.disallowed);
    let candidates = activate_search(&mut e, land);
    assert!(
        candidates.contains(&mountain),
        "the allowed mountain is offered"
    );
    assert!(!candidates.contains(&island));
    e.apply_scenario_command(0, &submit_resolution_choice(vec![mountain]))
        .expect("choose the mountain");
    assert_eq!(e.state.objects[&mountain].zone, Zone::Battlefield);
    assert!(e.state.objects[&mountain].tapped);
    assert_eq!(e.state.objects[&island].zone, Zone::Library);
    assert_eq!(
        e.state.players[0]
            .battlefield
            .iter()
            .filter(|o| **o == land)
            .count(),
        0,
        "the panorama sacrificed itself"
    );
}

#[test]
fn q37_panorama_lands_cards_offer_only_the_printed_basic_types() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(37_011_000 + index as u64, case.id);
        let land = move_ready_to_battlefield(&mut e, 0, case.id);
        let mut good_ids = Vec::new();
        for good in case.allowed {
            good_ids.push(inject_library_card(&mut e, 0, good));
        }
        let bad = inject_library_card(&mut e, 0, case.disallowed);
        let candidates = activate_search(&mut e, land);
        for good in good_ids {
            assert!(
                candidates.contains(&good),
                "{}: an allowed basic is offered",
                case.id
            );
        }
        assert!(
            !candidates.contains(&bad),
            "{}: the off-color basic is not offered",
            case.id
        );
        e.apply_scenario_command(0, &submit_resolution_choice(vec![]))
            .expect("decline to find a card");
        assert_eq!(e.state.objects[&bad].zone, Zone::Library);
    }
}

#[test]
fn q37_panorama_lands_cards_reject_search_without_the_printed_mana() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(37_012_000 + index as u64, case.id);
        let land = move_ready_to_battlefield(&mut e, 0, case.id);
        e.apply_scenario_command(0, &activate_ability_for(&e, land, 1, vec![]))
            .expect_err("the search needs {1}");
        assert_eq!(e.state.objects[&land].zone, Zone::Battlefield);
        assert!(!e.state.objects[&land].tapped);
        assert!(e.state.stack.is_empty());
    }
}
