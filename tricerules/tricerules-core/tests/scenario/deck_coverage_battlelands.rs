//! Exact deck-corpus coverage for four lands that check for two controlled basic lands.
//!
//! Oracle text is pinned to the Scryfall corpus; the shared August 25, 2015 ruling was checked
//! on the card reference. Expectations follow CR 305.6 and 305.8 (basic land type versus Basic
//! supertype) and CR 614.1c (enter-tapped replacement).

use super::helpers::*;
use tricerules_core::GameEngine;

type ManaOption = (u32, u32, u32, u32, u32, u32);

struct BattleLand {
    card: &'static str,
    mana_options: &'static [ManaOption],
}

const LANDS: [BattleLand; 4] = [
    BattleLand {
        card: "canopy_vista",
        mana_options: &[(0, 0, 0, 0, 1, 0), (1, 0, 0, 0, 0, 0)],
    },
    BattleLand {
        card: "cinder_glade",
        mana_options: &[(0, 0, 0, 1, 0, 0), (0, 0, 0, 0, 1, 0)],
    },
    BattleLand {
        card: "prairie_stream",
        mana_options: &[(1, 0, 0, 0, 0, 0), (0, 1, 0, 0, 0, 0)],
    },
    BattleLand {
        card: "sunken_hollow",
        mana_options: &[(0, 1, 0, 0, 0, 0), (0, 0, 1, 0, 0, 0)],
    },
];

fn land_engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("island", &[]), deck_with("mountain", &[])]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn add_basic_lands(engine: &mut GameEngine, player: usize, count: usize) {
    for basic in ["plains", "island"].into_iter().take(count) {
        inject_permanent_on_battlefield(engine, player, basic);
    }
}

fn enter_land(engine: &mut GameEngine, player: usize, card: &str) -> u32 {
    let source = inject_card_into_hand(engine, player, card);
    let slot = hand_index_for_card(engine, player, card);
    semantic::accepted(engine, player as i32, &play_land(slot));
    source
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

fn advance_to_controller_next_main1(engine: &mut GameEngine) {
    end_active_turn(engine, 0);
    advance_to_main1_from_game_start(engine);
    assert_eq!(engine.state.active_player_id(), 1);
    end_active_turn(engine, 1);
    advance_to_main1_from_game_start(engine);
    assert_eq!(engine.state.active_player_id(), 0);
}

#[test]
fn deck_coverage_battlelands_check_basic_lands_controlled_by_the_entering_player() {
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
                    .apply_command(0, &mana_option(&engine, source, 0))
                    .expect_err("a tapped entering land cannot pay a tap cost");
                assert_eq!(engine.state.command_index, command_index);
                assert!(engine.state.objects[&source].tapped);

                advance_to_controller_next_main1(&mut engine);
                let command = mana_option(&engine, source, 0);
                semantic::accepted(&mut engine, 0, &command);
                assert_eq!(mana_pool(&engine), land.mana_options[0]);
            }
        }
    }
}

#[test]
fn deck_coverage_battlelands_produce_each_land_type_color_when_untapped() {
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

#[test]
fn deck_coverage_battlelands_do_not_count_nonbasic_lands_with_basic_land_types() {
    let mut engine = land_engine(20_260_960);
    inject_permanent_on_battlefield(&mut engine, 0, "hallowed_fountain");
    inject_permanent_on_battlefield(&mut engine, 0, "scattered_groves");
    let source = enter_land(&mut engine, 0, "canopy_vista");

    assert!(
        engine.state.objects[&source].tapped,
        "Forest and Plains subtypes do not make nonbasic lands count as basic"
    );
}

#[test]
fn deck_coverage_battlelands_use_the_entering_lands_controller_not_the_opponent() {
    let mut engine = land_engine(20_260_961);
    add_basic_lands(&mut engine, 0, 2);
    add_basic_lands(&mut engine, 1, 1);

    end_active_turn(&mut engine, 0);
    advance_to_main1_from_game_start(&mut engine);
    assert_eq!(engine.state.active_player_id(), 1);

    let source = enter_land(&mut engine, 1, "canopy_vista");
    assert!(
        engine.state.objects[&source].tapped,
        "the opponent's two basics do not satisfy the entering controller's condition"
    );
}
