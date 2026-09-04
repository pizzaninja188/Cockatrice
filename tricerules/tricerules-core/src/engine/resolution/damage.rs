use super::*;
use crate::engine::damage::{DamageEvent, DamageRecipient, DamageSpec};

pub(super) fn damage_target(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::DamageTarget { amount, .. } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let source_has_deathtouch = cx
        .engine
        .resolving_source_has_keyword(cx.top, Keyword::Deathtouch);
    let source_has_lifelink = cx
        .engine
        .resolving_source_has_keyword(cx.top, Keyword::Lifelink);
    let engine = &mut *cx.engine;
    let events = &mut *cx.events;
    let targets = cx.targets;
    let top = cx.top;
    let spell_label = cx.spell_label;

    // CR 107.3: `amount` may be the cast-time X (Fireball) or a literal (Bolt).
    let amount = engine.resolve_amount(
        &amount,
        AmountContext::for_stack_item(top, top.controller)
            .with_previous_effect_result(cx.previous_effect_result),
    );
    let Some(&tid) = targets.first() else {
        return Ok(EffectOutcome::Continue);
    };
    let recipient = engine
        .state
        .player_idx(tid as i32)
        .map(|index| DamageRecipient::Player(engine.state.players[index].id))
        .unwrap_or(DamageRecipient::Permanent(tid));
    let damage = vec![DamageSpec {
        event: DamageEvent::noncombat(
            resolving_damage_source_id(top),
            top.controller,
            spell_label,
            recipient,
            amount,
        ),
        source_has_deathtouch,
        source_has_lifelink,
    }];
    let Some(completed) = engine.process_or_park_damage_batch(top, damage, events) else {
        return Ok(EffectOutcome::Suspended);
    };
    engine.commit_completed_damage_batch(&completed, events);

    Ok(EffectOutcome::Continue)
}

pub(super) fn creature_deals_damage_equal_to_power(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::CreatureDealsDamageEqualToPower { .. } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let Some(source) = cx
        .targets_by_role
        .first()
        .and_then(|targets| targets.first())
        .copied()
    else {
        return Ok(EffectOutcome::Continue);
    };
    let Some(recipient) = cx
        .targets_by_role
        .get(1)
        .and_then(|targets| targets.first())
        .copied()
    else {
        return Ok(EffectOutcome::Continue);
    };
    let Some(characteristics) = cx.engine.characteristics(source) else {
        return Ok(EffectOutcome::Continue);
    };
    if !characteristics.is_creature() {
        return Ok(EffectOutcome::Continue);
    }
    let amount = characteristics.power.unwrap_or(0);
    let controller = characteristics.controller;
    let source_has_deathtouch = cx.engine.effective_has_keyword(source, Keyword::Deathtouch);
    let source_has_lifelink = cx.engine.effective_has_keyword(source, Keyword::Lifelink);
    let damage = vec![DamageSpec {
        event: DamageEvent::noncombat(
            source,
            controller,
            object_display_name(&cx.engine.state, cx.engine.registry, source),
            DamageRecipient::Permanent(recipient),
            amount,
        ),
        source_has_deathtouch,
        source_has_lifelink,
    }];
    let Some(completed) = cx
        .engine
        .process_or_park_damage_batch(cx.top, damage, cx.events)
    else {
        return Ok(EffectOutcome::Suspended);
    };
    cx.engine
        .commit_completed_damage_batch(&completed, cx.events);
    Ok(EffectOutcome::Continue)
}

