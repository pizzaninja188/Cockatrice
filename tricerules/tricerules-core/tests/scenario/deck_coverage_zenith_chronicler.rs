//! Exact Zenith Chronicler cast-event and caster-relative draw evidence.

use super::helpers::*;
use tricerules_cards::{Color, ContinuousEffectKind, EffectDuration};
use tricerules_core::state::{AffectedScope, ContinuousEffect};
use tricerules_core::GameEngine;
use tricerules_core::TurnStep;

fn setup() -> GameEngine {
    let deck = deck_with("forest", &[]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        503_002,
        &[10, 20, 30],
        20,
        Some(vec![deck.clone(), deck.clone(), deck]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    cast_card(&mut engine, 0, "zenith_chronicler", vec![]);
    pass_priority_round(&mut engine);
    engine
}

fn cast_card(engine: &mut GameEngine, seat: usize, card: &str, targets: Vec<TargetRef>) -> u32 {
    let actor = engine.state.players[seat].id;
    give_mana(
        engine,
        actor,
        ManaGift {
            c: 10,
            w: 10,
            u: 10,
            b: 10,
            r: 10,
            g: 10,
        },
    );
    inject_card_into_hand(engine, seat, card);
    let index = hand_index_for_card(engine, seat, card);
    engine
        .apply_command(actor, &cast_spell(index, targets))
        .unwrap();
    engine
        .state
        .stack
        .iter()
        .find(|item| !item.is_triggered && item.card_id == card)
        .unwrap()
        .id
}

fn colors(engine: &mut GameEngine, oid: u32, colors: Vec<Color>) {
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(oid),
        kind: ContinuousEffectKind::Layer5SetColors(colors),
        condition: None,
        duration: EffectDuration::Indefinite,
        timestamp: engine.state.command_index,
    });
}

fn advance(engine: &mut GameEngine, player: i32) {
    for _ in 0..150 {
        if engine.state.active_player_id() == player && engine.state.turn_step == TurnStep::Main1 {
            return;
        }
        if let Some(actor) = engine.state.cleanup_discard_player {
            let seat = engine.state.player_idx(actor).unwrap();
            let excess = engine.state.players[seat].hand.len() - 7;
            engine
                .apply_command(actor, &discard_cleanup_batch((0..excess as u32).collect()))
                .unwrap();
        } else {
            pass_priority_round(engine);
        }
    }
    panic!("did not reach main phase for {player}");
}

fn resolve_all(engine: &mut GameEngine) {
    for _ in 0..20 {
        if engine.state.stack.is_empty() {
            return;
        }
        pass_priority_round(engine);
    }
    panic!("stack did not finish");
}

fn rejected(engine: &mut GameEngine, actor: i32, command: &RuledCommand) {
    let before = serde_json::to_vec(&engine.state).unwrap();
    assert!(engine.apply_command(actor, command).is_err());
    assert_eq!(serde_json::to_vec(&engine.state).unwrap(), before);
}

fn layer(engine: &mut GameEngine, oid: u32, kind: ContinuousEffectKind) {
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(oid),
        kind,
        condition: None,
        duration: EffectDuration::Indefinite,
        timestamp: engine.state.command_index,
    });
}

#[test]
fn zenith_real_clone_has_independent_trigger_and_order_replies_are_atomic() {
    let mut engine = setup();
    let zenith = battlefield_object_for_card(&engine, 0, "zenith_chronicler");
    cast_card(&mut engine, 0, "clone", vec![]);
    pass_priority_round(&mut engine);
    assert!(engine.state.pending_resolution.is_some());
    engine
        .apply_command(10, &submit_resolution_choice(vec![zenith]))
        .unwrap();
    let copy = battlefield_object_for_card(&engine, 0, "clone");
    assert_eq!(engine.effective_power(copy), Some(3));
    assert_eq!(engine.effective_toughness(copy), Some(1));
    cast_card(&mut engine, 0, "lightning_helix", target_player(30));
    let pending = engine.state.pending_trigger_order.as_ref().unwrap();
    assert_eq!(pending.deciding_player, 10);
    assert_eq!(pending.candidates.len(), 2);
    let first = pending.candidates[0].object_id;
    rejected(&mut engine, 20, &submit_trigger_order(first));
    rejected(&mut engine, 10, &submit_trigger_order(u32::MAX));
    answer_trigger_order_in_engine_order(&mut engine);
    assert_eq!(
        engine.state.stack.iter().filter(|s| s.is_triggered).count(),
        2
    );
    let hands: Vec<_> = engine.state.players.iter().map(|p| p.hand.len()).collect();
    pass_priority_round(&mut engine);
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.players[0].hand.len(), hands[0]);
    assert_eq!(engine.state.players[1].hand.len(), hands[1] + 2);
    assert_eq!(engine.state.players[2].hand.len(), hands[2] + 2);
    resolve_all(&mut engine);
}

