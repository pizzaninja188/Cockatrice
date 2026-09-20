//! Issue #456 — the conditional dual-land mana family through the command boundary.
//!
//! Every expectation is the reviewed printed Oracle behavior, not a copy of generator output.
//! Exact Scryfall records and `rulings_uri` responses were fetched 2026-09-20 against pinned
//! snapshot `27bf3214-1271-490b-bdfe-c0be6c23d02e`; none returned a ruling. Governing CR
//! concepts: CR 605.1a / 605.3 (mana abilities at normal timing), CR 602.5 (activation
//! restrictions), CR 400.7 and 603.6 (source-relative "entered this turn" uses the exact object
//! generation), and CR 205.4a (only the Basic supertype counts for "a basic land").

use super::helpers::*;
use tricerules_core::{GameEngine, Zone};

/// `(w, u, b, r, g, c)`; a named alias keeps the reviewed fixtures readable.
type ManaOption = (u32, u32, u32, u32, u32, u32);

struct DualLand {
    card: &'static str,
    /// Printed option order of the conditional pair ability.
    colors: [ManaOption; 2],
}

const DUAL_LANDS: [DualLand; 4] = [
    DualLand {
        card: "dark_fortress",
        colors: [(0, 0, 1, 0, 0, 0), (0, 0, 0, 1, 0, 0)],
    },
    DualLand {
        card: "gathering_place",
        colors: [(0, 0, 0, 0, 1, 0), (1, 0, 0, 0, 0, 0)],
    },
    DualLand {
        card: "training_compound",
        colors: [(0, 0, 0, 1, 0, 0), (0, 0, 0, 0, 1, 0)],
    },
    DualLand {
        card: "gleaming_bastion",
        colors: [(1, 0, 0, 0, 0, 0), (0, 1, 0, 0, 0, 0)],
    },
];

/// P0 plays only nonbasic lands, so the "basic land" branch cannot be satisfied accidentally.
fn dual_land_engine(seed: u64) -> GameEngine {
    let decks = Some(vec![
        deck_with("tropical_island", &[]),
        deck_with("mountain", &[]),
    ]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

/// Play the reviewed land from hand through the engine so its real entry is recorded.
fn play_reviewed_land(engine: &mut GameEngine, card_id: &str) -> u32 {
    let object_id = inject_card_into_hand(engine, 0, card_id);
    let slot = hand_index_for_card(engine, 0, card_id);
    semantic::accepted(engine, 0, &play_land(slot));
    assert_eq!(engine.state.objects[&object_id].zone, Zone::Battlefield);
    object_id
}

fn mana_option(engine: &GameEngine, source: u32, ability_index: u32, option: u32) -> RuledCommand {
    let mut command = activate_ability_for(engine, source, ability_index, vec![]);
    let Some(Cmd::ActivateAbility(ability)) = command.cmd.as_mut() else {
        unreachable!()
    };
    ability.mana_option_index = option;
    command
}

fn pool(engine: &GameEngine) -> ManaOption {
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

/// Roll P0's turn over to P0's next main phase: P1 takes a full turn in between, and P0's
/// permanents untap. The played land's entry is no longer "this turn".
fn advance_to_controller_next_main1(engine: &mut GameEngine) {
    end_active_turn(engine, 0);
    advance_to_main1_from_game_start(engine);
    assert_eq!(engine.state.active_player_id(), 1);
    end_active_turn(engine, 1);
    advance_to_main1_from_game_start(engine);
    assert_eq!(engine.state.active_player_id(), 0);
}

#[test]
fn issue_456_entry_turn_branch_produces_each_pair_color_and_taps_the_source() {
    for (index, land) in DUAL_LANDS.into_iter().enumerate() {
        for (option_index, expected) in land.colors.into_iter().enumerate() {
            let mut engine = dual_land_engine(456_100 + (index * 2 + option_index) as u64);
            let source = play_reviewed_land(&mut engine, land.card);
            assert_eq!(
                zone_view_ability_flags(&mut engine, 0, source),
                [true, true],
                "{} entered this turn, so both branches hold",
                land.card
            );
            assert!(!engine.state.objects[&source].tapped);

            let hand_before = engine.state.players[0].hand.clone();
            let command = mana_option(&engine, source, 1, option_index as u32);
            semantic::accepted(&mut engine, 0, &command);
            assert_eq!(
                pool(&engine),
                expected,
                "{} printed option {option_index}",
                land.card
            );
            assert!(
                engine.state.objects[&source].tapped,
                "{} tapping is the mana cost",
                land.card
            );
            assert!(
                engine.state.stack.is_empty(),
                "mana abilities do not use the stack"
            );
            assert_eq!(
                engine.state.players[0].hand, hand_before,
                "{} activation consumes no card",
                land.card
            );
        }
    }
}

#[test]
fn issue_456_later_turn_without_a_basic_land_rejects_the_pair_but_keeps_colorless() {
    for (index, land) in DUAL_LANDS.into_iter().enumerate() {
        let mut engine = dual_land_engine(456_200 + index as u64);
        let source = play_reviewed_land(&mut engine, land.card);
        advance_to_controller_next_main1(&mut engine);
        assert!(!engine.state.objects[&source].tapped);

        assert_eq!(
            zone_view_ability_flags(&mut engine, 0, source),
            [true, false],
            "{} has neither branch on a later turn without a basic land",
            land.card
        );

        let pool_before = pool(&engine);
        let command_before = engine.state.command_index;
        engine
            .apply_command(0, &mana_option(&engine, source, 1, 0))
            .expect_err("the pair ability is restricted while neither branch holds");
        assert!(!engine.state.objects[&source].tapped);
        assert_eq!(pool(&engine), pool_before);
        assert_eq!(engine.state.command_index, command_before);

        // CR 605.3: the printed `{T}: Add {C}.` ability carries no condition.
        let colorless = mana_option(&engine, source, 0, 0);
        semantic::accepted(&mut engine, 0, &colorless);
        assert_eq!(pool(&engine), (0, 0, 0, 0, 0, 1), "{}", land.card);
        assert!(engine.state.objects[&source].tapped, "{}", land.card);
    }
}

#[test]
fn issue_456_later_turn_basic_land_branch_is_controller_relative() {
    for (index, land) in DUAL_LANDS.into_iter().enumerate() {
        let mut engine = dual_land_engine(456_300 + index as u64);
        let source = play_reviewed_land(&mut engine, land.card);
        advance_to_controller_next_main1(&mut engine);

        // An opponent's basic land does not satisfy "you control".
        inject_permanent_on_battlefield(&mut engine, 1, "mountain");
        assert_eq!(
            zone_view_ability_flags(&mut engine, 0, source),
            [true, false],
            "{}: an opponent's basic land is not the controller's",
            land.card
        );

        let forest = inject_permanent_on_battlefield(&mut engine, 0, "forest");
        assert_eq!(
            zone_view_ability_flags(&mut engine, 0, source),
            [true, true],
            "{}: a controlled basic land enables the pair",
            land.card
        );
        let pair = mana_option(&engine, source, 1, 0);
        semantic::accepted(&mut engine, 0, &pair);
        assert_eq!(pool(&engine), land.colors[0], "{}", land.card);
        assert!(engine.state.objects[&source].tapped, "{}", land.card);
        assert!(
            !engine.state.objects[&forest].tapped,
            "the enabling basic land is not tapped"
        );
    }
}
