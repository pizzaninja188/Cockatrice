//! Exact deck-corpus coverage for Rootbound Crag and Hinterland Harbor.
//!
//! Oracle text and rulings are pinned to Scryfall. The Rootbound Crag ruling confirms that lands
//! with the named land type count even when nonbasic, and that simultaneous entrants are not
//! counted. Conditions use the current battlefield snapshot (CR 305.5–305.6 and 614.12).

use super::helpers::*;
use tricerules_core::GameEngine;

type ManaOption = (u32, u32, u32, u32, u32, u32);

struct CheckLand {
    card: &'static str,
    qualifying_lands: &'static [&'static str],
    mana_options: &'static [ManaOption],
}

const LANDS: [CheckLand; 2] = [
    CheckLand {
        card: "rootbound_crag",
        qualifying_lands: &["mountain", "forest"],
        mana_options: &[(0, 0, 0, 1, 0, 0), (0, 0, 0, 0, 1, 0)],
    },
    CheckLand {
        card: "hinterland_harbor",
        qualifying_lands: &["forest", "island"],
        mana_options: &[(0, 0, 0, 0, 1, 0), (0, 1, 0, 0, 0, 0)],
    },
];

fn land_engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("island", &[]), deck_with("mountain", &[])]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine
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

#[test]
fn deck_coverage_checklands_require_one_of_their_named_land_types() {
    for (land_index, land) in LANDS.iter().enumerate() {
        let mut engine = land_engine(20_260_925 + land_index as u64);
        let source = enter_land(&mut engine, 0, land.card);
        assert!(
            engine.state.objects[&source].tapped,
            "{} enters tapped with no qualifying land type",
            land.card
        );

        for (type_index, qualifying_land) in land.qualifying_lands.iter().enumerate() {
            let mut engine = land_engine(20_260_930 + (land_index * 2 + type_index) as u64);
            inject_permanent_on_battlefield(&mut engine, 0, qualifying_land);
            let source = enter_land(&mut engine, 0, land.card);
            assert!(
                !engine.state.objects[&source].tapped,
                "{} enters untapped with {} controlled",
                land.card, qualifying_land
            );
        }
    }
}

#[test]
fn deck_coverage_checklands_count_nonbasic_lands_with_the_required_subtypes() {
    for (land_index, land) in LANDS.iter().enumerate() {
        let mut engine = land_engine(20_260_940 + land_index as u64);
        inject_permanent_on_battlefield(&mut engine, 0, "canopy_vista");
        let source = enter_land(&mut engine, 0, land.card);
        assert!(
            !engine.state.objects[&source].tapped,
            "{} counts Canopy Vista's Forest subtype even though it is nonbasic",
            land.card
        );
    }
}

#[test]
fn deck_coverage_checklands_ignore_qualifying_lands_controlled_by_opponents() {
    for (land_index, land) in LANDS.iter().enumerate() {
        let mut engine = land_engine(20_260_950 + land_index as u64);
        inject_permanent_on_battlefield(&mut engine, 1, land.qualifying_lands[0]);
        let source = enter_land(&mut engine, 0, land.card);
        assert!(
            engine.state.objects[&source].tapped,
            "{} does not count the opponent's qualifying land",
            land.card
        );
    }
}

#[test]
fn deck_coverage_checklands_produce_each_color_and_tapped_lands_cannot_activate() {
    for (land_index, land) in LANDS.iter().enumerate() {
        for (option_index, expected) in land.mana_options.iter().enumerate() {
            let mut engine = land_engine(20_260_960 + (land_index * 2 + option_index) as u64);
            inject_permanent_on_battlefield(&mut engine, 0, "forest");
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

        let mut engine = land_engine(20_260_970 + land_index as u64);
        let source = enter_land(&mut engine, 0, land.card);
        let command_index = engine.state.command_index;
        engine
            .apply_command(0, &mana_option(&engine, source, 0))
            .expect_err("a tapped entering land cannot pay a tap cost");
        assert_eq!(engine.state.command_index, command_index);
        assert!(engine.state.objects[&source].tapped);
    }
}