#[test]
fn zenith_suppression_before_capture_and_independence_after_capture() {
    for before_cast in [true, false] {
        let mut engine = setup();
        let zenith = battlefield_object_for_card(&engine, 0, "zenith_chronicler");
        if before_cast {
            layer(
                &mut engine,
                zenith,
                ContinuousEffectKind::Layer6RemoveAllAbilities,
            );
        }
        cast_card(&mut engine, 0, "lightning_helix", target_player(30));
        assert_eq!(
            engine.state.stack.iter().filter(|s| s.is_triggered).count(),
            usize::from(!before_cast)
        );
        if !before_cast {
            layer(
                &mut engine,
                zenith,
                ContinuousEffectKind::Layer6RemoveAllAbilities,
            );
        }
        let hands: Vec<_> = engine.state.players.iter().map(|p| p.hand.len()).collect();
        pass_priority_round(&mut engine);
        for (seat, hand) in hands.into_iter().enumerate() {
            assert_eq!(
                engine.state.players[seat].hand.len(),
                hand + usize::from(!before_cast && seat != 0)
            );
        }
        resolve_all(&mut engine);
    }
}

#[test]
fn zenith_source_departure_and_control_change_preserve_captured_caster() {
    for leave in [false, true] {
        let mut engine = setup();
        advance(&mut engine, 20);
        let zenith = battlefield_object_for_card(&engine, 0, "zenith_chronicler");
        cast_card(&mut engine, 1, "lightning_helix", target_player(30));
        if leave {
            engine.apply_command(20, &pass()).unwrap();
            cast_card(&mut engine, 2, "unsummon", target_object(zenith));
            pass_priority_round(&mut engine);
            assert_eq!(
                engine.state.objects[&zenith].zone,
                tricerules_core::Zone::Hand
            );
        } else {
            layer(
                &mut engine,
                zenith,
                ContinuousEffectKind::Layer2Control {
                    controller: tricerules_cards::ControllerReference::Fixed(30),
                },
            );
        }
        let hands: Vec<_> = engine.state.players.iter().map(|p| p.hand.len()).collect();
        assert_eq!(engine.state.stack.last().unwrap().controller, 10);
        assert_eq!(
            engine
                .state
                .stack
                .last()
                .unwrap()
                .trigger_context
                .affected_player,
            Some(20)
        );
        pass_priority_round(&mut engine);
        assert_eq!(engine.state.players[0].hand.len(), hands[0] + 1);
        assert_eq!(engine.state.players[1].hand.len(), hands[1]);
        assert_eq!(engine.state.players[2].hand.len(), hands[2] + 1);
        if !leave {
            assert_eq!(engine.state.objects[&zenith].controller, 30);
        }
        resolve_all(&mut engine);
    }
}

#[test]
fn zenith_caster_departure_retains_anchor_and_lost_other_player_does_not_draw() {
    use tricerules_proto::ruled::v1 as rv1;
    for departing in [20, 30] {
        let mut engine = setup();
        advance(&mut engine, 20);
        cast_card(&mut engine, 1, "lightning_helix", target_player(10));
        let cmd = rv1::RuledCommand {
            cmd: Some(rv1::ruled_command::Cmd::Concede(rv1::Concede {})),
        };
        engine.apply_command(departing, &cmd).unwrap();
        let hands: Vec<_> = engine.state.players.iter().map(|p| p.hand.len()).collect();
        let libraries: Vec<_> = engine
            .state
            .players
            .iter()
            .map(|p| p.library.len())
            .collect();
        pass_priority_round(&mut engine);
        for seat in 0..3 {
            let player = &engine.state.players[seat];
            let draw = usize::from(!player.has_lost && player.id != 20);
            assert_eq!(player.hand.len(), hands[seat] + draw);
            assert_eq!(player.library.len(), libraries[seat] - draw);
        }
        resolve_all(&mut engine);
    }
}

