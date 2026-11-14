//! Actual-card scenarios for the reviewed two-basic-land entry cycle.
//!
//! Expectations are printed Oracle values. CR 305.6/305.8 distinguish basic land
//! subtypes from the Basic supertype; CR 614.1c applies the enter-tapped
//! replacement; CR 605.1a/605.3b make the tap mana ability resolve immediately.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::GameEngine;

type ManaOption = (u32, u32, u32, u32, u32, u32);

struct BattleLand {
    card: &'static str,
    mana_options: &'static [ManaOption],
}

const LANDS: [BattleLand; 6] = [
    BattleLand {
        card: "eclipsed_steppe",
        mana_options: &[(1, 0, 0, 0, 0, 0), (0, 0, 1, 0, 0, 0)],
    },
    BattleLand {
        card: "radiant_summit",
        mana_options: &[(0, 0, 0, 1, 0, 0), (1, 0, 0, 0, 0, 0)],
    },
    BattleLand {
        card: "scorched_geyser",
        mana_options: &[(0, 1, 0, 0, 0, 0), (0, 0, 0, 1, 0, 0)],
    },
    BattleLand {
        card: "smoldering_marsh",
        mana_options: &[(0, 0, 1, 0, 0, 0), (0, 0, 0, 1, 0, 0)],
    },
    BattleLand {
        card: "sodden_verdure",
        mana_options: &[(0, 0, 0, 0, 1, 0), (0, 1, 0, 0, 0, 0)],
    },
    BattleLand {
        card: "vernal_fen",
        mana_options: &[(0, 0, 1, 0, 0, 0), (0, 0, 0, 0, 1, 0)],
    },
];

fn land_engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("island", &[]), deck_with("mountain", &[])]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn add_basic_lands(engine: &mut GameEngine, player: usize, count: usize) {
    for basic in ["plains", "island"].into_iter().take(count) {
        inject_permanent_on_battlefield(engine, player, basic);
    }
}

fn enter_land(engine: &mut GameEngine, player: usize, card: &str) -> u32 {
    inject_card_into_hand(engine, player, card);
    let slot = hand_index_for_card(engine, player, card);
    semantic::accepted(engine, player as i32, &play_land(slot));
    battlefield_object_for_card(engine, player, card)
}

fn mana_option(engine: &GameEngine, source: u32, option: u32) -> RuledCommand {
    let mut command = activate_ability_for(engine, source, 0, vec![]);
    let Some(Cmd::ActivateAbility(ability)) = command.cmd.as_mut() else {
        unreachable!()
    };
    ability.mana_option_index = option;
    command
}

fn mana_pool(engine: &GameEngine) -> ManaOption {
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

#[test]
fn battle_land_cycle_batch_cards_enter_tapped_unless_two_basic_lands_are_controlled() {
    for (index, land) in LANDS.iter().enumerate() {
        for controlled_basics in 0..=2 {
            let mut engine = land_engine(20_260_925 + (index * 3 + controlled_basics) as u64);
            add_basic_lands(&mut engine, 0, controlled_basics);
            if controlled_basics < 2 {
                add_basic_lands(&mut engine, 1, 2);
            }
            let source = enter_land(&mut engine, 0, land.card);
            assert_eq!(
                engine.state.objects[&source].tapped,
                controlled_basics < 2,
                "{} enters tapped with {controlled_basics} basics controlled by its player",
                land.card
            );
            if controlled_basics < 2 {
                let command_index = engine.state.command_index;
                engine
                    .apply_scenario_command(0, &mana_option(&engine, source, 0))
                    .expect_err("a tapped entering land cannot pay a tap cost");
                assert_eq!(engine.state.command_index, command_index);
                assert!(engine.state.objects[&source].tapped);
            }
        }
    }
}

#[test]
fn battle_land_cycle_batch_cards_count_only_basic_lands_controlled_by_their_controller() {
    for (index, land) in LANDS.iter().enumerate() {
        let mut engine = land_engine(20_260_960 + index as u64);
        inject_permanent_on_battlefield(&mut engine, 0, "hallowed_fountain");
        inject_permanent_on_battlefield(&mut engine, 0, "scattered_groves");
        let source = enter_land(&mut engine, 0, land.card);
        assert!(
            engine.state.objects[&source].tapped,
            "{}: basic land subtypes do not make a nonbasic land count as basic",
            land.card
        );

        let mut engine = land_engine(20_260_970 + index as u64);
        add_basic_lands(&mut engine, 0, 2);
        add_basic_lands(&mut engine, 1, 1);
        end_active_turn(&mut engine, 0);
        advance_to_main1_from_game_start(&mut engine);
        assert_eq!(engine.state.active_player_id(), 1);
        let opponent_land = enter_land(&mut engine, 1, land.card);
        assert!(
            engine.state.objects[&opponent_land].tapped,
            "{}: the entering controller's one basic is not satisfied by the opponent's two",
            land.card
        );
    }
}

#[test]
fn battle_land_cycle_batch_cards_produce_each_printed_color_when_untapped() {
    for (land_index, land) in LANDS.iter().enumerate() {
        for (option_index, expected) in land.mana_options.iter().enumerate() {
            let mut engine = land_engine(20_260_940 + (land_index * 2 + option_index) as u64);
            add_basic_lands(&mut engine, 0, 2);
            let source = enter_land(&mut engine, 0, land.card);
            assert!(!engine.state.objects[&source].tapped);
            let command = mana_option(&engine, source, option_index as u32);
            semantic::accepted(&mut engine, 0, &command);
            assert_eq!(mana_pool(&engine), *expected, "{} mana option", land.card);
            assert!(
                engine.state.stack.is_empty(),
                "mana ability resolves at once"
            );
        }
    }
}
