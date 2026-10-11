//! Exact deck-corpus coverage for opponent-draw triggers.
//!
//! The Oracle text and Scryfall rulings for these printings were checked in the batch preflight.
//! Separate draws create separate triggers; Consecrated Sphinx offers its full two-card draw on
//! each resolution, while Mind's Eye independently offers one generic mana for each draw.

use super::helpers::*;
use tricerules_cards::{Color, CounterKind, Keyword, Layout};
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
fn narset_limits_each_opponents_draws_during_her_controllers_turn() {
    let narset_face = tricerules_cards::registry::global()
        .get("narset,_parter_of_veils")
        .and_then(|card| card.face(0))
        .expect("complete Narset definition is registered");
    assert!(narset_face.static_abilities.iter().any(|ability| matches!(
        ability.definition,
        tricerules_card_model::primitives::StaticAbilityDef::LimitDrawsPerTurn {
            players: tricerules_card_model::primitives::RelativePlayerSet::Opponents,
            maximum: 1,
        }
    )));
    let decks = Some(vec![
        deck_with(
            "island",
            &[
                "narset,_parter_of_veils",
                "scrawling_crawler",
                "scrawling_crawler",
            ],
        ),
        deck_with("island", &[]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        202_610_111,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    move_ready_to_battlefield(&mut engine, 0, "narset,_parter_of_veils");
    relocate_to_battlefield(&mut engine, 0, "scrawling_crawler", false);
    relocate_to_battlefield(&mut engine, 0, "scrawling_crawler", false);

    end_active_turn(&mut engine, 0);
    advance_to_player_main1(&mut engine, 1);
    end_active_turn(&mut engine, 1);
    assert_eq!(engine.state.turn_step, TurnStep::Upkeep);
    answer_trigger_order_in_engine_order(&mut engine);
    assert_eq!(
        engine.state.stack.len(),
        2,
        "both Crawler triggers are waiting"
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
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.players[0].hand.len(), hands_before[0] + 2);
    assert_eq!(
        engine.state.players[0].library.len(),
        libraries_before[0] - 2
    );
    assert_eq!(engine.state.players[1].hand.len(), hands_before[1] + 1);
    assert_eq!(
        engine.state.players[1].library.len(),
        libraries_before[1] - 1
    );
    assert_eq!(engine.state.turn_history.current.player(1).cards_drawn, 1);

    advance_to_player_main1(&mut engine, 1);
    end_active_turn(&mut engine, 1);
    assert_eq!(engine.state.active_player_id(), 0);
    assert_eq!(engine.state.turn_step, TurnStep::Upkeep);
    answer_trigger_order_in_engine_order(&mut engine);
    assert_eq!(
        engine.state.stack.len(),
        2,
        "both Crawlers trigger again next turn"
    );
    let next_turn_hands: Vec<_> = engine
        .state
        .players
        .iter()
        .map(|player| player.hand.len())
        .collect();
    let next_turn_libraries: Vec<_> = engine
        .state
        .players
        .iter()
        .map(|player| player.library.len())
        .collect();
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.players[0].hand.len(), next_turn_hands[0] + 2);
    assert_eq!(
        engine.state.players[0].library.len(),
        next_turn_libraries[0] - 2
    );
    assert_eq!(engine.state.players[1].hand.len(), next_turn_hands[1] + 1);
    assert_eq!(
        engine.state.players[1].library.len(),
        next_turn_libraries[1] - 1
    );
    assert_eq!(engine.state.turn_history.current.player(1).cards_drawn, 1);
}

#[test]
fn narset_rechecks_capacity_for_each_thought_reflection_child_draw() {
    let decks = Some(vec![
        deck_with("island", &["narset,_parter_of_veils", "scrawling_crawler"]),
        deck_with("island", &["thought_reflection"]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        202_610_115,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    move_ready_to_battlefield(&mut engine, 0, "narset,_parter_of_veils");
    move_ready_to_battlefield(&mut engine, 0, "scrawling_crawler");
    move_ready_to_battlefield(&mut engine, 1, "thought_reflection");

    end_active_turn(&mut engine, 0);
    advance_to_player_main1(&mut engine, 1);
    end_active_turn(&mut engine, 1);
    assert_eq!(engine.state.turn_step, TurnStep::Upkeep);
    answer_trigger_order_in_engine_order(&mut engine);
    assert_eq!(
        engine.state.stack.len(),
        1,
        "Crawler's upkeep trigger is waiting"
    );
    let hand_before: Vec<_> = engine
        .state
        .players
        .iter()
        .map(|player| player.hand.len())
        .collect();
    let library_before: Vec<_> = engine
        .state
        .players
        .iter()
        .map(|player| player.library.len())
        .collect();

    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.players[0].hand.len(), hand_before[0] + 1);
    assert_eq!(engine.state.players[0].library.len(), library_before[0] - 1);
    assert_eq!(engine.state.players[1].hand.len(), hand_before[1] + 1);
    assert_eq!(engine.state.players[1].library.len(), library_before[1] - 1);
    assert_eq!(engine.state.turn_history.current.player(1).cards_drawn, 1);
}

#[test]
fn narset_makes_consecrated_sphinx_optional_draw_unavailable_after_the_limit() {
    let decks = Some(vec![
        deck_with("island", &["narset,_parter_of_veils"]),
        deck_with("island", &["consecrated_sphinx", "scrawling_crawler"]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        202_610_112,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    move_ready_to_battlefield(&mut engine, 0, "narset,_parter_of_veils");
    move_ready_to_battlefield(&mut engine, 1, "consecrated_sphinx");
    move_ready_to_battlefield(&mut engine, 1, "scrawling_crawler");

    let hands_before: Vec<_> = engine
        .state
        .players
        .iter()
        .map(|player| player.hand.len())
        .collect();
    end_active_turn(&mut engine, 0);
    assert_eq!(engine.state.active_player_id(), 1);
    assert_eq!(engine.state.turn_step, TurnStep::Upkeep);
    assert_eq!(
        engine.state.stack.len(),
        1,
        "Crawler upkeep trigger is waiting"
    );
    pass_both_players(&mut engine);

    assert_eq!(engine.state.players[0].hand.len(), hands_before[0] + 1);
    assert_eq!(engine.state.players[1].hand.len(), hands_before[1] + 1);
    assert_eq!(engine.state.turn_history.current.player(1).cards_drawn, 1);

    for _ in 0..4 {
        answer_trigger_order_in_engine_order(&mut engine);
        assert!(
            engine.state.pending_resolution.is_none(),
            "Sphinx cannot offer a draw choice after Narset's limit was reached"
        );
        if engine.state.stack.is_empty() {
            break;
        }
        pass_both_players(&mut engine);
    }
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.players[1].hand.len(), hands_before[1] + 1);
}

#[test]
fn narset_auto_declines_garruks_packleaders_optional_draw_after_the_cap() {
    let decks = Some(vec![
        deck_with("island", &["narset,_parter_of_veils"]),
        deck_with("island", &["garruks_packleader", "air_elemental"]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        202_610_113,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    move_ready_to_battlefield(&mut engine, 0, "narset,_parter_of_veils");
    let packleader = move_ready_to_battlefield(&mut engine, 1, "garruks_packleader");

    // The ability is about the resulting legality after the active player's first draw this
    // turn. Keep the game state's turn-history fact explicit so the test does not depend on an
    // unrelated card that would also add a draw or trigger.
    engine.state.turn_history.current.player_mut(1).cards_drawn = 1;
    move_ready_to_battlefield(&mut engine, 1, "air_elemental");
    assert_eq!(
        engine.state.stack.len(),
        1,
        "the qualifying creature actually creates Packleader's optional trigger"
    );
    assert_eq!(
        engine.state.stack[0].source_permanent_id,
        Some(packleader),
        "the pending draw is from Garruk's Packleader"
    );
    let hand_before = engine.state.players[1].hand.len();

    for _ in 0..8 {
        answer_trigger_order_in_engine_order(&mut engine);
        assert!(
            engine.state.pending_resolution.is_none(),
            "the optional draw has no legal branch after the cap"
        );
        if engine.state.stack.is_empty() && engine.state.pending_triggers.is_empty() {
            break;
        }
        pass_both_players(&mut engine);
    }

    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.players[1].hand.len(), hand_before);
    assert_eq!(engine.state.turn_history.current.player(1).cards_drawn, 1);
}

#[test]
fn narset_allows_one_brainstorm_draw_then_skips_prohibited_draws_before_empty_library_loss() {
    let decks = Some(vec![
        deck_with("island", &["narset,_parter_of_veils"]),
        deck_with("island", &["brainstorm"]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        202_610_114,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    move_ready_to_battlefield(&mut engine, 0, "narset,_parter_of_veils");
    end_active_turn(&mut engine, 0);
    assert_eq!(engine.state.turn_step, TurnStep::Upkeep);

    ensure_in_hand(&mut engine, 1, "brainstorm");
    let old_library = std::mem::take(&mut engine.state.players[1].library);
    for object_id in old_library {
        engine.state.players[1].graveyard.push(object_id);
        engine.state.objects.get_mut(&object_id).unwrap().zone = tricerules_core::Zone::Graveyard;
    }
    let only_drawable_card = inject_library_card(&mut engine, 1, "island");
    give_mana(
        &mut engine,
        1,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let hand_before_cast = engine.state.players[1].hand.len();
    let library_before = engine.state.players[1].library.len();
    let brainstorm = hand_index_for_card(&engine, 1, "brainstorm");
    engine
        .apply_command(1, &cast_spell(brainstorm, Vec::new()))
        .expect("cast Brainstorm during upkeep");
    pass_both_players(&mut engine);

    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Brainstorm waits for its hand choice after drawing");
    assert_eq!(pending.deciding_player, 1);
    assert_eq!(pending.presentation.min, 2);
    assert_eq!(pending.presentation.max, 2);
    assert_eq!(engine.state.players[1].hand.len(), hand_before_cast - 1 + 1);
    assert!(engine.state.players[1].library.is_empty());
    assert!(!engine.state.players[1].has_lost);
    assert_eq!(
        engine.state.turn_history.current.player(1).cards_drawn,
        1,
        "the first draw succeeds; later draws are prohibited before checking the empty library"
    );
    assert_eq!(library_before, 1);
    assert!(engine.state.players[1].hand.contains(&only_drawable_card));

    let cards_to_return = engine.state.players[1]
        .hand
        .iter()
        .take(2)
        .copied()
        .collect::<Vec<_>>();
    engine
        .apply_command(1, &submit_resolution_choice(cards_to_return))
        .expect("finish Brainstorm's hand choice");
    assert!(engine.state.pending_resolution.is_none());
    assert!(!engine.state.players[1].has_lost);
    assert_eq!(engine.state.players[1].library.len(), 2);
}

#[test]
fn narset_does_not_hide_keldon_raiders_optional_discard_choice() {
    let decks = Some(vec![
        deck_with("island", &["narset,_parter_of_veils"]),
        deck_with("mountain", &["keldon_raider"]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        202_610_116,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    move_ready_to_battlefield(&mut engine, 0, "narset,_parter_of_veils");
    engine.state.turn_history.current.player_mut(1).cards_drawn = 1;
    move_ready_to_battlefield(&mut engine, 1, "keldon_raider");
    answer_trigger_order_in_engine_order(&mut engine);
    let hand_before = engine.state.players[1].hand.len();
    let library_before = engine.state.players[1].library.len();

    let mut choice = None;
    for _ in 0..4 {
        let actor = engine.state.priority_player_id();
        let batch = engine
            .apply_command(actor, &pass())
            .expect("pass toward Keldon Raider's discard choice");
        if let Some(required) = batch.events.iter().find_map(|event| match &event.ev {
            Some(tricerules_proto::ruled::v1::ruled_event::Ev::ResolutionChoiceRequired(
                choice,
            )) => Some(choice.clone()),
            _ => None,
        }) {
            choice = Some(required);
            break;
        }
    }
    let choice = choice.expect("the optional discard choice is still offered");
    assert_eq!(choice.deciding_player_id, 1);
    assert_eq!((choice.min, choice.max), (0, 1));
    let discarded = choice.candidate_object_ids[0];
    engine
        .apply_command(1, &submit_resolution_choice(vec![discarded]))
        .expect("discard a card even though its following draw is prohibited");

    assert_eq!(engine.state.players[1].hand.len(), hand_before - 1);
    assert_eq!(engine.state.players[1].library.len(), library_before);
    assert!(engine.state.players[1].graveyard.contains(&discarded));
    assert_eq!(engine.state.turn_history.current.player(1).cards_drawn, 1);
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn narset_does_not_skip_decode_transmissions_first_applicable_tail() {
    let decks = Some(vec![
        deck_with("island", &["narset,_parter_of_veils"]),
        deck_with("swamp", &["decode_transmissions"]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        202_610_117,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    move_ready_to_battlefield(&mut engine, 0, "narset,_parter_of_veils");
    end_active_turn(&mut engine, 0);
    advance_to_player_main1(&mut engine, 1);
    assert_eq!(engine.state.turn_history.current.player(1).cards_drawn, 1);

    ensure_in_hand(&mut engine, 1, "decode_transmissions");
    give_mana(
        &mut engine,
        1,
        ManaGift {
            b: 1,
            c: 2,
            ..Default::default()
        },
    );
    let hand_before_cast = engine.state.players[1].hand.len();
    let library_before = engine.state.players[1].library.len();
    let life_before = engine.state.players[1].life;
    let card = hand_index_for_card(&engine, 1, "decode_transmissions");
    engine
        .apply_command(1, &cast_spell(card, Vec::new()))
        .expect("cast Decode Transmissions");
    pass_both_players(&mut engine);

    assert_eq!(engine.state.players[1].hand.len(), hand_before_cast - 1);
    assert_eq!(engine.state.players[1].library.len(), library_before);
    assert_eq!(engine.state.players[1].life, life_before - 2);
    assert_eq!(engine.state.turn_history.current.player(1).cards_drawn, 1);
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn narset_minus_two_finds_only_noncreature_nonland_cards_among_the_top_four() {
    let decks = Some(vec![
        deck_with("island", &["narset,_parter_of_veils"]),
        deck_with("island", &[]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        202_610_118,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let narset = move_ready_to_battlefield(&mut engine, 0, "narset,_parter_of_veils");
    let looked = ["opt", "grizzly_bears", "island", "brainstorm"]
        .iter()
        .map(|card_id| inject_library_card(&mut engine, 0, card_id))
        .collect::<Vec<_>>();
    engine.state.players[0]
        .library
        .retain(|object_id| !looked.contains(object_id));
    for object_id in looked.iter().rev() {
        engine.state.players[0].library.push_front(*object_id);
    }
    let library_before = engine.state.players[0].library.len();

    let minus_two = activate_ability_for(&engine, narset, 0, Vec::new());
    engine
        .apply_command(0, &minus_two)
        .expect("activate Narset's minus-two");
    assert_eq!(
        engine.state.objects[&narset]
            .counters
            .get(&CounterKind::Loyalty),
        Some(&3)
    );
    pass_both_players(&mut engine);

    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Narset waits for its optional top-four selection");
    assert_eq!(pending.deciding_player, 0);
    assert_eq!(pending.presentation.min, 0);
    assert_eq!(pending.presentation.max, 1);
    assert_eq!(pending.presentation.candidates, [looked[0], looked[3]]);
    let result = engine
        .apply_command(0, &submit_resolution_choice(vec![looked[0]]))
        .expect("put the revealed instant into hand");
    assert!(result.events.iter().any(|event| matches!(
        event.ev.as_ref(),
        Some(tricerules_proto::ruled::v1::ruled_event::Ev::CardsRevealed(
            _
        ))
    )));
    assert!(engine.state.players[0].hand.contains(&looked[0]));
    assert!(engine.state.players[0].library.contains(&looked[1]));
    assert!(engine.state.players[0].library.contains(&looked[2]));
    assert!(engine.state.players[0].library.contains(&looked[3]));
    assert_eq!(engine.state.players[0].library.len(), library_before - 1);
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
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
fn minds_eye_keeps_its_payment_choice_when_narset_would_prevent_the_draw() {
    let (mut engine, hand_before, library_before) =
        opponent_divination_engine(202_609_274, "minds_eye");
    inject_card_into_hand(&mut engine, 1, "narset,_parter_of_veils");
    move_ready_to_battlefield(&mut engine, 1, "narset,_parter_of_veils");
    engine.state.turn_history.current.player_mut(0).cards_drawn = 1;
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );

    pass_both_players(&mut engine);
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the costed payment choice remains available");
    assert_eq!(pending.deciding_player, 0);
    assert_eq!(pending.presentation.min, 0);

    engine
        .apply_command(
            0,
            &submit_resolution_decision(ResolutionChoiceDecision::Decline),
        )
        .expect("decline to pay for the draw");
    assert_eq!(engine.state.players[0].hand.len(), hand_before);
    assert_eq!(engine.state.players[0].library.len(), library_before);
    assert_eq!(engine.state.turn_history.current.player(0).cards_drawn, 1);
    assert_eq!(engine.state.stack.len(), 1);
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
