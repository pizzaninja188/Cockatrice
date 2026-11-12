//! Actual-card scenarios for the reviewed Warped Landscape land-search family.
//!
//! Expectations are printed Oracle values. CR 305.6 makes the tap ability a mana
//! ability; CR 602.1 pays all costs on activation; CR 701.23b/e allows a hidden-zone
//! search to find no card and requires the shuffle; CR 701.24a requires the found
//! basic land to enter tapped.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::ChoiceKind;

struct Case {
    id: &'static str,
    search_cost: u32,
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            id: "shire_terrace",
            search_cost: 1,
        },
        Case {
            id: "unexplored_territory",
            search_cost: 1,
        },
        Case {
            id: "warped_landscape",
            search_cost: 2,
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

#[test]
fn warped_landscape_batch_cards_tap_for_colorless() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(730_000 + index as u64, case.id);
        let land = move_ready_to_battlefield(&mut e, 0, case.id);
        assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0));
        e.apply_scenario_command(0, &activate_ability_for(&e, land, 0, vec![]))
            .expect("activate the mana ability");
        assert_eq!(pool(&e), (0, 0, 0, 0, 0, 1), "{case_id}", case_id = case.id);
        assert!(
            e.state.objects[&land].tapped,
            "{case_id}",
            case_id = case.id
        );
        assert_eq!(e.state.objects[&land].zone, Zone::Battlefield);
        assert!(
            e.state.stack.is_empty(),
            "mana abilities do not use the stack"
        );
    }
}

#[test]
fn warped_landscape_batch_cards_tap_for_colorless_and_search_a_basic_land_tapped() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(710_000 + index as u64, case.id);
        let land = move_ready_to_battlefield(&mut e, 0, case.id);
        let library_before = e.state.players[0].library.len();
        let hand_before = e.state.players[0].hand.len();
        let graveyard_before = e.state.players[0].graveyard.len();
        let forest = inject_library_card(&mut e, 0, "forest");
        assert_eq!(e.state.players[0].library.len(), library_before + 1);
        let generation_before = e
            .state
            .zone_change_generation
            .get(&forest)
            .copied()
            .unwrap_or(0);

        given_case_mana(&mut e, case.search_cost);
        assert_eq!(pool(&e).5, case.search_cost);

        e.apply_scenario_command(0, &activate_ability_for(&e, land, 1, vec![]))
            .expect("activate the search ability");
        assert_eq!(
            e.state.objects[&land].zone,
            Zone::Graveyard,
            "{case_id}",
            case_id = case.id
        );
        assert_eq!(
            e.state.stack.len(),
            1,
            "{case_id}: search waits on the stack",
            case_id = case.id
        );
        assert_eq!(
            pool(&e).5,
            0,
            "{case_id}: exact mana paid",
            case_id = case.id
        );

        e.apply_scenario_command(0, &pass()).expect("caster passes");
        let batch = e
            .apply_scenario_command(1, &pass())
            .expect("search resolves");
        let choice = find_resolution_choice(&batch).expect("basic-land search choice");
        assert_eq!(choice.choice_kind(), ChoiceKind::LibrarySearch);
        assert_eq!((choice.min, choice.max), (0, 1));
        assert!(choice.candidate_object_ids.contains(&forest));

        e.apply_scenario_command(0, &submit_resolution_choice(vec![forest]))
            .expect("choose the basic land");
        assert_eq!(e.state.objects[&forest].zone, Zone::Battlefield);
        assert!(e.state.objects[&forest].tapped, "found land enters tapped");
        assert_eq!(
            e.state.zone_change_generation.get(&forest).copied(),
            Some(generation_before + 1)
        );
        assert!(e.state.players[0].battlefield.contains(&forest));
        assert_eq!(e.state.players[0].hand.len(), hand_before);
        assert_eq!(
            e.state.players[0].graveyard.len(),
            graveyard_before + 1,
            "the sacrificed land is the only new graveyard card"
        );
        assert_eq!(e.state.players[0].library.len(), library_before);
        assert!(e.state.stack.is_empty());
    }
}

#[test]
fn warped_landscape_batch_cards_reject_search_without_the_exact_mana() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(720_000 + index as u64, case.id);
        let land = move_ready_to_battlefield(&mut e, 0, case.id);
        let library_before = e.state.players[0].library.len();
        let hand_before = e.state.players[0].hand.len();
        let graveyard_before = e.state.players[0].graveyard.len();
        let pool_before = pool(&e);

        assert!(
            e.apply_scenario_command(0, &activate_ability_for(&e, land, 1, vec![]))
                .is_err(),
            "{case_id}: missing printed mana rejects activation",
            case_id = case.id
        );
        assert_eq!(e.state.objects[&land].zone, Zone::Battlefield);
        assert!(!e.state.objects[&land].tapped);
        assert_eq!(e.state.players[0].library.len(), library_before);
        assert_eq!(e.state.players[0].hand.len(), hand_before);
        assert_eq!(e.state.players[0].graveyard.len(), graveyard_before);
        assert_eq!(pool(&e), pool_before);
        assert!(e.state.stack.is_empty());
    }
}

fn given_case_mana(e: &mut GameEngine, colorless: u32) {
    give_mana(
        e,
        0,
        ManaGift {
            c: colorless,
            ..Default::default()
        },
    );
}
