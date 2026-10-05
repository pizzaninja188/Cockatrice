use super::*;

/// CR 119.3 / 119.9: `player` gains `amount` life — emit `LifeChanged`, log it, and fire
/// [`GameEvent::LifeGained`].
///
/// The single funnel for every life *gain* edge (spell effects, drain, exile-for-life, lifelink),
/// the gain-side analog of `engine::set_tapped`: a "whenever you gain life" trigger hangs off this
/// one call instead of auditing every mutation site. Both gain and loss mutations commit their
/// history through `history::commit_life_change`, separately from trigger dispatch.
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
) -> Result<(), EngineError> {
    if let Some(event) = apply_life_gain_without_triggers(engine, events, player, amount, reason)? {
        engine.fire_triggers(&[event], events);
    }
    Ok(())
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
) -> Result<Option<GameEvent>, EngineError> {
    if amount == 0 || !engine.can_player_gain_life(player) {
        return Ok(None);
    }
    let Some(pi) = engine.state.player_idx(player) else {
        return Ok(None);
    };
    let mut amount = amount;
    let mut sources = engine
        .state
        .objects
        .iter()
        .filter_map(|(&oid, object)| (object.zone == Zone::Battlefield).then_some(oid))
        .collect::<Vec<_>>();
    sources.sort_unstable();
    // Mandatory identical pure x2 replacements commute. Apply every active occurrence once
    // (CR 614.5); future noncommuting/optional effects require affected-player ordering.
    for source in sources {
        if engine.controller_of(source) != Some(player) {
            continue;
        }
        for ability in engine.active_static_ability_definitions(source) {
            if matches!(ability, StaticAbilityDef::DoubleControllerLifeGain) {
                amount = amount
                    .checked_mul(2)
                    .ok_or(EngineError::LifeNumericRange("life gain replacement"))?;
            }
        }
    }
    let delta =
        i32::try_from(amount).map_err(|_| EngineError::LifeNumericRange("life gain delta"))?;
    let first_this_turn = engine.state.turn_history.current.player(player).life_gained == 0;
    crate::engine::history::commit_life_change_checked(&mut engine.state, pi, delta)?;
    events.push(rv1::RuledEvent {
        ev: Some(rv1::ruled_event::Ev::LifeChanged(rv1::LifeChanged {
            player_id: player,
            new_total: engine.state.players[pi].life,
            delta,
        })),
    });
    events.push(ev_log(format!("P{player} gains {amount} life ({reason}).")));
    Ok(Some(GameEvent::LifeGained {
        player,
        first_this_turn,
    }))
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
    apply_life_gain(engine, events, controller, amount, spell_label)?;

    Ok(EffectOutcome::Continue)
}

