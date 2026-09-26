//! Exact deck-corpus coverage for Garruk's Packleader.
//!
//! Oracle text and Scryfall rulings were checked on 2026-09-26. The rulings confirm simultaneous
//! entry triggers and that power is checked as the creature enters. CR 603.3/603.5 cover putting
//! the trigger on the stack and choosing the optional draw on resolution; CR 603.6a covers the
//! entry event, and CR 121.1 covers drawing a card.

use super::helpers::*;
use tricerules_core::{GameEngine, Zone};
use tricerules_proto::ruled::v1::{ChoiceKind, ResolutionChoiceDecision, RuledEventBatch};

const GARRUKS_PACKLEADER: &str = "garruks_packleader";

fn engine(seed: u64) -> GameEngine {
    let mut engine = GameEngine::new(
        seed,
        &[0, 1],
        20,
        Some(vec![forest_only_deck(), forest_only_deck()]),
        true,
    )
    .expect("new Garruk's Packleader engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn resolve_top_stack_item(engine: &mut GameEngine) -> RuledEventBatch {
    answer_trigger_order_in_engine_order(engine);
    let first = engine.state.priority_player_id();
    let second = if first == engine.state.players[0].id {
        engine.state.players[1].id
    } else {
        engine.state.players[0].id
    };
    engine
        .apply_command(first, &pass())
        .expect("first player passes priority");
    engine
        .apply_command(second, &pass())
        .expect("second player passes priority and the top item resolves")
}

fn advance_to_player_main1(engine: &mut GameEngine, player: i32) {
    for _ in 0..40 {
        if engine.state.turn_step == tricerules_core::TurnStep::Main1
            && engine.state.active_player_id() == player
        {
            return;
        }
        let (actor, command) = match engine.state.cleanup_discard_player {
            Some(player) => {
                let player_index = engine.state.player_idx(player).expect("cleanup player");
                let excess = engine.state.players[player_index].hand.len() - 7;
                (player, discard_cleanup_batch((0..excess as u32).collect()))
            }
            None => (engine.state.priority_player_id(), pass()),
        };
        engine
            .apply_command(actor, &command)
            .expect("pass priority toward the requested player's main phase");
    }
    panic!("game did not reach player {player}'s main phase");
}

fn cast_permanent(engine: &mut GameEngine, player: i32, card_id: &str) -> u32 {
    let card = inject_card_into_hand(engine, player as usize, card_id);
    grant_pool(engine, player as usize);
    let slot = hand_index_for_card(engine, player as usize, card_id);
    engine
        .apply_command(player, &cast_spell(slot, vec![]))
        .unwrap_or_else(|error| panic!("cast {card_id}: {error:?}"));
    let _ = resolve_top_stack_item(engine);
    assert_eq!(engine.state.objects[&card].zone, Zone::Battlefield);
    card
}

fn optional_draw_choice(
    engine: &mut GameEngine,
) -> tricerules_proto::ruled::v1::ResolutionChoiceRequired {
    let batch = resolve_top_stack_item(engine);
    let choice = find_resolution_choice(&batch).expect("optional draw resolution branch");
    assert_eq!(choice.deciding_player_id, 0);
    assert_eq!(choice.choice_kind(), ChoiceKind::ResolutionBranch);
    assert_eq!((choice.min, choice.max), (0, 1));
    choice
}

#[test]
fn garruks_packleader_draws_for_a_controlled_creature_with_power_three_or_greater() {
    let mut engine = engine(202_609_263);
    let packleader = cast_permanent(&mut engine, 0, GARRUKS_PACKLEADER);
    assert!(
        engine.state.stack.is_empty(),
        "Packleader is not another creature"
    );

    cast_permanent(&mut engine, 0, "grizzly_bears");
    assert!(
        engine.state.stack.is_empty(),
        "a 2/2 does not qualify at entry"
    );

    let library_before = engine.state.players[0].library.len();
    let expected_draw = *engine.state.players[0]
        .library
        .front()
        .expect("a library card to draw");
    cast_permanent(&mut engine, 0, "air_elemental");
    assert_eq!(
        engine.state.stack.len(),
        1,
        "the qualifying entry triggers once"
    );

    let choice = optional_draw_choice(&mut engine);
    engine
        .apply_command(
            choice.deciding_player_id,
            &submit_resolution_decision(ResolutionChoiceDecision::SelectBranch),
        )
        .expect("choose to draw");

    assert_eq!(engine.state.players[0].library.len(), library_before - 1);
    assert!(engine.state.players[0].hand.contains(&expected_draw));
    assert!(engine.state.players[1]
        .library
        .iter()
        .all(|card| *card != expected_draw));
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.objects[&packleader].zone == Zone::Battlefield);

    advance_to_player_main1(&mut engine, 1);
    cast_permanent(&mut engine, 1, "air_elemental");
    assert!(
        engine.state.stack.is_empty(),
        "an opponent's creature does not qualify"
    );
}

#[test]
fn garruks_packleader_can_decline_its_optional_draw() {
    let mut engine = engine(202_609_264);
    cast_permanent(&mut engine, 0, GARRUKS_PACKLEADER);
    cast_permanent(&mut engine, 0, "air_elemental");

    let library_before = engine.state.players[0].library.len();
    let expected_draw = *engine.state.players[0]
        .library
        .front()
        .expect("a library card to remain");
    let _choice = optional_draw_choice(&mut engine);
    engine
        .apply_command(
            0,
            &submit_resolution_decision(ResolutionChoiceDecision::Decline),
        )
        .expect("decline to draw");

    assert_eq!(engine.state.players[0].library.len(), library_before);
    assert!(engine.state.players[0].library.front() == Some(&expected_draw));
    assert!(!engine.state.players[0].hand.contains(&expected_draw));
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_none());
}
