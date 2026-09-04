use super::damage::{DamageClassification, DamageRecipient};
use super::events::{ev_log, ev_trigger_order_required};
use super::presentation::{ability_presentation, child_presentation_ref, PresentationPath};
use super::targeting::{
    compute_ability_targets_with_context, legal_target_group_has_minimum, target_schema,
    TargetSourceIdentity,
};
use super::*;

/// One triggered ability that matched an event and is about to go on the stack (or be parked for
/// target selection) — the unit a trigger scan yields and [`GameEngine::push_trigger`] consumes.
///
/// The complete ability definition is captured when it triggers. This is required for abilities
/// granted by continuous effects, which can disappear before the trigger reaches the stack.
#[derive(Clone)]
pub(super) struct CollectedTrigger {
    pub source_id: ObjectId,
    pub card_id: String,
    pub face_index: usize,
    pub source_zone_change: u64,
    pub source_face_change: u64,
    /// Event-time/LKI characteristics of the permanent whose ability triggered. Synthetic
    /// delayed triggers are not abilities of their historical source and therefore omit this.
    pub source_fact: Option<TurnObjectFact>,
    /// Extra trigger instances fixed at the event boundary. Trigger placement can be delayed by
    /// cost payment and must not re-evaluate a modifier that appeared or disappeared meanwhile.
    pub additional_instances: u32,
    /// CR 603.3d: the ability's controller — the controller of its source permanent.
    pub controller: PlayerId,
    pub ability_index: usize,
    pub ability_origin: Option<TriggerAbilityOrigin>,
    pub presentation: Option<rv1::PresentationRef>,
    pub ability: TriggeredAbilityDef,
    pub ability_text: String,
    /// The event's affected player ("that player"), when distinct from the ability controller.
    pub trigger_context: TriggerContext,
}

fn trigger_ability_path(
    origin: &TriggerAbilityOrigin,
    ability: &TriggeredAbilityDef,
) -> Vec<tricerules_cards::AbilityId> {
    match origin {
        TriggerAbilityOrigin::Printed(definition)
        | TriggerAbilityOrigin::StaticGrant { definition, .. } => definition.ability_path.clone(),
        TriggerAbilityOrigin::ResolvingGrant(_) => vec![ability.ability_id.clone()],
    }
}

impl GameEngine {
    pub(super) fn event_object_fact(&self, object_id: ObjectId) -> Option<TurnObjectFact> {
        let object = self.state.objects.get(&object_id)?;
        let c = self.characteristics(object_id)?;
        Some(TurnObjectFact {
            object_id,
            zone_change_generation: self
                .state
                .zone_change_generation
                .get(&object_id)
                .copied()
                .unwrap_or(0),
            owner: object.owner,
            controller: c.controller,
            is_token: object.is_token(),
            types: c.types,
            all_creature_types: c.all_creature_types,
            keywords: c.keywords,
            power: c.power,
        })
    }

    pub(super) fn event_filter_matches(
        &self,
        filter: &PermanentEventFilter,
        fact: &TurnObjectFact,
        source: &TriggerSourceSnapshot,
    ) -> bool {
        super::history::permanent_event_fact_matches(
            &self.state,
            filter,
            fact,
            ConditionContext {
                controller: source.controller,
                source_object_id: source.object_id,
                source_zone_change: source.zone_change_generation,
                resolving_spell_id: None,
                stack_item: None,
            },
        )
    }

    pub(super) fn battlefield_leave_event(&self, object_id: ObjectId) -> Option<GameEvent> {
        self.state
            .objects
            .get(&object_id)
            .is_some_and(|object| object.zone == Zone::Battlefield)
            .then(|| self.trigger_source_snapshot(object_id))
            .flatten()
            .map(|source| GameEvent::LeavesBattlefield { source })
    }

    fn source_trigger_active(
        &self,
        source_id: ObjectId,
        source_generation: u64,
        predicate: impl Fn(&TriggeredAbilityDef) -> bool,
    ) -> bool {
        self.state.stack.iter().any(|item| {
            item.source_permanent_id == Some(source_id)
                && item.source_zone_change == source_generation
                && item.triggered_ability.as_ref().is_some_and(&predicate)
        }) || self.state.pending_triggers.iter().any(|trigger| {
            trigger.source_permanent_id == source_id
                && trigger.source_zone_change == source_generation
                && predicate(&trigger.ability)
        }) || self.state.staged_trigger_groups.iter().any(|group| {
            group.triggers.iter().any(|trigger| {
                trigger.source_permanent_id == source_id
                    && trigger.source_zone_change == source_generation
                    && predicate(&trigger.ability)
            })
        }) || self
            .state
            .pending_trigger_order
            .as_ref()
            .is_some_and(|pending| {
                pending.candidates.iter().any(|trigger| {
                    trigger.source_permanent_id == source_id
                        && trigger.source_zone_change == source_generation
                        && predicate(&trigger.ability)
                })
            })
    }

    pub(super) fn siege_defeat_trigger_active(
        &self,
        source_id: ObjectId,
        source_generation: u64,
    ) -> bool {
        self.source_trigger_active(source_id, source_generation, |ability| {
            ability
                .effect
                .iter()
                .any(|effect| matches!(effect, SpellEffectKind::SiegeDefeat))
        })
    }

    pub(super) fn saga_chapter_trigger_active(
        &self,
        source_id: ObjectId,
        source_generation: u64,
    ) -> bool {
        self.source_trigger_active(source_id, source_generation, |ability| {
            matches!(ability.trigger, TriggerCondition::SagaChapter { .. })
        })
    }

    pub(super) fn stage_siege_defeat_trigger(&mut self, source_id: ObjectId) {
        let Some(object) = self.state.objects.get(&source_id) else {
            return;
        };
        if object.zone != Zone::Battlefield {
            return;
        }
        let source_generation = self
            .state
            .zone_change_generation
            .get(&source_id)
            .copied()
            .unwrap_or(0);
        if self.siege_defeat_trigger_active(source_id, source_generation) {
            return;
        }
        let ability = TriggeredAbilityDef {
            ability_id: tricerules_cards::AbilityId::new("siege_defeat")
                .expect("intrinsic ability id"),
            presentation: tricerules_cards::AbilityPresentation::Fallback,
            trigger: TriggerCondition::WhenSelfDies,
            effect: vec![SpellEffectKind::SiegeDefeat],
            modal: None,
            targeting: None,
            may: false,
            intervening_if: None,
            max_triggers_per_turn: None,
            triggers_only_once: false,
        };
        let face_index = object.face_up_index;
        let card_id = object.card_id.clone();
        let controller = object.controller;
        let source_face_change = self
            .state
            .face_change_generation
            .get(&source_id)
            .copied()
            .unwrap_or(0);
        let mut trigger = CollectedTrigger {
            source_id,
            card_id: card_id.clone(),
            face_index,
            source_zone_change: source_generation,
            source_face_change,
            source_fact: self.event_object_fact(source_id),
            additional_instances: 0,
            controller,
            ability_index: usize::MAX,
            ability_origin: None,
            presentation: None,
            ability: ability.clone(),
            ability_text: ability.fallback_text(
                self.registry
                    .get(&card_id)
                    .and_then(|definition| definition.faces.get(face_index))
                    .map(|face| face.name.as_str())
                    .unwrap_or(&card_id),
            ),
            trigger_context: TriggerContext::default(),
        };
        trigger.additional_instances = self.additional_trigger_instances(&trigger);
        self.stage_triggers(vec![trigger]);
    }

    /// Collect one simultaneous event set and enqueue all matching triggered abilities as one
    /// CR 603.3b group.
    pub(super) fn fire_triggers(&mut self, events: &[GameEvent]) {
        if events.is_empty() {
            return;
        }

        // All permanents in a simultaneous ETB set exist before history or trigger checks.
        // Register every static ability first so event-time facts and trigger conditions see the
        // completed event's derived characteristics.
        for event in events {
            if let GameEvent::EntersBattlefield { object_id, .. } = event {
                if !self.state.room_states.contains_key(object_id) {
                    self.emit_static_abilities_on_enter(*object_id);
                }
            }
        }

        // CR 702.195c: designation-dependent continuous effects are reapplied before checking
        // whether the event set matched any trigger conditions.
        self.refresh_enduring_story_designations();

        self.record_committed_events(events);

        let mut delayed = Vec::new();
        for event in events {
            match event {
                GameEvent::ZoneChanges(batch) => {
                    for source in &batch.sources {
                        if batch.moves.iter().any(|m| {
                            m.before.object_id == source.object_id
                                && m.origin == Zone::Battlefield
                                && m.destination != Zone::Battlefield
                        }) {
                            self.state.last_known_counters_by_generation.insert(
                                (source.object_id, source.zone_change_generation),
                                source.counters.clone(),
                            );
                        }
                    }
                    for movement in &batch.moves {
                        if movement.origin == Zone::Battlefield
                            && movement.destination != Zone::Battlefield
                        {
                            delayed.extend(self.state.dispatch_event_observers(
                                ObservedGameEvent::BattlefieldDeparture {
                                    object: TriggerObjectRef {
                                        object_id: movement.before.object_id,
                                        zone_change_generation:
                                            movement.before.zone_change_generation,
                                        controller_at_event: movement.before.controller,
                                    },
                                    destination: movement.destination,
                                    was_creature:
                                        movement.before.types.iter().any(|kind| kind == "Creature"),
                                },
                            ));
                        }
                    }
                }
                GameEvent::PhaseBegan {
                    phase: rv1::PhaseId::EndStep,
                    active_player,
                } => delayed.extend(self.state.dispatch_event_observers(
                    ObservedGameEvent::BeginningOfEndStep {
                        active_player: *active_player,
                        turn_instance: self.state.turn_instance,
                    },
                )),
                GameEvent::Dies { source, .. } => {
                    delayed.extend(self.state.dispatch_event_observers(ObservedGameEvent::Dies(
                        TriggerObjectRef {
                            object_id: source.object_id,
                            zone_change_generation: source.zone_change_generation,
                            controller_at_event: source.controller,
                        },
                    )))
                }
                _ => {}
            }
        }
        let mut collected = self.collect_event_triggers(events);
        collected.extend(delayed.into_iter().map(|(watched, delayed)| {
            let ability_text = delayed.ability.fallback_text(&delayed.card_name);
            CollectedTrigger {
                source_id: delayed.source.object_id,
                card_id: delayed.card_id,
                face_index: delayed.source_face_index,
                source_zone_change: delayed.source.zone_change_generation,
                source_face_change: 0,
                source_fact: None,
                additional_instances: 0,
                controller: delayed.controller,
                ability_index: 0,
                ability_origin: None,
                presentation: delayed.presentation,
                ability_text,
                trigger_context: TriggerContext {
                    observed_object: Some(watched),
                    ..TriggerContext::default()
                },
                ability: delayed.ability,
            }
        }));
        self.stage_triggers(collected);
    }

    /// Collect matching triggers without staging them. Casts and activations use this boundary to
    /// snapshot becomes-the-target triggers at target selection, then commit them only after every
    /// cost has been paid and the spell or ability has successfully reached the stack.
    pub(super) fn collect_event_triggers(&self, events: &[GameEvent]) -> Vec<CollectedTrigger> {
        let mut sources = self.battlefield_sources_apnap();
        // Zone-leaving sources are no longer in the battlefield index. Their event-local snapshot
        // supplies the identity/controller needed for LKI trigger matching (CR 603.6/603.10).
        for source in events.iter().filter_map(|event| match event {
            GameEvent::Dies { source, .. }
            | GameEvent::LeavesBattlefield { source }
            | GameEvent::Sacrificed { source, .. } => Some(source.clone()),
            _ => None,
        }) {
            if !sources.iter().any(|existing| {
                existing.object_id == source.object_id
                    && existing.zone_change_generation == source.zone_change_generation
            }) {
                sources.push(source);
            }
        }

        let mut collected = Vec::new();
        let mut grouped_taps = HashSet::new();
        for event in events {
            let trigger_player = Self::trigger_player_for(event);
            let event_sources = match event {
                GameEvent::BecameTapped { action, .. } => &action.sources,
                GameEvent::LeavesBattlefield { source }
                | GameEvent::Dies { source, .. }
                | GameEvent::Sacrificed { source, .. } => events
                    .iter()
                    .find_map(|event| match event {
                        GameEvent::ZoneChanges(batch)
                            if batch.moves.iter().any(|movement| {
                                movement.before.object_id == source.object_id
                                    && movement.before.zone_change_generation
                                        == source.zone_change_generation
                            }) =>
                        {
                            Some(&batch.sources)
                        }
                        _ => None,
                    })
                    .unwrap_or(&sources),
                _ => &sources,
            };
            let mut event_triggers = self.collect_triggers(event, event_sources);
            if let GameEvent::BecameTapped { action, .. } = event {
                event_triggers.retain(|trigger| {
                    !matches!(
                        trigger.ability.trigger,
                        TriggerCondition::WheneverPlayerTapsCreature {
                            cardinality: TapTriggerCardinality::OneOrMorePerAction,
                            ..
                        }
                    ) || grouped_taps.insert((
                        action.id,
                        TriggerUseKey {
                            object_id: trigger.source_id,
                            zone_change_generation: trigger.source_zone_change,
                            ability_origin: trigger
                                .ability_origin
                                .clone()
                                .expect("battlefield ability origin"),
                        },
                    ))
                });
            }
            for trigger in &mut event_triggers {
                trigger.trigger_context.affected_player = match event {
                    GameEvent::BecameTapped { action, .. }
                        if matches!(
                            trigger.ability.trigger,
                            TriggerCondition::WheneverPlayerTapsCreature { .. }
                        ) =>
                    {
                        Some(action.actor)
                    }
                    _ => trigger_player,
                };
                if let GameEvent::SpellCast { fact } = event {
                    trigger.trigger_context.triggering_spell_mana_spent = Some(fact.mana_spent);
                }
                // Rampaging Ferocidon needs the entering creature's controller, and Aether
                // Flash needs the creature itself. Reuse the generation-bound reference so
                // either instruction follows current characteristics or the correct LKI.
                if let GameEvent::EntersBattlefield {
                    object_id,
                    chosen_x,
                } = event
                {
                    if trigger.source_id == *object_id
                        && trigger.ability.trigger == TriggerCondition::WhenSelfEntersBattlefield
                    {
                        trigger.trigger_context.entering_chosen_x = Some(*chosen_x);
                    }
                    if matches!(
                        trigger.ability.trigger,
                        TriggerCondition::WheneverPermanentEntersBattlefield { .. }
                    ) {
                        trigger.trigger_context.observed_object =
                            self.trigger_object_ref(*object_id);
                    }
                }
            }
            event_triggers.retain(|trigger| {
                let requires_event_context = matches!(
                    trigger.ability.intervening_if.as_ref(),
                    Some(GameCondition::TriggeringSpellManaSpent { .. })
                );
                !requires_event_context
                    || self.intervening_if_holds_at_generation(
                        trigger.source_id,
                        trigger.controller,
                        trigger.ability.intervening_if.as_ref(),
                        Some(trigger.source_zone_change),
                        Some(&trigger.trigger_context),
                    )
            });
            collected.extend(event_triggers);
        }
        for trigger in &mut collected {
            trigger.additional_instances = self.additional_trigger_instances(trigger);
        }
        collected
    }

