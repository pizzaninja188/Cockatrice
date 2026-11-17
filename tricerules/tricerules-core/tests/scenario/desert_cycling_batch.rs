//! Actual-card scenarios for the reviewed Desert cycling land cycle.
//!
//! Expectations are printed Oracle values. CR 614.1d and 614.12 make the
//! enters-tapped clause a replacement effect; CR 605.1a and 605.3b make the
//! mana ability resolve immediately; CR 702.29a-b define cycling as a
//! hand-zone activated ability with discard as a cost; CR 118.3 requires the
//! printed colored cycling cost. The pinned ruling confirms Desert is a
//! subtype with no intrinsic mana ability.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, AbilitySourceZone, ActivateAbility, RuledCommand,
};

type ManaPool = (u32, u32, u32, u32, u32, u32);

struct Case {
    id: &'static str,
    mana: ManaPool,
    cycling: ManaPool,
}

const CASES: [Case; 5] = [
    Case {
        id: "desert_of_the_fervent",
        mana: (0, 0, 0, 1, 0, 0),
        cycling: (0, 0, 0, 1, 0, 1),
    },
    Case {
        id: "desert_of_the_glorified",
        mana: (0, 0, 1, 0, 0, 0),
        cycling: (0, 0, 1, 0, 0, 1),
    },
    Case {
        id: "desert_of_the_indomitable",
        mana: (0, 0, 0, 0, 1, 0),
        cycling: (0, 0, 0, 0, 1, 1),
    },
    Case {
        id: "desert_of_the_mindful",
        mana: (0, 1, 0, 0, 0, 0),
        cycling: (0, 1, 0, 0, 0, 1),
    },
    Case {
        id: "desert_of_the_true",
        mana: (1, 0, 0, 0, 0, 0),
        cycling: (1, 0, 0, 0, 0, 1),
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

fn hand_ability(engine: &GameEngine, source: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ActivateAbility(ActivateAbility {
            source_object_id: source,
            source_zone: AbilitySourceZone::Hand as i32,
            expected_zone_change_generation: engine
                .state
                .zone_change_generation
                .get(&source)
                .copied()
                .unwrap_or(0),
            ability_index: 1,
            ..Default::default()
        })),
    }
}

fn advance_to_controller_next_main1(engine: &mut GameEngine) {
    end_active_turn(engine, 0);
    advance_to_main1_from_game_start(engine);
    assert_eq!(engine.state.active_player_id(), 1);
    end_active_turn(engine, 1);
    advance_to_main1_from_game_start(engine);
    assert_eq!(engine.state.active_player_id(), 0);
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
fn desert_cycling_batch_cards_enter_tapped_and_produce_their_color() {
    for (case_index, case) in CASES.iter().enumerate() {
        let mut engine = engine(25_040_000 + case_index as u64);
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
            .apply_scenario_command(0, &activate_mana_option(&engine, land, 0))
            .expect_err("a tapped land cannot pay the tap cost");
        advance_to_controller_next_main1(&mut engine);
        assert!(!engine.state.objects[&land].tapped, "{} untaps", case.id);
        engine
            .apply_scenario_command(0, &activate_mana_option(&engine, land, 0))
            .unwrap_or_else(|error| panic!("{} mana: {error:?}", case.id));
        assert_eq!(mana_pool(&engine), case.mana, "{}", case.id);
        assert!(engine.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn desert_cycling_batch_cards_reject_cycling_without_the_exact_colored_cost() {
    for (case_index, case) in CASES.iter().enumerate() {
        let mut engine = engine(25_041_000 + case_index as u64);
        let card = inject_card_into_hand(&mut engine, 0, case.id);
        let hand_size = engine.state.players[0].hand.len();

        engine
            .apply_scenario_command(0, &hand_ability(&engine, card))
            .expect_err("cycling requires its mana cost");
        give_mana(
            &mut engine,
            0,
            ManaGift {
                c: 2,
                ..Default::default()
            },
        );
        engine
            .apply_scenario_command(0, &hand_ability(&engine, card))
            .expect_err("generic mana cannot pay the printed colored symbol");

        assert_eq!(engine.state.objects[&card].zone, Zone::Hand, "{}", case.id);
        assert_eq!(engine.state.players[0].hand.len(), hand_size, "{}", case.id);
        assert_eq!(mana_pool(&engine), (0, 0, 0, 0, 0, 2), "{}", case.id);
        assert!(engine.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn desert_cycling_batch_cards_discard_then_draw_one() {
    for (case_index, case) in CASES.iter().enumerate() {
        let mut engine = engine(25_042_000 + case_index as u64);
        let card = inject_card_into_hand(&mut engine, 0, case.id);
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
                w: case.cycling.0,
                u: case.cycling.1,
                b: case.cycling.2,
                r: case.cycling.3,
                g: case.cycling.4,
                c: case.cycling.5,
            },
        );
        engine
            .apply_scenario_command(0, &hand_ability(&engine, card))
            .unwrap_or_else(|error| panic!("{} cycling: {error:?}", case.id));

        assert_eq!(
            engine.state.objects[&card].zone,
            Zone::Graveyard,
            "{}",
            case.id
        );
        assert!(
            engine.state.players[0].graveyard.contains(&card),
            "{}",
            case.id
        );
        assert_eq!(
            engine.state.players[0].hand.len(),
            hand_size - 1,
            "{}",
            case.id
        );
        assert_eq!(
            engine.state.players[0].library.len(),
            library_size,
            "{}",
            case.id
        );
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
        assert_eq!(engine.state.players[0].hand.len(), hand_size, "{}", case.id);
        assert!(engine.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn desert_cycling_batch_cards_cycle_only_from_hand() {
    for (case_index, case) in CASES.iter().enumerate() {
        let mut engine = engine(25_043_000 + case_index as u64);
        inject_card_into_hand(&mut engine, 0, case.id);
        let land = relocate_to_battlefield(&mut engine, 0, case.id, false);
        let before = engine.state.command_index;

        engine
            .apply_scenario_command(0, &activate_ability_for(&engine, land, 1, vec![]))
            .expect_err("the hand-only cycling ability is not offered from the battlefield");

        assert_eq!(engine.state.command_index, before, "{}", case.id);
        assert_eq!(
            engine.state.objects[&land].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
        assert!(!engine.state.objects[&land].tapped, "{}", case.id);
        assert!(engine.state.stack.is_empty(), "{}", case.id);
    }
}
