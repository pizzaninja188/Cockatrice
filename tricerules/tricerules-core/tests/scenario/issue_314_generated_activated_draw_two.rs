//! Issue #314 — the reviewed creature activated draw-two cohort.
//!
//! These scenarios exercise the generated `{3}{U}{U}` and `{6}{U}` battlefield
//! abilities through the engine command path.  CR 602.1/602.2 and 118.1-118.3
//! govern the activated cost and atomic mana payment; CR 113.7a makes the
//! ability independent after activation; CR 121.1-121.4 and 704.5b govern the
//! ordered draw and empty-library state-based action; CR 110.2 and 113.8 keep
//! control and the ability's draw recipient on the activating player.

use super::helpers::*;
use prost::Message;
use tricerules_core::{TurnStep, Zone};
use tricerules_proto::ruled::v1::CanonicalGameplayCommand;

fn put_on_top(engine: &mut GameEngine, player: usize, card_ids: &[&str]) -> Vec<u32> {
    let objects = card_ids
        .iter()
        .map(|card_id| inject_library_card(engine, player, card_id))
        .collect::<Vec<_>>();
    engine.state.players[player]
        .library
        .retain(|object_id| !objects.contains(object_id));
    for object_id in objects.iter().rev() {
        engine.state.players[player].library.push_front(*object_id);
    }
    objects
}

fn creature_engine(seed: u64, card_id: &str, draws: &[&str]) -> (GameEngine, u32, Vec<u32>) {
    let mut engine = GameEngine::new(seed, &[0, 1], 20, None, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let source = inject_creature_on_battlefield(&mut engine, 0, card_id);
    let drawn = put_on_top(&mut engine, 0, draws);
    (engine, source, drawn)
}

fn resolve_top_two_player(engine: &mut GameEngine) {
    let first = engine.state.priority_player_id();
    engine
        .apply_command(first, &pass())
        .expect("first player passes priority");
    let second = engine.state.priority_player_id();
    engine
        .apply_command(second, &pass())
        .expect("second player passes priority");
}

fn three_player_main1(seed: u64) -> GameEngine {
    let mut engine = GameEngine::new(seed, &[0, 1], 20, None, true).expect("new game");
    engine
        .state
        .players
        .push(tricerules_core::state::PlayerState::new(2, 20));
    while engine.state.turn_step != TurnStep::Main1 {
        let player = engine.state.priority_player_id();
        engine
            .apply_command(player, &pass())
            .expect("advance three-player turn");
    }
    engine
}

fn resolve_stack_three_player(engine: &mut GameEngine) {
    while !engine.state.stack.is_empty() {
        for _ in 0..engine.state.players.len() {
            if engine.state.stack.is_empty() {
                break;
            }
            let player = engine.state.priority_player_id();
            engine
                .apply_command(player, &pass())
                .expect("three-player priority pass");
        }
    }
}

fn canonical_command(inner: RuledCommand) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::CanonicalGameplay(CanonicalGameplayCommand {
            command: inner.encode_to_vec(),
            auto_pass_policies: Vec::new(),
        })),
    }
}

#[test]
fn issue_314_mystic_pays_exact_cost_and_draws_two_in_order() {
    let (mut engine, source, drawn) =
        creature_engine(314_001, "mystic_archaeologist", &["forest", "island"]);
    let hand_before = engine.state.players[0].hand.len();
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 3,
            u: 2,
            ..Default::default()
        },
    );

    engine
        .apply_command(0, &activate_ability_for(&engine, source, 0, Vec::new()))
        .expect("Mystic Archaeologist pays {3}{U}{U}");
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    assert_eq!(engine.state.players[0].mana_pool.blue, 0);
    assert_eq!(engine.state.players[0].hand.len(), hand_before);
    assert_eq!(engine.state.stack.len(), 1);
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);

    resolve_top_two_player(&mut engine);
    assert_eq!(engine.state.players[0].hand.len(), hand_before + 2);
    assert_eq!(engine.state.objects[&drawn[0]].zone, Zone::Hand);
    assert_eq!(engine.state.objects[&drawn[1]].zone, Zone::Hand);
    assert!(engine.state.stack.is_empty());
}