    /// Turn a collected simultaneous group into a staged group (CR 603.3b), reserving each
    /// trigger's stack ObjectId.
    ///
    /// The APNAP sort lives here rather than in each `collect_triggers` arm: the invariant the
    /// drain depends on — triggers are contiguous per controller, active player's block first — is
    /// then stated once, and a new event arm cannot forget it. `sort_by_key` is stable, so each
    /// player's own printed/battlefield order survives.
    ///
    /// Nothing reaches the stack here. Placement is deferred to [`Self::flush_staged_triggers`],
    /// because this is called from inside resolution and from the SBA fixed point, neither of which
    /// can stop to ask a player a question.
    pub(super) fn stage_triggers(&mut self, mut collected: Vec<CollectedTrigger>) {
        collected = collected
            .into_iter()
            .flat_map(|trigger| {
                let count = trigger.additional_instances.saturating_add(1) as usize;
                std::iter::repeat_n(trigger, count)
            })
            .collect();
        if collected.is_empty() {
            return;
        }
        collected.sort_by_key(|trigger| self.state.apnap_rank(trigger.controller));
        collected.retain(|trigger| {
            let lifetime = trigger.ability.triggers_only_once;
            let cap = trigger.ability.max_triggers_per_turn;
            if !lifetime && cap.is_none() {
                return true;
            }
            let key = TriggerUseKey {
                object_id: trigger.source_id,
                zone_change_generation: trigger.source_zone_change,
                // Synthetic delayed triggers have already consumed their one-shot observer.
                // Give each such occurrence its own identity rather than a printed slot.
                ability_origin: trigger
                    .ability_origin
                    .clone()
                    .unwrap_or_else(|| self.state.allocate_trigger_grant_origin()),
            };
            let turn_key = (self.state.turn_instance, key.clone());
            if lifetime && self.state.triggered_once.contains(&key)
                || cap.is_some_and(|max| {
                    self.state
                        .trigger_uses_this_turn
                        .get(&turn_key)
                        .copied()
                        .unwrap_or(0)
                        >= max
                })
            {
                return false;
            }
            // Commit both restrictions together, before any ordering, targeting or optional
            // choice. A later decline, counter or failed resolution cannot refund a trigger.
            if lifetime {
                self.state.triggered_once.insert(key);
            }
            if cap.is_some() {
                *self
                    .state
                    .trigger_uses_this_turn
                    .entry(turn_key)
                    .or_default() += 1;
            }
            true
        });
        if collected.is_empty() {
            return;
        }
        let triggers = collected
            .into_iter()
            .map(|trigger| {
                let object_id = self.state.next_object_id;
                self.state.next_object_id += 1;
                let def = self.registry.get(&trigger.card_id);
                let card_name = def
                    .and_then(|definition| definition.face_display_name(trigger.face_index))
                    .map(str::to_owned)
                    .unwrap_or_default();
                let may = trigger.ability.may;
                let ability_definition = trigger
                    .ability_origin
                    .as_ref()
                    .and_then(|origin| match origin {
                        TriggerAbilityOrigin::Printed(definition)
                        | TriggerAbilityOrigin::StaticGrant { definition, .. } => {
                            Some(definition.clone())
                        }
                        TriggerAbilityOrigin::ResolvingGrant(_) => None,
                    })
                    .unwrap_or_else(|| {
                        self.ability_definition(
                            trigger.source_id,
                            trigger.face_index,
                            vec![trigger.ability.ability_id.clone()],
                        )
                    });
                let presentation = trigger.presentation.or_else(|| {
                    Some(ability_presentation(
                        self.registry,
                        &ability_definition,
                        &trigger.ability.presentation,
                        trigger.ability_text.clone(),
                    ))
                });
                StagedTrigger {
                    object_id,
                    source_permanent_id: trigger.source_id,
                    source_owner: self
                        .state
                        .objects
                        .get(&trigger.source_id)
                        .map(|object| object.owner)
                        .unwrap_or(trigger.controller),
                    source_face_index: trigger.face_index,
                    source_zone_change: trigger.source_zone_change,
                    source_face_change: trigger.source_face_change,
                    card_id: trigger.card_id,
                    card_name,
                    controller: trigger.controller,
                    ability_index: trigger.ability_index,
                    ability: trigger.ability,
                    ability_text: trigger.ability_text,
                    presentation,
                    trigger_context: trigger.trigger_context,
                    may,
                }
            })
            .collect();
        self.state
            .staged_trigger_groups
            .push_back(StagedTriggerGroup { triggers });
    }

    /// CR 603.2c / 603.2d: determine how many extra instances a just-triggered ability creates.
    /// Each modifier sees the source permanent's event-time/LKI fact, while the modifier's own
    /// controller and condition remain live at the trigger-check boundary.
    fn additional_trigger_instances(&self, trigger: &CollectedTrigger) -> u32 {
        let Some(source_fact) = trigger.source_fact.as_ref() else {
            return 0;
        };
        self.state
            .players
            .iter()
            .flat_map(|player| player.battlefield.iter().copied())
            .flat_map(|modifier_id| {
                let controller = self.controller_of(modifier_id);
                self.active_static_ability_definitions(modifier_id)
                    .into_iter()
                    .filter_map(move |ability| {
                        controller.map(|controller| (modifier_id, controller, ability))
                    })
            })
            .filter_map(|(modifier_id, controller, ability)| {
                let StaticAbilityDef::AdditionalTriggeredAbilityInstances {
                    controllers,
                    source_filter,
                    condition,
                    additional_count,
                } = ability
                else {
                    return None;
                };
                let context = ConditionContext {
                    controller,
                    source_object_id: modifier_id,
                    source_zone_change: self
                        .state
                        .zone_change_generation
                        .get(&modifier_id)
                        .copied()
                        .unwrap_or(0),
                    resolving_spell_id: None,
                    stack_item: None,
                };
                (super::history::relative_player_set_contains(
                    &self.state,
                    controllers,
                    controller,
                    trigger.controller,
                ) && super::history::permanent_event_fact_matches(
                    &self.state,
                    &source_filter,
                    source_fact,
                    context,
                ) && condition
                    .as_ref()
                    .is_none_or(|condition| self.condition_holds(condition, context)))
                .then_some(additional_count)
            })
            .fold(0, u32::saturating_add)
    }

    /// Put staged triggers on the stack, stopping at the first player decision (CR 603.3b/603.3d).
    ///
    /// Idempotent and re-entrant-safe: it places what it can and returns, so every path that can
    /// *unblock* a decision calls it again rather than tracking where it left off.
    ///
    /// Cross-player order is never prompted — the active player's whole block is placed before any
    /// nonactive player's (CR 603.3b/101.4). Within a block, a player with two or more triggers
    /// chooses. Because [`Self::stage_triggers`] sorts by APNAP rank, one player's block is exactly
    /// the leading run of triggers sharing a controller, which is what makes the per-player prompts
    /// fall out as successive rounds with no extra bookkeeping.
    pub(super) fn flush_staged_triggers(&mut self, events: &mut Vec<rv1::RuledEvent>) {
        loop {
            // A parked resolution outranks: its spell is still resolving, and its triggers wait for
            // the next time a player would receive priority (CR 603.3). A parked *target* choice
            // (CR 603.3d) stops the drain because targets are chosen as each ability is put on the
            // stack — so the next trigger cannot be placed until this one is finished.
            match self.state.blocking_choice() {
                Some(BlockingChoice::Resolution) | Some(BlockingChoice::TriggerTarget) => return,
                // Not a stopping condition here: this is the block we are draining, and the
                // handling below is what either prompts for it or finishes it.
                Some(BlockingChoice::TriggerOrder) | None => {}
            }
            // An ordering block already in progress is drained before any new group is opened.
            if let Some(pending) = self.state.pending_trigger_order.as_mut() {
                match pending.candidates.len() {
                    // Still a real choice: ask (once per candidate set) and wait.
                    2.. => {
                        if !pending.prompt_emitted {
                            pending.prompt_emitted = true;
                            let controller = pending.deciding_player;
                            let candidates = pending.candidates.clone();
                            events.push(ev_trigger_order_required(controller, &candidates));
                            events.push(ev_log(format!(
                                "Simultaneous triggers: P{controller} chooses which of {} goes on the stack next",
                                candidates.len()
                            )));
                        }
                        return;
                    }
                    // Exactly one left, so there is nothing to choose — place it rather than
                    // making the player confirm a foregone conclusion.
                    1 => {
                        let staged = pending.candidates.remove(0);
                        self.state.pending_trigger_order = None;
                        self.push_trigger(staged, events);
                        continue;
                    }
                    _ => {
                        self.state.pending_trigger_order = None;
                        continue;
                    }
                }
            }
            let Some(group) = self.state.staged_trigger_groups.front_mut() else {
                return;
            };
            if group.triggers.is_empty() {
                self.state.staged_trigger_groups.pop_front();
                continue;
            }
            let controller = group.triggers[0].controller;
            let run = group
                .triggers
                .iter()
                .take_while(|trigger| trigger.controller == controller)
                .count();
            if run >= 2 {
                // The block leaves the group for good: it is now drained one pick at a time out of
                // `pending_trigger_order`, and what remains in the group is the other players'
                // blocks, each of which raises its own prompt when its turn comes.
                let candidates: Vec<StagedTrigger> = group.triggers.drain(..run).collect();
                self.state.pending_trigger_order = Some(PendingTriggerOrder {
                    deciding_player: controller,
                    candidates,
                    prompt_emitted: false,
                });
                continue;
            }
            let staged = group.triggers.remove(0);
            self.push_trigger(staged, events);
        }
    }

    /// Every permanent on every battlefield as a trigger-source snapshot, in APNAP order
    /// (CR 603.3b / 101.4): the active player's permanents first, then each nonactive player's.
    /// Triggers collected in this order go on the stack in the order the rules require.
    ///
    /// The single source of this ordering. Five event arms need it, and the one that rolled its
    /// own — the former upkeep-specific arm — scanned only the active player's battlefield, so a
    /// nonactive player's upkeep trigger never fired at all (CR 603.2: *every* permanent observes
    /// the event). `sort_by_key` is stable, so each player's battlefield keeps its own order.
    pub(super) fn battlefield_sources_apnap(&self) -> Vec<TriggerSourceSnapshot> {
        let mut ordered: Vec<usize> = (0..self.state.players.len()).collect();
        ordered.sort_by_key(|&i| self.state.apnap_rank(self.state.players[i].id));
        let mut sources = Vec::new();
        for pi in ordered {
            for &source_id in &self.state.players[pi].battlefield {
                if let Some(source) = self.trigger_source_snapshot(source_id) {
                    sources.push(source);
                }
            }
        }
        sources
    }

