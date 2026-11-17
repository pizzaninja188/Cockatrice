//! Actual-card scenarios for the queue-v31 bounce-land cycle.
//!
//! Expectations are printed Oracle values. CR 614.1d/614.12 make the entry
//! replacement effect tap the land; CR 603.6a puts the enters trigger on the
//! stack; CR 115.1d/608.2c-d choose a land its controller controls during
//! resolution (not as a target); CR 605.1a/605.3b cover the intrinsic mana.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::RuledEventBatch;

struct Case {
    id: &'static str,
    options: (u32, u32, u32, u32, u32, u32),
}

const CASES: [Case; 10] = [
    Case {
        id: "azorius_chancery",
        options: (1, 1, 0, 0, 0, 0),
    },
    Case {
        id: "dimir_aqueduct",
        options: (0, 1, 1, 0, 0, 0),
    },
    Case {
        id: "rakdos_carnarium",
        options: (0, 0, 1, 1, 0, 0),
    },
    Case {
        id: "gruul_turf",
        options: (0, 0, 0, 1, 1, 0),
    },
    Case {
        id: "selesnya_sanctuary",
        options: (1, 0, 0, 0, 1, 0),
    },
    Case {
        id: "orzhov_basilica",
        options: (1, 0, 1, 0, 0, 0),
    },
    Case {
        id: "izzet_boilerworks",
        options: (0, 1, 0, 1, 0, 0),
    },
    Case {
        id: "golgari_rot_farm",
        options: (0, 0, 1, 0, 1, 0),
    },
    Case {
        id: "boros_garrison",
        options: (1, 0, 0, 1, 0, 0),
    },
    Case {
        id: "simic_growth_chamber",
        options: (0, 1, 0, 0, 1, 0),
    },
];

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[]), deck_with("island", &[])]);
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

fn mana_pool(e: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let pool = &e.state.players[0].mana_pool;
    (
        pool.white,
        pool.blue,
        pool.black,
        pool.red,
        pool.green,
        pool.colorless,
    )
}

fn play_bounce(e: &mut GameEngine, case: &Case) -> u32 {
    inject_card_into_hand(e, 0, case.id);
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &play_land(slot))
        .unwrap_or_else(|error| panic!("{}: play land: {error:?}", case.id));
    assert!(
        !e.state.stack.is_empty(),
        "{}: enters trigger is staged",
        case.id
    );
    battlefield_object_for_card(e, 0, case.id)
}

fn resolve_one(e: &mut GameEngine) -> RuledEventBatch {
    let mut batch = RuledEventBatch::default();
    for _ in 0..e.state.players.len() {
        let actor = e.state.priority_player_id();
        batch = e
            .apply_scenario_command(actor, &pass())
            .expect("pass priority");
    }
    batch
}

fn choose_land(e: &mut GameEngine, object_id: u32) {
    let batch = resolve_one(e);
    let Some(choice) = find_resolution_choice(&batch) else {
        // A single legal candidate is resolved automatically without a choice prompt.
        resolve_entire_stack_two_player(e);
        return;
    };
    assert!(
        choice.candidate_object_ids.contains(&object_id),
        "candidate list contains the chosen land"
    );
    let deciding_player = e
        .state
        .pending_resolution
        .as_ref()
        .expect("pending")
        .deciding_player;
    e.apply_scenario_command(deciding_player, &submit_resolution_choice(vec![object_id]))
        .expect("choose the land during resolution");
    resolve_entire_stack_two_player(e);
}

fn candidate_ids(e: &mut GameEngine) -> Vec<u32> {
    let batch = resolve_one(e);
    let choice = find_resolution_choice(&batch).expect("choose-land resolution choice");
    choice.candidate_object_ids.clone()
}

#[test]
fn q31_bounce_lands_canary_simplest() {
    let mut e = engine(31_020_001);
    let basic = relocate_to_battlefield(&mut e, 0, "forest", false);
    let land = play_bounce(&mut e, &CASES[0]);
    assert!(e.state.objects[&land].tapped);
    choose_land(&mut e, basic);
    assert_eq!(e.state.objects[&basic].zone, Zone::Hand);
    assert_eq!(e.state.objects[&land].zone, Zone::Battlefield);
}

