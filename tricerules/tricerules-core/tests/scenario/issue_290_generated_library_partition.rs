//! Issue #290 — Sage of Days and Gurmag Nightwatch look at three cards and optionally keep one.
//!
//! CR 603.2/603.6a govern the ETB trigger, CR 701.20e governs privately looking without
//! revealing, CR 404.3 governs ordering multiple cards entering a graveyard, CR 609.3 governs a
//! short library, and CR 400.7 governs zone-change identity. This is a `Look` partition, not the
//! `Surveil` keyword (CR 701.25), so completion must not emit a Surveilled event or trigger.

use super::helpers::*;
use tricerules_cards::primitives::{
    CastTriggerPlayer, ContinuousEffectKind, EffectDuration, TriggerCondition,
};
use tricerules_core::{AffectedScope, ContinuousEffect, EngineError, Zone};
use tricerules_proto::ruled::v1::ChoiceKind;

const ISSUE_290_CARDS: [&str; 2] = ["sage_of_days", "gurmag_nightwatch"];

fn issue_290_engine(seed: u64) -> GameEngine {
    let mut engine = GameEngine::new(seed, &[0, 1], 20, None, true).expect("issue #290 engine");
    advance_to_main1_from_game_start(&mut engine);
    engine.state.players[0].library.clear();
    engine
}

/// Put `card_ids` on top of `player`'s library, first entry on top, and return their OIDs.
fn seat_on_top(engine: &mut GameEngine, player: usize, card_ids: &[&str]) -> Vec<u32> {
    let oids: Vec<u32> = card_ids
        .iter()
        .map(|card_id| inject_library_card(engine, player, card_id))
        .collect();
    engine.state.players[player]
        .library
        .retain(|oid| !oids.contains(oid));
    for &oid in oids.iter().rev() {
        engine.state.players[player].library.push_front(oid);
    }
    oids
}

fn resolve_top_stack(engine: &mut GameEngine) -> RuledEventBatch {
    let first = engine.state.priority_player_id();
    let second = 1 - first;
    engine
        .apply_command(first, &pass())
        .expect("first pass on ETB stack item");
    engine
        .apply_command(second, &pass())
        .expect("second pass resolves ETB stack item")
}

fn open_look(
    engine: &mut GameEngine,
    card_id: &str,
) -> (u32, RuledEventBatch, ResolutionChoiceRequired) {
    inject_card_into_hand(engine, 0, card_id);
    let source = move_ready_to_battlefield(engine, 0, card_id);
    let parked = resolve_top_stack(engine);
    let choice = find_resolution_choice(&parked).expect("ETB look choice");
    (source, parked, choice)
}

fn move_library_to_graveyard(engine: &mut GameEngine, player: usize, object_id: u32) {
    engine.state.players[player]
        .library
        .retain(|id| *id != object_id);
    engine.state.players[player].graveyard.push(object_id);
    engine.state.objects.get_mut(&object_id).unwrap().zone = Zone::Graveyard;
    *engine
        .state
        .zone_change_generation
        .entry(object_id)
        .or_default() += 1;
}

fn move_graveyard_to_library(engine: &mut GameEngine, player: usize, object_id: u32) {
    engine.state.players[player]
        .graveyard
        .retain(|id| *id != object_id);
    engine.state.players[player].library.push_front(object_id);
    engine.state.objects.get_mut(&object_id).unwrap().zone = Zone::Library;
    *engine
        .state
        .zone_change_generation
        .entry(object_id)
        .or_default() += 1;
}

fn assert_no_surveil(batch: &RuledEventBatch) {
    assert!(!batch.events.iter().any(|event| {
        matches!(&event.ev, Some(Ev::CardsRevealed(_)))
            || matches!(&event.ev, Some(Ev::Log(log)) if log.text.to_ascii_lowercase().contains("surveil"))
    }));
}

