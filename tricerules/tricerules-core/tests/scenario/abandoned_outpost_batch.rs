//! Actual-card scenarios for the reviewed Abandoned Outpost land cycle.
//!
//! Expectations are printed Oracle values. CR 614.1d and 614.12 make the
//! enters-tapped clause a replacement effect; CR 605.1a and 605.3b make both
//! targetless mana abilities resolve immediately; CR 118.3 and 701.21a require
//! the tap and sacrifice costs; CR 106.1a names the five any-color options.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

type ManaPool = (u32, u32, u32, u32, u32, u32);

struct Case {
    id: &'static str,
    color: ManaPool,
}

const CASES: [Case; 5] = [
    Case {
        id: "abandoned_outpost",
        color: (1, 0, 0, 0, 0, 0),
    },
    Case {
        id: "bog_wreckage",
        color: (0, 0, 1, 0, 0, 0),
    },
    Case {
        id: "ravaged_highlands",
        color: (0, 0, 0, 1, 0, 0),
    },
    Case {
        id: "seafloor_debris",
        color: (0, 1, 0, 0, 0, 0),
    },
    Case {
        id: "timberland_ruins",
        color: (0, 0, 0, 0, 1, 0),
    },
];

const ANY_COLOR: [ManaPool; 5] = [
    (1, 0, 0, 0, 0, 0),
    (0, 1, 0, 0, 0, 0),
    (0, 0, 1, 0, 0, 0),
    (0, 0, 0, 1, 0, 0),
    (0, 0, 0, 0, 1, 0),
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

fn activate_mana_option(engine: &GameEngine, land: u32, ability: u32, option: u32) -> RuledCommand {
    let mut command = activate_ability_for(engine, land, ability, vec![]);
    let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
        unreachable!()
    };
    activation.mana_option_index = option;
    command
}

#[test]
fn abandoned_outpost_batch_cards_enter_tapped_and_produce_their_color() {
    for (case_index, case) in CASES.iter().enumerate() {
        let mut engine = engine(25_090_000 + case_index as u64);
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
            .apply_scenario_command(0, &activate_mana_option(&engine, land, 0, 0))
            .expect_err("a tapped land cannot pay the tap cost");
        end_active_turn(&mut engine, 0);
        advance_to_main1_from_game_start(&mut engine);
        end_active_turn(&mut engine, 1);
        advance_to_main1_from_game_start(&mut engine);
        assert!(!engine.state.objects[&land].tapped, "{} untaps", case.id);
        engine
            .apply_scenario_command(0, &activate_mana_option(&engine, land, 0, 0))
            .unwrap_or_else(|error| panic!("{} mana: {error:?}", case.id));
        assert_eq!(mana_pool(&engine), case.color, "{}", case.id);
        assert!(engine.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn abandoned_outpost_batch_cards_sacrifice_for_any_printed_color() {
    for (case_index, case) in CASES.iter().enumerate() {
        for (option, expected) in ANY_COLOR.iter().enumerate() {
            let mut engine = engine(25_091_000 + (case_index * 5 + option) as u64);
            inject_card_into_hand(&mut engine, 0, case.id);
            let land = relocate_to_battlefield(&mut engine, 0, case.id, false);

            engine
                .apply_scenario_command(0, &activate_mana_option(&engine, land, 1, option as u32))
                .unwrap_or_else(|error| panic!("{} option {option}: {error:?}", case.id));

            assert_eq!(mana_pool(&engine), *expected, "{} option {option}", case.id);
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
            assert!(
                engine.state.stack.is_empty(),
                "{}: mana abilities resolve immediately",
                case.id
            );
        }
    }
}

#[test]
fn abandoned_outpost_batch_cards_reject_the_sacrifice_while_tapped() {
    for (case_index, case) in CASES.iter().enumerate() {
        let mut engine = engine(25_092_000 + case_index as u64);
        inject_card_into_hand(&mut engine, 0, case.id);
        let land = relocate_to_battlefield(&mut engine, 0, case.id, false);

        engine
            .apply_scenario_command(0, &activate_mana_option(&engine, land, 0, 0))
            .expect("tap for the printed color");
        let before = engine.state.command_index;
        engine
            .apply_scenario_command(0, &activate_mana_option(&engine, land, 1, 0))
            .expect_err("a tapped land cannot also pay the sacrifice ability");

        assert_eq!(engine.state.command_index, before, "{}", case.id);
        assert_eq!(
            engine.state.objects[&land].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
        assert!(engine.state.objects[&land].tapped, "{}", case.id);
        assert_eq!(mana_pool(&engine), case.color, "{}", case.id);
    }
}
