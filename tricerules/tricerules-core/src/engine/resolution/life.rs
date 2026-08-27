use super::*;

/// CR 119.3 / 119.9: `player` gains `amount` life — emit `LifeChanged`, log it, and fire
/// [`GameEvent::LifeGained`].
///
/// The single funnel for every life *gain* edge (spell effects, drain, exile-for-life, lifelink),
/// the gain-side analog of `engine::set_tapped`: a "whenever you gain life" trigger hangs off this
/// one call instead of auditing every mutation site. Life *loss* has no such funnel yet — no
/// implemented card watches for it.
///
/// One call is one life-gain event, so callers must not pre-sum unrelated gains: two lifelink
/// creatures in the same damage step gain separately and trigger separately. A gain of 0 is not an
/// event (CR 119.9) — no life change, no log line, no trigger. The same is true of a
/// prohibited gain (CR 119.7 / 614.17); the enclosing spell or ability still resolves.
///
/// `reason` is the parenthetical shown in the game log (a spell label, or "lifelink").
pub(in crate::engine) fn apply_life_gain(
    engine: &mut GameEngine,
    events: &mut Vec<rv1::RuledEvent>,
    player: PlayerId,
    amount: u32,
    reason: &str,
) {
    if let Some(event) = apply_life_gain_without_triggers(engine, events, player, amount, reason) {
        engine.fire_triggers(&[event]);
    }
}

/// Apply one life-gain event without firing its triggers yet. Simultaneous producers such as a
/// combat-damage step collect the returned event with their other trigger-driving events and fire
/// the whole set once.
pub(in crate::engine) fn apply_life_gain_without_triggers(
    engine: &mut GameEngine,
    events: &mut Vec<rv1::RuledEvent>,
    player: PlayerId,
    amount: u32,
    reason: &str,
) -> Option<GameEvent> {
    if amount == 0 || !engine.can_player_gain_life(player) {
        return None;
    }
    let pi = engine.state.player_idx(player)?;
    engine.state.players[pi].life += amount as i32;
    events.push(rv1::RuledEvent {
        ev: Some(rv1::ruled_event::Ev::LifeChanged(rv1::LifeChanged {
            player_id: player,
            new_total: engine.state.players[pi].life,
            delta: amount as i32,
        })),
    });
    events.push(ev_log(format!("P{player} gains {amount} life ({reason}).")));
    Some(GameEvent::LifeGained { player })
}

pub(super) fn gain_life(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::GainLife { amount } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let engine = &mut *cx.engine;
    let events = &mut *cx.events;
    let top = cx.top;
    let controller = cx.controller;
    let spell_label = cx.spell_label;

    let amount = engine.resolve_amount(
        &amount,
        AmountContext::for_stack_item(top, controller)
            .with_previous_effect_result(cx.previous_effect_result),
    );
    apply_life_gain(engine, events, controller, amount, spell_label);

    Ok(EffectOutcome::Continue)
}

