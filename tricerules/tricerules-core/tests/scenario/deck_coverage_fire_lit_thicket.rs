//! Exact deck-corpus coverage for Fire-Lit Thicket.
//!
//! Oracle and rulings checked against the pinned 2XM printing on 2026-09-26; Scryfall returned no
//! rulings. CR 107.4c/107.4e and 107.5 govern its colorless/hybrid symbols and tap cost, CR
//! 602.2b/601.2h govern payment, and CR 605.1a/605.3b classify and immediately resolve both mana
//! abilities.

use super::helpers::*;
use tricerules_core::{GameEngine, Zone};
use tricerules_proto::ruled::v1::ruled_command::Cmd;

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

fn fire_lit_thicket_engine(seed: u64) -> (GameEngine, u32) {
    let decks = Some(vec![deck_with("island", &[]), deck_with("forest", &[])]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    inject_card_into_hand(&mut engine, 0, "fire-lit_thicket");
    let slot = hand_index_for_card(&engine, 0, "fire-lit_thicket");
    engine
        .apply_command(0, &play_land(slot))
        .expect("play Fire-Lit Thicket as a land");
    let thicket = battlefield_object_for_card(&engine, 0, "fire-lit_thicket");
    (engine, thicket)
}

fn activate_mana_option(
    engine: &GameEngine,
    thicket: u32,
    ability_index: u32,
    option: u32,
) -> RuledCommand {
    let mut command = activate_ability_for(engine, thicket, ability_index, vec![]);
    let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
        unreachable!("constructed an activated ability command");
    };
    activation.mana_option_index = option;
    command
}

#[test]
fn fire_lit_thicket_taps_for_colorless_without_using_the_stack() {
    let (mut engine, thicket) = fire_lit_thicket_engine(20_260_950);

    apply_ability(&mut engine, 0, thicket, 0, vec![])
        .expect("activate Fire-Lit Thicket's colorless mana ability");

    assert_eq!(
        mana_pool(&engine),
        (0, 0, 0, 0, 0, 1),
        "the first ability adds exactly one colorless mana"
    );
    assert!(engine.state.objects[&thicket].tapped);
    assert_eq!(engine.state.objects[&thicket].zone, Zone::Battlefield);
    assert!(
        engine.state.stack.is_empty(),
        "the mana ability resolves immediately"
    );
}

#[test]
fn fire_lit_thicket_pays_red_or_green_for_each_of_its_three_mana_choices() {
    let cases = [
        (
            0,
            "red",
            ManaGift {
                r: 1,
                ..Default::default()
            },
            (0, 0, 0, 2, 0, 0),
        ),
        (
            0,
            "green",
            ManaGift {
                g: 1,
                ..Default::default()
            },
            (0, 0, 0, 2, 0, 0),
        ),
        (
            1,
            "red",
            ManaGift {
                r: 1,
                ..Default::default()
            },
            (0, 0, 0, 1, 1, 0),
        ),
        (
            1,
            "green",
            ManaGift {
                g: 1,
                ..Default::default()
            },
            (0, 0, 0, 1, 1, 0),
        ),
        (
            2,
            "red",
            ManaGift {
                r: 1,
                ..Default::default()
            },
            (0, 0, 0, 0, 2, 0),
        ),
        (
            2,
            "green",
            ManaGift {
                g: 1,
                ..Default::default()
            },
            (0, 0, 0, 0, 2, 0),
        ),
    ];

    for (offset, (option, payment_color, payment, expected_pool)) in cases.into_iter().enumerate() {
        let (mut engine, thicket) = fire_lit_thicket_engine(20_260_951 + offset as u64);
        give_mana(&mut engine, 0, payment);

        engine
            .apply_command(0, &activate_mana_option(&engine, thicket, 1, option))
            .expect("pay either half of {R/G}, tap, and choose the mana output");

        assert_eq!(
            mana_pool(&engine),
            expected_pool,
            "mana output option {option}, paid with {payment_color}"
        );
        assert!(engine.state.objects[&thicket].tapped);
        assert!(
            engine.state.stack.is_empty(),
            "the mana ability resolves immediately"
        );
    }
}

#[test]
fn fire_lit_thicket_rejects_unpaid_or_invalid_hybrid_activations_atomically() {
    let (mut engine, thicket) = fire_lit_thicket_engine(20_260_954);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    let before_command = engine.state.command_index;

    engine
        .apply_command(0, &activate_mana_option(&engine, thicket, 1, 0))
        .expect_err("colorless mana cannot pay the {R/G} hybrid cost");
    assert_eq!(engine.state.command_index, before_command);
    assert_eq!(mana_pool(&engine), (0, 0, 0, 0, 0, 1));
    assert!(!engine.state.objects[&thicket].tapped);
    assert!(engine.state.stack.is_empty());

    let (mut engine, thicket) = fire_lit_thicket_engine(20_260_955);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let before_command = engine.state.command_index;

    let error = engine
        .apply_command(0, &activate_mana_option(&engine, thicket, 1, 3))
        .expect_err("the ability offers only three mana output choices");
    assert!(
        matches!(
            error,
            tricerules_core::EngineError::Illegal("invalid mana option")
        ),
        "unexpected rejection: {error:?}"
    );
    assert_eq!(engine.state.command_index, before_command);
    assert_eq!(mana_pool(&engine), (0, 0, 0, 1, 0, 0));
    assert!(!engine.state.objects[&thicket].tapped);
    assert!(engine.state.stack.is_empty());
}