#[test]
fn q31_bounce_lands_canary_hardest() {
    let mut e = engine(31_020_002);
    let basic = inject_permanent_on_battlefield(&mut e, 0, "island");
    let land = play_bounce(&mut e, &CASES[9]);
    assert!(e.state.objects[&land].tapped);
    choose_land(&mut e, basic);
    assert_eq!(e.state.objects[&basic].zone, Zone::Hand);
}

#[test]
fn q31_bounce_lands_enter_tapped_and_return_a_land_you_control() {
    for (index, case) in CASES.iter().enumerate() {
        let mut e = engine(31_021_000 + index as u64);
        let basic = relocate_to_battlefield(&mut e, 0, "forest", false);
        let land = play_bounce(&mut e, case);
        assert!(e.state.objects[&land].tapped, "{}: enters tapped", case.id);
        choose_land(&mut e, basic);
        assert_eq!(
            e.state.objects[&basic].zone,
            Zone::Hand,
            "{}: returned to owner's hand",
            case.id
        );
        assert!(e.state.players[0].hand.contains(&basic), "{}", case.id);
        assert_eq!(
            e.state.objects[&land].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
    }
}

#[test]
fn q31_bounce_lands_accept_the_entering_land_itself() {
    for (index, case) in CASES.iter().enumerate() {
        let mut e = engine(31_022_000 + index as u64);
        let land = play_bounce(&mut e, case);
        choose_land(&mut e, land);
        assert_eq!(
            e.state.objects[&land].zone,
            Zone::Hand,
            "{}: may return itself",
            case.id
        );
        assert!(e.state.players[0].hand.contains(&land), "{}", case.id);
    }
}

#[test]
fn q31_bounce_lands_candidate_list_excludes_opponents_lands() {
    for (index, case) in CASES.iter().enumerate() {
        let mut e = engine(31_023_000 + index as u64);
        let own = relocate_to_battlefield(&mut e, 0, "forest", false);
        let theirs = relocate_to_battlefield(&mut e, 1, "island", false);
        let _land = play_bounce(&mut e, case);
        let candidates = candidate_ids(&mut e);
        assert!(candidates.contains(&own), "{}: own land offered", case.id);
        assert!(
            !candidates.contains(&theirs),
            "{}: an opponent's land is not offered",
            case.id
        );
        let deciding_player = e
            .state
            .pending_resolution
            .as_ref()
            .expect("pending")
            .deciding_player;
        assert!(
            e.apply_scenario_command(deciding_player, &submit_resolution_choice(vec![theirs]))
                .is_err(),
            "{}: an opponent's land is rejected",
            case.id
        );
        e.apply_scenario_command(deciding_player, &submit_resolution_choice(vec![own]))
            .unwrap_or_else(|error| panic!("{}: own land rejected: {error:?}", case.id));
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.state.objects[&own].zone, Zone::Hand, "{}", case.id);
        assert_eq!(
            e.state.objects[&theirs].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
    }
}

#[test]
fn q31_bounce_lands_return_a_foreign_owned_land_to_its_owners_hand() {
    for (index, case) in CASES.iter().enumerate() {
        let mut e = engine(31_024_000 + index as u64);
        let foreign = inject_creature_under_foreign_control(&mut e, 1, 0, "forest");
        let _land = play_bounce(&mut e, case);
        choose_land(&mut e, foreign);
        assert_eq!(
            e.state.objects[&foreign].zone,
            Zone::Hand,
            "{}: returned to owner's hand",
            case.id
        );
        assert!(
            e.state.players[1].hand.contains(&foreign),
            "{}: owner's hand",
            case.id
        );
        assert!(
            !e.state.players[0].hand.contains(&foreign),
            "{}: controller's hand is not the owner's hand",
            case.id
        );
    }
}

#[test]
fn q31_bounce_lands_produce_the_printed_two_mana() {
    for (index, case) in CASES.iter().enumerate() {
        let mut e = engine(31_025_000 + index as u64);
        let land = inject_permanent_on_battlefield(&mut e, 0, case.id);
        e.apply_scenario_command(0, &activate_ability_for(&e, land, 0, vec![]))
            .unwrap_or_else(|error| panic!("{}: mana ability: {error:?}", case.id));
        assert_eq!(mana_pool(&e), case.options, "{}: printed pair", case.id);
        assert!(e.state.objects[&land].tapped, "{}", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}