    /// Collect every triggered ability whose condition matches `event`, ordered APNAP (CR 603.3b).
    pub(super) fn collect_triggers(
        &self,
        event: &GameEvent,
        sources: &[TriggerSourceSnapshot],
    ) -> Vec<CollectedTrigger> {
        match event {
            GameEvent::ZoneChanges(batch) => self.collect_zone_triggers(batch),
            // The completed operation is available for Blight observers; this delivery adds
            // payment consumers, not a new authored observer condition.
            GameEvent::Blighted(_) | GameEvent::Waterbent { .. } => vec![],
            GameEvent::EntersBattlefield { object_id, .. } => {
                let Some(obj) = self.state.objects.get(object_id) else {
                    return vec![];
                };
                let entering_id = *object_id;
                let (entering_card_id, entering_face_index) = self
                    .effective_card_identity(entering_id)
                    .map(|(card_id, face_index)| (card_id.to_string(), face_index))
                    .unwrap_or_else(|| (obj.card_id.clone(), obj.face_up_index));
                let Some(entering_characteristics) = self.characteristics(entering_id) else {
                    return vec![];
                };
                let entering_controller = entering_characteristics.controller;

                let mut out = Vec::new();
                out.extend(self.matching_triggered_abilities(
                    &entering_card_id,
                    entering_id,
                    entering_controller,
                    entering_face_index,
                    |tc| *tc == TriggerCondition::WhenSelfEntersBattlefield,
                ));

                for source in sources {
                    let src_ctrl = source.controller;
                    out.extend(self.matching_snapshot_abilities(source, |tc| {
                        let TriggerCondition::WheneverPermanentEntersBattlefield {
                            controller,
                            filter,
                            creature_filter,
                        } = tc
                        else {
                            return false;
                        };
                        let rel_ok = self.relative_player_matches(
                            *controller,
                            entering_controller,
                            src_ctrl,
                        );
                        if !rel_ok {
                            return false;
                        }
                        let fact = self.event_object_fact(entering_id).expect("entrant exists");
                        self.event_filter_matches(filter, &fact, source)
                            && creature_filter.as_ref().is_none_or(|filter| {
                                Self::creature_event_filter_matches_characteristics(
                                    &entering_characteristics,
                                    filter,
                                )
                            })
                    }));
                }
                out
            }
            GameEvent::LibrarySearched {
                searcher,
                library_owner,
            } => {
                if searcher != library_owner {
                    return vec![];
                }
                sources
                    .iter()
                    .flat_map(|source| {
                        self.matching_snapshot_abilities(source, |condition| {
                            let TriggerCondition::WheneverPlayerSearchesOwnLibrary { player } =
                                condition
                            else {
                                return false;
                            };
                            self.relative_player_matches(*player, *searcher, source.controller)
                        })
                    })
                    .collect()
            }
            GameEvent::CountersPlaced {
                object,
                kind: CounterKind::Lore,
                before,
                after,
                read_ahead_entry,
            } => {
                if self
                    .state
                    .zone_change_generation
                    .get(&object.object_id)
                    .copied()
                    .unwrap_or(0)
                    != object.zone_change_generation
                {
                    return vec![];
                }
                let Some(characteristics) = self.characteristics(object.object_id) else {
                    return vec![];
                };
                if !characteristics.has_type("Enchantment") || !characteristics.has_type("Saga") {
                    return vec![];
                }
                let Some((card_id, face_index)) = self.effective_card_identity(object.object_id)
                else {
                    return vec![];
                };
                let triggers = self.matching_triggered_abilities(
                    card_id,
                    object.object_id,
                    object.controller_at_event,
                    face_index,
                    |condition| matches!(condition, TriggerCondition::SagaChapter { .. }),
                );
                let mut out = Vec::new();
                for trigger in triggers {
                    let TriggerCondition::SagaChapter { chapters } = &trigger.ability.trigger
                    else {
                        continue;
                    };
                    let occurrences = chapters
                        .iter()
                        .filter(|chapter| {
                            if *read_ahead_entry {
                                **chapter == *after
                            } else {
                                *before < **chapter && **chapter <= *after
                            }
                        })
                        .count();
                    out.extend(std::iter::repeat_n(trigger, occurrences));
                }
                out
            }
            GameEvent::CountersPlaced { .. } => vec![],
            GameEvent::RoomDoorUnlocked {
                object_id,
                face_index,
                player,
                fully_unlocked,
            } => {
                let mut out = Vec::new();
                if let Some(source) = sources.iter().find(|source| source.object_id == *object_id) {
                    if let Some(face) = self
                        .room_faces(*object_id)
                        .and_then(|faces| faces.get(*face_index))
                    {
                        let prior_abilities = self
                            .room_faces(*object_id)
                            .into_iter()
                            .flatten()
                            .take(*face_index)
                            .map(|door| door.triggered_abilities.len())
                            .sum::<usize>();
                        out.extend(
                            face.triggered_abilities
                                .iter()
                                .enumerate()
                                .filter(|(_, ability)| {
                                    ability.trigger == TriggerCondition::WhenThisDoorUnlocked
                                })
                                .filter(|(_, ability)| {
                                    self.intervening_if_holds(
                                        source.object_id,
                                        source.controller,
                                        ability.intervening_if.as_ref(),
                                    )
                                })
                                .map(|(ability_index, ability)| CollectedTrigger {
                                    source_id: source.object_id,
                                    card_id: source.card_id.clone(),
                                    face_index: *face_index,
                                    source_zone_change: source.zone_change_generation,
                                    source_face_change: source.face_change_generation,
                                    source_fact: Some(source.event_fact()),
                                    additional_instances: 0,
                                    controller: source.controller,
                                    ability_index: prior_abilities + ability_index,
                                    ability_origin: Some(TriggerAbilityOrigin::Printed(
                                        self.ability_definition(
                                            source.object_id,
                                            *face_index,
                                            vec![ability.ability_id.clone()],
                                        ),
                                    )),
                                    presentation: None,
                                    ability: ability.clone(),
                                    ability_text: ability.fallback_text(&face.name),
                                    trigger_context: TriggerContext::default(),
                                }),
                        );
                    }
                }
                if *fully_unlocked {
                    for source in sources {
                        out.extend(self.matching_snapshot_abilities(source, |condition| {
                            let TriggerCondition::WheneverPlayerFullyUnlocksRoom { player: who } =
                                condition
                            else {
                                return false;
                            };
                            self.relative_player_matches(*who, *player, source.controller)
                        }));
                    }
                }
                out
            }
            GameEvent::BecameTapped {
                object,
                is_creature,
                action,
            } => {
                let mut out = Vec::new();
                for source in sources {
                    if *is_creature {
                        let mut matching = self.matching_snapshot_abilities(source, |condition| {
                            let TriggerCondition::WheneverPlayerTapsCreature {
                                player,
                                controllers,
                                ..
                            } = condition
                            else {
                                return false;
                            };
                            self.relative_player_matches(*player, action.actor, source.controller)
                                && super::history::relative_player_set_contains(
                                    &self.state,
                                    *controllers,
                                    source.controller,
                                    object.controller_at_event,
                                )
                        });
                        for trigger in &mut matching {
                            if matches!(
                                trigger.ability.trigger,
                                TriggerCondition::WheneverPlayerTapsCreature {
                                    cardinality: TapTriggerCardinality::EachObject,
                                    ..
                                }
                            ) {
                                trigger.trigger_context.observed_object = Some(*object);
                            }
                        }
                        out.extend(matching);
                    }
                    if source.object_id == object.object_id
                        && source.zone_change_generation == object.zone_change_generation
                    {
                        out.extend(self.matching_snapshot_abilities(source, |condition| {
                            *condition == TriggerCondition::WheneverSelfBecomesTapped
                                || matches!(
                                    condition,
                                    TriggerCondition::WheneverSelfTappedForCastCost { kind }
                                        if action.cast_cost_kind == Some(*kind)
                                )
                        }));
                    }
                    if source.attached_to
                        == Some(AttachmentSnapshot::Object(
                            object.object_id,
                            object.zone_change_generation,
                        ))
                    {
                        let mut matching = self.matching_snapshot_abilities(source, |condition| {
                            *condition == TriggerCondition::WheneverAttachedObjectBecomesTapped
                        });
                        for trigger in &mut matching {
                            trigger.trigger_context.observed_object = Some(*object);
                        }
                        out.extend(matching);
                    }
                }
                out
            }
            GameEvent::LeavesBattlefield { source } => {
                let source = source.at_event(sources);
                self.matching_snapshot_abilities(source, |condition| {
                    *condition == TriggerCondition::WhenSelfLeavesBattlefield
                })
            }
            GameEvent::Sacrificed {
                source: sacrificed,
                player,
            } => {
                let sacrificed = sacrificed.at_event(sources);
                let observed = TriggerObjectRef {
                    object_id: sacrificed.object_id,
                    zone_change_generation: sacrificed.zone_change_generation,
                    controller_at_event: sacrificed.controller,
                };
                let mut out = Vec::new();
                for source in sources {
                    let mut matching = self.matching_snapshot_abilities(source, |condition| {
                        let TriggerCondition::WheneverPlayerSacrificesPermanent {
                            player: who,
                            filter,
                        } = condition
                        else {
                            return false;
                        };
                        self.event_filter_matches(filter, &sacrificed.event_fact(), source)
                            && self.relative_player_matches(*who, *player, source.controller)
                    });
                    for trigger in &mut matching {
                        trigger.trigger_context.observed_object = Some(observed);
                    }
                    out.extend(matching);
                }
                out
            }
            GameEvent::Dies {
                source: dying,
                was_creature,
            } => {
                let dying = dying.at_event(sources);
                let mut out = self
                    .matching_snapshot_abilities(dying, |tc| *tc == TriggerCondition::WhenSelfDies);
                // The committed battlefield-to-graveyard move advances exactly one generation.
                // Derive from the event snapshot, never the possibly newer current object.
                for trigger in &mut out {
                    trigger.trigger_context.source_after_zone_change = Some(TriggerObjectRef {
                        object_id: dying.object_id,
                        zone_change_generation: dying.zone_change_generation + 1,
                        controller_at_event: dying.controller,
                    });
                }
                if !was_creature {
                    return out;
                }
                let dying_ref = TriggerObjectRef {
                    object_id: dying.object_id,
                    zone_change_generation: dying.zone_change_generation,
                    controller_at_event: dying.controller,
                };
                for source in sources {
                    if source.attached_to
                        == Some(AttachmentSnapshot::Object(
                            dying_ref.object_id,
                            dying_ref.zone_change_generation,
                        ))
                    {
                        let mut matching = self.matching_snapshot_abilities(source, |condition| {
                            *condition == TriggerCondition::WheneverAttachedObjectDies
                        });
                        for trigger in &mut matching {
                            trigger.trigger_context.observed_object = Some(dying_ref);
                        }
                        out.extend(matching);
                    }
                    out.extend(self.matching_snapshot_abilities(source, |tc| {
                        let TriggerCondition::WheneverCreatureDies { controller, filter } = tc
                        else {
                            return false;
                        };
                        if !self.event_filter_matches(filter, &dying.event_fact(), source) {
                            return false;
                        }
                        self.relative_player_matches(
                            *controller,
                            dying.controller,
                            source.controller,
                        )
                    }));
                }
                out
            }
            GameEvent::AttackersDeclared {
                attacking_player,
                attacks,
            } => {
                let other_attacker_count =
                    u32::try_from(attacks.len().saturating_sub(1)).unwrap_or(u32::MAX);
                let mut out = Vec::new();
                for attack in attacks {
                    let attacker_id = attack.attacker.object_id;
                    let Some(obj) = self.state.objects.get(&attacker_id) else {
                        continue;
                    };
                    if obj.controller != *attacking_player {
                        continue;
                    }
                    let (card_id, face_index) = self
                        .effective_card_identity(attacker_id)
                        .map(|(card_id, face_index)| (card_id.to_string(), face_index))
                        .unwrap_or_else(|| (obj.card_id.clone(), obj.face_up_index));
                    let mut matching = self.matching_triggered_abilities(
                        &card_id,
                        attacker_id,
                        obj.controller,
                        face_index,
                        |tc| {
                            let TriggerCondition::WheneverSelfAttacks {
                                minimum_other_attackers,
                            } = tc
                            else {
                                return false;
                            };
                            other_attacker_count >= *minimum_other_attackers
                        },
                    );
                    for trigger in &mut matching {
                        trigger.trigger_context.attacking_player = Some(*attacking_player);
                        trigger.trigger_context.defending_player = Some(attack.defending_player);
                        match attack.defender {
                            CombatDefenderTarget::Player(player) => {
                                trigger.trigger_context.attacked_player = Some(player);
                            }
                            CombatDefenderTarget::Permanent(permanent)
                                if self.characteristics(permanent.object_id).is_some_and(
                                    |characteristics| characteristics.has_type("Planeswalker"),
                                ) =>
                            {
                                trigger.trigger_context.attacked_planeswalker = Some(permanent);
                            }
                            CombatDefenderTarget::Permanent(_) => {}
                        }
                    }
                    out.extend(matching);

                    for source in sources {
                        if source.attached_to
                            != Some(AttachmentSnapshot::Object(
                                attack.attacker.object_id,
                                attack.attacker.zone_change_generation,
                            ))
                        {
                            continue;
                        }
                        let mut matching = self.matching_snapshot_abilities(source, |condition| {
                            *condition == TriggerCondition::WheneverAttachedObjectAttacks
                        });
                        for trigger in &mut matching {
                            trigger.trigger_context.observed_object = Some(attack.attacker);
                            trigger.trigger_context.attacking_player = Some(*attacking_player);
                            trigger.trigger_context.defending_player =
                                Some(attack.defending_player);
                        }
                        out.extend(matching);
                    }
                }

                let attacker_count = u32::try_from(attacks.len()).unwrap_or(u32::MAX);
                for source in sources {
                    if source.controller != *attacking_player {
                        continue;
                    }
                    let mut matching = self.matching_snapshot_abilities(source, |condition| {
                        let TriggerCondition::WheneverControllerAttacks {
                            min_attackers,
                            max_attackers,
                        } = condition
                        else {
                            return false;
                        };
                        min_attackers.is_none_or(|minimum| attacker_count >= minimum)
                            && max_attackers.is_none_or(|maximum| attacker_count <= maximum)
                    });
                    if let [attack] = attacks.as_slice() {
                        for trigger in &mut matching {
                            trigger.trigger_context.observed_object = Some(attack.attacker);
                        }
                    }
                    out.extend(matching);
                }

                for source in sources {
                    let Some(AttachmentSnapshot::Player(attached_player)) = source.attached_to
                    else {
                        continue;
                    };
                    let Some(attack) = attacks
                        .iter()
                        .find(|attack| attack.defending_player == attached_player)
                    else {
                        continue;
                    };
                    let mut matching = self.matching_snapshot_abilities(source, |condition| {
                        *condition == TriggerCondition::WheneverAttachedPlayerIsAttacked
                    });
                    for trigger in &mut matching {
                        trigger.trigger_context.attacking_player = Some(*attacking_player);
                        trigger.trigger_context.defending_player = Some(attack.defending_player);
                    }
                    out.extend(matching);
                }
                out
            }
            GameEvent::BlockersDeclared { edges } => {
                let mut out = Vec::new();
                for source in sources {
                    for edge in edges {
                        let (related, source_is_blocker) =
                            if source.object_id == edge.blocker.object_id {
                                (edge.attacker, true)
                            } else if source.object_id == edge.attacker.object_id {
                                (edge.blocker, false)
                            } else {
                                continue;
                            };
                        let mut matching =
                            self.matching_snapshot_abilities(source, |condition| match condition {
                                TriggerCondition::WheneverSelfBlocksCreature { attacker }
                                    if source_is_blocker =>
                                {
                                    self.creature_event_filter_matches(
                                        edge.attacker.object_id,
                                        attacker,
                                    )
                                }
                                TriggerCondition::WheneverSelfBecomesBlockedByCreature {
                                    blocker,
                                } if !source_is_blocker => self
                                    .creature_event_filter_matches(edge.blocker.object_id, blocker),
                                _ => false,
                            });
                        for trigger in &mut matching {
                            trigger.trigger_context.observed_object = Some(related);
                        }
                        out.extend(matching);
                    }
                }
                out
            }
            GameEvent::DamageDealt { event } => {
                let source_id = event.source.object_id;
                let source_generation = self
                    .state
                    .zone_change_generation
                    .get(&source_id)
                    .copied()
                    .unwrap_or(0);
                let source_ref = TriggerObjectRef {
                    object_id: source_id,
                    zone_change_generation: source_generation,
                    controller_at_event: event.source.controller,
                };
                let mut out = Vec::new();

                if let DamageRecipient::Player(defender_id) = event.recipient {
                    if let Some(obj) = self.state.objects.get(&source_id) {
                        let (card_id, face_index) = self
                            .effective_card_identity(source_id)
                            .map(|(card_id, face_index)| (card_id.to_string(), face_index))
                            .unwrap_or_else(|| (obj.card_id.clone(), obj.face_up_index));
                        let controller = event.source.controller;
                        out.extend(self.matching_triggered_abilities(
                            &card_id,
                            source_id,
                            controller,
                            face_index,
                            |tc| match tc {
                                TriggerCondition::WheneverSelfDealsCombatDamageToPlayer => {
                                    event.classification == DamageClassification::Combat
                                }
                                TriggerCondition::WheneverSelfDealsDamageToOpponent => {
                                    self.state.are_opponents(defender_id, controller)
                                }
                                _ => false,
                            },
                        ));
                    }

                    if event.classification == DamageClassification::Combat {
                        for source in sources {
                            if source.attached_to
                                != Some(AttachmentSnapshot::Object(source_id, source_generation))
                            {
                                continue;
                            }
                            let mut matching = self.matching_snapshot_abilities(source, |condition| {
                                *condition
                                    == TriggerCondition::WheneverAttachedObjectDealsCombatDamageToPlayer
                            });
                            for trigger in &mut matching {
                                trigger.trigger_context.observed_object = Some(source_ref);
                            }
                            out.extend(matching);
                        }
                    }
                }

                if let DamageRecipient::Permanent(recipient_id) = event.recipient {
                    let recipient_generation = self
                        .state
                        .zone_change_generation
                        .get(&recipient_id)
                        .copied()
                        .unwrap_or(0);
                    let recipient_controller = self
                        .characteristics(recipient_id)
                        .map(|characteristics| characteristics.controller)
                        .or_else(|| {
                            self.state
                                .objects
                                .get(&recipient_id)
                                .map(|object| object.controller)
                        })
                        .unwrap_or(0);
                    let recipient_ref = TriggerObjectRef {
                        object_id: recipient_id,
                        zone_change_generation: recipient_generation,
                        controller_at_event: recipient_controller,
                    };
                    for source in sources {
                        if source.attached_to
                            != Some(AttachmentSnapshot::Object(
                                recipient_id,
                                recipient_generation,
                            ))
                        {
                            continue;
                        }
                        let mut matching = self.matching_snapshot_abilities(source, |condition| {
                            *condition == TriggerCondition::WheneverAttachedObjectIsDealtDamage
                        });
                        for trigger in &mut matching {
                            trigger.trigger_context.observed_object = Some(recipient_ref);
                        }
                        out.extend(matching);
                    }
                }

                out
            }
            GameEvent::PhaseBegan {
                phase,
                active_player,
            } => sources
                .iter()
                .flat_map(|source| {
                    self.matching_snapshot_abilities(source, |condition| {
                        let player_filter = match (phase, condition) {
                            (
                                rv1::PhaseId::Upkeep,
                                TriggerCondition::AtBeginningOfUpkeep { player },
                            )
                            | (
                                rv1::PhaseId::Draw,
                                TriggerCondition::AtBeginningOfDrawStep { player },
                            )
                            | (
                                rv1::PhaseId::EndStep,
                                TriggerCondition::AtBeginningOfEndStep { player },
                            )
                            | (
                                rv1::PhaseId::BeginCombat,
                                TriggerCondition::AtBeginningOfCombat { player },
                            )
                            | (
                                rv1::PhaseId::Main2,
                                TriggerCondition::AtBeginningOfSecondMainPhase { player },
                            ) => player,
                            _ => return false,
                        };
                        self.relative_player_matches(
                            *player_filter,
                            *active_player,
                            source.controller,
                        )
                    })
                })
                .collect(),
            GameEvent::CardDrawn { drawer, ordinal } => sources
                .iter()
                .flat_map(|source| {
                    self.matching_snapshot_abilities(source, |condition| {
                        let TriggerCondition::WheneverPlayerDrawsNthCard {
                            drawer: drawer_filter,
                            ordinal: trigger_ordinal,
                        } = condition
                        else {
                            return false;
                        };
                        *trigger_ordinal == *ordinal
                            && self.relative_player_matches(
                                *drawer_filter,
                                *drawer,
                                source.controller,
                            )
                    })
                })
                .collect(),
            GameEvent::LifeGained { player: gaining } => {
                // Every player's permanents watch, in APNAP order (CR 603.3b) — the amount is
                // irrelevant, one gain event fires each matching ability once.
                sources
                    .iter()
                    .flat_map(|source| {
                        self.matching_snapshot_abilities(source, |tc| {
                            let TriggerCondition::WheneverPlayerGainsLife {
                                player: player_filter,
                            } = tc
                            else {
                                return false;
                            };
                            self.relative_player_matches(
                                *player_filter,
                                *gaining,
                                source.controller,
                            )
                        })
                    })
                    .collect()
            }
            GameEvent::Surveilled {
                player: surveilling,
            } => sources
                .iter()
                .flat_map(|source| {
                    self.matching_snapshot_abilities(source, |condition| {
                        let TriggerCondition::WheneverPlayerSurveils {
                            player: player_filter,
                        } = condition
                        else {
                            return false;
                        };
                        self.relative_player_matches(
                            *player_filter,
                            *surveilling,
                            source.controller,
                        )
                    })
                })
                .collect(),
            GameEvent::TargetsChosen {
                controller: targeting_controller,
                source: targeting_source,
                stack_object,
                targets,
            } => {
                // CR 603.2/115.9: becoming a target is observed after the complete legal target
                // set is chosen. A single object named multiple times by that object produces one
                // trigger per matching ability, while distinct watched permanents or spells each
                // produce their own trigger and retain their generation-bound event identity.
                let mut seen = HashSet::new();
                let distinct_targets: Vec<StackTarget> = targets
                    .iter()
                    .copied()
                    .filter(|target| seen.insert((target.object_id, target.zone_change_generation)))
                    .collect();
                let mut out = Vec::new();
                for source in sources {
                    for target in &distinct_targets {
                        let target_kind = rv1::TargetRefKind::try_from(target.kind)
                            .unwrap_or(rv1::TargetRefKind::Unspecified);
                        if matches!(
                            target_kind,
                            rv1::TargetRefKind::Unspecified | rv1::TargetRefKind::Permanent
                        ) && targeting::stack_target_identity_is_current(self, target)
                            && self
                                .state
                                .objects
                                .get(&target.object_id)
                                .is_some_and(|object| object.zone == Zone::Battlefield)
                        {
                            let Some(target_characteristics) =
                                self.characteristics(target.object_id)
                            else {
                                continue;
                            };
                            let target_controller = target_characteristics.controller;
                            let mut matching =
                                self.matching_snapshot_abilities(source, |condition| {
                                    let Some(permanent_type) = self
                                        .target_trigger_permanent_filter(
                                            condition,
                                            *targeting_source,
                                            *targeting_controller,
                                            source.object_id,
                                            source.controller,
                                            target.object_id,
                                            target_controller,
                                        )
                                    else {
                                        return false;
                                    };
                                    match permanent_type {
                                        Some(PermanentTypeFilter::Creature) => {
                                            target_characteristics.is_creature()
                                        }
                                        Some(PermanentTypeFilter::Artifact) => {
                                            target_characteristics.is_artifact()
                                        }
                                        Some(PermanentTypeFilter::Enchantment) => {
                                            target_characteristics.has_type("Enchantment")
                                        }
                                        Some(PermanentTypeFilter::Land) => {
                                            target_characteristics.has_type("Land")
                                        }
                                        Some(PermanentTypeFilter::Planeswalker) => {
                                            target_characteristics.has_type("Planeswalker")
                                        }
                                        Some(PermanentTypeFilter::Battle) => {
                                            target_characteristics.has_type("Battle")
                                        }
                                        None => true,
                                    }
                                });
                            for trigger in &mut matching {
                                trigger.trigger_context.observed_object =
                                    self.trigger_object_ref(target.object_id);
                                trigger.trigger_context.targeting_stack_object =
                                    Some(*stack_object);
                            }
                            out.extend(matching);
                        }

                        if !matches!(
                            target_kind,
                            rv1::TargetRefKind::Unspecified | rv1::TargetRefKind::Stack
                        ) || !targeting::stack_target_identity_is_current(self, target)
                        {
                            continue;
                        }
                        let Some(target_stack_item) = self.state.stack.iter().find(|item| {
                            item.id == target.object_id && item.ability_text.is_none()
                        }) else {
                            continue;
                        };
                        let target_controller = target_stack_item.controller;
                        let mut matching = self.matching_snapshot_abilities(source, |condition| {
                            let Some(spell_filter) = self.target_trigger_spell_filter(
                                condition,
                                *targeting_source,
                                *targeting_controller,
                                source.controller,
                                target_controller,
                            ) else {
                                return false;
                            };
                            targeting::stack_spell_target_legal(
                                &self.state,
                                self.registry,
                                target.object_id,
                                spell_filter,
                            )
                        });
                        for trigger in &mut matching {
                            trigger.trigger_context.observed_stack_object =
                                Some(TriggerStackObjectRef {
                                    stack_object: StackObjectRef {
                                        object_id: target.object_id,
                                        zone_change_generation: target.zone_change_generation,
                                    },
                                    controller_at_event: target_controller,
                                });
                            trigger.trigger_context.targeting_stack_object = Some(*stack_object);
                        }
                        out.extend(matching);
                    }
                }
                out
            }
            GameEvent::ManaSpentCastingSpell {
                player,
                before,
                after,
            } => sources
                .iter()
                .flat_map(|source| {
                    self.matching_snapshot_abilities(source, |condition| {
                        let TriggerCondition::WheneverPlayerExpendsMana {
                            player: who,
                            amount,
                        } = condition
                        else {
                            return false;
                        };
                        self.relative_player_matches(*who, *player, source.controller)
                            && *before < u64::from(*amount)
                            && u64::from(*amount) <= *after
                    })
                })
                .collect(),
            GameEvent::CrimeCommitted { player } => sources
                .iter()
                .flat_map(|source| {
                    self.matching_snapshot_abilities(source, |condition| {
                    matches!(condition, TriggerCondition::WheneverPlayerCommitsCrime { player: who }
                        if self.relative_player_matches(*who, *player, source.controller))
                })
                })
                .collect(),
            GameEvent::SpellCast { fact } => sources
                .iter()
                .flat_map(|source| {
                    self.matching_snapshot_abilities(source, |tc| {
                        let TriggerCondition::WheneverPlayerCastsSpell {
                            caster: caster_filter,
                            filter,
                            ordinal,
                            ordinal_scope,
                        } = tc
                        else {
                            return false;
                        };
                        if !self.relative_player_matches(
                            *caster_filter,
                            fact.caster,
                            source.controller,
                        ) || !super::history::spell_cast_matches(filter, fact)
                        {
                            return false;
                        }
                        ordinal.is_none_or(|expected| {
                            let actual = match ordinal_scope {
                                CastOrdinalScope::AllSpells => fact.ordinal,
                                CastOrdinalScope::MatchingFilter => {
                                    self.state
                                        .turn_history
                                        .current
                                        .spell_casts
                                        .iter()
                                        .filter(|prior| {
                                            prior.caster == fact.caster
                                                && prior.ordinal <= fact.ordinal
                                                && super::history::spell_cast_matches(filter, prior)
                                        })
                                        .count() as u32
                                }
                            };
                            expected == actual
                        })
                    })
                })
                .collect(),
        }
    }

