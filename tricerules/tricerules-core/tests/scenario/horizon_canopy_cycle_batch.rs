//! Actual-card scenarios for the reviewed Horizon Canopy land cycle.
//!
//! Expectations are printed Oracle values. CR 605.1a and 605.3a-b make the
//! targetless mana abilities resolve immediately without using the stack;
//! CR 119.4 governs the one-life payment, CR 118.3 and 701.21a the tap and
//! sacrifice costs, and CR 121.1 the draw.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

type ManaPool = (u32, u32, u32, u32, u32, u32);

struct Case {
    id: &'static str,
    options: [ManaPool; 2],
}

const CASES: [Case; 5] = [
    Case {
        id: "fiery_islet",
        options: [(0, 1, 0, 0, 0, 0), (0, 0, 0, 1, 0, 0)],
    },
    Case {
        id: "horizon_canopy",
        options: [(0, 0, 0, 0, 1, 0), (1, 0, 0, 0, 0, 0)],
    },
    Case {
        id: "nurturing_peatland",
        options: [(0, 0, 1, 0, 0, 0), (0, 0, 0, 0, 1, 0)],
    },
    Case {
        id: "silent_clearing",
        options: [(1, 0, 0, 0, 0, 0), (0, 0, 1, 0, 0, 0)],
    },
    Case {
        id: "sunbaked_canyon",
        options: [(0, 0, 0, 1, 0, 0), (1, 0, 0, 0, 0, 0)],
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

fn engine(seed: u64, id: &str) -> (GameEngine, u32) {
    let decks = Some(vec![deck_with("forest", &[id]), deck_with("island", &[])]);
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
    let land = relocate_to_battlefield(&mut engine, 0, id, false);
    (engine, land)
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
fn horizon_canopy_cycle_batch_cards_pay_one_life_for_each_printed_color() {
    for (case_index, case) in CASES.iter().enumerate() {
        for (option, expected_pool) in case.options.iter().enumerate() {
            let (mut engine, land) = engine(25_000_000 + (case_index * 2 + option) as u64, case.id);

            engine
                .apply_scenario_command(0, &activate_mana_option(&engine, land, option as u32))
                .unwrap_or_else(|error| panic!("{} option {option}: {error:?}", case.id));

            assert_eq!(engine.state.players[0].life, 19, "{}", case.id);
            assert_eq!(
                mana_pool(&engine),
                *expected_pool,
                "{} option {option}",
                case.id
            );
            assert!(engine.state.objects[&land].tapped, "{}", case.id);
            assert!(
                engine.state.stack.is_empty(),
                "{}: mana abilities resolve immediately",
                case.id
            );
        }
    }
}

#[test]
fn horizon_canopy_cycle_batch_cards_reject_the_life_payment_at_zero_life() {
    for (case_index, case) in CASES.iter().enumerate() {
        let (mut engine, land) = engine(25_001_000 + case_index as u64, case.id);
        engine.state.players[0].life = 0;
        let before_pool = mana_pool(&engine);

        engine
            .apply_scenario_command(0, &activate_mana_option(&engine, land, 0))
            .expect_err("a player at zero life cannot pay one life");

        assert_eq!(engine.state.players[0].life, 0, "{}", case.id);
        assert_eq!(mana_pool(&engine), before_pool, "{}", case.id);
        assert!(!engine.state.objects[&land].tapped, "{}", case.id);
        assert!(engine.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn horizon_canopy_cycle_batch_cards_draw_after_sacrificing_the_source() {
    for (case_index, case) in CASES.iter().enumerate() {
        let (mut engine, land) = engine(25_002_000 + case_index as u64, case.id);
        let top_card = *engine.state.players[0]
            .library
            .front()
            .expect("library has a card to draw");
        let library_size = engine.state.players[0].library.len();
        let hand_size = engine.state.players[0].hand.len();
        let opponent_library = engine.state.players[1].library.len();
        let opponent_hand = engine.state.players[1].hand.len();
        let generation_before = engine
            .state
            .zone_change_generation
            .get(&land)
            .copied()
            .unwrap_or_default();

        engine
            .apply_scenario_command(0, &activate_ability_for(&engine, land, 1, vec![]))
            .expect_err("the draw ability requires one generic mana");
        assert_eq!(
            engine.state.objects[&land].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
        assert!(!engine.state.objects[&land].tapped, "{}", case.id);
        assert_eq!(
            engine.state.players[0].library.len(),
            library_size,
            "{}",
            case.id
        );
        assert_eq!(engine.state.players[0].hand.len(), hand_size, "{}", case.id);

        give_mana(
            &mut engine,
            0,
            ManaGift {
                c: 1,
                ..Default::default()
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
        assert_eq!(
            engine
                .state
                .zone_change_generation
                .get(&land)
                .copied()
                .unwrap_or_default(),
            generation_before + 1,
            "{}",
            case.id
        );
        assert_eq!(
            engine.state.players[0].library.len(),
            library_size,
            "{}",
            case.id
        );
        assert_eq!(engine.state.players[0].hand.len(), hand_size, "{}", case.id);
        assert_eq!(engine.state.stack.len(), 1, "{}", case.id);

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
        assert_eq!(
            engine.state.players[1].library.len(),
            opponent_library,
            "{}",
            case.id
        );
        assert_eq!(
            engine.state.players[1].hand.len(),
            opponent_hand,
            "{}",
            case.id
        );
        assert!(engine.state.stack.is_empty(), "{}", case.id);
    }
}