#[test]
fn issue_290_retains_selected_one_and_preserves_top_and_graveyard_order() {
    for (offset, card_id) in ISSUE_290_CARDS.into_iter().enumerate() {
        let mut engine = issue_290_engine(290_001 + offset as u64);
        let top = seat_on_top(&mut engine, 0, &["island", "forest", "swamp"]);
        let generations = top
            .iter()
            .map(|object_id| {
                engine
                    .state
                    .zone_change_generation
                    .get(object_id)
                    .copied()
                    .unwrap_or(0)
            })
            .collect::<Vec<_>>();
        let (_source, parked, choice) = open_look(&mut engine, card_id);

        assert_eq!(choice.choice_kind(), ChoiceKind::LibraryLook);
        assert_eq!(choice.deciding_player_id, 0);
        assert_eq!((choice.min, choice.max), (2, 3));
        assert!(choice.ordered, "the graveyard destination is ordered");
        assert_eq!(choice.candidate_object_ids, top);
        assert!(choice.public_reveal.is_none(), "look remains private");
        assert_no_surveil(&parked);

        let completion = engine
            .apply_command(0, &submit_resolution_choice(vec![top[0], top[1]]))
            .expect("retain one card on top");

        assert!(engine.state.pending_resolution.is_none());
        assert!(engine.state.stack.is_empty());
        assert_eq!(engine.state.players[0].library.front(), Some(&top[2]));
        assert_eq!(
            engine.state.players[0].graveyard,
            vec![top[0], top[1]],
            "selected cards retain their submitted graveyard order"
        );
        assert_eq!(
            engine.state.players[0]
                .library
                .iter()
                .copied()
                .collect::<Vec<_>>(),
            vec![top[2]]
        );
        for (object_id, before) in top.iter().zip(generations) {
            assert_eq!(
                engine
                    .state
                    .zone_change_generation
                    .get(object_id)
                    .copied()
                    .unwrap_or(0),
                before + u64::from(*object_id != top[2]),
                "each moved card gets a new physical incarnation"
            );
        }
        assert!(completion.events.iter().any(|event| {
            matches!(&event.ev, Some(Ev::PermanentMoved(move_event))
                if move_event.object_id == top[0]
                    && move_event.source_library_position == Some(0))
        }));
        assert!(completion.events.iter().any(|event| {
            matches!(&event.ev, Some(Ev::PermanentMoved(move_event))
                if move_event.object_id == top[1]
                    && move_event.source_library_position == Some(0))
        }));
        assert_no_surveil(&completion);
    }
}

#[test]
fn issue_290_retains_none_and_moves_all_three_in_order() {
    let mut engine = issue_290_engine(290_010);
    let top = seat_on_top(&mut engine, 0, &["island", "forest", "swamp"]);
    let (_source, _parked, choice) = open_look(&mut engine, ISSUE_290_CARDS[0]);
    assert_eq!((choice.min, choice.max), (2, 3));

    let completion = engine
        .apply_command(0, &submit_resolution_choice(top.clone()))
        .expect("retain no card on top");
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.players[0].library.is_empty());
    assert_eq!(engine.state.players[0].graveyard, top);
    assert_no_surveil(&completion);
}

#[test]
fn issue_290_short_library_clamps_choice_to_available_cards() {
    let mut engine = issue_290_engine(290_011);
    engine.state.players[0].library.clear();
    let top = seat_on_top(&mut engine, 0, &["island", "forest"]);
    let (_source, parked, choice) = open_look(&mut engine, ISSUE_290_CARDS[1]);
    assert_eq!(choice.choice_kind(), ChoiceKind::LibraryLook);
    assert_eq!((choice.min, choice.max), (1, 2));
    assert_eq!(choice.candidate_object_ids, top);
    assert!(choice.public_reveal.is_none());
    assert_no_surveil(&parked);

    let completion = engine
        .apply_command(0, &submit_resolution_choice(vec![top[0]]))
        .expect("put one short-library card in the graveyard");
    assert_eq!(engine.state.players[0].graveyard, vec![top[0]]);
    assert_eq!(engine.state.players[0].library.front(), Some(&top[1]));
    assert_no_surveil(&completion);
}

#[test]
fn issue_290_same_seed_produces_the_same_private_choice_and_completion() {
    fn prepared(seed: u64) -> (GameEngine, Vec<u32>, ResolutionChoiceRequired) {
        let mut engine = issue_290_engine(seed);
        let top = seat_on_top(&mut engine, 0, &["island", "forest", "swamp"]);
        let (_source, _parked, choice) = open_look(&mut engine, ISSUE_290_CARDS[0]);
        (engine, top, choice)
    }

    let (mut left, top_left, choice_left) = prepared(290_015);
    let (mut right, top_right, choice_right) = prepared(290_015);
    assert_eq!(top_left, top_right);
    assert_eq!(choice_left, choice_right);
    let left_completion = left
        .apply_command(0, &submit_resolution_choice(vec![top_left[0], top_left[1]]))
        .expect("left deterministic completion");
    let right_completion = right
        .apply_command(
            0,
            &submit_resolution_choice(vec![top_right[0], top_right[1]]),
        )
        .expect("right deterministic completion");
    assert_eq!(left_completion, right_completion);
    assert_eq!(
        left.state.players[0].library,
        right.state.players[0].library
    );
    assert_eq!(
        left.state.players[0].graveyard,
        right.state.players[0].graveyard
    );
    assert_eq!(
        left.state.zone_change_generation,
        right.state.zone_change_generation
    );
}