    fn targeting_source_matches(
        filter: TargetingSourceFilter,
        source: TargetingSourceKind,
    ) -> bool {
        match filter {
            TargetingSourceFilter::SpellCast => source == TargetingSourceKind::SpellCast,
            TargetingSourceFilter::Spell => matches!(
                source,
                TargetingSourceKind::SpellCast | TargetingSourceKind::SpellCopy
            ),
            TargetingSourceFilter::Ability => source == TargetingSourceKind::Ability,
            TargetingSourceFilter::SpellOrAbility => true,
        }
    }

    /// Return the watched permanent-type filter when a target-trigger condition matches all
    /// source/controller/identity predicates. `Some(None)` is a match with no type restriction;
    /// `None` means the condition does not match this targeting event.
    #[allow(clippy::too_many_arguments)]
    fn target_trigger_permanent_filter(
        &self,
        condition: &TriggerCondition,
        targeting_source: TargetingSourceKind,
        targeting_controller: PlayerId,
        source_id: ObjectId,
        source_controller: PlayerId,
        target_id: ObjectId,
        target_controller: PlayerId,
    ) -> Option<Option<PermanentTypeFilter>> {
        match condition {
            TriggerCondition::WheneverSelfBecomesTarget {
                source,
                source_controller: source_controller_filter,
            } => (source_id == target_id
                && Self::targeting_source_matches(*source, targeting_source)
                && self.relative_player_matches(
                    *source_controller_filter,
                    targeting_controller,
                    source_controller,
                ))
            .then_some(None),
            TriggerCondition::WheneverPermanentBecomesTarget {
                source,
                source_controller: source_controller_filter,
                target_controller: target_controller_filter,
                permanent_type,
                exclude_self,
            } => ((!*exclude_self || source_id != target_id)
                && Self::targeting_source_matches(*source, targeting_source)
                && self.relative_player_matches(
                    *source_controller_filter,
                    targeting_controller,
                    source_controller,
                )
                && self.relative_player_matches(
                    *target_controller_filter,
                    target_controller,
                    source_controller,
                ))
            .then_some(*permanent_type),
            _ => None,
        }
    }

