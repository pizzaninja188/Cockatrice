//! Issue #254 — exact Standard utility-land recipes reuse established trigger, target,
//! private-library, damage, and mana-payment paths.
//!
//! Oracle and rulings checked 2026-09-11. Current CR 115.1d and 603.3d govern triggered
//! targets, 608.2b revalidates them, 701.22 and 701.25 define scry and surveil, 120.1
//! preserves the land as damage source, and 605.1a/605.3b govern the mana ability.

use super::helpers::*;
use tricerules_cards::primitives::{
    CastTriggerPlayer, ContinuousEffectKind, EffectDuration, Keyword, TriggerCondition,
};
use tricerules_cards::CardRegistry;
use tricerules_core::{AffectedScope, ContinuousEffect, Zone};
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, ChoiceKind, ChooseTriggerTarget, RuledCommand,
};

fn choose_trigger_targets(targets: Vec<TargetRef>) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            targets,
            ..Default::default()
        })),
    }
}

fn seat_on_top(engine: &mut GameEngine, player: usize, card_id: &str) -> u32 {
    let object_id = inject_library_card(engine, player, card_id);
    engine.state.players[player]
        .library
        .retain(|candidate| *candidate != object_id);
    engine.state.players[player].library.push_front(object_id);
    object_id
}

fn resolve_top_two_player(engine: &mut GameEngine) -> RuledEventBatch {
    engine
        .apply_command(engine.state.priority_player_id(), &pass())
        .expect("first pass");
    engine
        .apply_command(engine.state.priority_player_id(), &pass())
        .expect("second pass resolves")
}

fn add_third_player(engine: &mut GameEngine) {
    let mut third = engine.state.players[1].clone();
    third.id = 2;
    third.hand.clear();
    third.library.clear();
    third.battlefield.clear();
    third.graveyard.clear();
    engine.state.players.push(third);
}