#[test]
fn issue_314_oscorp_requires_exact_blue_payment_and_rejects_atomic_misses() {
    let (mut wrong_color, source, _) = creature_engine(314_002, "oscorp_research_team", &[]);
    give_mana(
        &mut wrong_color,
        0,
        ManaGift {
            c: 7,
            ..Default::default()
        },
    );
    let before = wrong_color.state.players[0].mana_pool;
    wrong_color
        .apply_command(
            0,
            &activate_ability_for(&wrong_color, source, 0, Vec::new()),
        )
        .expect_err("colorless mana cannot pay Oscorp's blue pip");
    assert_eq!(wrong_color.state.players[0].mana_pool, before);
    assert!(wrong_color.state.stack.is_empty());

    let (mut short, source, _) = creature_engine(314_003, "oscorp_research_team", &[]);
    give_mana(
        &mut short,
        0,
        ManaGift {
            c: 5,
            u: 1,
            ..Default::default()
        },
    );
    let before = short.state.players[0].mana_pool;
    short
        .apply_command(0, &activate_ability_for(&short, source, 0, Vec::new()))
        .expect_err("five generic mana is insufficient for Oscorp");
    assert_eq!(short.state.players[0].mana_pool, before);
    assert!(short.state.stack.is_empty());

    let (mut exact, source, _) = creature_engine(314_004, "oscorp_research_team", &[]);
    give_mana(
        &mut exact,
        0,
        ManaGift {
            c: 7,
            u: 1,
            ..Default::default()
        },
    );
    exact
        .apply_command(0, &activate_ability_for(&exact, source, 0, Vec::new()))
        .expect("Oscorp pays six generic and one blue");
    assert_eq!(exact.state.players[0].mana_pool.colorless, 1);
    assert_eq!(exact.state.players[0].mana_pool.blue, 0);
    assert_eq!(exact.state.stack.len(), 1);
}

#[test]
fn issue_314_activation_is_controller_only_and_controlled_player_draws() {
    let (mut engine, source, _) = creature_engine(314_005, "mystic_archaeologist", &[]);
    engine
        .apply_command(0, &pass())
        .expect("P0 passes so the noncontroller holds priority");
    assert_eq!(engine.state.priority_player_id(), 1);
    give_mana(
        &mut engine,
        1,
        ManaGift {
            c: 3,
            u: 2,
            ..Default::default()
        },
    );
    let before = engine.state.players[1].mana_pool;
    let command_index_before = engine.state.command_index;
    engine
        .apply_command(1, &activate_ability_for(&engine, source, 0, Vec::new()))
        .expect_err("an opponent holding priority cannot activate a creature it does not control");
    assert_eq!(engine.state.players[1].mana_pool, before);
    assert_eq!(engine.state.command_index, command_index_before);
    assert_eq!(engine.state.priority_player_id(), 1);
    assert!(engine.state.stack.is_empty());
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);

    let mut engine = three_player_main1(314_006);
    let source = inject_creature_under_foreign_control(&mut engine, 0, 2, "mystic_archaeologist");
    let drawn = put_on_top(&mut engine, 2, &["forest", "island"]);
    let owner_hand_before = engine.state.players[0].hand.len();
    let controller_hand_before = engine.state.players[2].hand.len();
    give_mana(
        &mut engine,
        2,
        ManaGift {
            c: 3,
            u: 2,
            ..Default::default()
        },
    );

    engine
        .apply_command(0, &pass())
        .expect("P0 passes priority");
    engine
        .apply_command(1, &pass())
        .expect("P1 passes priority");
    engine
        .apply_command(2, &activate_ability_for(&engine, source, 0, Vec::new()))
        .expect("P2 controls and activates the source");
    assert_eq!(engine.state.objects[&source].owner, 0);
    assert_eq!(engine.state.objects[&source].controller, 2);
    assert_eq!(engine.state.players[2].mana_pool.colorless, 0);
    assert_eq!(engine.state.players[2].mana_pool.blue, 0);
    resolve_stack_three_player(&mut engine);

    assert_eq!(engine.state.players[0].hand.len(), owner_hand_before);
    assert_eq!(
        engine.state.players[2].hand.len(),
        controller_hand_before + 2
    );
    assert_eq!(engine.state.objects[&drawn[0]].zone, Zone::Hand);
    assert_eq!(engine.state.objects[&drawn[1]].zone, Zone::Hand);
}