    fn target_trigger_spell_filter<'a>(
        &self,
        condition: &'a TriggerCondition,
        targeting_source: TargetingSourceKind,
        targeting_controller: PlayerId,
        source_controller: PlayerId,
        target_controller: PlayerId,
    ) -> Option<&'a StackSpellFilter> {
        let TriggerCondition::WheneverSpellBecomesTarget {
            source,
            source_controller: source_controller_filter,
            target_controller: target_controller_filter,
            spell_filter,
        } = condition
        else {
            return None;
        };
        (Self::targeting_source_matches(*source, targeting_source)
            && self.relative_player_matches(
                *source_controller_filter,
                targeting_controller,
                source_controller,
            )
            && self.relative_player_matches(
                *target_controller_filter,
                target_controller,
                source_controller,
            ))
        .then_some(spell_filter)
    }

    pub(super) fn relative_player_matches(
        &self,
        filter: CastTriggerPlayer,
        player: PlayerId,
        source_controller: PlayerId,
    ) -> bool {
        match filter {
            CastTriggerPlayer::Controller => player == source_controller,
            CastTriggerPlayer::Opponent => self.state.are_opponents(player, source_controller),
            CastTriggerPlayer::AnyPlayer => true,
        }
    }

    pub(super) fn matching_triggered_abilities(
        &self,
        card_id: &str,
        source_id: ObjectId,
        controller: PlayerId,
        face_index: usize,
        filter: impl Fn(&TriggerCondition) -> bool,
    ) -> Vec<CollectedTrigger> {
        let abilities = self.effective_triggered_abilities(source_id, card_id, face_index);
        abilities
            .into_iter()
            .filter(|(_, ta, _)| filter(&ta.trigger))
            // CR 603.4, first of the two checks: an intervening-"if" clause that is false as the
            // ability would go on the stack means it never triggers at all.
            .filter(|(_, ta, _)| {
                self.intervening_if_holds(source_id, controller, ta.intervening_if.as_ref())
            })
            .map(|(idx, ta, origin)| CollectedTrigger {
                source_id,
                card_id: card_id.to_string(),
                face_index,
                source_zone_change: {
                    let current = self
                        .state
                        .zone_change_generation
                        .get(&source_id)
                        .copied()
                        .unwrap_or(0);
                    if self
                        .state
                        .objects
                        .get(&source_id)
                        .is_some_and(|object| object.zone != Zone::Battlefield)
                    {
                        current.saturating_sub(1)
                    } else {
                        current
                    }
                },
                source_face_change: self
                    .state
                    .face_change_generation
                    .get(&source_id)
                    .copied()
                    .unwrap_or(0),
                source_fact: self.event_object_fact(source_id),
                additional_instances: 0,
                controller,
                ability_index: idx,
                ability_origin: Some(origin.clone()),
                presentation: None,
                ability: ta.clone(),
                ability_text: ta.fallback_text_with_path(
                    self.registry
                        .get(card_id)
                        .and_then(|definition| definition.faces.get(face_index))
                        .map(|face| face.name.as_str())
                        .unwrap_or(card_id),
                    &trigger_ability_path(&origin, &ta),
                ),
                trigger_context: TriggerContext::default(),
            })
            .collect()
    }

    pub(super) fn trigger_source_snapshot(
        &self,
        source_id: ObjectId,
    ) -> Option<TriggerSourceSnapshot> {
        let object = self.state.objects.get(&source_id)?;
        let (card_id, face_index) = self
            .effective_card_identity(source_id)
            .map(|(card_id, face_index)| (card_id.to_string(), face_index))
            .unwrap_or_else(|| (object.card_id.clone(), object.face_up_index));
        let characteristics = self.characteristics(source_id);
        let controller = characteristics
            .as_ref()
            .map(|characteristics| characteristics.controller)
            .unwrap_or(object.controller);
        let attached_to = object.attached_to.map(|recipient| match recipient {
            AttachmentRecipient::Object(object_id) => {
                let current_generation = self
                    .state
                    .zone_change_generation
                    .get(&object_id)
                    .copied()
                    .unwrap_or(0);
                let attached_generation = self
                    .state
                    .objects
                    .get(&object_id)
                    .filter(|target| target.zone == Zone::Battlefield)
                    .map(|_| current_generation)
                    .unwrap_or_else(|| current_generation.saturating_sub(1));
                AttachmentSnapshot::Object(object_id, attached_generation)
            }
            AttachmentRecipient::Player(player_id) => AttachmentSnapshot::Player(player_id),
        });
        Some(TriggerSourceSnapshot {
            counters: object.counters.clone(),
            owner: object.owner,
            is_token: object.is_token(),
            all_creature_types: characteristics
                .as_ref()
                .is_some_and(|c| c.all_creature_types),
            types: characteristics
                .as_ref()
                .map(|c| c.types.clone())
                .unwrap_or_default(),
            power_toughness: characteristics
                .as_ref()
                .map(|c| (c.signed_power, c.signed_toughness))
                .unwrap_or_default(),
            event_conditions_checked: false,
            object_id: source_id,
            triggered_abilities: self
                .effective_triggered_abilities(source_id, &card_id, face_index),
            card_id,
            face_name: self
                .effective_face(source_id)
                .map(|face| face.name.clone())
                .unwrap_or_else(|| object.card_id.clone()),
            controller,
            face_index,
            zone_change_generation: self
                .state
                .zone_change_generation
                .get(&source_id)
                .copied()
                .unwrap_or(0),
            face_change_generation: self
                .state
                .face_change_generation
                .get(&source_id)
                .copied()
                .unwrap_or(0),
            attached_to,
        })
    }

    /// Definition provenance follows copiable values, not the physical card's registry ID.
    /// Room callers supply the original door slot rather than a flattened unlocked-face index.
    pub(super) fn ability_definition(
        &self,
        source_id: ObjectId,
        face_index: usize,
        ability_path: Vec<tricerules_cards::AbilityId>,
    ) -> AbilityDefinitionId {
        let object = &self.state.objects[&source_id];
        let values = object
            .copiable_values
            .as_ref()
            .or(object.token_origin.as_ref());
        let card_id = values
            .filter(|v| !v.source_card_id.is_empty())
            .map(|v| v.source_card_id.clone())
            .unwrap_or_else(|| object.card_id.clone());
        let face_id = values
            .and_then(|values| {
                values
                    .room_faces
                    .as_ref()
                    .and_then(|faces| faces.get(face_index))
                    .or_else(|| values.room_faces.is_none().then_some(&values.face))
            })
            .map(|face| face.face_id.clone())
            .or_else(|| {
                self.registry
                    .get(&card_id)
                    .and_then(|card| card.faces.get(face_index))
                    .map(|face| face.face_id.clone())
            })
            .expect("validated ability provenance has a stable face id");
        AbilityDefinitionId {
            card_id,
            face_id,
            ability_path,
        }
    }

    pub(super) fn effective_triggered_abilities(
        &self,
        source_id: ObjectId,
        _card_id: &str,
        face_index: usize,
    ) -> Vec<(usize, TriggeredAbilityDef, TriggerAbilityOrigin)> {
        let face_down = self
            .state
            .objects
            .get(&source_id)
            .is_some_and(|object| object.face_down);
        let removed_at =
            super::characteristics::latest_remove_all_abilities_timestamp(&self.state, source_id);
        let mut printed = Vec::new();
        if !face_down && removed_at.is_none() {
            if let Some(faces) = self.room_faces(source_id) {
                for door in self
                    .state
                    .room_states
                    .get(&source_id)
                    .copied()
                    .unwrap_or_default()
                    .unlocked_indices()
                {
                    for ability in &faces[door].triggered_abilities {
                        printed.push((
                            ability.clone(),
                            self.ability_definition(
                                source_id,
                                door,
                                vec![ability.ability_id.clone()],
                            ),
                        ));
                    }
                }
            } else if let Some(face) = self.effective_face(source_id) {
                for ability in &face.triggered_abilities {
                    printed.push((
                        ability.clone(),
                        self.ability_definition(
                            source_id,
                            face_index,
                            vec![ability.ability_id.clone()],
                        ),
                    ));
                }
            }
        }
        let mut abilities: Vec<_> = printed
            .into_iter()
            .enumerate()
            .map(|(index, (ability, definition))| {
                (index, ability, TriggerAbilityOrigin::Printed(definition))
            })
            .collect();
        let mut next_index = abilities.len();
        let Some(characteristics) = self.characteristics(source_id) else {
            return abilities;
        };
        for effect in &self.state.continuous_effects {
            let ContinuousEffectKind::GrantTriggeredAbility(ability) = &effect.kind else {
                continue;
            };
            if removed_at.is_some_and(|timestamp| effect.timestamp <= timestamp) {
                continue;
            }
            if super::characteristics::effect_affects(
                &self.state,
                self.registry,
                effect,
                source_id,
                &characteristics,
            ) && self.continuous_effect_condition_holds(effect)
            {
                abilities.push((
                    next_index,
                    (**ability).clone(),
                    effect
                        .trigger_grant_origin
                        .clone()
                        .expect("trigger grant has provenance"),
                ));
                next_index += 1;
            }
        }
        abilities
    }

    pub(super) fn matching_snapshot_abilities(
        &self,
        source: &TriggerSourceSnapshot,
        filter: impl Fn(&TriggerCondition) -> bool,
    ) -> Vec<CollectedTrigger> {
        source
            .triggered_abilities
            .iter()
            .filter(|(_, ability, _)| filter(&ability.trigger))
            .filter(|(_, ability, _)| {
                let requires_event_context = matches!(
                    ability.intervening_if.as_ref(),
                    Some(GameCondition::TriggeringSpellManaSpent { .. })
                );
                source.event_conditions_checked
                    || requires_event_context
                    || self.intervening_if_holds(
                        source.object_id,
                        source.controller,
                        ability.intervening_if.as_ref(),
                    )
            })
            .map(|(ability_index, ability, origin)| CollectedTrigger {
                source_id: source.object_id,
                card_id: source.card_id.clone(),
                face_index: source.face_index,
                source_zone_change: source.zone_change_generation,
                source_face_change: source.face_change_generation,
                source_fact: Some(source.event_fact()),
                additional_instances: 0,
                controller: source.controller,
                ability_index: *ability_index,
                ability_origin: Some(origin.clone()),
                presentation: None,
                ability: ability.clone(),
                ability_text: ability.fallback_text_with_path(
                    &source.face_name,
                    &trigger_ability_path(origin, ability),
                ),
                trigger_context: TriggerContext::default(),
            })
            .collect()
    }

    pub(super) fn trigger_object_ref(&self, object_id: ObjectId) -> Option<TriggerObjectRef> {
        let characteristics = self.characteristics(object_id)?;
        Some(TriggerObjectRef {
            object_id,
            zone_change_generation: self
                .state
                .zone_change_generation
                .get(&object_id)
                .copied()
                .unwrap_or(0),
            controller_at_event: characteristics.controller,
        })
    }

    fn creature_event_filter_matches(
        &self,
        object_id: ObjectId,
        filter: &CreatureEventFilter,
    ) -> bool {
        self.characteristics(object_id)
            .is_some_and(|characteristics| {
                Self::creature_event_filter_matches_characteristics(&characteristics, filter)
            })
    }

    fn creature_event_filter_matches_characteristics(
        characteristics: &Characteristics,
        filter: &CreatureEventFilter,
    ) -> bool {
        characteristics.is_creature()
            && filter
                .required_subtypes
                .iter()
                .all(|subtype| characteristics.has_type(subtype))
            && filter
                .required_keywords
                .iter()
                .all(|keyword| characteristics.has_keyword(*keyword))
            && filter
                .excluded_keywords
                .iter()
                .all(|keyword| !characteristics.has_keyword(*keyword))
            && filter.power.is_none_or(|comparison| {
                characteristics.power.is_some_and(|power| match comparison {
                    PowerComparison::AtLeast(minimum) => power >= minimum,
                    PowerComparison::AtMost(maximum) => power <= maximum,
                })
            })
    }

    /// CR 603.4: evaluate a triggered ability's intervening-"if" clause against the current state.
    /// `None` (no clause) always holds. Called once when the trigger would go on the stack and
    /// again when it resolves — both checks read live state, which is the whole point of the rule.
    pub(super) fn intervening_if_holds(
        &self,
        source_id: ObjectId,
        controller: PlayerId,
        clause: Option<&GameCondition>,
    ) -> bool {
        self.intervening_if_holds_at_generation(source_id, controller, clause, None, None)
    }

    pub(super) fn intervening_if_holds_at_generation(
        &self,
        source_id: ObjectId,
        controller: PlayerId,
        clause: Option<&GameCondition>,
        source_generation: Option<u64>,
        trigger_context: Option<&TriggerContext>,
    ) -> bool {
        clause.is_none_or(|condition| {
            self.condition_holds_with_trigger_context(
                condition,
                ConditionContext {
                    controller,
                    source_object_id: source_id,
                    source_zone_change: source_generation.unwrap_or_else(|| {
                        self.state
                            .zone_change_generation
                            .get(&source_id)
                            .copied()
                            .unwrap_or(0)
                    }),
                    resolving_spell_id: None,
                    stack_item: None,
                },
                trigger_context,
            )
        })
    }

    /// The player a trigger's effects act on when the trigger names a player other than its
    /// controller ("**that player** draws an additional card"). `None` means "the ability's
    /// controller", which is every other trigger today. Stored on the stack item so the
    /// beneficiary survives the trip through the stack and any responses.
    fn trigger_player_for(event: &GameEvent) -> Option<PlayerId> {
        match event {
            GameEvent::PhaseBegan { active_player, .. } => Some(*active_player),
            GameEvent::Sacrificed { player, .. } => Some(*player),
            GameEvent::Surveilled { player } => Some(*player),
            GameEvent::LibrarySearched { searcher, .. } => Some(*searcher),
            GameEvent::CrimeCommitted { player } => Some(*player),
            GameEvent::Blighted(receipt) => Some(receipt.player),
            GameEvent::Waterbent { player } => Some(*player),
            GameEvent::ManaSpentCastingSpell { player, .. } => Some(*player),
            GameEvent::CardDrawn { drawer, .. } => Some(*drawer),
            GameEvent::SpellCast { fact } => Some(fact.caster),
            GameEvent::TargetsChosen { controller, .. } => Some(*controller),
            _ => None,
        }
    }

    /// Put one staged trigger on the stack, or park it in `pending_triggers` when its effect needs
    /// a target (CR 603.3d).
    ///
    /// Only ever reached through [`Self::flush_staged_triggers`], which is what guarantees the
    /// caller has already fixed the order and that at most one target choice is outstanding.
    ///
    /// The effect is looked up from the registry here and again at resolution rather than being
    /// carried on the staged trigger, so an ability whose source has since left the battlefield
    /// still knows what it does.
    pub(super) fn push_trigger(
        &mut self,
        trigger: StagedTrigger,
        events: &mut Vec<rv1::RuledEvent>,
    ) {
        let StagedTrigger {
            object_id: virtual_id,
            source_permanent_id: source_id,
            source_owner,
            source_face_index,
            source_zone_change,
            source_face_change,
            card_id,
            card_name,
            controller,
            ability_index,
            ability,
            ability_text,
            presentation,
            trigger_context,
            may,
        } = trigger;
        let needs_target = target_schema(&ability.effect, ability.targeting.as_ref()).has_targets();
        let modal_modes = ability.modal.as_ref().map(|modal| {
            modal
                .modes
                .iter()
                .enumerate()
                .map(|(mode_index, mode)| {
                    let mode_needs_target =
                        target_schema(&mode.effects, mode.targeting.as_ref()).has_targets();
                    let targets = compute_ability_targets_with_context(
                        self,
                        controller,
                        TargetSourceIdentity::captured(source_id, source_zone_change),
                        &mode.effects,
                        mode.targeting.as_ref(),
                        trigger_context,
                    );
                    let selectable = !mode_needs_target
                        || targets
                            .groups
                            .iter()
                            .all(|group| legal_target_group_has_minimum(&self.state, group));
                    rv1::LegalSpellMode {
                        mode_index: mode_index as u32,
                        label: mode_fallback(&ability_text, &mode.mode_id),
                        selectable,
                        needs_target: mode_needs_target,
                        targets: Some(targets),
                        presentation: presentation.as_ref().map(|parent| {
                            child_presentation_ref(
                                parent,
                                PresentationPath::Mode(&mode.mode_id),
                                &mode.presentation,
                                mode_fallback(&ability_text, &mode.mode_id),
                            )
                        }),
                        linked_cast_cost: None,
                    }
                })
                .collect::<Vec<_>>()
        });
        let legal_targets = if needs_target {
            Some(compute_ability_targets_with_context(
                self,
                controller,
                TargetSourceIdentity::captured(source_id, source_zone_change),
                &ability.effect,
                ability.targeting.as_ref(),
                trigger_context,
            ))
        } else {
            None
        };
        let has_legal_target = legal_targets.as_ref().is_none_or(|targets| {
            targets
                .groups
                .iter()
                .all(|group| legal_target_group_has_minimum(&self.state, group))
        });

        let needs_choice = needs_target || modal_modes.is_some();
        let (min_modes, max_modes) = ability
            .modal
            .as_ref()
            .map_or((0, 0), |modal| (modal.min_modes, modal.max_modes));
        let modal_has_enough_choices = ability.modal.as_ref().is_none_or(|modal| {
            modal_modes.as_ref().is_some_and(|modes| {
                modes.iter().filter(|mode| mode.selectable).count() >= modal.min_modes as usize
            })
        });
        if needs_choice && ((has_legal_target && modal_has_enough_choices) || may) {
            let was_empty = self.state.pending_triggers.is_empty();
            self.state.pending_triggers.push_back(PendingTrigger {
                object_id: virtual_id,
                source_permanent_id: source_id,
                source_owner,
                source_face_index,
                source_zone_change,
                source_face_change,
                ability_index,
                ability,
                ability_text: ability_text.clone(),
                presentation: presentation.clone(),
                card_id,
                controller,
                trigger_context,
                may,
            });
            if was_empty {
                events.push(rv1::RuledEvent {
                    ev: Some(rv1::ruled_event::Ev::TriggerNeedsTarget(
                        rv1::TriggerNeedsTarget {
                            source_permanent_id: source_id,
                            ability_index: ability_index as u32,
                            ability_text: ability_text.clone(),
                            controller_player_id: controller,
                            may_decline: may,
                            targets: legal_targets.clone(),
                            min_modes,
                            max_modes,
                            modes: modal_modes.unwrap_or_default(),
                            ability_presentation: presentation.clone(),
                        },
                    )),
                });
                events.push(ev_log(format!(
                    "Triggered: {card_name} — choose for: {ability_text}"
                )));
            }
        } else if needs_choice {
            // CR 603.3d: a targeted trigger with no legal target is removed from the stack. An
            // optional trigger remains pending so its controller can explicitly decline it.
        } else {
            self.state.stack_presentations.insert(
                virtual_id,
                StackPresentation {
                    primary: presentation.clone(),
                    ..Default::default()
                },
            );
            self.state.stack.push(StackItem {
                id: virtual_id,
                controller,
                card_id,
                targets: vec![],
                ability_text: Some(ability_text.clone()),
                source_permanent_id: Some(source_id),
                source_owner: Some(source_owner),
                source_zone_change,
                source_face_change,
                ability_index: Some(ability_index),
                activated_ability: None,
                triggered_ability: Some(ability),
                is_triggered: true,
                is_copy: false,
                chosen_x: trigger_context.entering_chosen_x.unwrap_or(0),
                face_index: source_face_index,
                chosen_modes: vec![],
                cast_condition_results: Vec::new(),
                cast_occurrence: None,
                cast_cost_receipts: vec![],
                payment_result: CardResultCohort::default(),
                resolution_branch_choices: Default::default(),
                blight_receipts: Vec::new(),
                trigger_context,
                cast_method: SpellCastMethod::Normal,
                sneak_attack: None,
            });
            self.state.passes_since_stack_change = 0;
            events.push(rv1::RuledEvent {
                ev: Some(rv1::ruled_event::Ev::StackPushed(rv1::StackPushed {
                    object_id: virtual_id,
                    description: card_name.clone(),
                    targets: vec![],
                    ability_annotation: ability_text.clone(),
                    card_id: String::new(),
                    is_copy: false,
                    is_triggered: true,
                    copy_source_object_id: 0,
                    chosen_mode_indices: vec![],
                    chosen_mode_labels: vec![],
                    chosen_cast_cost_labels: vec![],
                    source_token_identity: None,
                    primary_presentation: presentation,
                    chosen_mode_presentations: vec![],
                    chosen_cast_cost_presentations: vec![],
                })),
            });
            events.push(ev_log(format!("Triggered: {card_name} — {ability_text}")));
        }
    }
}

impl TriggerSourceSnapshot {
    /// Prefer the instruction's pre-event source set to an older cost-preflight snapshot.
    fn at_event<'a>(&'a self, sources: &'a [Self]) -> &'a Self {
        sources
            .iter()
            .find(|source| {
                source.object_id == self.object_id
                    && source.zone_change_generation == self.zone_change_generation
            })
            .unwrap_or(self)
    }

