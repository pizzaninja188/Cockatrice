//! Actual-card coverage for Raging Ravine's enters-tapped, mana, and animation abilities.
//!
//! Exact Oracle text and three WotC rulings checked against Scryfall on 2026-09-30.
//! CR 611.2a and 613.1d-g govern its temporary animation; CR 302.6 and 508.1 govern
//! summoning sickness for the resulting creature and declaring it as an attacker.

use super::helpers::*;
use tricerules_cards::{Color, CounterKind};
use tricerules_core::{GameEngine, TurnStep};

const RAGING_RAVINE: &str = "raging_ravine";

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![
        deck_with("forest", &[RAGING_RAVINE, "mountain", "mountain"]),
        deck_with("island", &[]),
    ]);
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

fn animate_ravine(engine: &mut GameEngine, ravine: u32) {
    give_mana(
        engine,
        0,
        ManaGift {
            c: 2,
            r: 1,
            g: 1,
            ..Default::default()
        },
    );
    apply_ability(engine, 0, ravine, 1, vec![]).expect("activate Raging Ravine");
    resolve_entire_stack_two_player(engine);
}

#[test]
fn raging_ravine_enters_tapped() {
    let mut engine = engine(20_260_930);
    ensure_in_hand(&mut engine, 0, RAGING_RAVINE);
    let card = hand_index_for_card(&engine, 0, RAGING_RAVINE);
    engine
        .apply_command(0, &play_land(card))
        .expect("play Raging Ravine");
    let ravine = battlefield_object_for_card(&engine, 0, RAGING_RAVINE);
    assert!(engine.state.objects[&ravine].tapped);
    assert!(engine
        .characteristics(ravine)
        .expect("land")
        .has_type("Land"));
}

#[test]
fn raging_ravine_repeated_animations_grant_matching_attack_triggers_and_counters_persist() {
    let mut engine = engine(20_260_931);
    let ravine = inject_permanent_on_battlefield(&mut engine, 0, RAGING_RAVINE);
    animate_ravine(&mut engine, ravine);
    animate_ravine(&mut engine, ravine);
    let animated = engine.characteristics(ravine).expect("animated Ravine");
    assert!(animated.has_type("Land") && animated.is_creature());
    assert!(animated.has_type("Elemental"));
    assert_eq!(animated.colors, [Color::Red, Color::Green]);
    assert_eq!((animated.power, animated.toughness), (Some(3), Some(3)));

    engine
        .apply_command(0, &primitive_yield())
        .expect("move to beginning of combat");
    pass_both_players(&mut engine);
    assert_eq!(engine.state.turn_step, TurnStep::DeclareAttackers);
    engine
        .apply_command(0, &declare_attackers(vec![ravine]))
        .expect("attack with animated Raging Ravine");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&ravine]
            .counters
            .get(&CounterKind::PlusOnePlusOne)
            .copied()
            .unwrap_or_default(),
        2,
        "each activation grants another instance of the attack trigger"
    );

    let turn = engine.state.turn;
    for _ in 0..30 {
        if engine.state.turn > turn {
            break;
        }
        if engine.state.turn_step == TurnStep::Cleanup {
            resolve_cleanup_discards_if_any(&mut engine);
        }
        pass_priority_round(&mut engine);
    }
    assert!(
        engine.state.turn > turn,
        "the turn advances through cleanup"
    );
    let after_expiry = engine.characteristics(ravine).expect("Ravine after turn");
    assert!(after_expiry.has_type("Land") && !after_expiry.is_creature());
    assert_eq!(
        engine.state.objects[&ravine]
            .counters
            .get(&CounterKind::PlusOnePlusOne)
            .copied()
            .unwrap_or_default(),
        2,
        "counters remain after the animation expires"
    );
}

#[test]
fn raging_ravine_mana_ability_offers_red_or_green_without_using_the_stack() {
    for (choice, expected) in [(0, (0, 0, 0, 1, 0, 0)), (1, (0, 0, 0, 0, 1, 0))] {
        let mut engine = engine(20_260_932 + choice as u64);
        let ravine = inject_permanent_on_battlefield(&mut engine, 0, RAGING_RAVINE);
        let mut command = activate_ability_for(&engine, ravine, 0, vec![]);
        let Some(tricerules_proto::ruled::v1::ruled_command::Cmd::ActivateAbility(activation)) =
            command.cmd.as_mut()
        else {
            unreachable!("constructed a mana ability activation")
        };
        activation.mana_option_index = choice;
        engine
            .apply_command(0, &command)
            .expect("choose Raging Ravine's mana color");

        let pool = &engine.state.players[0].mana_pool;
        assert_eq!(
            (
                pool.white,
                pool.blue,
                pool.black,
                pool.red,
                pool.green,
                pool.colorless
            ),
            expected
        );
        assert!(engine.state.objects[&ravine].tapped);
        assert!(engine.state.stack.is_empty());
    }
}

#[test]
fn raging_ravine_creature_obeys_summoning_sickness() {
    let mut engine = engine(20_260_933);
    let ravine = inject_permanent_on_battlefield(&mut engine, 0, RAGING_RAVINE);
    engine
        .state
        .objects
        .get_mut(&ravine)
        .expect("Ravine")
        .summoning_sick = true;
    animate_ravine(&mut engine, ravine);
    assert!(engine
        .characteristics(ravine)
        .expect("animated Ravine")
        .is_creature());

    let mana = activate_ability_for(&engine, ravine, 0, vec![]);
    assert!(engine.apply_command(0, &mana).is_err());
    assert!(!engine.state.objects[&ravine].tapped);

    inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    engine
        .apply_command(0, &primitive_yield())
        .expect("move to beginning of combat");
    pass_both_players(&mut engine);
    assert_eq!(engine.state.turn_step, TurnStep::DeclareAttackers);
    assert!(engine
        .apply_command(0, &declare_attackers(vec![ravine]))
        .is_err());
    assert!(engine
        .state
        .combat
        .as_ref()
        .expect("combat state")
        .attacking
        .is_empty());
}