#[test]
fn issue_290_rejects_unauthorized_noncandidate_and_duplicate_choices_atomically() {
    let mut engine = issue_290_engine(290_012);
    let top = seat_on_top(&mut engine, 0, &["island", "forest", "swamp"]);
    let below_window = inject_library_card(&mut engine, 0, "mountain");
    let unauthorized = inject_library_card(&mut engine, 1, "island");
    let (_source, _parked, choice) = open_look(&mut engine, ISSUE_290_CARDS[0]);
    assert_eq!(choice.candidate_object_ids, top);
    let library_before = engine.state.players[0]
        .library
        .iter()
        .copied()
        .collect::<Vec<_>>();

    for (player, chosen, label) in [
        (1, vec![top[0]], "unauthorized player"),
        (0, vec![below_window], "card below look window"),
        (0, vec![top[0], top[0]], "duplicate card"),
        (0, vec![top[0], unauthorized], "opponent-owned card"),
    ] {
        assert!(
            matches!(
                engine.apply_command(player, &submit_resolution_choice(chosen)),
                Err(EngineError::Illegal(_))
            ),
            "{label} must be rejected"
        );
        assert!(
            engine.state.pending_resolution.is_some(),
            "{label} leaves choice pending"
        );
        assert_eq!(
            engine.state.players[0]
                .library
                .iter()
                .copied()
                .collect::<Vec<_>>(),
            library_before,
            "{label} does not partially move the library"
        );
        assert!(engine.state.players[0].graveyard.is_empty(), "{label}");
    }

    engine
        .apply_command(0, &submit_resolution_choice(vec![top[0], top[1]]))
        .expect("valid choice after rejected submissions");
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.players[0].graveyard, vec![top[0], top[1]]);
}

#[test]
fn issue_290_rejects_a_stale_zone_generation_even_when_the_card_returns_to_the_top() {
    let mut engine = issue_290_engine(290_013);
    let top = seat_on_top(&mut engine, 0, &["island", "forest", "swamp"]);
    let (_source, _parked, choice) = open_look(&mut engine, ISSUE_290_CARDS[0]);
    let before_generation = engine
        .state
        .zone_change_generation
        .get(&top[0])
        .copied()
        .unwrap_or(0);

    move_library_to_graveyard(&mut engine, 0, top[0]);
    move_graveyard_to_library(&mut engine, 0, top[0]);
    assert_eq!(engine.state.players[0].library.front(), Some(&top[0]));
    assert_eq!(
        engine.state.zone_change_generation[&top[0]],
        before_generation + 2
    );
    assert!(
        matches!(
            engine.apply_command(0, &submit_resolution_choice(vec![top[0], top[1]])),
            Err(EngineError::Illegal(_))
        ),
        "the choice must bind the original physical generation"
    );
    assert!(engine.state.pending_resolution.is_some());
    assert_eq!(engine.state.players[0].graveyard, Vec::<u32>::new());
    assert_eq!(engine.state.players[0].library.front(), Some(&top[0]));
    assert_eq!(choice.candidate_object_ids, top);
}

#[test]
fn issue_290_look_is_private_and_does_not_fire_a_surveil_trigger() {
    let mut engine = issue_290_engine(290_014);
    let top = seat_on_top(&mut engine, 0, &["island", "forest", "swamp"]);
    let observer = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let mut ability = tricerules_cards::CardRegistry::global()
        .get("audacious_thief")
        .expect("Audacious Thief definition")
        .primary_face()
        .triggered_abilities[0]
        .clone();
    ability.trigger = TriggerCondition::WheneverPlayerSurveils {
        player: CastTriggerPlayer::Controller,
    };
    engine.state.add_triggered_ability_grant(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(observer),
        kind: ContinuousEffectKind::GrantTriggeredAbility(Box::new(ability)),
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: engine.state.command_index,
    });

    let (_source, parked, choice) = open_look(&mut engine, ISSUE_290_CARDS[1]);
    assert_eq!(choice.choice_kind(), ChoiceKind::LibraryLook);
    assert_eq!(choice.candidate_object_ids, top);
    assert_eq!(choice.deciding_player_id, 0);
    assert!(choice.public_reveal.is_none());
    assert_no_surveil(&parked);

    let completion = engine
        .apply_command(0, &submit_resolution_choice(vec![top[0], top[1]]))
        .expect("finish private look partition");
    assert!(engine.state.pending_triggers.is_empty());
    assert!(engine.state.staged_trigger_groups.is_empty());
    assert!(engine
        .state
        .stack
        .iter()
        .all(|item| item.source_permanent_id != Some(observer)));
    assert_no_surveil(&completion);
}