    fn event_fact(&self) -> TurnObjectFact {
        TurnObjectFact {
            object_id: self.object_id,
            zone_change_generation: self.zone_change_generation,
            owner: self.owner,
            controller: self.controller,
            is_token: self.is_token,
            types: self.types.clone(),
            all_creature_types: self.all_creature_types,
            keywords: vec![],
            power: self.power_toughness.0.map(|p| p.max(0) as u32),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tricerules_cards::{AbilityId, AbilityPresentation, CardFaceId, IdentifiedAbility};

    #[test]
    fn issue_208_own_library_search_keeps_searcher_and_owner_distinct() {
        let (mut engine, source) = trigger_limit_source();
        add_limited_grant(
            &mut engine,
            source,
            TriggerCondition::WheneverPlayerSearchesOwnLibrary {
                player: CastTriggerPlayer::Opponent,
            },
        );
        assert_eq!(
            engine
                .collect_event_triggers(&[GameEvent::LibrarySearched {
                    searcher: 1,
                    library_owner: 1,
                }])
                .len(),
            1
        );
        assert!(engine
            .collect_event_triggers(&[GameEvent::LibrarySearched {
                searcher: 0,
                library_owner: 0,
            }])
            .is_empty());
        assert!(engine
            .collect_event_triggers(&[GameEvent::LibrarySearched {
                searcher: 1,
                library_owner: 0,
            }])
            .is_empty());
    }

    #[test]
    fn issue_208_only_the_entrants_own_etb_trigger_inherits_chosen_x() {
        let (mut engine, source) = trigger_limit_source();
        add_limited_grant(
            &mut engine,
            source,
            TriggerCondition::WheneverPermanentEntersBattlefield {
                controller: CastTriggerPlayer::AnyPlayer,
                filter: Default::default(),
                creature_filter: None,
            },
        );
        let triggers = engine.collect_event_triggers(&[GameEvent::EntersBattlefield {
            object_id: source,
            chosen_x: 7,
        }]);
        assert_eq!(triggers.len(), 1);
        assert_eq!(triggers[0].trigger_context.entering_chosen_x, None);
    }

    #[test]
    fn issue_185_multiple_lore_counters_create_one_occurrence_per_crossed_numeral() {
        let (mut engine, source) = trigger_limit_source();
        let face = engine
            .registry
            .get("burn,_burn,_tree_and_fern")
            .expect("Burn")
            .primary_face()
            .clone();
        let object = engine.state.objects.get_mut(&source).expect("source");
        object.card_id = "burn,_burn,_tree_and_fern".into();
        object.copiable_values = Some(CopiableValues {
            source_card_id: "burn,_burn,_tree_and_fern".into(),
            source_face_index: 0,
            display_name: "Burn, Burn, Tree and Fern".into(),
            room_faces: None,
            face,
        });

        let event = engine
            .place_counters_with_event(source, CounterKind::Lore, 4, false)
            .expect("place lore");
        let triggers = engine.collect_event_triggers(&[event]);
        assert_eq!(triggers.len(), 4);
        assert_eq!(
            triggers
                .iter()
                .filter(|trigger| trigger.ability.ability_id.as_str() == "chapter_iii_iv")
                .count(),
            2,
            "the combined III, IV ability triggers once for each crossed numeral"
        );
    }

    #[test]
    fn issue_168_entry_observer_uses_shared_subtype_filter() {
        let data = r#"(id: "entry_probe", name: "Entry Probe", face_id: "entry_probe", types: ["Creature"],
            power: 1, toughness: 1, triggered_abilities: [(
            ability_id: "triggered_01",
            presentation: Fallback,
            trigger: WheneverPermanentEntersBattlefield(controller: Controller,
                filter: (any_subtypes: ["Bird", "Fish"])),
            effect: [GainLife(amount: 1)], )])"#;
        let registry = CardRegistry::from_chunks_and_tokens(&[data], &[]).unwrap();
        let trigger = registry
            .get("entry_probe")
            .unwrap()
            .primary_face()
            .triggered_abilities[0]
            .trigger
            .clone();
        let (mut engine, source) = trigger_limit_source();
        add_limited_grant(&mut engine, source, trigger);
        assert!(
            engine
                .collect_event_triggers(&[GameEvent::EntersBattlefield {
                    object_id: source,
                    chosen_x: 0,
                }])
                .is_empty(),
            "a Bear is neither a Bird nor a Fish"
        );
    }

    #[test]
    fn issue_168_departure_vocabulary_loads() {
        let data = r#"(id: "departure_probe", name: "Departure Probe", face_id: "departure_probe", types: ["Creature"],
            power: 1, toughness: 1, triggered_abilities: [(
            ability_id: "triggered_01",
            presentation: Fallback,
            trigger: WheneverPermanentLeavesBattlefield(controller: Controller,
                filter: (permanent_type: Some(Creature)), destination: Except([Graveyard])),
            effect: [GainLife(amount: 1)], ), (
            ability_id: "triggered_02",
            presentation: Fallback,
            trigger: WheneverCardsLeaveGraveyard(owner: Controller,
                filter: (permanent_type: Some(Creature)), cardinality: OneOrMore),
            effect: [GainLife(amount: 1)], )])"#;
        CardRegistry::from_chunks_and_tokens(&[data], &[])
            .expect("shared zone observer vocabulary");
    }

    fn test_ability_origin(index: usize) -> TriggerAbilityOrigin {
        TriggerAbilityOrigin::Printed(AbilityDefinitionId {
            card_id: "test_trigger_source".into(),
            face_id: CardFaceId::new("front").unwrap(),
            ability_path: vec![AbilityId::new(format!("triggered_{:02}", index + 1)).unwrap()],
        })
    }

