//! Actual-card scenarios for the reviewed Botanical Plaza land cycle.
//!
//! Expectations are printed Oracle values. CR 614.1d and 614.12 make the
//! enters-tapped clause a replacement effect; CR 605.1a and 605.3b cover the
//! mana ability; CR 118.3 and 701.21a require the tap and sacrifice costs and
//! CR 121.1 the draw.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

type ManaPool = (u32, u32, u32, u32, u32, u32);

struct Case {
    id: &'static str,
    options: [ManaPool; 2],
    exact: ManaPool,
}

const CASES: [Case; 5] = [
    Case {
        id: "botanical_plaza",
        options: [(0, 0, 0, 0, 1, 0), (1, 0, 0, 0, 0, 0)],
        exact: (1, 0, 0, 0, 1, 2),
    },
    Case {
        id: "racers_ring",
        options: [(0, 0, 0, 1, 0, 0), (0, 0, 0, 0, 1, 0)],
        exact: (0, 0, 0, 1, 1, 2),
    },
    Case {
        id: "skybridge_towers",
        options: [(1, 0, 0, 0, 0, 0), (0, 1, 0, 0, 0, 0)],
        exact: (1, 1, 0, 0, 0, 2),
    },
    Case {
        id: "tramway_station",
        options: [(0, 0, 1, 0, 0, 0), (0, 0, 0, 1, 0, 0)],
        exact: (0, 0, 1, 1, 0, 2),
    },
    Case {
        id: "waterfront_district",
        options: [(0, 1, 0, 0, 0, 0), (0, 0, 1, 0, 0, 0)],
        exact: (0, 1, 1, 0, 0, 2),
    },
];

fn mana_pool(engine: &GameEngine) -> ManaPool {
    let pool = &engine.state.players[0].mana_pool;
    (
        pool.white,
        pool.blue,
        pool.black,
        pool.red,
        pool.green,
        pool.colorless,
    )
}

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[]), deck_with("island", &[])]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn activate_mana_option(engine: &GameEngine, land: u32, option: u32) -> RuledCommand {
    let mut command = activate_ability_for(engine, land, 0, vec![]);
    let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
        unreachable!()
    };
    activation.mana_option_index = option;
    command
}

#[test]
fn botanical_plaza_batch_cards_enter_tapped_and_produce_each_printed_color() {
    for (case_index, case) in CASES.iter().enumerate() {
        for (option, expected) in case.options.iter().enumerate() {
            let mut engine = engine(25_050_000 + (case_index * 2 + option) as u64);
            let land = inject_card_into_hand(&mut engine, 0, case.id);
            let slot = hand_index_for_card(&engine, 0, case.id);
            engine
                .apply_scenario_command(0, &play_land(slot))
                .unwrap_or_else(|error| panic!("play {}: {error:?}", case.id));
            assert!(
                engine.state.objects[&land].tapped,
                "{} enters tapped",
                case.id
            );

            engine
                .apply_scenario_command(0, &activate_mana_option(&engine, land, option as u32))
                .expect_err("a tapped land cannot pay the tap cost");
            end_active_turn(&mut engine, 0);
            advance_to_main1_from_game_start(&mut engine);
            end_active_turn(&mut engine, 1);
            advance_to_main1_from_game_start(&mut engine);
            assert!(!engine.state.objects[&land].tapped, "{} untaps", case.id);
            engine
                .apply_scenario_command(0, &activate_mana_option(&engine, land, option as u32))
                .unwrap_or_else(|error| panic!("{} option {option}: {error:?}", case.id));

            assert_eq!(mana_pool(&engine), *expected, "{} option {option}", case.id);
            assert!(engine.state.objects[&land].tapped, "{}", case.id);
            assert!(engine.state.stack.is_empty(), "{}", case.id);
        }
    }
}

#[test]
fn botanical_plaza_batch_cards_reject_the_draw_without_the_exact_colored_cost() {
    for (case_index, case) in CASES.iter().enumerate() {
        let mut engine = engine(25_051_000 + case_index as u64);
        inject_card_into_hand(&mut engine, 0, case.id);
        let land = relocate_to_battlefield(&mut engine, 0, case.id, false);

        engine
            .apply_scenario_command(0, &activate_ability_for(&engine, land, 1, vec![]))
            .expect_err("the draw ability requires its mana cost");
        give_mana(
            &mut engine,
            0,
            ManaGift {
                c: 4,
                ..Default::default()
            },
        );
        engine
            .apply_scenario_command(0, &activate_ability_for(&engine, land, 1, vec![]))
            .expect_err("generic mana cannot pay the two printed colored symbols");

        assert_eq!(
            engine.state.objects[&land].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
        assert!(!engine.state.objects[&land].tapped, "{}", case.id);
        assert!(engine.state.players[0].graveyard.is_empty(), "{}", case.id);
        assert!(engine.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn botanical_plaza_batch_cards_sacrifice_then_draw_one() {
    for (case_index, case) in CASES.iter().enumerate() {
        let mut engine = engine(25_052_000 + case_index as u64);
        inject_card_into_hand(&mut engine, 0, case.id);
        let land = relocate_to_battlefield(&mut engine, 0, case.id, false);
        let top_card = *engine.state.players[0]
            .library
            .front()
            .expect("library has a card to draw");
        let library_size = engine.state.players[0].library.len();
        let hand_size = engine.state.players[0].hand.len();

        give_mana(
            &mut engine,
            0,
            ManaGift {
                w: case.exact.0,
                u: case.exact.1,
                b: case.exact.2,
                r: case.exact.3,
                g: case.exact.4,
                c: case.exact.5,
            },
        );
        engine
            .apply_scenario_command(0, &activate_ability_for(&engine, land, 1, vec![]))
            .unwrap_or_else(|error| panic!("{}: {error:?}", case.id));

        assert_eq!(
            engine.state.objects[&land].zone,
            Zone::Graveyard,
            "{}",
            case.id
        );
        assert!(
            engine.state.players[0].graveyard.contains(&land),
            "{}",
            case.id
        );
        assert!(engine.state.players[1].graveyard.is_empty(), "{}", case.id);
        assert_eq!(
            engine.state.players[0].library.len(),
            library_size,
            "{}",
            case.id
        );
        assert_eq!(engine.state.players[0].hand.len(), hand_size, "{}", case.id);
        assert_eq!(engine.state.stack.len(), 1, "{}", case.id);
        assert_eq!(mana_pool(&engine), (0, 0, 0, 0, 0, 0), "{}", case.id);

        resolve_entire_stack_two_player(&mut engine);

        assert_eq!(
            engine.state.players[0].library.len(),
            library_size - 1,
            "{}",
            case.id
        );
        assert!(
            engine.state.players[0].hand.contains(&top_card),
            "{}",
            case.id
        );
        assert_eq!(
            engine.state.players[0].hand.len(),
            hand_size + 1,
            "{}",
            case.id
        );
        assert!(engine.state.stack.is_empty(), "{}", case.id);
    }
}