/// CR 119.3: the players named by `who` lose life. Untargeted (CR 115.1).
///
/// `LifeAmount::TargetManaValue` (CR 202.3) reads the mana value of the object the *spell*
/// targets — a sibling effect declared it, this one only borrows it. A battlefield departure
/// uses the target generation's last-known derived mana value, including copied characteristics.
/// Reanimate's graveyard-to-battlefield move reads the resulting live object instead.
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
            .and_then(|tid| {
                let selected_generation = cx.top.targets.iter().find_map(|target| {
                    (target.object_id == *tid)
                        .then_some(target.zone_change_generation)
                        .flatten()
                });
                selected_generation
                    .and_then(|generation| {
                        engine
                            .state
                            .last_known_mana_value_by_generation
                            .get(&(*tid, generation))
                            .copied()
                    })
                    .or_else(|| {
                        super::super::characteristics::characteristics_from(
                            &engine.state,
                            engine.registry,
                            *tid,
                        )
                        .map(|characteristics| characteristics.mana_value)
                    })
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
        crate::engine::history::commit_life_change(&mut engine.state, pi, -(amount as i32));
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
    let amount = cx.engine.resolve_amount(
        &amount,
        AmountContext::for_stack_item(cx.top, cx.controller)
            .with_previous_effect_result(cx.previous_effect_result),
    );
    let engine = &mut *cx.engine;
    let events = &mut *cx.events;
    let targets = cx.targets;
    let spell_label = cx.spell_label;

    if let Some(&tid) = targets.first() {
        if let Some(pi) = engine.state.player_idx(tid as i32) {
            let pid = engine.state.players[pi].id;
            apply_life_gain(engine, events, pid, amount, spell_label)?;
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
            crate::engine::history::commit_life_change(&mut engine.state, pi, -(amount as i32));
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
        let delta = crate::engine::life_numeric::loss_delta(amount)?;
        crate::engine::history::commit_life_change_checked(&mut engine.state, pi, delta)?;
        total_lost = total_lost
            .checked_add(amount)
            .ok_or(EngineError::LifeNumericRange("opponent drain sum"))?;
        events.push(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::LifeChanged(rv1::LifeChanged {
                player_id: pid,
                new_total: engine.state.players[pi].life,
                delta,
            })),
        });
        events.push(ev_log(format!(
            "P{pid} loses {amount} life ({spell_label})."
        )));
    }
    // One event, not one per opponent: the card gains "that much life" as a single amount.
    apply_life_gain(engine, events, controller, total_lost, spell_label)?;

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
            let delta = crate::engine::life_numeric::loss_delta(amount)?;
            crate::engine::history::commit_life_change_checked(&mut engine.state, pi, delta)?;
            events.push(rv1::RuledEvent {
                ev: Some(rv1::ruled_event::Ev::LifeChanged(rv1::LifeChanged {
                    player_id: pid,
                    new_total: engine.state.players[pi].life,
                    delta,
                })),
            });
            events.push(ev_log(format!(
                "P{pid} loses {amount} life ({spell_label})."
            )));
        }
        apply_life_gain(engine, events, controller, amount, spell_label)?;
    }

    Ok(EffectOutcome::Continue)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::PlayerState;

    fn archive_source(engine: &mut GameEngine, player: PlayerId) -> ObjectId {
        let pi = engine.state.player_idx(player).unwrap();
        let source = engine.state.players[pi].hand[0];
        engine.state.objects.get_mut(&source).unwrap().card_id = "alhammarrets_archive".into();
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            source,
            Zone::Battlefield,
            None,
        )
        .unwrap();
        source
    }

    #[test]
    fn archive_occurrences_copy_control_blanking_and_departure_use_current_recipient() {
        let mut engine = GameEngine::new(104_802, &[10, 20], 20, None, true).unwrap();
        let source = archive_source(&mut engine, 10);
        let second = archive_source(&mut engine, 20);
        let mut copy = engine.copiable_values_for(source).unwrap();
        // A nonlegendary copied face isolates the primitive from the separate legend rule.
        copy.face.supertypes.clear();
        engine
            .state
            .objects
            .get_mut(&second)
            .unwrap()
            .copiable_values = Some(copy);
        engine.state.continuous_effects.push(ContinuousEffect {
            trigger_grant_origin: None,
            source_id: None,
            affected: AffectedScope::Single(second),
            kind: ContinuousEffectKind::Layer2Control {
                controller: ControllerReference::Fixed(10),
            },
            condition: None,
            duration: EffectDuration::UntilEndOfTurn,
            timestamp: 0,
        });
        let mut events = Vec::new();
        apply_life_gain(&mut engine, &mut events, 10, 2, "two copies").unwrap();
        apply_life_gain(&mut engine, &mut events, 20, 2, "other player").unwrap();
        assert_eq!(engine.state.players[0].life, 28);
        assert_eq!(engine.state.players[1].life, 22);
        assert_eq!(engine.state.objects[&second].owner, 20);
        engine.state.continuous_effects.push(ContinuousEffect {
            trigger_grant_origin: None,
            source_id: None,
            affected: AffectedScope::Single(source),
            kind: ContinuousEffectKind::Layer6RemoveAllAbilities,
            condition: None,
            duration: EffectDuration::UntilEndOfTurn,
            timestamp: 1,
        });
        apply_life_gain(&mut engine, &mut events, 10, 2, "one unblanked copy").unwrap();
        assert_eq!(engine.state.players[0].life, 32);
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            second,
            Zone::Graveyard,
            None,
        )
        .unwrap();
        apply_life_gain(&mut engine, &mut events, 10, 2, "departed copy").unwrap();
        assert_eq!(engine.state.players[0].life, 34);
    }

    #[test]
    fn archive_zero_unknown_and_prohibited_huge_gain_short_circuit() {
        let mut engine = GameEngine::new(104_803, &[10, 20], 20, None, true).unwrap();
        archive_source(&mut engine, 10);
        for (player, amount) in [(10, 0), (99, u32::MAX)] {
            let mut events = Vec::new();
            assert!(apply_life_gain_without_triggers(
                &mut engine,
                &mut events,
                player,
                amount,
                "none"
            )
            .unwrap()
            .is_none());
            assert!(events.is_empty());
        }
        prohibition_source(&mut engine, 20);
        let before = engine.diagnostic_snapshot().unwrap();
        let mut events = Vec::new();
        assert!(apply_life_gain_without_triggers(
            &mut engine,
            &mut events,
            10,
            u32::MAX,
            "prohibited"
        )
        .unwrap()
        .is_none());
        assert!(events.is_empty());
        assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
    }

    #[test]
    fn archive_numeric_boundaries_reject_before_history_events_or_triggers() {
        for (amount, life, history) in [
            (u32::MAX, 20, 0),
            (i32::MAX as u32 / 2 + 1, 20, 0),
            (1, i32::MAX - 1, 0),
            (1, 20, u64::MAX - 1),
        ] {
            let mut engine = GameEngine::new(104_804, &[10, 20], 20, None, true).unwrap();
            archive_source(&mut engine, 10);
            engine.state.players[0].life = life;
            engine.state.turn_history.current.player_mut(10).life_gained = history;
            let before = engine.diagnostic_snapshot().unwrap();
            let mut events = Vec::new();
            assert!(matches!(
                apply_life_gain(&mut engine, &mut events, 10, amount, "overflow"),
                Err(EngineError::LifeNumericRange(_))
            ));
            assert!(events.is_empty());
            assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
        }
    }

    #[test]
    fn archive_replaces_one_life_gain_before_history_and_public_event() {
        let mut engine = GameEngine::new(104_801, &[10, 20], 20, None, true).unwrap();
        let source = engine.state.players[0].hand[0];
        engine.state.objects.get_mut(&source).unwrap().card_id = "alhammarrets_archive".into();
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            source,
            Zone::Battlefield,
            None,
        )
        .unwrap();
        let mut events = Vec::new();
        apply_life_gain(&mut engine, &mut events, 10, 5, "gain").unwrap();
        assert_eq!(engine.state.players[0].life, 30);
        assert_eq!(engine.state.turn_history.current.player(10).life_gained, 10);
        let deltas = events
            .iter()
            .filter_map(|event| match event.ev.as_ref() {
                Some(rv1::ruled_event::Ev::LifeChanged(change)) => Some(change.delta),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(deltas, vec![10]);
    }

    #[test]
    fn issue_170_gain_history_commits_before_triggers_and_rolls_over() {
        let mut engine = GameEngine::new(170001, &[10, 20], 20, None, true).unwrap();
        let mut events = Vec::new();
        assert_eq!(engine.state.turn_history.current.player(10).life_gained, 0);
        assert!(
            apply_life_gain_without_triggers(&mut engine, &mut events, 10, 0, "zero")
                .unwrap()
                .is_none()
        );
        let gained = apply_life_gain_without_triggers(&mut engine, &mut events, 10, 2, "gain")
            .unwrap()
            .unwrap();
        assert_eq!(engine.state.turn_history.current.player(10).life_gained, 2);
        engine.fire_triggers(&[gained], &mut Vec::new());
        assert_eq!(engine.state.turn_history.current.player(10).life_gained, 2);
        assert_eq!(engine.state.turn_history.current.player(10).life_lost, 0);
        engine.state.turn_history.finish_turn();
        assert_eq!(engine.state.turn_history.previous.player(10).life_gained, 2);
        assert_eq!(engine.state.turn_history.current.player(10).life_gained, 0);
    }

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
            values.face.static_abilities = vec![tricerules_cards::IdentifiedAbility::fallback(
                "static_01",
                StaticAbilityDef::ProhibitLifeGain { players: scope },
            )
            .unwrap()];
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
                        RelativePlayerSet::TargetedPlayer { .. } => false,
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
                    )
                    .unwrap();
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
            .unwrap()
            .is_none());
            assert!(events.is_empty());
            assert_eq!(
                engine.state.turn_history.current.player(player).life_gained,
                0
            );
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