/// CR 119.3: the players named by `who` lose life. Untargeted (CR 115.1).
///
/// `LifeAmount::TargetManaValue` (CR 202.3) reads the mana value of the object the *spell*
/// targets — a sibling effect declared it, this one only borrows it. Position relative to that
/// sibling does not matter: the object keeps its `card_id` across a zone change, so Reanimate's
/// `[MoveGraveyardCards, LoseLife]` reads the same value before or after the creature moves.
/// Position relative to a *suspending* effect does matter — see the `EffectOutcome::Suspended`
/// early return in the caller, and Thoughtseize's RON for the one card that has to care.
pub(super) fn lose_life(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::LoseLife { amount, who } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let recipients = player_recipients(cx, who);
    let engine = &mut *cx.engine;
    let events = &mut *cx.events;
    let targets = cx.targets;
    let spell_label = cx.spell_label;

    let amount = match amount {
        LifeAmount::Fixed(n) => n,
        LifeAmount::TargetManaValue => targets
            .first()
            .and_then(|tid| engine.state.objects.get(tid))
            .and_then(|o| {
                let def = engine.registry.get(&o.card_id)?;
                if let Some(values) = &o.copiable_values {
                    return Some(values.face.mana_cost.mana_value());
                }
                // CR 202.3b: a face with no printed cost (a transforming DFC's back face) has
                // the mana value of the front face, so fall back rather than reading 0.
                let face = def
                    .face(o.face_up_index)
                    .unwrap_or_else(|| def.primary_face());
                let cost = if face.mana_cost.is_empty() {
                    &def.primary_face().mana_cost
                } else {
                    &face.mana_cost
                };
                Some(cost.mana_value())
            })
            // Unreachable in practice: registry load requires an object-targeting sibling, and
            // the CR 608.2b fizzle check kills the whole spell before resolution when that
            // target is gone. Resolve to 0 rather than panicking if it ever is.
            .unwrap_or(0),
    };

    if amount == 0 {
        return Ok(EffectOutcome::Continue);
    }
    for player in recipients {
        let Some(pi) = engine.state.player_idx(player) else {
            continue;
        };
        engine.state.players[pi].life -= amount as i32;
        events.push(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::LifeChanged(rv1::LifeChanged {
                player_id: player,
                new_total: engine.state.players[pi].life,
                delta: -(amount as i32),
            })),
        });
        events.push(ev_log(format!(
            "P{player} loses {amount} life ({spell_label})."
        )));
    }

    Ok(EffectOutcome::Continue)
}

pub(super) fn target_player_gains_life(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::TargetPlayerGainsLife { amount, .. } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let engine = &mut *cx.engine;
    let events = &mut *cx.events;
    let targets = cx.targets;
    let spell_label = cx.spell_label;

    if let Some(&tid) = targets.first() {
        if let Some(pi) = engine.state.player_idx(tid as i32) {
            let pid = engine.state.players[pi].id;
            apply_life_gain(engine, events, pid, amount, spell_label);
        }
    }

    Ok(EffectOutcome::Continue)
}

pub(super) fn target_player_loses_life(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::TargetPlayerLosesLife { amount, .. } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let engine = &mut *cx.engine;
    let events = &mut *cx.events;
    let targets = cx.targets;
    let spell_label = cx.spell_label;

    if let Some(&tid) = targets.first() {
        if let Some(pi) = engine.state.player_idx(tid as i32) {
            let pid = engine.state.players[pi].id;
            engine.state.players[pi].life -= amount as i32;
            events.push(rv1::RuledEvent {
                ev: Some(rv1::ruled_event::Ev::LifeChanged(rv1::LifeChanged {
                    player_id: pid,
                    new_total: engine.state.players[pi].life,
                    delta: -(amount as i32),
                })),
            });
            events.push(ev_log(format!(
                "P{pid} loses {amount} life ({spell_label})."
            )));
        }
    }

    Ok(EffectOutcome::Continue)
}

pub(super) fn each_opponent_loses_life_you_gain_equal(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::EachOpponentLosesLifeYouGainEqual { amount } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let engine = &mut *cx.engine;
    let events = &mut *cx.events;
    let controller = cx.controller;
    let spell_label = cx.spell_label;

    let opps: Vec<(usize, PlayerId)> = engine
        .state
        .players
        .iter()
        .enumerate()
        .filter(|(_, p)| engine.state.are_opponents(p.id, controller) && !p.has_lost)
        .map(|(i, p)| (i, p.id))
        .collect();
    let mut total_lost: u32 = 0;
    for (pi, pid) in opps {
        engine.state.players[pi].life -= amount as i32;
        total_lost += amount;
        events.push(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::LifeChanged(rv1::LifeChanged {
                player_id: pid,
                new_total: engine.state.players[pi].life,
                delta: -(amount as i32),
            })),
        });
        events.push(ev_log(format!(
            "P{pid} loses {amount} life ({spell_label})."
        )));
    }
    // One event, not one per opponent: the card gains "that much life" as a single amount.
    apply_life_gain(engine, events, controller, total_lost, spell_label);

    Ok(EffectOutcome::Continue)
}

