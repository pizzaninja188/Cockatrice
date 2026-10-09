//! Exact deck-corpus coverage for opponent-draw triggers.
//!
//! The Oracle text and Scryfall rulings for these printings were checked in the batch preflight.
//! Separate draws create separate triggers; Consecrated Sphinx offers its full two-card draw on
//! each resolution, while Mind's Eye independently offers one generic mana for each draw.

use super::helpers::*;
use tricerules_cards::{Color, Keyword, Layout};
use tricerules_core::{GameEngine, TurnStep};
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, ResolutionChoiceDecision, RuledCommand, SubmitResolutionChoice,
};

fn select_branch_with_index(index: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            chosen_object_ids: Vec::new(),
            decision: ResolutionChoiceDecision::SelectBranch as i32,
            selected_branch_index: index,
            cast_spell: None,
            spell_cast_announcement: None,
            chosen_combat_defender: None,
            payment: None,
            restricted_mana: vec![],
            chosen_player_ids: vec![],
        })),
    }
}

fn assert_rejected_choice_preserves_state(
    engine: &mut GameEngine,
    player: i32,
    command: &RuledCommand,
) {
    assert!(engine.state.pending_resolution.is_some());
    let pending_before = format!("{:?}", engine.state.pending_resolution);
    let stack_before = format!("{:?}", engine.state.stack);
    let hand_before = engine.state.players[0].hand.clone();
    let library_before = engine.state.players[0].library.clone();

    assert!(
        engine.apply_command(player, command).is_err(),
        "invalid resolution choice must be rejected"
    );

    assert_eq!(
        format!("{:?}", engine.state.pending_resolution),
        pending_before
    );
    assert_eq!(format!("{:?}", engine.state.stack), stack_before);
    assert_eq!(engine.state.players[0].hand, hand_before);
    assert_eq!(engine.state.players[0].library, library_before);
}

fn mana_pool(engine: &GameEngine, player: usize) -> (u32, u32, u32, u32, u32, u32) {
    let pool = &engine.state.players[player].mana_pool;
    (
        pool.white,
        pool.blue,
        pool.black,
        pool.red,
        pool.green,
        pool.colorless,
    )
}

fn advance_to_player_main1(engine: &mut GameEngine, player: i32) {
    for _ in 0..40 {
        if engine.state.turn_step == TurnStep::Main1 && engine.state.active_player_id() == player {
            return;
        }

        answer_trigger_order_in_engine_order(engine);
        let (actor, command) = match engine.state.cleanup_discard_player {
            Some(discarding_player) => {
                let player_index = engine
                    .state
                    .player_idx(discarding_player)
                    .expect("cleanup discard player");
                let excess = engine.state.players[player_index].hand.len() - 7;
                (
                    discarding_player,
                    discard_cleanup_batch((0..excess as u32).collect()),
                )
            }
            None => (engine.state.priority_player_id(), pass()),
        };
        engine
            .apply_command(actor, &command)
            .expect("pass priority toward the requested player's main phase");
    }
    panic!("game did not reach player {player}'s main phase");
}