#[test]
fn generated_lifeland_gains_exactly_once_on_entry() {
    let decks = Some(vec![
        deck_with("plains", &["scoured_barrens"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(254_001, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);

    move_ready_to_battlefield(&mut engine, 0, "scoured_barrens");
    assert_eq!(engine.state.stack.len(), 1);
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.players[0].life, 21);
    assert!(engine.state.stack.is_empty());

    engine
        .apply_command(0, &pass())
        .expect("ordinary priority pass");
    assert_eq!(engine.state.players[0].life, 21, "entry trigger fires once");
}

#[test]
fn generated_scry_and_surveil_keep_private_exact_library_identity_and_emit_surveil() {
    let scry_decks = Some(vec![
        deck_with("island", &["crystal_grotto"]),
        deck_with("forest", &[]),
    ]);
    let mut scry = GameEngine::new(254_002, &[0, 1], 20, scry_decks, true).expect("scry engine");
    advance_to_main1_from_game_start(&mut scry);
    let scried = seat_on_top(&mut scry, 0, "storm_crow");
    move_ready_to_battlefield(&mut scry, 0, "crystal_grotto");
    let batch = resolve_top_two_player(&mut scry);
    let choice = find_resolution_choice(&batch).expect("private scry choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibraryTop);
    assert_eq!(choice.deciding_player_id, 0);
    assert_eq!(choice.candidate_object_ids, [scried]);
    assert!(choice.public_reveal.is_none());
    scry.apply_command(0, &submit_resolution_choice(vec![scried]))
        .expect("put the exact card on bottom");
    assert_eq!(scry.state.players[0].library.back(), Some(&scried));

    let surveil_decks = Some(vec![
        deck_with("swamp", &["conduit_pylons"]),
        deck_with("forest", &[]),
    ]);
    let mut surveil =
        GameEngine::new(254_003, &[0, 1], 20, surveil_decks, true).expect("surveil engine");
    advance_to_main1_from_game_start(&mut surveil);
    let surveilled = seat_on_top(&mut surveil, 0, "storm_crow");
    let observer = inject_creature_on_battlefield(&mut surveil, 0, "grizzly_bears");
    let mut observer_ability = CardRegistry::global()
        .get("audacious_thief")
        .expect("observer definition")
        .primary_face()
        .triggered_abilities[0]
        .clone();
    observer_ability.trigger = TriggerCondition::WheneverPlayerSurveils {
        player: CastTriggerPlayer::Controller,
    };
    surveil.state.add_triggered_ability_grant(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(observer),
        kind: ContinuousEffectKind::GrantTriggeredAbility(Box::new(observer_ability)),
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: surveil.state.command_index,
    });
    move_ready_to_battlefield(&mut surveil, 0, "conduit_pylons");
    let batch = resolve_top_two_player(&mut surveil);
    let choice = find_resolution_choice(&batch).expect("private surveil choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibraryLook);
    assert_eq!(choice.deciding_player_id, 0);
    assert_eq!(choice.candidate_object_ids, [surveilled]);
    assert!(choice.public_reveal.is_none());
    surveil
        .apply_command(0, &submit_resolution_choice(vec![surveilled]))
        .expect("move exact surveilled card");
    assert_eq!(surveil.state.objects[&surveilled].zone, Zone::Graveyard);
    assert!(surveil
        .state
        .stack
        .iter()
        .any(|item| item.is_triggered && item.source_permanent_id == Some(observer)));
}

#[test]
fn generated_desert_targets_one_chosen_opponent_and_attributes_land_damage() {
    let decks = Some(vec![
        deck_with("island", &["lonely_arroyo"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(254_004, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    add_third_player(&mut engine);
    let land = move_ready_to_battlefield(&mut engine, 0, "lonely_arroyo");
    assert_eq!(engine.state.pending_triggers.len(), 1);
    engine
        .apply_command(0, &choose_trigger_targets(target_player(0)))
        .expect_err("controller is not an opponent");
    engine
        .apply_command(0, &choose_trigger_targets(target_player(2)))
        .expect("choose either legal opponent in multiplayer");
    assert_eq!(
        engine.state.stack.last().unwrap().source_permanent_id,
        Some(land)
    );

    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(land),
        kind: ContinuousEffectKind::Layer6AddKeyword(Keyword::Lifelink),
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: engine.state.command_index,
    });
    while !engine.state.stack.is_empty() {
        let player = engine.state.priority_player_id();
        engine
            .apply_command(player, &pass())
            .expect("multiplayer pass");
    }
    assert_eq!(
        engine
            .state
            .players
            .iter()
            .map(|player| player.life)
            .collect::<Vec<_>>(),
        [21, 20, 19],
        "lifelink proves the land remained the damage source"
    );
}

#[test]
fn generated_desert_revalidates_a_departed_player_target_before_resolution() {
    let decks = Some(vec![
        deck_with("swamp", &["jagged_barrens"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(254_005, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    add_third_player(&mut engine);
    move_ready_to_battlefield(&mut engine, 0, "jagged_barrens");
    engine
        .apply_command(0, &choose_trigger_targets(target_player(1)))
        .expect("choose opponent");
    engine.state.players[1].has_lost = true;
    while !engine.state.stack.is_empty() {
        let player = engine.state.priority_player_id();
        engine
            .apply_command(player, &pass())
            .expect("live player pass");
    }
    assert_eq!(
        engine.state.players[1].life, 20,
        "illegal target is not damaged"
    );
}

#[test]
fn generated_paid_mana_ability_is_atomic_and_offers_exactly_five_outputs() {
    for (option, expected) in [
        (0, [1, 0, 0, 0, 0]),
        (1, [0, 1, 0, 0, 0]),
        (2, [0, 0, 1, 0, 0]),
        (3, [0, 0, 0, 1, 0]),
        (4, [0, 0, 0, 0, 1]),
    ] {
        let mut engine =
            GameEngine::new(254_100 + option as u64, &[0, 1], 20, None, true).expect("engine");
        advance_to_main1_from_game_start(&mut engine);
        let land = inject_permanent_on_battlefield(&mut engine, 0, "crystal_grotto");

        let mut command = activate_ability_for(&engine, land, 1, vec![]);
        let Some(Cmd::ActivateAbility(ability)) = command.cmd.as_mut() else {
            unreachable!()
        };
        ability.mana_option_index = option;
        engine
            .apply_command(0, &command)
            .expect_err("generic mana cost is required");
        assert!(
            !engine.state.objects[&land].tapped,
            "failed payment is atomic"
        );

        give_mana(
            &mut engine,
            0,
            ManaGift {
                c: 1,
                ..Default::default()
            },
        );
        engine
            .apply_command(0, &command)
            .expect("activate paid mana option");
        let pool = &engine.state.players[0].mana_pool;
        assert_eq!(
            [pool.white, pool.blue, pool.black, pool.red, pool.green],
            expected
        );
        assert_eq!(pool.colorless, 0, "generic payment consumed the seed mana");
        assert!(engine.state.objects[&land].tapped);
        assert!(
            engine.state.stack.is_empty(),
            "mana ability resolves immediately"
        );
    }
}
