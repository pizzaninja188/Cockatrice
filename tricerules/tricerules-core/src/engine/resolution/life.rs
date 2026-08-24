use super::*;

/// CR 118.3: `player` gains `amount` life — emit `LifeChanged`, log it, and fire
/// [`GameEvent::LifeGained`].
///
/// The single funnel for every life *gain* edge (spell effects, drain, exile-for-life, lifelink),
/// the gain-side analog of `engine::set_tapped`: a "whenever you gain life" trigger hangs off this
/// one call instead of auditing every mutation site. Life *loss* has no such funnel yet — no
/// implemented card watches for it.
///
/// One call is one life-gain event, so callers must not pre-sum unrelated gains: two lifelink
/// creatures in the same damage step gain separately and trigger separately. A gain of 0 is not an
/// event (CR 118.4) — no life change, no log line, no trigger.
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
    if amount == 0 {
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