#[test]
fn issue_314_cannot_activate_from_real_non_battlefield_zone_without_mutation() {
    let (mut engine, source, _) = creature_engine(314_011, "mystic_archaeologist", &[]);
    inject_card_into_hand(&mut engine, 0, "unsummon");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 3,
            u: 3,
            ..Default::default()
        },
    );
    let unsummon = hand_index_for_card(&engine, 0, "unsummon");
    engine
        .apply_command(0, &cast_spell(unsummon, target_object(source)))
        .expect("cast Unsummon to move the source out of the battlefield");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&source].zone, Zone::Hand);

    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 3,
            u: 2,
            ..Default::default()
        },
    );
    let mana_before = engine.state.players[0].mana_pool;
    let command_index_before = engine.state.command_index;
    let generation = engine.state.zone_change_generation[&source];
    engine
        .apply_command(0, &activate_ability_for(&engine, source, 0, Vec::new()))
        .expect_err("a creature activated ability is unavailable from its hand");
    assert_eq!(engine.state.players[0].mana_pool, mana_before);
    assert_eq!(engine.state.command_index, command_index_before);
    assert!(engine.state.stack.is_empty());
    assert_eq!(engine.state.priority_player_id(), 0);
    assert_eq!(engine.state.objects[&source].zone, Zone::Hand);
    assert_eq!(engine.state.zone_change_generation[&source], generation);
}

#[test]
fn issue_314_ability_draws_after_source_leaves_before_resolution() {
    let (mut engine, source, drawn) =
        creature_engine(314_007, "mystic_archaeologist", &["forest", "island"]);
    let hand_before = engine.state.players[0].hand.len();
    inject_card_into_hand(&mut engine, 0, "unsummon");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 3,
            u: 3,
            ..Default::default()
        },
    );
    engine
        .apply_command(0, &activate_ability_for(&engine, source, 0, Vec::new()))
        .expect("activate before responding");
    let unsummon = hand_index_for_card(&engine, 0, "unsummon");
    engine
        .apply_command(0, &cast_spell(unsummon, target_object(source)))
        .expect("return the source while its ability is on the stack");
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.objects[&source].zone, Zone::Hand);
    // Unsummon is cast from hand and returns the source to hand; the resolved
    // ability then adds the two drawn cards.  Relative to the pre-response
    // snapshot this is a net increase of three cards.
    assert_eq!(engine.state.players[0].hand.len(), hand_before + 3);
    assert_eq!(engine.state.objects[&drawn[0]].zone, Zone::Hand);
    assert_eq!(engine.state.objects[&drawn[1]].zone, Zone::Hand);
}

#[test]
fn issue_314_short_library_draws_available_card_then_commits_sba_loss() {
    let (mut engine, source, _) = creature_engine(314_008, "mystic_archaeologist", &[]);
    engine.state.players[0].library.clear();
    let drawn = put_on_top(&mut engine, 0, &["forest"])[0];
    let hand_before = engine.state.players[0].hand.len();
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 3,
            u: 2,
            ..Default::default()
        },
    );
    engine
        .apply_command(0, &activate_ability_for(&engine, source, 0, Vec::new()))
        .expect("activate draw-two ability");
    resolve_top_two_player(&mut engine);

    assert_eq!(engine.state.objects[&drawn].zone, Zone::Hand);
    assert_eq!(engine.state.players[0].hand.len(), hand_before + 1);
    assert!(engine.state.players[0].has_lost);
    assert_eq!(engine.state.winner, Some(1));
}