#[test]
fn zenith_group_draw_finishes_before_empty_library_loss_and_draw_triggers() {
    use tricerules_proto::ruled::v1::ruled_event::Ev;
    for empty_first in [false, true] {
        let mut engine = setup();
        advance(&mut engine, 20);
        cast_card(&mut engine, 1, "scrawling_crawler", vec![]);
        resolve_all(&mut engine);
        cast_card(&mut engine, 1, "lightning_helix", target_player(30));
        engine.state.active_player_idx = 2; // Exercise 30 then 10, rather than numeric player order.
        if empty_first {
            engine.state.players[2].library.clear();
        }
        let hands: Vec<_> = engine.state.players.iter().map(|p| p.hand.len()).collect();
        let life: Vec<_> = engine.state.players.iter().map(|p| p.life).collect();
        let mut logs = vec![];
        for _ in 0..3 {
            let actor = engine.state.priority_player_id();
            let batch = engine.apply_command(actor, &pass()).unwrap();
            logs.extend(batch.events.iter().filter_map(|event| match &event.ev {
                Some(Ev::Log(log)) => Some(log.text.clone()),
                _ => None,
            }));
        }
        assert_eq!(
            engine.state.players[0].hand.len(),
            hands[0] + 1,
            "later recipient always draws"
        );
        assert_eq!(engine.state.players[1].hand.len(), hands[1]);
        assert_eq!(
            engine.state.players[2].hand.len(),
            if empty_first { 0 } else { hands[2] + 1 }
        );
        assert_eq!(engine.state.players[2].has_lost, empty_first);
        assert_eq!(
            engine
                .state
                .players
                .iter()
                .map(|p| p.life)
                .collect::<Vec<_>>(),
            life,
            "draw observers wait until the complete draw instruction has finished"
        );
        let first = logs
            .iter()
            .position(|log| log.starts_with("P30 draws"))
            .unwrap();
        let second = logs
            .iter()
            .position(|log| log.starts_with("P10 draws"))
            .unwrap();
        assert!(first < second);
        answer_trigger_order_in_engine_order(&mut engine);
        let draw_triggers = engine
            .state
            .stack
            .iter()
            .filter(|item| item.is_triggered)
            .count();
        assert_eq!(draw_triggers, 1 + usize::from(!empty_first));
        resolve_all(&mut engine);
        assert!(engine.state.stack.is_empty() && engine.state.pending_resolution.is_none());
    }
}

#[test]
fn zenith_accepted_cast_and_draw_commands_replay_identically() {
    fn start() -> GameEngine {
        let mut engine = setup();
        inject_card_into_hand(&mut engine, 0, "lightning_helix");
        give_mana(
            &mut engine,
            10,
            ManaGift {
                w: 1,
                r: 1,
                ..Default::default()
            },
        );
        engine
    }
    let mut engine = start();
    let card = hand_index_for_card(&engine, 0, "lightning_helix");
    let cast = cast_spell(card, target_player(20));
    rejected(&mut engine, 30, &cast);
    let invalid = cast_spell(card, target_object(u32::MAX));
    rejected(&mut engine, 10, &invalid);
    let mut commands = vec![(10, cast)];
    let mut batches = vec![engine.apply_command(commands[0].0, &commands[0].1).unwrap()];
    while !engine.state.stack.is_empty() {
        let actor = engine.state.priority_player_id();
        let command = pass();
        batches.push(engine.apply_command(actor, &command).unwrap());
        commands.push((actor, command));
    }
    let mut replay = start();
    let replayed: Vec<_> = commands
        .iter()
        .map(|(actor, command)| replay.apply_command(*actor, command).unwrap())
        .collect();
    assert_eq!(replayed, batches);
    assert_eq!(
        serde_json::to_value(&replay.state).unwrap(),
        serde_json::to_value(&engine.state).unwrap()
    );
}

