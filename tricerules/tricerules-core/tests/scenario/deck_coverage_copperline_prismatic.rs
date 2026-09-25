//! Exact deck-corpus coverage for a conditional mana land and a two-mode mana rock.
//!
//! Oracle and rulings checked 2026-09-25. CR 614.12 governs Copperline Gorge's conditional
//! entry replacement. CR 605.1a and 605.3a-b govern Prismatic Lens's activated mana abilities.

use super::helpers::*;
use tricerules_core::GameEngine;

type ManaPool = (u32, u32, u32, u32, u32, u32);

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

fn activate_with_mana_option(
    engine: &GameEngine,
    source: u32,
    ability_index: u32,
    option: u32,
) -> RuledCommand {
    let mut command = activate_ability_for(engine, source, ability_index, vec![]);
    let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
        unreachable!()
    };
    activation.mana_option_index = option;
    command
}

fn copperline_engine(seed: u64, controller_lands: usize) -> (GameEngine, u32) {
    let decks = Some(vec![
        deck_with("island", &["copperline_gorge"]),
        deck_with("island", &[]),
    ]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);

    for _ in 0..controller_lands {
        inject_permanent_on_battlefield(&mut engine, 0, "island");
    }
    for _ in 0..3 {
        inject_permanent_on_battlefield(&mut engine, 1, "island");
        inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    }

    let land = relocate_to_hand(&mut engine, 0, "copperline_gorge");
    let slot = hand_index_for_card(&engine, 0, "copperline_gorge");
    semantic::accepted(&mut engine, 0, &play_land(slot));
    (engine, land)
}

fn prismatic_engine(seed: u64) -> (GameEngine, u32) {
    let decks = Some(vec![
        deck_with("island", &["prismatic_lens"]),
        deck_with("mountain", &[]),
    ]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let lens = relocate_to_battlefield(&mut engine, 0, "prismatic_lens", false);
    (engine, lens)
}

#[test]
fn deck_coverage_copperline_gorge_counts_only_its_controllers_other_lands() {
    for (other_lands, option, expected_tapped, expected_pool) in [
        (0, Some(0), false, (0, 0, 0, 1, 0, 0)),
        (2, Some(1), false, (0, 0, 0, 0, 1, 0)),
        (3, None, true, (0, 0, 0, 0, 0, 0)),
    ] {
        let seed = 20_260_925 + other_lands as u64;
        let (mut engine, land) = copperline_engine(seed, other_lands);
        assert_eq!(
            engine.state.objects[&land].tapped, expected_tapped,
            "Copperline Gorge with {other_lands} other lands controlled by its player"
        );

        if let Some(option) = option {
            let command = activate_with_mana_option(&engine, land, 0, option);
            semantic::accepted(&mut engine, 0, &command);
            assert_eq!(mana_pool(&engine), expected_pool, "mana choice {option}");
            assert!(engine.state.objects[&land].tapped);
            assert!(
                engine.state.stack.is_empty(),
                "mana abilities do not use the stack"
            );
        } else {
            let before = engine.state.command_index;
            engine
                .apply_command(0, &activate_with_mana_option(&engine, land, 0, 0))
                .expect_err("three other controlled lands make Copperline enter tapped");
            assert_eq!(engine.state.command_index, before);
            assert_eq!(mana_pool(&engine), expected_pool);
            assert!(engine.state.objects[&land].tapped);
            assert!(engine.state.stack.is_empty());
        }
    }
}

#[test]
fn deck_coverage_prismatic_lens_taps_for_one_colorless_mana_immediately() {
    let (mut engine, lens) = prismatic_engine(20_260_926);
    let command = activate_with_mana_option(&engine, lens, 0, 0);
    semantic::accepted(&mut engine, 0, &command);

    assert_eq!(mana_pool(&engine), (0, 0, 0, 0, 0, 1));
    assert!(engine.state.objects[&lens].tapped);
    assert!(
        engine.state.stack.is_empty(),
        "mana abilities do not use the stack"
    );
}

#[test]
fn deck_coverage_prismatic_lens_pays_one_and_adds_each_color_option() {
    let colors = [
        (1, 0, 0, 0, 0, 0),
        (0, 1, 0, 0, 0, 0),
        (0, 0, 1, 0, 0, 0),
        (0, 0, 0, 1, 0, 0),
        (0, 0, 0, 0, 1, 0),
    ];

    for (option, expected) in colors.into_iter().enumerate() {
        let (mut engine, lens) = prismatic_engine(20_260_930 + option as u64);
        let unfunded = activate_with_mana_option(&engine, lens, 1, option as u32);
        let before = mana_pool(&engine);
        engine
            .apply_command(0, &unfunded)
            .expect_err("the colored-mana ability requires one generic mana");
        assert_eq!(mana_pool(&engine), before, "failed activation pays nothing");
        assert!(!engine.state.objects[&lens].tapped);
        assert!(engine.state.stack.is_empty());

        give_mana(
            &mut engine,
            0,
            ManaGift {
                c: 1,
                ..Default::default()
            },
        );
        let command = activate_with_mana_option(&engine, lens, 1, option as u32);
        semantic::accepted(&mut engine, 0, &command);
        assert_eq!(mana_pool(&engine), expected, "colored option {option}");
        assert!(engine.state.objects[&lens].tapped);
        assert!(
            engine.state.stack.is_empty(),
            "mana abilities resolve immediately"
        );
    }
}