fn opponent_divination_engine(seed: u64, permanent: &str) -> (GameEngine, usize, usize) {
    let decks = Some(vec![
        deck_with("island", &[permanent]),
        deck_with("island", &["divination"]),
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

    end_active_turn(&mut engine, 0);
    advance_to_player_main1(&mut engine, 1);
    relocate_to_battlefield(&mut engine, 0, permanent, false);
    ensure_in_hand(&mut engine, 1, "divination");
    let controller_hand_before = engine.state.players[0].hand.len();
    let controller_library_before = engine.state.players[0].library.len();

    give_mana(
        &mut engine,
        1,
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    let divination = hand_index_for_card(&engine, 1, "divination");
    engine
        .apply_command(1, &cast_spell(divination, Vec::new()))
        .expect("opponent casts Divination");
    pass_both_players(&mut engine);
    answer_trigger_order_in_engine_order(&mut engine);

    assert_eq!(
        engine.state.stack.len(),
        2,
        "one trigger for each drawn card"
    );
    assert!(engine.state.pending_resolution.is_none());
    (engine, controller_hand_before, controller_library_before)
}

#[test]
fn scrawling_crawler_upkeep_draws_for_each_player_and_each_opponent_loses_one_life() {
    let decks = Some(vec![
        deck_with("island", &["scrawling_crawler"]),
        deck_with("island", &[]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        202_609_271,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    relocate_to_battlefield(&mut engine, 0, "scrawling_crawler", false);

    end_active_turn(&mut engine, 0);
    advance_to_player_main1(&mut engine, 1);
    end_active_turn(&mut engine, 1);

    assert_eq!(engine.state.turn_step, TurnStep::Upkeep);
    assert_eq!(
        engine.state.stack.len(),
        1,
        "Crawler's upkeep trigger is waiting"
    );
    let hands_before: Vec<_> = engine
        .state
        .players
        .iter()
        .map(|player| player.hand.len())
        .collect();
    let libraries_before: Vec<_> = engine
        .state
        .players
        .iter()
        .map(|player| player.library.len())
        .collect();
    let lives_before: Vec<_> = engine
        .state
        .players
        .iter()
        .map(|player| player.life)
        .collect();

    resolve_entire_stack_two_player(&mut engine);

    for player in 0..2 {
        assert_eq!(
            engine.state.players[player].hand.len(),
            hands_before[player] + 1
        );
        assert_eq!(
            engine.state.players[player].library.len(),
            libraries_before[player] - 1
        );
    }
    assert_eq!(engine.state.players[0].life, lives_before[0]);
    assert_eq!(engine.state.players[1].life, lives_before[1] - 1);
}

#[test]
fn consecrated_sphinx_gets_two_triggers_and_each_choice_is_draw_two_or_decline() {
    let (mut engine, hand_before, library_before) =
        opponent_divination_engine(202_609_272, "consecrated_sphinx");

    pass_both_players(&mut engine);
    assert_rejected_choice_preserves_state(
        &mut engine,
        1,
        &submit_resolution_decision(ResolutionChoiceDecision::SelectBranch),
    );
    assert_rejected_choice_preserves_state(&mut engine, 0, &select_branch_with_index(99));
    engine
        .apply_command(
            0,
            &submit_resolution_decision(ResolutionChoiceDecision::SelectBranch),
        )
        .expect("choose to draw two cards for the first trigger");
    assert_eq!(engine.state.players[0].hand.len(), hand_before + 2);
    assert_eq!(engine.state.players[0].library.len(), library_before - 2);
    assert_eq!(
        engine.state.stack.len(),
        1,
        "the second draw trigger remains"
    );

    pass_both_players(&mut engine);
    engine
        .apply_command(
            0,
            &submit_resolution_decision(ResolutionChoiceDecision::Decline),
        )
        .expect("decline the second trigger");

    assert_eq!(engine.state.players[0].hand.len(), hand_before + 2);
    assert_eq!(engine.state.players[0].library.len(), library_before - 2);
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn minds_eye_pays_once_for_two_opponent_draw_triggers() {
    let (mut engine, hand_before, library_before) =
        opponent_divination_engine(202_609_273, "minds_eye");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    let mana_before = engine.state.players[0].mana_pool.colorless;

    pass_both_players(&mut engine);
    assert_rejected_choice_preserves_state(
        &mut engine,
        1,
        &submit_resolution_decision(ResolutionChoiceDecision::SelectBranch),
    );
    assert_rejected_choice_preserves_state(&mut engine, 0, &select_branch_with_index(99));
    engine
        .apply_command(
            0,
            &submit_resolution_decision(ResolutionChoiceDecision::SelectBranch),
        )
        .expect("choose to pay for the first trigger");
    submit_mana_resolution_decision(&mut engine, 0, ResolutionChoiceDecision::PayMana)
        .expect("pay {1}");
    assert_eq!(engine.state.players[0].hand.len(), hand_before + 1);
    assert_eq!(engine.state.players[0].library.len(), library_before - 1);
    assert_eq!(engine.state.players[0].mana_pool.colorless, mana_before - 1);
    assert_eq!(
        engine.state.stack.len(),
        1,
        "the second draw trigger remains"
    );

    pass_both_players(&mut engine);
    engine
        .apply_command(
            0,
            &submit_resolution_decision(ResolutionChoiceDecision::SelectBranch),
        )
        .expect("select the payment branch for the second trigger");
    assert!(engine.state.pending_resolution.is_some());
    let pending_before = format!("{:?}", engine.state.pending_resolution);
    let stack_before = format!("{:?}", engine.state.stack);
    let hand_before_failed_payment = engine.state.players[0].hand.clone();
    let library_before_failed_payment = engine.state.players[0].library.clone();
    let mana_before_failed_payment = mana_pool(&engine, 0);
    assert!(
        submit_mana_resolution_decision(&mut engine, 0, ResolutionChoiceDecision::PayMana).is_err(),
        "the second {{1}} payment is rejected with no mana remaining"
    );
    assert_eq!(
        format!("{:?}", engine.state.pending_resolution),
        pending_before
    );
    assert_eq!(format!("{:?}", engine.state.stack), stack_before);
    assert_eq!(engine.state.players[0].hand, hand_before_failed_payment);
    assert_eq!(
        engine.state.players[0].library,
        library_before_failed_payment
    );
    assert_eq!(mana_pool(&engine, 0), mana_before_failed_payment);
    submit_mana_resolution_decision(&mut engine, 0, ResolutionChoiceDecision::Decline)
        .expect("decline to pay for the second trigger");

    assert_eq!(engine.state.players[0].hand.len(), hand_before + 1);
    assert_eq!(engine.state.players[0].library.len(), library_before - 1);
    assert_eq!(engine.state.players[0].mana_pool.colorless, mana_before - 1);
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn opponent_draw_cards_have_reviewed_registry_identity_and_printed_characteristics() {
    let registry = tricerules_cards::registry::global();

    let crawler = registry
        .get("scrawling_crawler")
        .expect("Scrawling Crawler is registered");
    assert_eq!(crawler.id, "scrawling_crawler");
    assert_eq!(crawler.name, "Scrawling Crawler");
    assert_eq!(crawler.layout, Layout::Normal);
    assert_eq!(crawler.face_count(), 1);
    let face = crawler.primary_face();
    assert_eq!(face.face_id.as_str(), "scrawling_crawler");
    assert_eq!(face.name, "Scrawling Crawler");
    assert_eq!(face.mana_cost.to_string(), "{3}");
    assert_eq!(
        face.types,
        vec![
            "Artifact".to_string(),
            "Creature".to_string(),
            "Phyrexian".to_string(),
            "Construct".to_string(),
        ]
    );
    assert!(face.colors().is_empty());
    assert_eq!(face.power, Some(3));
    assert_eq!(face.toughness, Some(2));
    assert!(face.keywords.is_empty());
    assert_eq!(face.triggered_abilities.len(), 2);

    let sphinx = registry
        .get("consecrated_sphinx")
        .expect("Consecrated Sphinx is registered");
    assert_eq!(sphinx.id, "consecrated_sphinx");
    assert_eq!(sphinx.name, "Consecrated Sphinx");
    assert_eq!(sphinx.layout, Layout::Normal);
    assert_eq!(sphinx.face_count(), 1);
    let face = sphinx.primary_face();
    assert_eq!(face.face_id.as_str(), "consecrated_sphinx");
    assert_eq!(face.name, "Consecrated Sphinx");
    assert_eq!(face.mana_cost.to_string(), "{4}{U}{U}");
    assert_eq!(
        face.types,
        vec!["Creature".to_string(), "Sphinx".to_string()]
    );
    assert_eq!(face.colors(), vec![Color::Blue]);
    assert_eq!(face.power, Some(4));
    assert_eq!(face.toughness, Some(6));
    assert_eq!(face.keywords, vec![Keyword::Flying]);
    assert_eq!(face.triggered_abilities.len(), 1);

    let minds_eye = registry.get("minds_eye").expect("Mind's Eye is registered");
    assert_eq!(minds_eye.id, "minds_eye");
    assert_eq!(minds_eye.name, "Mind's Eye");
    assert_eq!(minds_eye.layout, Layout::Normal);
    assert_eq!(minds_eye.face_count(), 1);
    let face = minds_eye.primary_face();
    assert_eq!(face.face_id.as_str(), "minds_eye");
    assert_eq!(face.name, "Mind's Eye");
    assert_eq!(face.mana_cost.to_string(), "{5}");
    assert_eq!(face.types, vec!["Artifact".to_string()]);
    assert!(face.colors().is_empty());
    assert!(face.power.is_none());
    assert!(face.toughness.is_none());
    assert!(face.keywords.is_empty());
    assert_eq!(face.triggered_abilities.len(), 1);
}