#[test]
fn zenith_chronicler_opponent_first_multicolor_draws_casters_complement() {
    assert!(
        tricerules_cards::registry::global()
            .get("zenith_chronicler")
            .is_some(),
        "the complete Zenith Chronicler definition must be admitted"
    );
    let decks = Some(vec![
        deck_with("island", &["zenith_chronicler"]),
        deck_with("mountain", &["lightning_helix"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        503_001,
        &[10, 20, 30],
        20,
        decks,
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "zenith_chronicler");
    give_mana(
        &mut engine,
        10,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    let hand_index = hand_index_for_card(&engine, 0, "zenith_chronicler");
    engine
        .apply_command(10, &cast_spell(hand_index, vec![]))
        .unwrap();
    pass_priority_round(&mut engine);
    let zenith = battlefield_object_for_card(&engine, 0, "zenith_chronicler");
    assert_eq!(engine.state.objects[&zenith].controller, 10);

    engine.apply_command(10, &pass()).unwrap();
    ensure_in_hand(&mut engine, 1, "lightning_helix");
    give_mana(
        &mut engine,
        20,
        ManaGift {
            r: 1,
            w: 1,
            ..Default::default()
        },
    );
    let helix = hand_index_for_card(&engine, 1, "lightning_helix");
    engine
        .apply_command(20, &cast_spell(helix, target_player(30)))
        .unwrap();
    assert_eq!(
        engine.state.stack.len(),
        2,
        "one cast produces one Zenith trigger"
    );
    assert_eq!(engine.state.stack.last().unwrap().controller, 10);
    assert_eq!(
        engine
            .state
            .stack
            .last()
            .unwrap()
            .trigger_context
            .affected_player,
        Some(20)
    );
    let hands: Vec<_> = engine
        .state
        .players
        .iter()
        .map(|p| p.hand.clone())
        .collect();
    let libraries: Vec<_> = engine
        .state
        .players
        .iter()
        .map(|p| p.library.clone())
        .collect();

    pass_priority_round(&mut engine);
    assert_eq!(
        engine.state.stack.len(),
        1,
        "Helix remains below the resolved trigger"
    );
    for seat in [0, 2] {
        assert_eq!(engine.state.players[seat].hand.len(), hands[seat].len() + 1);
        assert_eq!(
            engine.state.players[seat].library.len(),
            libraries[seat].len() - 1
        );
        assert_eq!(
            engine.state.players[seat].hand.last(),
            libraries[seat].front()
        );
    }
    assert_eq!(engine.state.players[1].hand, hands[1]);
    assert_eq!(engine.state.players[1].library, libraries[1]);
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn zenith_event_colors_exclude_mono_colorless_and_survive_later_color_change() {
    for (card, event_colors, qualifies) in [
        ("ornithopter", vec![], false),
        ("lightning_bolt", vec![Color::Red], false),
        ("lightning_helix", vec![Color::Red, Color::White], true),
    ] {
        let mut engine = setup();
        let targets = if card == "ornithopter" {
            vec![]
        } else {
            target_player(20)
        };
        let spell = cast_card(&mut engine, 0, card, targets);
        assert_eq!(
            engine
                .state
                .turn_history
                .current
                .spell_casts
                .last()
                .unwrap()
                .colors,
            event_colors
        );
        assert_eq!(
            engine.state.stack.iter().filter(|s| s.is_triggered).count(),
            usize::from(qualifies)
        );
        colors(&mut engine, spell, vec![Color::Green]);
        let before: Vec<_> = engine.state.players.iter().map(|p| p.hand.len()).collect();
        pass_priority_round(&mut engine);
        for (seat, hand) in before.into_iter().enumerate() {
            assert_eq!(
                engine.state.players[seat].hand.len(),
                hand + usize::from(qualifies && seat != 0)
            );
        }
        resolve_all(&mut engine);
    }
}

#[test]
fn zenith_first_matching_cast_is_per_caster_and_resets_next_turn() {
    let mut engine = setup();
    cast_card(&mut engine, 0, "lightning_bolt", target_player(30));
    assert_eq!(engine.state.stack.len(), 1);
    resolve_all(&mut engine);
    for expected in [1, 0] {
        cast_card(&mut engine, 0, "lightning_helix", target_player(30));
        assert_eq!(
            engine.state.stack.iter().filter(|s| s.is_triggered).count(),
            expected
        );
        resolve_all(&mut engine);
    }
    engine.apply_command(10, &pass()).unwrap();
    cast_card(&mut engine, 1, "lightning_helix", target_player(30));
    assert_eq!(
        engine.state.stack.iter().filter(|s| s.is_triggered).count(),
        1
    );
    resolve_all(&mut engine);
    advance(&mut engine, 20);
    cast_card(&mut engine, 1, "lightning_helix", target_player(30));
    assert_eq!(
        engine.state.stack.iter().filter(|s| s.is_triggered).count(),
        1
    );
    resolve_all(&mut engine);
}

#[test]
fn zenith_history_before_entry_and_countered_cast_still_count() {
    let deck = deck_with("forest", &[]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        503_003,
        &[10, 20, 30],
        20,
        Some(vec![deck.clone(), deck.clone(), deck]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    cast_card(&mut engine, 0, "lightning_helix", target_player(30));
    resolve_all(&mut engine);
    cast_card(&mut engine, 0, "zenith_chronicler", vec![]);
    resolve_all(&mut engine);
    cast_card(&mut engine, 0, "lightning_helix", target_player(30));
    assert_eq!(
        engine.state.stack.len(),
        1,
        "qualifying pre-entry history already consumed first cast"
    );
    resolve_all(&mut engine);
    advance(&mut engine, 20);
    let helix = cast_card(&mut engine, 1, "lightning_helix", target_player(30));
    engine.apply_command(20, &pass()).unwrap();
    cast_card(&mut engine, 2, "counterspell", target_object(helix));
    pass_priority_round(&mut engine);
    assert!(!engine.state.stack.iter().any(|s| s.id == helix));
    assert_eq!(
        engine.state.stack.len(),
        1,
        "captured Zenith trigger survives countering its spell"
    );
    let hands: Vec<_> = engine.state.players.iter().map(|p| p.hand.len()).collect();
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.players[0].hand.len(), hands[0] + 1);
    assert_eq!(engine.state.players[1].hand.len(), hands[1]);
    assert_eq!(engine.state.players[2].hand.len(), hands[2] + 1);
    cast_card(&mut engine, 1, "lightning_helix", target_player(30));
    assert_eq!(
        engine.state.stack.len(),
        1,
        "countered first spell remains in turn history"
    );
    resolve_all(&mut engine);
}

#[test]
fn zenith_twincast_created_stack_copy_is_not_another_cast() {
    let mut engine = setup();
    advance(&mut engine, 20);
    let helix = cast_card(&mut engine, 1, "lightning_helix", target_player(30));
    engine.apply_command(20, &pass()).unwrap();
    cast_card(&mut engine, 2, "twincast", target_object(helix));
    let history = engine.state.turn_history.current.spell_casts.clone();
    pass_priority_round(&mut engine);
    assert!(engine.state.pending_resolution.is_some());
    rejected(&mut engine, 10, &submit_resolution_choice(vec![30]));
    engine
        .apply_command(30, &submit_resolution_choice(vec![30]))
        .unwrap();
    let copy = engine.state.stack.last().unwrap();
    assert!(copy.is_copy);
    assert_eq!(copy.cast_occurrence, None);
    assert_eq!(copy.controller, 30);
    assert_eq!(engine.state.turn_history.current.spell_casts, history);
    assert_eq!(
        engine.state.stack.iter().filter(|s| s.is_triggered).count(),
        1
    );
    resolve_all(&mut engine);
}

#[test]
fn zenith_preparation_copy_actually_cast_records_exile_event_and_mono_filter() {
    use tricerules_proto::ruled::v1 as rv1;
    let mut engine = setup();
    advance(&mut engine, 20);
    inject_card_into_hand(&mut engine, 1, "infirmary_healer_stream_of_life");
    give_mana(
        &mut engine,
        20,
        ManaGift {
            g: 2,
            ..Default::default()
        },
    );
    let index = hand_index_for_card(&engine, 1, "infirmary_healer_stream_of_life");
    engine
        .apply_command(20, &cast_spell_face(index, vec![], 0))
        .unwrap();
    let healer = engine.state.stack.last().unwrap().id;
    resolve_all(&mut engine);
    let prepared = engine.state.prepared_permanents[&healer];
    give_mana(
        &mut engine,
        20,
        ManaGift {
            g: 1,
            ..Default::default()
        },
    );
    let batch = engine.initial_response_batch();
    let action = batch.legal_by_player[&20]
        .zone_cast_actions
        .iter()
        .find(|a| a.object_id == prepared)
        .unwrap();
    let cmd = rv1::RuledCommand {
        cmd: Some(rv1::ruled_command::Cmd::CastSpell(rv1::CastSpell {
            source: Some(rv1::CastSource {
                location: Some(rv1::cast_source::Location::ExileObjectId(prepared)),
                expected_zone_change_generation: Some(action.zone_change_generation),
            }),
            face_index: action.face_index,
            casting_permission_id: action.casting_permission_id,
            cast_method: action.cast_method,
            targets: target_player(30),
            x_value: 0,
            ..Default::default()
        })),
    };
    engine.apply_command(20, &cmd).unwrap();
    let spell = engine.state.stack.iter().find(|s| !s.is_triggered).unwrap();
    assert!(spell.is_copy && spell.cast_occurrence.is_some());
    assert_eq!(spell.cast_by, Some(20));
    let fact = engine
        .state
        .turn_history
        .current
        .spell_casts
        .last()
        .unwrap();
    assert_eq!(fact.caster, 20);
    assert_eq!(fact.origin, tricerules_core::Zone::Exile);
    assert_eq!(fact.colors, [Color::Green]);
    assert_eq!(
        engine.state.stack.iter().filter(|s| s.is_triggered).count(),
        0
    );
    let hands: Vec<_> = engine.state.players.iter().map(|p| p.hand.len()).collect();
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.players[0].hand.len(), hands[0]);
    assert_eq!(engine.state.players[1].hand.len(), hands[1]);
    assert_eq!(engine.state.players[2].hand.len(), hands[2]);
    resolve_all(&mut engine);
}