pub(super) fn drain_target(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::DrainTarget { amount, .. } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let engine = &mut *cx.engine;
    let events = &mut *cx.events;
    let targets = cx.targets;
    let controller = cx.controller;
    let spell_label = cx.spell_label;

    if let Some(&tid) = targets.first() {
        if let Some(pi) = engine.state.player_idx(tid as i32) {
            let pid = engine.state.players[pi].id;
            engine.state.players[pi].life -= amount as i32;
            events.push(rv1::RuledEvent {
                ev: Some(rv1::ruled_event::Ev::LifeChanged(rv1::LifeChanged {
                    player_id: pid,
                    new_total: engine.state.players[pi].life,
                    delta: -(amount as i32),
                })),
            });
            events.push(ev_log(format!(
                "P{pid} loses {amount} life ({spell_label})."
            )));
        }
        apply_life_gain(engine, events, controller, amount, spell_label);
    }

    Ok(EffectOutcome::Continue)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::PlayerState;

    fn prohibition_source(engine: &mut GameEngine, player: PlayerId) -> ObjectId {
        let pi = engine.state.player_idx(player).unwrap();
        let source = engine.state.players[pi].hand[0];
        engine.state.objects.get_mut(&source).unwrap().card_id = "giant_cindermaw".into();
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            source,
            Zone::Battlefield,
            None,
        )
        .expect("put prohibition source onto battlefield");
        source
    }

    #[test]
    fn issue_175_relative_prohibitions_follow_derived_control_in_three_seats() {
        for scope in [
            RelativePlayerSet::All,
            RelativePlayerSet::Controller,
            RelativePlayerSet::Opponents,
        ] {
            let mut engine = GameEngine::new(175_101, &[10, 20], 20, None, true).unwrap();
            engine.state.players.push(PlayerState::new(30, 20));
            let source = prohibition_source(&mut engine, 20);
            let mut values = engine.copiable_values_for(source).unwrap();
            values.face.static_abilities =
                vec![StaticAbilityDef::ProhibitLifeGain { players: scope }];
            engine
                .state
                .objects
                .get_mut(&source)
                .unwrap()
                .copiable_values = Some(values);
            for controller in [20, 30] {
                if controller == 30 {
                    engine.state.continuous_effects.push(ContinuousEffect {
                        trigger_grant_origin: None,
                        source_id: None,
                        affected: AffectedScope::Single(source),
                        kind: ContinuousEffectKind::Layer2Control {
                            controller: tricerules_cards::ControllerReference::Fixed(30),
                        },
                        condition: None,
                        duration: EffectDuration::UntilEndOfTurn,
                        timestamp: engine.state.command_index,
                    });
                }
                for player in [10, 20, 30] {
                    let prohibited = match scope {
                        RelativePlayerSet::All => true,
                        RelativePlayerSet::Controller => player == controller,
                        RelativePlayerSet::Opponents => player != controller,
                    };
                    let before =
                        engine.state.players[engine.state.player_idx(player).unwrap()].life;
                    let mut events = Vec::new();
                    let gained = apply_life_gain_without_triggers(
                        &mut engine,
                        &mut events,
                        player,
                        1,
                        "scope test",
                    );
                    assert_eq!(
                        gained.is_none(),
                        prohibited,
                        "{scope:?}: controller {controller}, recipient {player}"
                    );
                    assert_eq!(
                        engine.state.players[engine.state.player_idx(player).unwrap()].life,
                        before + i32::from(!prohibited)
                    );
                    assert_eq!(
                        events.is_empty(),
                        prohibited,
                        "prohibited gains emit no log or life event"
                    );
                }
                assert_eq!(engine.state.objects[&source].owner, 20);
            }
        }
    }

    #[test]
    fn issue_175_prohibition_tracks_copy_blanking_face_down_and_zone_lifetime() {
        let mut engine = GameEngine::new(175_102, &[0, 1], 20, None, true).unwrap();
        let source = prohibition_source(&mut engine, 0);
        let second = prohibition_source(&mut engine, 1);
        let values = engine.copiable_values_for(source).unwrap();
        let copy = engine.state.objects.get_mut(&second).unwrap();
        copy.card_id = "clone".into();
        copy.copiable_values = Some(values);
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            source,
            Zone::Graveyard,
            None,
        )
        .unwrap();
        assert!(
            !engine.can_player_gain_life(0),
            "the remaining copy still prohibits gain"
        );
        engine.state.objects.get_mut(&second).unwrap().face_down = true;
        assert!(engine.can_player_gain_life(0));
        engine.state.objects.get_mut(&second).unwrap().face_down = false;
        engine.state.continuous_effects.push(ContinuousEffect {
            trigger_grant_origin: None,
            source_id: None,
            affected: AffectedScope::Single(second),
            kind: ContinuousEffectKind::Layer6RemoveAllAbilities,
            condition: None,
            duration: EffectDuration::UntilEndOfTurn,
            timestamp: engine.state.command_index,
        });
        assert!(
            engine.can_player_gain_life(0),
            "copied printed abilities can be removed"
        );
        engine.state.continuous_effects.clear();
        assert!(
            !engine.can_player_gain_life(0),
            "restoring abilities restores the prohibition"
        );
        move_object_to_zone(&mut engine.state, engine.registry, second, Zone::Hand, None).unwrap();
        assert!(
            engine.can_player_gain_life(0),
            "no stale prohibition after the last source leaves"
        );
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            second,
            Zone::Battlefield,
            None,
        )
        .unwrap();
        assert!(
            engine.can_player_gain_life(0),
            "a new Clone occurrence does not retain copied values"
        );
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            source,
            Zone::Battlefield,
            None,
        )
        .unwrap();
        assert!(
            !engine.can_player_gain_life(0),
            "a returned Cindermaw has its printed ability"
        );
    }

    #[test]
    fn issue_175_zero_or_unknown_recipient_gains_emit_nothing() {
        let mut engine = GameEngine::new(175_103, &[0, 1], 20, None, true).unwrap();
        for (player, amount) in [(0, 0), (99, 3)] {
            let mut events = Vec::new();
            assert!(apply_life_gain_without_triggers(
                &mut engine,
                &mut events,
                player,
                amount,
                "no gain"
            )
            .is_none());
            assert!(events.is_empty());
        }
    }

    #[test]
    fn lose_life_recipient_sets_are_player_generic_and_skip_lost_players() {
        let mut engine = GameEngine::new(87, &[10, 20], 20, None, true).expect("two-player engine");
        engine.state.players.push(PlayerState::new(30, 20));
        let mut lost_player = PlayerState::new(40, 20);
        lost_player.has_lost = true;
        engine.state.players.push(lost_player);

        assert_eq!(
            simple_player_recipients(
                &engine.state,
                10,
                30,
                None,
                None,
                PlayerRecipient::Controller
            ),
            vec![10]
        );
        assert_eq!(
            simple_player_recipients(
                &engine.state,
                10,
                30,
                None,
                None,
                PlayerRecipient::AffectedPlayer
            ),
            vec![30]
        );
        assert_eq!(
            simple_player_recipients(
                &engine.state,
                10,
                30,
                None,
                None,
                PlayerRecipient::EachOpponent
            ),
            vec![20, 30]
        );
        assert_eq!(
            simple_player_recipients(
                &engine.state,
                10,
                30,
                None,
                None,
                PlayerRecipient::EachPlayer
            ),
            vec![10, 20, 30]
        );
    }
}