    fn trigger_limit_source() -> (GameEngine, ObjectId) {
        let mut engine = GameEngine::new(164_001, &[0, 1], 20, None, true).unwrap();
        let source = engine.state.players[0].hand[0];
        engine.state.objects.get_mut(&source).unwrap().card_id = "grizzly_bears".into();
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            source,
            Zone::Battlefield,
            None,
        )
        .unwrap();
        (engine, source)
    }

    fn add_limited_grant(engine: &mut GameEngine, source: ObjectId, trigger: TriggerCondition) {
        let mut ability = engine
            .registry
            .get("gravedigger")
            .unwrap()
            .primary_face()
            .triggered_abilities[0]
            .clone();
        ability.trigger = trigger;
        ability.triggers_only_once = true;
        engine.state.add_triggered_ability_grant(ContinuousEffect {
            trigger_grant_origin: None,
            source_id: None,
            affected: AffectedScope::Single(source),
            kind: ContinuousEffectKind::GrantTriggeredAbility(Box::new(ability)),
            condition: None,
            duration: EffectDuration::UntilEndOfTurn,
            timestamp: engine.state.command_index,
        });
    }

    fn stage_limited_dies_grants(engine: &mut GameEngine, source: ObjectId) {
        let triggers =
            engine.matching_triggered_abilities("grizzly_bears", source, 0, 0, |trigger| {
                *trigger == TriggerCondition::WhenSelfDies
            });
        engine.stage_triggers(triggers);
    }

    #[test]
    fn issue_164_removing_an_earlier_grant_does_not_refresh_a_spent_ability() {
        let (mut engine, source) = trigger_limit_source();
        add_limited_grant(
            &mut engine,
            source,
            TriggerCondition::WhenSelfEntersBattlefield,
        );
        add_limited_grant(&mut engine, source, TriggerCondition::WhenSelfDies);
        stage_limited_dies_grants(&mut engine, source);
        assert_eq!(
            engine
                .state
                .staged_trigger_groups
                .pop_front()
                .unwrap()
                .triggers
                .len(),
            1
        );
        engine.state.continuous_effects.remove(0);
        stage_limited_dies_grants(&mut engine, source);
        assert!(
            engine.state.staged_trigger_groups.is_empty(),
            "the remaining grant already triggered"
        );
    }

    #[test]
    fn issue_164_a_new_identical_grant_does_not_inherit_a_removed_grants_usage() {
        let (mut engine, source) = trigger_limit_source();
        add_limited_grant(&mut engine, source, TriggerCondition::WhenSelfDies);
        stage_limited_dies_grants(&mut engine, source);
        engine.state.staged_trigger_groups.clear();
        engine.state.continuous_effects.clear();
        add_limited_grant(&mut engine, source, TriggerCondition::WhenSelfDies);
        stage_limited_dies_grants(&mut engine, source);
        assert_eq!(
            engine.state.staged_trigger_groups.len(),
            1,
            "a new grant has its own allowance"
        );
    }

    fn take_staged_count(engine: &mut GameEngine) -> usize {
        engine
            .state
            .staged_trigger_groups
            .drain(..)
            .map(|group| group.triggers.len())
            .sum()
    }

    #[test]
    fn issue_164_caps_bound_a_batch_and_reset_on_a_new_turn_instance() {
        for cap in [None, Some(1), Some(2)] {
            let (mut engine, source) = trigger_limit_source();
            add_limited_grant(&mut engine, source, TriggerCondition::WhenSelfDies);
            let ContinuousEffectKind::GrantTriggeredAbility(ability) =
                &mut engine.state.continuous_effects[0].kind
            else {
                unreachable!()
            };
            ability.triggers_only_once = false;
            ability.max_triggers_per_turn = cap;
            let mut batch = Vec::new();
            for _ in 0..3 {
                batch.extend(engine.matching_triggered_abilities(
                    "grizzly_bears",
                    source,
                    0,
                    0,
                    |trigger| *trigger == TriggerCondition::WhenSelfDies,
                ));
            }
            engine.stage_triggers(batch);
            assert_eq!(take_staged_count(&mut engine), cap.unwrap_or(3) as usize);
            stage_limited_dies_grants(&mut engine, source);
            assert_eq!(take_staged_count(&mut engine), usize::from(cap.is_none()));

            // Model another turn for the same seat: this tests extra-turn-safe bookkeeping,
            // not extra-turn scheduling (which the engine does not implement yet).
            engine.state.turn_instance += 1;
            stage_limited_dies_grants(&mut engine, source);
            assert_eq!(take_staged_count(&mut engine), 1);
        }
    }

    fn limited_dies_ability(engine: &GameEngine) -> TriggeredAbilityDef {
        let mut ability = engine
            .registry
            .get("gravedigger")
            .unwrap()
            .primary_face()
            .triggered_abilities[0]
            .clone();
        ability.trigger = TriggerCondition::WhenSelfDies;
        ability.max_triggers_per_turn = Some(1);
        ability
    }

    fn install_trigger_face(
        engine: &mut GameEngine,
        source: ObjectId,
        abilities: Vec<TriggeredAbilityDef>,
    ) {
        let mut values = engine.copiable_values_for(source).unwrap();
        values.face.triggered_abilities = abilities;
        engine
            .state
            .objects
            .get_mut(&source)
            .unwrap()
            .copiable_values = Some(values);
    }

    #[test]
    fn issue_164_both_limits_are_checked_before_either_is_spent() {
        for first_lifetime in [false, true] {
            let (mut engine, source) = trigger_limit_source();
            let mut ability = limited_dies_ability(&engine);
            ability.triggers_only_once = first_lifetime;
            ability.max_triggers_per_turn = (!first_lifetime).then_some(1);
            install_trigger_face(&mut engine, source, vec![ability.clone()]);
            stage_limited_dies_grants(&mut engine, source);
            assert_eq!(take_staged_count(&mut engine), 1);
            ability.triggers_only_once = true;
            ability.max_triggers_per_turn = Some(1);
            install_trigger_face(&mut engine, source, vec![ability]);
            let before = (
                engine.state.triggered_once.clone(),
                engine.state.trigger_uses_this_turn.clone(),
            );
            stage_limited_dies_grants(&mut engine, source);
            assert_eq!(take_staged_count(&mut engine), 0);
            assert_eq!(
                before,
                (
                    engine.state.triggered_once.clone(),
                    engine.state.trigger_uses_this_turn.clone()
                )
            );
            engine.state.turn_instance += 1;
            stage_limited_dies_grants(&mut engine, source);
            assert_eq!(take_staged_count(&mut engine), usize::from(!first_lifetime));
        }
    }

    #[test]
    fn issue_164_printed_slots_control_suppression_and_blink_keep_the_right_identity() {
        let (mut engine, source) = trigger_limit_source();
        let ability = limited_dies_ability(&engine);
        let mut second_ability = ability.clone();
        second_ability.ability_id = AbilityId::new("triggered_02").unwrap();
        install_trigger_face(&mut engine, source, vec![ability, second_ability]);
        let values = engine.state.objects[&source]
            .copiable_values
            .clone()
            .unwrap();
        stage_limited_dies_grants(&mut engine, source);
        assert_eq!(
            take_staged_count(&mut engine),
            2,
            "mechanically identical abilities with distinct stable IDs are independent"
        );
        engine.state.objects.get_mut(&source).unwrap().controller = 1;
        engine.state.continuous_effects.push(ContinuousEffect {
            trigger_grant_origin: None,
            source_id: None,
            affected: AffectedScope::Single(source),
            kind: ContinuousEffectKind::Layer6RemoveAllAbilities,
            condition: None,
            duration: EffectDuration::UntilEndOfTurn,
            timestamp: engine.state.command_index + 1,
        });
        stage_limited_dies_grants(&mut engine, source);
        assert_eq!(take_staged_count(&mut engine), 0);
        engine.state.continuous_effects.clear();
        engine.state.face_change_generation.insert(source, 7);
        stage_limited_dies_grants(&mut engine, source);
        assert_eq!(
            take_staged_count(&mut engine),
            0,
            "restoration and control/status changes do not refund usage"
        );
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            source,
            Zone::Exile,
            None,
        )
        .unwrap();
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            source,
            Zone::Battlefield,
            None,
        )
        .unwrap();
        engine
            .state
            .objects
            .get_mut(&source)
            .unwrap()
            .copiable_values = Some(values);
        stage_limited_dies_grants(&mut engine, source);
        assert_eq!(
            take_staged_count(&mut engine),
            2,
            "the returned source is a fresh incarnation"
        );
    }

    #[test]
    fn issue_164_static_grants_survive_refresh_and_conditional_suppression() {
        let (mut engine, source) = trigger_limit_source();
        let ability = limited_dies_ability(&engine);
        let mut second_ability = ability.clone();
        second_ability.ability_id = AbilityId::new("triggered_02").unwrap();
        let mut values = engine.copiable_values_for(source).unwrap();
        values.face.static_abilities = vec![IdentifiedAbility::fallback(
            "static_01",
            StaticAbilityDef::ConditionalSelfModifier {
                condition: GameCondition::ActivePlayer {
                    players: RelativePlayerSet::Controller,
                },
                add_types: tricerules_cards::primitives::TypeLineAddition::default(),
                base_power: None,
                base_toughness: None,
                delta_power: 0,
                delta_toughness: 0,
                keywords: vec![],
                activated_abilities: vec![],
                triggered_abilities: vec![ability, second_ability],
                can_attack_as_though_without_defender: false,
            },
        )
        .unwrap()];
        engine
            .state
            .objects
            .get_mut(&source)
            .unwrap()
            .copiable_values = Some(values);
        engine.emit_static_abilities_on_enter(source);
        stage_limited_dies_grants(&mut engine, source);
        assert_eq!(take_staged_count(&mut engine), 2);
        engine.state.active_player_idx = 1;
        assert!(engine
            .effective_triggered_abilities(source, "grizzly_bears", 0)
            .is_empty());
        engine.state.active_player_idx = 0;
        engine.refresh_source_static_abilities(source);
        stage_limited_dies_grants(&mut engine, source);
        assert_eq!(
            take_staged_count(&mut engine),
            0,
            "refresh and the same conditional grant retain provenance"
        );
    }

    #[test]
    fn issue_164_room_door_slots_survive_unlocking_an_earlier_door() {
        let (mut engine, source) = trigger_limit_source();
        let ability = limited_dies_ability(&engine);
        let mut values = engine.copiable_values_for(source).unwrap();
        let mut doors = engine
            .registry
            .get("glassworks_shattered_yard")
            .unwrap()
            .faces
            .clone();
        for door in &mut doors {
            door.triggered_abilities = vec![ability.clone()];
            door.static_abilities.clear();
        }
        values.room_faces = Some(doors);
        engine
            .state
            .objects
            .get_mut(&source)
            .unwrap()
            .copiable_values = Some(values);
        engine.state.room_states.insert(
            source,
            RoomState {
                unlocked: [false, true],
            },
        );
        stage_limited_dies_grants(&mut engine, source);
        assert_eq!(take_staged_count(&mut engine), 1);
        engine.state.room_states.get_mut(&source).unwrap().unlocked[0] = true;
        stage_limited_dies_grants(&mut engine, source);
        assert_eq!(
            take_staged_count(&mut engine),
            1,
            "only the newly unlocked door has an allowance"
        );
        stage_limited_dies_grants(&mut engine, source);
        assert_eq!(take_staged_count(&mut engine), 0);
    }

    #[test]
    fn issue_164_departed_grant_lki_keeps_the_consumed_identity() {
        let (mut engine, source) = trigger_limit_source();
        add_limited_grant(&mut engine, source, TriggerCondition::WhenSelfDies);
        let snapshot = engine.trigger_source_snapshot(source).unwrap();
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            source,
            Zone::Graveyard,
            None,
        )
        .unwrap();
        assert!(engine.state.continuous_effects.is_empty());
        let event = GameEvent::Dies {
            source: snapshot,
            was_creature: true,
        };
        engine.fire_triggers(std::slice::from_ref(&event));
        assert_eq!(take_staged_count(&mut engine), 1);
        engine.fire_triggers(&[event]);
        assert_eq!(
            take_staged_count(&mut engine),
            0,
            "lookback cannot reconstruct a fresh grant"
        );
    }

    #[test]
    fn issue_164_countering_and_repeated_cleanup_do_not_reset_usage() {
        let (mut engine, source) = trigger_limit_source();
        let mut ability = limited_dies_ability(&engine);
        ability.effect = vec![SpellEffectKind::GainLife {
            amount: Amount::Fixed(1),
        }];
        ability.targeting = None;
        ability.may = false;
        install_trigger_face(&mut engine, source, vec![ability]);
        stage_limited_dies_grants(&mut engine, source);
        let mut events = Vec::new();
        engine.flush_staged_triggers(&mut events);
        let trigger = engine.state.stack[0].id;
        let turn = engine.state.turn_instance;
        engine.state.turn_step = TurnStep::Cleanup;
        engine.finish_cleanup_roll_new_turn(Vec::new()).unwrap();
        assert!(engine.state.cleanup_priority_active);
        assert_eq!(engine.state.turn_instance, turn);
        let usage = engine.state.trigger_uses_this_turn.clone();
        super::super::resolution::counter_stack_object_ref(
            &mut engine,
            StackObjectRef {
                object_id: trigger,
                zone_change_generation: None,
            },
            "test counter",
            &mut events,
        )
        .unwrap();
        assert!(engine.state.stack.is_empty());
        assert!(events.iter().any(|e| matches!(&e.ev, Some(rv1::ruled_event::Ev::StackObjectCountered(c)) if c.object_id == trigger)));
        stage_limited_dies_grants(&mut engine, source);
        assert_eq!(take_staged_count(&mut engine), 0);
        assert_eq!(engine.state.trigger_uses_this_turn, usage);
        engine.finish_cleanup_roll_new_turn(Vec::new()).unwrap();
        assert_eq!(engine.state.turn_instance, turn + 1);
        assert!(engine.state.trigger_uses_this_turn.is_empty());
    }

    #[test]
    fn issue_172_thresholds_overshoot_and_relative_players_share_snapshot_matching() {
        let mut engine = GameEngine::new(172012, &[0, 1], 20, None, true).unwrap();
        engine.state.players.push(PlayerState::new(2, 20));
        let ability = engine
            .registry
            .get("wandertale_mentor")
            .unwrap()
            .primary_face()
            .triggered_abilities[0]
            .clone();
        let source = TriggerSourceSnapshot {
            counters: BTreeMap::new(),
            owner: 0,
            is_token: false,
            all_creature_types: false,
            types: vec!["Creature".into()],
            power_toughness: (None, None),
            event_conditions_checked: false,
            object_id: 100,
            card_id: "wandertale_mentor".into(),
            face_name: "Wandertale Mentor".into(),
            controller: 1,
            face_index: 0,
            zone_change_generation: 3,
            face_change_generation: 0,
            attached_to: None,
            triggered_abilities: [4, 8]
                .into_iter()
                .enumerate()
                .map(|(index, amount)| {
                    let mut ability = ability.clone();
                    ability.trigger = TriggerCondition::WheneverPlayerExpendsMana {
                        player: CastTriggerPlayer::AnyPlayer,
                        amount,
                    };
                    (index, ability, test_ability_origin(index))
                })
                .collect(),
        };
        for (before, after, count) in [
            (0, 3, 0),
            (3, 4, 1),
            (0, 9, 2),
            (4, 8, 1),
            (8, 16, 0),
            (3, 3, 0),
        ] {
            let event = GameEvent::ManaSpentCastingSpell {
                player: 2,
                before,
                after,
            };
            let matches = engine.collect_triggers(&event, std::slice::from_ref(&source));
            assert_eq!(matches.len(), count);
            assert!(matches
                .iter()
                .all(|t| t.source_zone_change == 3 && t.controller == 1));
        }
        for who in [
            CastTriggerPlayer::Controller,
            CastTriggerPlayer::Opponent,
            CastTriggerPlayer::AnyPlayer,
        ] {
            let mut source = source.clone();
            source.triggered_abilities.truncate(1);
            source.triggered_abilities[0].1.trigger = TriggerCondition::WheneverPlayerExpendsMana {
                player: who,
                amount: 4,
            };
            for player in [0, 1, 2] {
                let event = GameEvent::ManaSpentCastingSpell {
                    player,
                    before: 0,
                    after: 4,
                };
                let expected = match who {
                    CastTriggerPlayer::Controller => player == 1,
                    CastTriggerPlayer::Opponent => player != 1,
                    CastTriggerPlayer::AnyPlayer => true,
                };
                assert_eq!(
                    !engine
                        .collect_triggers(&event, std::slice::from_ref(&source))
                        .is_empty(),
                    expected
                );
            }
        }
    }

    fn target_condition_match(
        condition: &TriggerCondition,
        targeting_source: TargetingSourceKind,
        targeting_controller: PlayerId,
        source_id: ObjectId,
        source_controller: PlayerId,
        target_id: ObjectId,
        target_controller: PlayerId,
    ) -> Option<Option<PermanentTypeFilter>> {
        let engine = GameEngine::new(96907, &[0, 1], 20, None, true).expect("new");
        engine.target_trigger_permanent_filter(
            condition,
            targeting_source,
            targeting_controller,
            source_id,
            source_controller,
            target_id,
            target_controller,
        )
    }

    #[test]
    fn target_source_filters_distinguish_casts_copies_and_abilities() {
        let heroic = TriggerCondition::WheneverSelfBecomesTarget {
            source: TargetingSourceFilter::SpellCast,
            source_controller: CastTriggerPlayer::Controller,
        };
        assert_eq!(
            target_condition_match(&heroic, TargetingSourceKind::SpellCast, 0, 10, 0, 10, 0),
            Some(None)
        );
        assert_eq!(
            target_condition_match(&heroic, TargetingSourceKind::SpellCopy, 0, 10, 0, 10, 0),
            None,
            "a copied spell was not cast"
        );

        let bonecrusher = TriggerCondition::WheneverSelfBecomesTarget {
            source: TargetingSourceFilter::Spell,
            source_controller: CastTriggerPlayer::AnyPlayer,
        };
        assert_eq!(
            target_condition_match(
                &bonecrusher,
                TargetingSourceKind::SpellCopy,
                1,
                10,
                0,
                10,
                0,
            ),
            Some(None),
            "a copy is still a spell"
        );
        assert_eq!(
            target_condition_match(&bonecrusher, TargetingSourceKind::Ability, 1, 10, 0, 10, 0,),
            None
        );

        let altanak = TriggerCondition::WheneverSelfBecomesTarget {
            source: TargetingSourceFilter::SpellOrAbility,
            source_controller: CastTriggerPlayer::Opponent,
        };
        assert_eq!(
            target_condition_match(&altanak, TargetingSourceKind::Ability, 1, 10, 0, 10, 0),
            Some(None)
        );
        assert_eq!(
            target_condition_match(&altanak, TargetingSourceKind::SpellCast, 0, 10, 0, 10, 0),
            None,
            "an effect controlled by Altanak's controller does not qualify"
        );
    }

    #[test]
    fn observer_target_filter_supports_another_creature_you_control() {
        let monk = TriggerCondition::WheneverPermanentBecomesTarget {
            source: TargetingSourceFilter::SpellOrAbility,
            source_controller: CastTriggerPlayer::AnyPlayer,
            target_controller: CastTriggerPlayer::Controller,
            permanent_type: Some(PermanentTypeFilter::Creature),
            exclude_self: true,
        };
        assert_eq!(
            target_condition_match(&monk, TargetingSourceKind::Ability, 1, 10, 0, 11, 0),
            Some(Some(PermanentTypeFilter::Creature))
        );
        assert_eq!(
            target_condition_match(&monk, TargetingSourceKind::SpellCast, 1, 10, 0, 10, 0),
            None,
            "another excludes the source"
        );
        assert_eq!(
            target_condition_match(&monk, TargetingSourceKind::SpellCast, 1, 10, 0, 12, 1),
            None,
            "an opponent-controlled permanent does not qualify"
        );
    }

    #[test]
    fn spell_target_filter_accepts_opponent_abilities_only_for_controlled_spells() {
        let engine = GameEngine::new(219_007, &[0, 1], 20, None, true).expect("new");
        let condition = TriggerCondition::WheneverSpellBecomesTarget {
            source: TargetingSourceFilter::SpellOrAbility,
            source_controller: CastTriggerPlayer::Opponent,
            target_controller: CastTriggerPlayer::Controller,
            spell_filter: StackSpellFilter {
                card_type: Some(CardTypeFilter::Creature),
                ..Default::default()
            },
        };
        assert!(engine
            .target_trigger_spell_filter(&condition, TargetingSourceKind::Ability, 1, 0, 0)
            .is_some());
        assert!(engine
            .target_trigger_spell_filter(&condition, TargetingSourceKind::SpellCast, 1, 0, 0)
            .is_some());
        assert!(engine
            .target_trigger_spell_filter(&condition, TargetingSourceKind::Ability, 0, 0, 0)
            .is_none());
        assert!(engine
            .target_trigger_spell_filter(&condition, TargetingSourceKind::Ability, 1, 0, 1)
            .is_none());
    }

    fn issue_219_creature_spell_item(
        id: ObjectId,
        controller: PlayerId,
        is_copy: bool,
    ) -> StackItem {
        StackItem {
            id,
            controller,
            card_id: "grizzly_bears".into(),
            targets: Vec::new(),
            ability_text: None,
            source_permanent_id: None,
            source_owner: None,
            source_zone_change: 0,
            source_face_change: 0,
            ability_index: None,
            activated_ability: None,
            triggered_ability: None,
            is_triggered: false,
            is_copy,
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
    fn issue_219_spell_target_collection_deduplicates_and_preserves_stack_identity() {
        let decks = Some(vec![
            vec![
                "surrak,_elusive_hunter".into(),
                "grizzly_bears".into(),
                "grizzly_bears".into(),
                "forest".into(),
                "forest".into(),
                "forest".into(),
                "forest".into(),
            ],
            vec!["island".into(); 7],
        ]);
        let mut engine = GameEngine::new(219_009, &[0, 1], 20, decks, true).expect("new");
        let surrak = engine
            .state
            .objects
            .values()
            .find(|object| object.card_id == "surrak,_elusive_hunter")
            .expect("Surrak")
            .id;
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            surrak,
            Zone::Battlefield,
            Some(0),
        )
        .expect("put Surrak onto battlefield");
        let mut bears: Vec<_> = engine
            .state
            .objects
            .values()
            .filter(|object| object.card_id == "grizzly_bears")
            .map(|object| object.id)
            .collect();
        bears.sort_unstable();
        for bear in &bears {
            move_object_to_zone(&mut engine.state, engine.registry, *bear, Zone::Stack, None)
                .expect("put Bear spell onto stack");
            engine
                .state
                .stack
                .push(issue_219_creature_spell_item(*bear, 0, false));
        }
        let copy_id = u32::MAX - 1;
        engine
            .state
            .stack
            .push(issue_219_creature_spell_item(copy_id, 0, true));
        let physical_target = |object_id| StackTarget {
            object_id,
            group_index: 0,
            damage_amount: 0,
            kind: rv1::TargetRefKind::Stack as i32,
            zone_change_generation: engine.state.zone_change_generation.get(&object_id).copied(),
        };
        let bear_one = physical_target(bears[0]);
        let bear_two = physical_target(bears[1]);
        let copy = StackTarget {
            object_id: copy_id,
            zone_change_generation: None,
            ..bear_one
        };
        let triggers = engine.collect_event_triggers(&[GameEvent::TargetsChosen {
            controller: 1,
            source: TargetingSourceKind::Ability,
            stack_object: StackObjectRef {
                object_id: 999_219,
                zone_change_generation: None,
            },
            targets: vec![bear_one, bear_one, bear_two, copy],
        }]);
        assert_eq!(triggers.len(), 3);
        assert_eq!(
            triggers
                .iter()
                .filter_map(|trigger| trigger.trigger_context.observed_stack_object)
                .map(|observed| {
                    (
                        observed.stack_object.object_id,
                        observed.stack_object.zone_change_generation,
                    )
                })
                .collect::<BTreeSet<_>>(),
            [
                (bears[0], bear_one.zone_change_generation),
                (bears[1], bear_two.zone_change_generation),
                (copy_id, None),
            ]
            .into_iter()
            .collect()
        );
        assert!(triggers.iter().all(|trigger| {
            trigger.trigger_context.targeting_stack_object
                == Some(StackObjectRef {
                    object_id: 999_219,
                    zone_change_generation: None,
                })
        }));

        *engine
            .state
            .zone_change_generation
            .entry(bears[0])
            .or_default() += 1;
        assert!(engine
            .collect_event_triggers(&[GameEvent::TargetsChosen {
                controller: 1,
                source: TargetingSourceKind::Ability,
                stack_object: StackObjectRef {
                    object_id: 999_220,
                    zone_change_generation: None,
                },
                targets: vec![bear_one],
            }])
            .is_empty());
    }

    #[test]
    fn target_collection_deduplicates_permanent_and_keeps_each_event_identity() {
        let decks = Some(vec![
            vec![
                "bonecrusher_giant_stomp".into(),
                "bonecrusher_giant_stomp".into(),
                "mountain".into(),
                "mountain".into(),
                "mountain".into(),
                "mountain".into(),
                "mountain".into(),
                "mountain".into(),
                "mountain".into(),
            ],
            vec!["forest".into(); 8],
        ]);
        let mut engine = GameEngine::new(94702, &[0, 1], 20, decks, true).expect("new engine");
        let mut giants: Vec<ObjectId> = engine
            .state
            .objects
            .values()
            .filter(|object| object.card_id == "bonecrusher_giant_stomp")
            .map(|object| object.id)
            .collect();
        giants.sort_unstable();
        assert_eq!(giants.len(), 2);
        for giant in &giants {
            move_object_to_zone(
                &mut engine.state,
                engine.registry,
                *giant,
                Zone::Battlefield,
                Some(0),
            )
            .expect("put Bonecrusher Giant onto battlefield");
        }

        engine.fire_triggers(&[GameEvent::TargetsChosen {
            controller: 1,
            source: TargetingSourceKind::SpellCast,
            stack_object: StackObjectRef {
                object_id: 999_001,
                zone_change_generation: None,
            },
            targets: vec![giants[0], giants[0], giants[1]]
                .into_iter()
                .map(|object_id| StackTarget {
                    object_id,
                    group_index: 0,
                    damage_amount: 0,
                    kind: rv1::TargetRefKind::Permanent as i32,
                    zone_change_generation: engine
                        .state
                        .zone_change_generation
                        .get(&object_id)
                        .copied(),
                })
                .collect(),
        }]);

        let group = engine
            .state
            .staged_trigger_groups
            .front()
            .expect("one target-trigger group");
        assert_eq!(group.triggers.len(), 2);
        assert!(group
            .triggers
            .iter()
            .all(|trigger| trigger.trigger_context.affected_player == Some(1)));
        assert_eq!(
            group
                .triggers
                .iter()
                .filter_map(|trigger| trigger.trigger_context.observed_object)
                .map(|object| object.object_id)
                .collect::<BTreeSet<_>>(),
            giants.into_iter().collect()
        );
    }

    #[test]
    fn attached_player_attack_trigger_fires_once_for_the_declaration_group() {
        let engine = GameEngine::new(6303, &[0, 1], 20, None, true).expect("engine");
        let source = TriggerSourceSnapshot {
            counters: BTreeMap::new(),
            owner: 0,
            is_token: false,
            all_creature_types: false,
            types: vec!["Enchantment".into()],
            power_toughness: (None, None),
            event_conditions_checked: false,
            object_id: 100,
            card_id: "curse_of_disturbance".into(),
            face_name: "Curse of Disturbance".into(),
            controller: 0,
            face_index: 0,
            zone_change_generation: 0,
            face_change_generation: 0,
            attached_to: Some(AttachmentSnapshot::Player(1)),
            triggered_abilities: vec![(
                0,
                TriggeredAbilityDef {
                    ability_id: AbilityId::new("triggered_01").unwrap(),
                    presentation: AbilityPresentation::Fallback,
                    trigger: TriggerCondition::WheneverAttachedPlayerIsAttacked,
                    effect: vec![SpellEffectKind::Draw {
                        who: PlayerRecipient::Controller,
                        count: Amount::Fixed(1),
                    }],
                    modal: None,
                    targeting: None,
                    may: false,
                    intervening_if: None,
                    max_triggers_per_turn: None,
                    triggers_only_once: false,
                },
                test_ability_origin(0),
            )],
        };
        let attacker = |object_id| TriggerObjectRef {
            object_id,
            zone_change_generation: 0,
            controller_at_event: 0,
        };
        let event = GameEvent::AttackersDeclared {
            attacking_player: 0,
            attacks: vec![
                AttackEdgeSnapshot {
                    attacker: attacker(200),
                    defender: CombatDefenderTarget::Player(1),
                    defending_player: 1,
                },
                AttackEdgeSnapshot {
                    attacker: attacker(201),
                    defender: CombatDefenderTarget::Player(1),
                    defending_player: 1,
                },
            ],
        };

        let triggers = engine.collect_triggers(&event, std::slice::from_ref(&source));
        assert_eq!(
            triggers.len(),
            1,
            "the Curse triggers once, not once per attacker"
        );
        assert_eq!(triggers[0].trigger_context.attacking_player, Some(0));
        assert_eq!(triggers[0].trigger_context.defending_player, Some(1));

        let other_defender = GameEvent::AttackersDeclared {
            attacking_player: 0,
            attacks: vec![AttackEdgeSnapshot {
                attacker: attacker(202),
                defender: CombatDefenderTarget::Player(2),
                defending_player: 2,
            }],
        };
        assert!(
            engine
                .collect_triggers(&other_defender, std::slice::from_ref(&source))
                .is_empty(),
            "attacking another participant does not trigger the attached player's Curse"
        );
    }

    #[test]
    fn simultaneous_etb_sources_observe_each_other() {
        let decks = Some(vec![
            vec![
                "soul_warden".into(),
                "soul_warden".into(),
                "plains".into(),
                "plains".into(),
                "plains".into(),
                "plains".into(),
                "plains".into(),
                "plains".into(),
            ],
            vec!["forest".into(); 8],
        ]);
        let mut engine = GameEngine::new(6036, &[0, 1], 20, decks, true).expect("new engine");
        let wardens: Vec<ObjectId> = engine
            .state
            .objects
            .values()
            .filter(|object| object.owner == 0 && object.card_id == "soul_warden")
            .map(|object| object.id)
            .collect();
        assert_eq!(wardens.len(), 2);
        for &warden in &wardens {
            move_object_to_zone(
                &mut engine.state,
                engine.registry,
                warden,
                Zone::Battlefield,
                Some(0),
            )
            .expect("put Soul Warden onto battlefield");
        }

        engine.fire_triggers(&[
            GameEvent::EntersBattlefield {
                object_id: wardens[0],
                chosen_x: 0,
            },
            GameEvent::EntersBattlefield {
                object_id: wardens[1],
                chosen_x: 0,
            },
        ]);

        let group = engine
            .state
            .staged_trigger_groups
            .front()
            .expect("one simultaneous trigger group");
        assert_eq!(group.triggers.len(), 2);
        assert_eq!(
            group
                .triggers
                .iter()
                .map(|trigger| trigger.source_permanent_id)
                .collect::<BTreeSet<_>>(),
            wardens.into_iter().collect(),
            "each entering Soul Warden observes the other"
        );
    }
    #[test]
    fn issue_168_group_observer_has_no_arbitrary_trigger_object() {
        use tricerules_cards::primitives::ZoneEventCardinality;
        let (mut engine, source) = trigger_limit_source();
        add_limited_grant(
            &mut engine,
            source,
            TriggerCondition::WheneverCardsLeaveGraveyard {
                owner: CastTriggerPlayer::Controller,
                filter: Default::default(),
                cardinality: ZoneEventCardinality::OneOrMore,
            },
        );
        let cards = engine.state.players[0].hand[..2].to_vec();
        for &card in &cards {
            move_object_to_zone(
                &mut engine.state,
                engine.registry,
                card,
                Zone::Graveyard,
                None,
            )
            .unwrap();
        }
        let snapshot = engine.snapshot_zone_event();
        for &card in &cards {
            move_object_to_zone(&mut engine.state, engine.registry, card, Zone::Exile, None)
                .unwrap();
        }
        let GameEvent::ZoneChanges(batch) = engine.finish_zone_event(snapshot) else {
            panic!("zone event")
        };
        let triggers = engine.collect_zone_triggers(&batch);
        assert_eq!(triggers.len(), 1);
        assert!(
            triggers[0].trigger_context.observed_object.is_none(),
            "a one-or-more group does not name one arbitrary member"
        );
    }

    #[test]
    fn issue_168_departure_intervening_condition_uses_pre_event_state() {
        for condition in [
            TriggerCondition::WhenSelfDies,
            TriggerCondition::WhenSelfLeavesBattlefield,
            TriggerCondition::WheneverPermanentLeavesBattlefield {
                controller: CastTriggerPlayer::Controller,
                filter: PermanentEventFilter {
                    source_only: true,
                    ..Default::default()
                },
                destination: Default::default(),
                cardinality: Default::default(),
            },
        ] {
            let (mut engine, source) = trigger_limit_source();
            let mut ability = engine
                .registry
                .get("ajanis_pridemate")
                .unwrap()
                .primary_face()
                .triggered_abilities[0]
                .clone();
            ability.trigger = condition.clone();
            ability.intervening_if = Some(GameCondition::SourceCounterCount {
                counter: CounterKind::PlusOnePlusOne,
                min: Some(1),
                max: None,
            });
            engine.state.add_triggered_ability_grant(ContinuousEffect {
                trigger_grant_origin: None,
                source_id: None,
                affected: AffectedScope::Single(source),
                kind: ContinuousEffectKind::GrantTriggeredAbility(Box::new(ability)),
                condition: None,
                duration: EffectDuration::UntilEndOfTurn,
                timestamp: 0,
            });
            engine.state.objects.get_mut(&source).unwrap().add_counters(
                CounterKind::PlusOnePlusOne,
                1,
                0,
            );
            let snapshot = engine.snapshot_zone_event();
            let old_source = engine.trigger_source_snapshot(source).unwrap();
            move_object_to_zone(
                &mut engine.state,
                engine.registry,
                source,
                Zone::Graveyard,
                None,
            )
            .unwrap();
            let events = [
                GameEvent::LeavesBattlefield {
                    source: old_source.clone(),
                },
                GameEvent::Dies {
                    source: old_source,
                    was_creature: true,
                },
                engine.finish_zone_event(snapshot),
            ];
            assert_eq!(
                engine.collect_event_triggers(&events).len(),
                1,
                "{condition:?} looks back before counters disappear"
            );
        }
    }

    fn issue_168_fixture_object(
        engine: &mut GameEngine,
        player: usize,
        card: &str,
        zone: Zone,
    ) -> ObjectId {
        let oid = engine.state.players[player].hand[0];
        engine.state.objects.get_mut(&oid).unwrap().card_id = card.into();
        move_object_to_zone(&mut engine.state, engine.registry, oid, zone, None).unwrap();
        oid
    }

    #[test]
    fn issue_168_slagstone_filter_keeps_self_token_exception_and_old_generation() {
        use tricerules_cards::primitives::{EventZone, ZoneEventDestination};
        let (mut engine, source) = trigger_limit_source();
        engine.state.objects.get_mut(&source).unwrap().card_id = "bonesplitter".into();
        let copy = engine.copiable_values_for(source).unwrap();
        engine.state.objects.get_mut(&source).unwrap().token_origin = Some(copy.clone());
        add_limited_grant(
            &mut engine,
            source,
            TriggerCondition::WheneverPermanentLeavesBattlefield {
                controller: CastTriggerPlayer::Controller,
                filter: PermanentEventFilter {
                    any_of: Some(vec![
                        PermanentEventFilter {
                            source_only: true,
                            ..Default::default()
                        },
                        PermanentEventFilter {
                            permanent_type: Some(PermanentTypeFilter::Artifact),
                            token: Some(false),
                            exclude_source: true,
                            ..Default::default()
                        },
                    ]),
                    ..Default::default()
                },
                destination: ZoneEventDestination::OneOf(vec![
                    EventZone::Graveyard,
                    EventZone::Exile,
                ]),
                cardinality: Default::default(),
            },
        );
        let artifact = issue_168_fixture_object(&mut engine, 0, "bonesplitter", Zone::Battlefield);
        let token = issue_168_fixture_object(&mut engine, 0, "bonesplitter", Zone::Battlefield);
        engine.state.objects.get_mut(&token).unwrap().token_origin = Some(copy);
        let other = issue_168_fixture_object(&mut engine, 0, "grizzly_bears", Zone::Battlefield);
        let snapshot = engine.snapshot_zone_event();
        let generation = engine
            .state
            .zone_change_generation
            .get(&source)
            .copied()
            .unwrap_or(0);
        for oid in [source, artifact, token, other] {
            move_object_to_zone(&mut engine.state, engine.registry, oid, Zone::Exile, None)
                .unwrap();
        }
        let GameEvent::ZoneChanges(batch) = engine.finish_zone_event(snapshot) else {
            panic!("zone event")
        };
        // The physical id can change zones again before collection without changing the receipt.
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            artifact,
            Zone::Hand,
            None,
        )
        .unwrap();
        let triggers = engine.collect_zone_triggers(&batch);
        assert_eq!(
            triggers.len(),
            2,
            "Slagstone's token copy sees itself plus the other nontoken artifact"
        );
        assert!(triggers
            .iter()
            .all(|trigger| trigger.source_zone_change == generation));
        assert_eq!(
            triggers
                .iter()
                .map(|trigger| trigger.trigger_context.observed_object.unwrap().object_id)
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([source, artifact])
        );
        assert!(batch
            .moves
            .iter()
            .all(|movement| movement.destination == Zone::Exile));
    }

    #[test]
    fn issue_168_mortipede_groups_printed_graveyard_creatures_per_instruction() {
        use tricerules_cards::primitives::ZoneEventCardinality;
        let (mut engine, source) = trigger_limit_source();
        add_limited_grant(
            &mut engine,
            source,
            TriggerCondition::WheneverCardsLeaveGraveyard {
                owner: CastTriggerPlayer::Controller,
                filter: PermanentEventFilter {
                    permanent_type: Some(PermanentTypeFilter::Creature),
                    ..Default::default()
                },
                cardinality: ZoneEventCardinality::OneOrMore,
            },
        );
        let first = issue_168_fixture_object(&mut engine, 0, "grizzly_bears", Zone::Graveyard);
        let second = issue_168_fixture_object(&mut engine, 0, "hill_giant", Zone::Graveyard);
        let noncreature = issue_168_fixture_object(&mut engine, 0, "bonesplitter", Zone::Graveyard);
        // Copied/animated battlefield characteristics must not turn this artifact card into a
        // creature card in the graveyard (Mortipede and Desecrated Tomb).
        engine
            .state
            .objects
            .get_mut(&noncreature)
            .unwrap()
            .copiable_values = engine.copiable_values_for(source);
        let enemy = issue_168_fixture_object(&mut engine, 1, "grizzly_bears", Zone::Graveyard);
        let mut events = vec![];
        for cohort in [vec![first, noncreature, enemy], vec![second]] {
            let snapshot = engine.snapshot_zone_event();
            for oid in cohort {
                move_object_to_zone(&mut engine.state, engine.registry, oid, Zone::Hand, None)
                    .unwrap();
            }
            events.push(engine.finish_zone_event(snapshot));
        }
        assert_eq!(
            engine.collect_event_triggers(&events).len(),
            2,
            "two sequential instructions remain two groups in one trigger flush"
        );
        let empty = engine.finish_zone_event(engine.snapshot_zone_event());
        assert!(engine.collect_event_triggers(&[empty]).is_empty());
        // Returning only a noncreature or only an opponent's creature must not trigger.
        for oid in [noncreature, enemy] {
            move_object_to_zone(
                &mut engine.state,
                engine.registry,
                oid,
                Zone::Graveyard,
                None,
            )
            .unwrap();
            let snapshot = engine.snapshot_zone_event();
            move_object_to_zone(&mut engine.state, engine.registry, oid, Zone::Library, None)
                .unwrap();
            assert!(engine
                .collect_event_triggers(&[engine.finish_zone_event(snapshot)])
                .is_empty());
        }
    }

    #[test]
    fn issue_168_departure_controller_and_owner_are_distinct_snapshots() {
        use tricerules_cards::primitives::{EventZone, ZoneEventDestination};
        let (mut engine, source) = trigger_limit_source();
        add_limited_grant(
            &mut engine,
            source,
            TriggerCondition::WheneverPermanentLeavesBattlefield {
                controller: CastTriggerPlayer::Controller,
                filter: PermanentEventFilter {
                    owner: Some(CastTriggerPlayer::Opponent),
                    ..Default::default()
                },
                destination: ZoneEventDestination::Except(vec![EventZone::Graveyard]),
                cardinality: Default::default(),
            },
        );
        let stolen = issue_168_fixture_object(&mut engine, 1, "grizzly_bears", Zone::Battlefield);
        engine.state.players[1]
            .battlefield
            .retain(|oid| *oid != stolen);
        engine.state.players[0].battlefield.push(stolen);
        engine
            .state
            .objects
            .get_mut(&stolen)
            .unwrap()
            .base_controller = 0;
        engine.state.objects.get_mut(&stolen).unwrap().controller = 0;
        let snapshot = engine.snapshot_zone_event();
        move_object_to_zone(&mut engine.state, engine.registry, stolen, Zone::Hand, None).unwrap();
        let triggers = engine.collect_event_triggers(&[engine.finish_zone_event(snapshot)]);
        assert_eq!(triggers.len(), 1);
        assert_eq!(
            triggers[0]
                .trigger_context
                .observed_object
                .unwrap()
                .controller_at_event,
            0
        );
        assert_eq!(
            engine.state.objects[&stolen].controller, 1,
            "post-move ownership does not replace event-time control"
        );
    }

    #[test]
    fn issue_168_knightfisher_rejects_token_birds_and_noncontroller_birds() {
        let (mut engine, source) = trigger_limit_source();
        engine.state.objects.get_mut(&source).unwrap().card_id = "knightfisher".into();
        let bird = issue_168_fixture_object(&mut engine, 0, "storm_crow", Zone::Battlefield);
        let enemy = issue_168_fixture_object(&mut engine, 1, "storm_crow", Zone::Battlefield);
        let entry = |object_id| GameEvent::EntersBattlefield {
            object_id,
            chosen_x: 0,
        };
        assert_eq!(engine.collect_event_triggers(&[entry(bird)]).len(), 1);
        assert!(engine.collect_event_triggers(&[entry(enemy)]).is_empty());
        assert!(engine.collect_event_triggers(&[entry(source)]).is_empty());
        let copy = engine.copiable_values_for(bird).unwrap();
        engine.state.objects.get_mut(&bird).unwrap().token_origin = Some(copy);
        assert!(engine.collect_event_triggers(&[entry(bird)]).is_empty());
    }

    #[test]
    fn issue_168_zone_vocabulary_rejects_empty_conflicting_and_group_object_filters() {
        for trigger in [
            "WheneverPermanentLeavesBattlefield(filter: (source_only: true, exclude_source: true))",
            "WheneverPermanentLeavesBattlefield(filter: (permanent_type: Some(Land), excluded_types: [Land]))",
            "WheneverPermanentLeavesBattlefield(filter: (any_of: Some([])))",
            "WheneverPermanentLeavesBattlefield(filter: (any_subtypes: [\"\"]))",
            "WheneverPermanentLeavesBattlefield(destination: OneOf([]))",
            "WheneverPermanentLeavesBattlefield(destination: Except([]))",
            "WheneverCardsLeaveGraveyard(cardinality: OneOrMore)",
        ] {
            let data = format!(r#"(id: "probe", name: "Probe", face_id: "probe", types: ["Creature"], power: 1, toughness: 1,
                triggered_abilities: [(ability_id: "triggered_01", presentation: Fallback, trigger: {trigger}, effect: [PumpTarget(power: 1, toughness: 0, subject: TriggerObject)], )])"#);
            assert!(CardRegistry::from_chunks_and_tokens(&[&data], &[]).is_err(), "{trigger}");
        }
    }
}
