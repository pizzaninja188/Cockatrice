//! Exact deck-corpus coverage for two enters-tapped three-color lands.
//!
//! Expectations come from the pinned Oracle text and current Comprehensive Rules: CR 614.1c
//! (enters-tapped replacement) and CR 605.1a/605.3b (activated mana abilities resolve immediately).

use super::helpers::*;
use tricerules_core::GameEngine;

type ManaOption = (u32, u32, u32, u32, u32, u32);

struct ThreeColorLand {
    card: &'static str,
    mana_options: &'static [ManaOption],
}

const LANDS: [ThreeColorLand; 2] = [
    ThreeColorLand {
        card: "arcane_sanctum",
        mana_options: &[(1, 0, 0, 0, 0, 0), (0, 1, 0, 0, 0, 0), (0, 0, 1, 0, 0, 0)],
    },
    ThreeColorLand {
        card: "seaside_citadel",
        mana_options: &[(0, 0, 0, 0, 1, 0), (1, 0, 0, 0, 0, 0), (0, 1, 0, 0, 0, 0)],
    },
];

fn land_engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("island", &[]), deck_with("mountain", &[])]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine
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
fn deck_coverage_tri_lands_enter_tapped_and_produce_each_printed_color() {
    for (land_index, land) in LANDS.iter().enumerate() {
        for (option_index, expected) in land.mana_options.iter().enumerate() {
            let mut engine = land_engine(20_260_925 + (land_index * 3 + option_index) as u64);
            let source = inject_card_into_hand(&mut engine, 0, land.card);
            let slot = hand_index_for_card(&engine, 0, land.card);
            semantic::accepted(&mut engine, 0, &play_land(slot));

            assert!(
                engine.state.objects[&source].tapped,
                "{} enters tapped",
                land.card
            );
            let before = engine.state.command_index;
            engine
                .apply_command(0, &mana_option(&engine, source, option_index as u32))
                .expect_err("a tapped land cannot pay the tap cost of its mana ability");
            assert_eq!(engine.state.command_index, before);
            assert!(engine.state.objects[&source].tapped);

            advance_to_controller_next_main1(&mut engine);
            assert!(
                !engine.state.objects[&source].tapped,
                "{} untaps naturally",
                land.card
            );
            let command = mana_option(&engine, source, option_index as u32);
            semantic::accepted(&mut engine, 0, &command);
            assert_eq!(mana_pool(&engine), *expected, "{} mana option", land.card);
            assert!(engine.state.objects[&source].tapped);
            assert!(
                engine.state.stack.is_empty(),
                "mana ability resolves immediately"
            );
        }
    }
}