pub(super) fn fight(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::Fight { first, second } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };

    let first_filter_index = matches!(first, EffectSubject::Chosen(_)).then_some(0);
    let second_filter_index = matches!(second, EffectSubject::Chosen(_))
        .then_some(usize::from(first_filter_index.is_some()));
    let first_targets = first_filter_index
        .and_then(|index| cx.targets_by_role.get(index))
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let second_targets = second_filter_index
        .and_then(|index| cx.targets_by_role.get(index))
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let Some(first_id) = resolve_effect_subject(cx.engine, cx.top, first_targets, &first) else {
        return Ok(EffectOutcome::Continue);
    };
    let Some(second_id) = resolve_effect_subject(cx.engine, cx.top, second_targets, &second) else {
        return Ok(EffectOutcome::Continue);
    };

    let Some(first_characteristics) = cx.engine.characteristics(first_id) else {
        return Ok(EffectOutcome::Continue);
    };
    let Some(second_characteristics) = cx.engine.characteristics(second_id) else {
        return Ok(EffectOutcome::Continue);
    };
    if !first_characteristics.is_creature() || !second_characteristics.is_creature() {
        return Ok(EffectOutcome::Continue);
    }

    let first_power = first_characteristics.power.unwrap_or(0);
    let second_power = second_characteristics.power.unwrap_or(0);
    let first_controller = first_characteristics.controller;
    let second_controller = second_characteristics.controller;
    let first_has_deathtouch = cx
        .engine
        .effective_has_keyword(first_id, Keyword::Deathtouch);
    let first_has_lifelink = cx.engine.effective_has_keyword(first_id, Keyword::Lifelink);
    let second_has_deathtouch = cx
        .engine
        .effective_has_keyword(second_id, Keyword::Deathtouch);
    let second_has_lifelink = cx
        .engine
        .effective_has_keyword(second_id, Keyword::Lifelink);
    let first_name = object_display_name(&cx.engine.state, cx.engine.registry, first_id);
    let second_name = object_display_name(&cx.engine.state, cx.engine.registry, second_id);

    let damage = if first_id == second_id {
        vec![DamageSpec {
            event: DamageEvent::noncombat(
                first_id,
                first_controller,
                first_name,
                DamageRecipient::Permanent(first_id),
                first_power.saturating_mul(2),
            ),
            source_has_deathtouch: first_has_deathtouch,
            source_has_lifelink: first_has_lifelink,
        }]
    } else {
        vec![
            DamageSpec {
                event: DamageEvent::noncombat(
                    first_id,
                    first_controller,
                    first_name,
                    DamageRecipient::Permanent(second_id),
                    first_power,
                ),
                source_has_deathtouch: first_has_deathtouch,
                source_has_lifelink: first_has_lifelink,
            },
            DamageSpec {
                event: DamageEvent::noncombat(
                    second_id,
                    second_controller,
                    second_name,
                    DamageRecipient::Permanent(first_id),
                    second_power,
                ),
                source_has_deathtouch: second_has_deathtouch,
                source_has_lifelink: second_has_lifelink,
            },
        ]
    };
    let Some(completed) = cx
        .engine
        .process_or_park_damage_batch(cx.top, damage, cx.events)
    else {
        return Ok(EffectOutcome::Suspended);
    };
    cx.engine
        .commit_completed_damage_batch(&completed, cx.events);
    Ok(EffectOutcome::Continue)
}

pub(super) fn damage_targets(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::DamageTargets {
        target: filter,
        amount,
        division,
        ..
    } = effect
    else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let source_has_deathtouch = cx
        .engine
        .resolving_source_has_keyword(cx.top, Keyword::Deathtouch);
    let source_has_lifelink = cx
        .engine
        .resolving_source_has_keyword(cx.top, Keyword::Lifelink);
    let target_source = TargetSourceIdentity::for_stack_item(cx.engine, cx.top);
    let engine = &mut *cx.engine;
    let events = &mut *cx.events;
    let targets = cx.targets;
    let controller = cx.controller;
    let spell_label = cx.spell_label;

    // CR 608.2b: skip targets that became illegal at resolution. Fireball's even division is
    // calculated only after that filtering; Fire's choose-at-cast allocation remains attached to
    // each target independently.
    let even_damage = if matches!(division, DamageDivision::EvenAtResolution) {
        let legal_count = targets
            .iter()
            .filter(|&&tid| {
                super::targeting::target_filter_legal_at_resolution(
                    engine,
                    &filter,
                    tid,
                    controller,
                    target_source,
                    cx.top.trigger_context,
                )
            })
            .count() as u32;
        if legal_count == 0 {
            0
        } else {
            engine
                .resolve_amount(
                    &amount,
                    AmountContext::for_stack_item(cx.top, controller)
                        .with_previous_effect_result(cx.previous_effect_result),
                )
                .checked_div(legal_count)
                .unwrap_or(0)
        }
    } else {
        0
    };
    let mut damage = Vec::new();
    for (i, &tid) in targets.iter().enumerate() {
        let damage_amount = if matches!(division, DamageDivision::EvenAtResolution) {
            even_damage
        } else {
            cx.target_damage.get(i).copied().unwrap_or(0)
        };
        if damage_amount == 0 {
            continue;
        }
        if !super::targeting::target_filter_legal_at_resolution(
            engine,
            &filter,
            tid,
            controller,
            target_source,
            cx.top.trigger_context,
        ) {
            events.push(ev_log(format!(
                "{spell_label}: target {} is no longer legal, skipping.",
                tid
            )));
            continue;
        }
        let recipient = if let Some(pi) = engine.state.player_idx(tid as i32) {
            let pid = engine.state.players[pi].id;
            DamageRecipient::Player(pid)
        } else {
            DamageRecipient::Permanent(tid)
        };
        damage.push(DamageSpec {
            event: DamageEvent::noncombat(
                resolving_damage_source_id(cx.top),
                controller,
                spell_label,
                recipient,
                damage_amount,
            ),
            source_has_deathtouch,
            source_has_lifelink,
        });
    }
    let Some(completed) = engine.process_or_park_damage_batch(cx.top, damage, events) else {
        return Ok(EffectOutcome::Suspended);
    };
    engine.commit_completed_damage_batch(&completed, events);

    Ok(EffectOutcome::Continue)
}