#[test]
fn issue_314_stale_generation_rejects_before_payment_and_commands_replay_identically() {
    let (mut stale, source, drawn) =
        creature_engine(314_009, "mystic_archaeologist", &["forest", "island"]);
    let initial_generation = stale
        .state
        .zone_change_generation
        .get(&source)
        .copied()
        .unwrap_or(0);
    let stale_command = activate_ability_for(&stale, source, 0, Vec::new());

    inject_card_into_hand(&mut stale, 0, "unsummon");
    give_mana(
        &mut stale,
        0,
        ManaGift {
            c: 3,
            u: 3,
            ..Default::default()
        },
    );
    let unsummon = hand_index_for_card(&stale, 0, "unsummon");
    stale
        .apply_command(0, &cast_spell(unsummon, target_object(source)))
        .expect("cast Unsummon to produce the first real zone change");
    resolve_entire_stack_two_player(&mut stale);
    assert_eq!(stale.state.objects[&source].zone, Zone::Hand);
    let hand_generation = stale.state.zone_change_generation[&source];
    assert_ne!(hand_generation, initial_generation);

    give_mana(
        &mut stale,
        0,
        ManaGift {
            c: 1,
            u: 1,
            ..Default::default()
        },
    );
    let source_hand_index = hand_index_for_card(&stale, 0, "mystic_archaeologist");
    stale
        .apply_command(0, &cast_spell(source_hand_index, Vec::new()))
        .expect("cast the same physical source for a fresh battlefield generation");
    resolve_entire_stack_two_player(&mut stale);
    assert_eq!(stale.state.objects[&source].zone, Zone::Battlefield);
    let fresh_generation = stale.state.zone_change_generation[&source];
    assert_ne!(fresh_generation, hand_generation);

    let pool_before_activation = stale.state.players[0].mana_pool;
    give_mana(
        &mut stale,
        0,
        ManaGift {
            c: 3,
            u: 2,
            ..Default::default()
        },
    );
    let mana_before = stale.state.players[0].mana_pool;
    let command_index_before = stale.state.command_index;
    stale
        .apply_command(0, &stale_command)
        .expect_err("a command from the old battlefield generation cannot be replayed");
    assert_eq!(stale.state.players[0].mana_pool, mana_before);
    assert_eq!(stale.state.command_index, command_index_before);
    assert!(stale.state.stack.is_empty());
    assert_eq!(stale.state.objects[&source].zone, Zone::Battlefield);
    assert_eq!(
        stale.state.zone_change_generation[&source],
        fresh_generation
    );

    stale
        .apply_command(0, &activate_ability_for(&stale, source, 0, Vec::new()))
        .expect("a command captured from the fresh battlefield generation is accepted");
    assert_eq!(
        stale.state.players[0].mana_pool, pool_before_activation,
        "fresh activation spends exactly the newly added mana"
    );
    assert_eq!(stale.state.stack.len(), 1);
    resolve_top_two_player(&mut stale);
    assert_eq!(stale.state.objects[&drawn[0]].zone, Zone::Hand);
    assert_eq!(stale.state.objects[&drawn[1]].zone, Zone::Hand);

    fn replay() -> (Vec<Vec<u8>>, usize, u64) {
        let (mut engine, source, drawn) =
            creature_engine(314_010, "mystic_archaeologist", &["forest", "island"]);
        let hand_before = engine.state.players[0].hand.len();
        give_mana(
            &mut engine,
            0,
            ManaGift {
                c: 3,
                u: 2,
                ..Default::default()
            },
        );
        let first = engine.state.priority_player_id();
        let second = if first == 0 { 1 } else { 0 };
        let commands = [
            (
                0,
                canonical_command(activate_ability_for(&engine, source, 0, Vec::new())),
            ),
            (first, canonical_command(pass())),
            (second, canonical_command(pass())),
        ];
        let mut responses = Vec::new();
        for (player, command) in commands {
            responses.push(
                engine
                    .apply_command(player, &command)
                    .expect("replay command accepted")
                    .encode_to_vec(),
            );
        }
        assert_eq!(engine.state.objects[&drawn[0]].zone, Zone::Hand);
        assert_eq!(engine.state.objects[&drawn[1]].zone, Zone::Hand);
        (
            responses,
            engine.state.players[0].hand.len() - hand_before,
            engine.state.command_index,
        )
    }

    assert_eq!(replay(), replay());
}