/// CR 120.3a + CR 115.1: untargeted damage to a named player (Sulfuric Vortex, Serendib Efreet).
///
/// Deliberately *not* folded into [`damage_targets`]: that one gates both its branches on a
/// per-target `damage_amount == 0` check and logs unconditionally, so sharing a body would
/// change its behaviour.
pub(super) fn damage_player(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::DamagePlayer { amount, who } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let amount = cx.engine.resolve_amount(
        &amount,
        AmountContext::for_stack_item(cx.top, cx.controller)
            .with_previous_effect_result(cx.previous_effect_result),
    );
    let source_has_lifelink = cx
        .engine
        .resolving_source_has_keyword(cx.top, Keyword::Lifelink);
    // CR 101.4: APNAP for the multi-player recipients, so the log and the life-loss order are
    // reproducible in a replay.
    let recipients = player_recipients(cx, who);
    let damage: Vec<_> = recipients
        .into_iter()
        .map(|player| DamageSpec {
            event: DamageEvent::noncombat(
                resolving_damage_source_id(cx.top),
                cx.controller,
                cx.spell_label,
                DamageRecipient::Player(player),
                amount,
            ),
            source_has_deathtouch: false,
            source_has_lifelink,
        })
        .collect();
    let Some(completed) = cx
        .engine
        .process_or_park_damage_batch(cx.top, damage, cx.events)
    else {
        return Ok(EffectOutcome::Suspended);
    };
    cx.engine
        .commit_completed_damage_batch(&completed, cx.events);
    Ok(EffectOutcome::Continue)
}

pub(super) fn damage_attacked_player_or_planeswalker(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::DamageAttackedPlayerOrPlaneswalker { amount } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let recipient = if let Some(player) = cx.top.trigger_context.attacked_player {
        Some(DamageRecipient::Player(player))
    } else if let Some(permanent) = cx.top.trigger_context.attacked_planeswalker {
        let generation = cx
            .engine
            .state
            .zone_change_generation
            .get(&permanent.object_id)
            .copied()
            .unwrap_or(0);
        cx.engine
            .state
            .objects
            .get(&permanent.object_id)
            .is_some_and(|object| object.zone == Zone::Battlefield)
            .then_some(())
            .filter(|_| generation == permanent.zone_change_generation)
            .map(|_| DamageRecipient::Permanent(permanent.object_id))
    } else {
        None
    };
    let Some(recipient) = recipient else {
        return Ok(EffectOutcome::Continue);
    };
    let amount = cx.engine.resolve_amount(
        &amount,
        AmountContext::for_stack_item(cx.top, cx.controller)
            .with_previous_effect_result(cx.previous_effect_result),
    );
    let source_has_lifelink = cx
        .engine
        .resolving_source_has_keyword(cx.top, Keyword::Lifelink);
    let damage = vec![DamageSpec {
        event: DamageEvent::noncombat(
            resolving_damage_source_id(cx.top),
            cx.controller,
            cx.spell_label,
            recipient,
            amount,
        ),
        source_has_deathtouch: false,
        source_has_lifelink,
    }];
    let Some(completed) = cx
        .engine
        .process_or_park_damage_batch(cx.top, damage, cx.events)
    else {
        return Ok(EffectOutcome::Suspended);
    };
    cx.engine
        .commit_completed_damage_batch(&completed, cx.events);
    Ok(EffectOutcome::Continue)
}

#[cfg(test)]
mod damage_source_tests {
    use super::*;

    fn item(id: ObjectId, source_permanent_id: Option<ObjectId>) -> StackItem {
        StackItem {
            id,
            controller: 0,
            card_id: "test".into(),
            targets: Vec::new(),
            ability_text: source_permanent_id.map(|_| "ability".into()),
            source_permanent_id,
            source_owner: source_permanent_id.map(|_| 0),
            source_zone_change: 0,
            source_face_change: 0,
            ability_index: None,
            activated_ability: None,
            triggered_ability: None,
            is_triggered: source_permanent_id.is_some(),
            is_copy: false,
            face_index: 0,
            cast_method: SpellCastMethod::Normal,
            sneak_attack: None,
            chosen_x: 0,
            chosen_modes: Vec::new(),
            cast_condition_results: Vec::new(),
            cast_occurrence: None,
            cast_cost_receipts: Vec::new(),
            payment_result: CardResultCohort::default(),
            resolution_branch_choices: Default::default(),
            blight_receipts: Vec::new(),
            trigger_context: TriggerContext::default(),
        }
    }

    #[test]
    fn spells_deal_damage_as_the_stack_card_and_abilities_as_their_physical_source() {
        assert_eq!(resolving_damage_source_id(&item(40, None)), 40);
        assert_eq!(resolving_damage_source_id(&item(90, Some(12))), 12);
    }

    fn fight_engine() -> (GameEngine, ObjectId, ObjectId) {
        let decks = Some(vec![
            vec!["grizzly_bears".to_string(); 20],
            vec!["hill_giant".to_string(); 20],
        ]);
        let mut engine = GameEngine::new(117_900, &[0, 1], 20, decks, true).expect("new");
        let first = engine.state.players[0]
            .library
            .pop_front()
            .expect("first fighter");
        let second = engine.state.players[1]
            .library
            .pop_front()
            .expect("second fighter");
        for (player, object_id) in [(0, first), (1, second)] {
            engine.state.players[player].battlefield.push(object_id);
            let object = engine.state.objects.get_mut(&object_id).expect("fighter");
            object.zone = Zone::Battlefield;
            object.summoning_sick = false;
        }
        (engine, first, second)
    }

    fn resolve_source_fight(
        engine: &mut GameEngine,
        source: ObjectId,
        chosen: ObjectId,
    ) -> Result<EffectOutcome, EngineError> {
        let top = item(900, Some(source));
        let targets = vec![chosen];
        let targets_by_role = vec![vec![chosen]];
        let target_damage = Vec::new();
        let target_group_indices = vec![0];
        let previous_effect_result = EffectResult::default();
        let mut effect_result = EffectResult::default();
        let mut events = Vec::new();
        let mut cx = EffectCx {
            engine,
            events: &mut events,
            targets: &targets,
            targets_by_role: &targets_by_role,
            target_damage: &target_damage,
            target_group_indices: &target_group_indices,
            top: &top,
            controller: 0,
            affected_player: 0,
            spell_label: "test fight ability",
            previous_effect_result: &previous_effect_result,
            effect_result: &mut effect_result,
            effect_index: 0,
        };
        fight(
            &mut cx,
            SpellEffectKind::Fight {
                first: EffectSubject::Source,
                second: EffectSubject::Chosen(Box::new(TargetFilter::default_creature())),
            },
        )
    }

    #[test]
    fn source_bound_fight_resolves_with_only_the_opposing_creature_targeted() {
        let (mut engine, first, second) = fight_engine();

        assert_eq!(
            resolve_source_fight(&mut engine, first, second).expect("fight"),
            EffectOutcome::Continue
        );

        assert_eq!(engine.state.objects[&first].damage, 3);
        assert_eq!(engine.state.objects[&second].damage, 2);
    }

    #[test]
    fn stale_source_bound_fighter_causes_no_fight_damage() {
        let (mut engine, first, second) = fight_engine();
        *engine
            .state
            .zone_change_generation
            .entry(first)
            .or_default() += 1;

        assert_eq!(
            resolve_source_fight(&mut engine, first, second).expect("no-op fight"),
            EffectOutcome::Continue
        );

        assert_eq!(engine.state.objects[&first].damage, 0);
        assert_eq!(engine.state.objects[&second].damage, 0);
    }

    #[test]
    fn a_creature_fighting_itself_deals_twice_its_power_once() {
        let (mut engine, first, _) = fight_engine();
        engine.state.players[0].life = 10;
        engine.state.continuous_effects.push(ContinuousEffect {
            trigger_grant_origin: None,
            source_id: None,
            affected: AffectedScope::Single(first),
            kind: ContinuousEffectKind::Layer6AddKeyword(Keyword::Lifelink),
            condition: None,
            duration: EffectDuration::UntilEndOfTurn,
            timestamp: engine.state.command_index,
        });

        assert_eq!(
            resolve_source_fight(&mut engine, first, first).expect("self fight"),
            EffectOutcome::Continue
        );

        assert_eq!(engine.state.players[0].life, 14);
        assert_eq!(engine.state.objects[&first].damage, 4);
    }
}
