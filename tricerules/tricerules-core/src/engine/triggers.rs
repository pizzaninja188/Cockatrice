use super::damage::{DamageClassification, DamageRecipient};
use super::events::{ev_log, ev_trigger_order_required};
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
pub(super) struct CollectedTrigger {
    pub source_id: ObjectId,
    pub card_id: String,
    pub face_index: usize,
    pub source_zone_change: u64,
    pub source_face_change: u64,
    /// CR 603.3d: the ability's controller — the controller of its source permanent.
    pub controller: PlayerId,
    pub ability_index: usize,
    pub ability_origin: Option<TriggerAbilityOrigin>,
    pub ability: TriggeredAbilityDef,
    pub ability_text: String,
    /// The event's affected player ("that player"), when distinct from the ability controller.
    pub trigger_context: TriggerContext,
}

impl GameEngine {
    pub(super) fn battlefield_leave_event(&self, object_id: ObjectId) -> Option<GameEvent> {
        self.state
            .objects
            .get(&object_id)
            .is_some_and(|object| object.zone == Zone::Battlefield)
            .then(|| self.trigger_source_snapshot(object_id))
            .flatten()
            .map(|source| GameEvent::LeavesBattlefield { source })
    }

    pub(super) fn siege_defeat_trigger_active(
        &self,
        source_id: ObjectId,
        source_generation: u64,
    ) -> bool {
        let is_siege = |ability: &TriggeredAbilityDef| {
            ability
                .effect
                .iter()
                .any(|effect| matches!(effect, SpellEffectKind::SiegeDefeat))
        };
        self.state.stack.iter().any(|item| {
            item.source_permanent_id == Some(source_id)
                && item.source_zone_change == source_generation
                && item.triggered_ability.as_ref().is_some_and(&is_siege)
        }) || self.state.pending_triggers.iter().any(|trigger| {
            trigger.source_permanent_id == source_id
                && trigger.source_zone_change == source_generation
                && is_siege(&trigger.ability)
        }) || self.state.staged_trigger_groups.iter().any(|group| {
            group.triggers.iter().any(|trigger| {
                trigger.source_permanent_id == source_id
                    && trigger.source_zone_change == source_generation
                    && is_siege(&trigger.ability)
            })
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
            trigger: TriggerCondition::WhenSelfDies,
            effect: vec![SpellEffectKind::SiegeDefeat],
            modal: None,
            targeting: None,
            text: "When the last defense counter is removed from this Siege, exile it, then you may cast it transformed without paying its mana cost.".into(),
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
        self.stage_triggers(vec![CollectedTrigger {
            source_id,
            card_id,
            face_index,
            source_zone_change: source_generation,
            source_face_change,
            controller,
            ability_index: usize::MAX,
            ability_origin: None,
            ability: ability.clone(),
            ability_text: ability.text.clone(),
            trigger_context: TriggerContext::default(),
        }]);
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
            if let GameEvent::EntersBattlefield { object_id } = event {
                if !self.state.room_states.contains_key(object_id) {
                    self.emit_static_abilities_on_enter(*object_id);
                }
            }
        }

        self.record_committed_events(events);

        let mut delayed = Vec::new();
        for event in events {
            match event {
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
        collected.extend(
            delayed
                .into_iter()
                .map(|(watched, delayed)| CollectedTrigger {
                    source_id: watched.object_id,
                    card_id: delayed.card_id,
                    face_index: delayed.source_face_index,
                    source_zone_change: watched.zone_change_generation,
                    source_face_change: 0,
                    controller: delayed.controller,
                    ability_index: 0,
                    ability_origin: None,
                    ability_text: delayed.ability.text.clone(),
                    trigger_context: TriggerContext {
                        observed_object: Some(watched),
                        ..TriggerContext::default()
                    },
                    ability: delayed.ability,
                }),
        );
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
                // Rampaging Ferocidon needs the entering creature's controller, and Aether
                // Flash needs the creature itself. Reuse the generation-bound reference so
                // either instruction follows current characteristics or the correct LKI.
                if let GameEvent::EntersBattlefield { object_id } = event {
                    if matches!(
                        trigger.ability.trigger,
                        TriggerCondition::WheneverPermanentEntersBattlefield { .. }
                    ) {
                        trigger.trigger_context.observed_object =
                            self.trigger_object_ref(*object_id);
                    }
                }
            }
            collected.extend(event_triggers);
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
                StagedTrigger {
                    object_id,
                    source_permanent_id: trigger.source_id,
                    source_face_index: trigger.face_index,
                    source_zone_change: trigger.source_zone_change,
                    source_face_change: trigger.source_face_change,
                    card_id: trigger.card_id,
                    card_name,
                    controller: trigger.controller,
                    ability_index: trigger.ability_index,
                    ability: trigger.ability,
                    ability_text: trigger.ability_text,
                    trigger_context: trigger.trigger_context,
                    may,
                }
            })
            .collect();
        self.state
            .staged_trigger_groups
            .push_back(StagedTriggerGroup { triggers });
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
            GameEvent::EntersBattlefield { object_id } => {
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
                    let src_id = source.object_id;
                    let src_ctrl = source.controller;
                    out.extend(self.matching_snapshot_abilities(source, |tc| {
                        let TriggerCondition::WheneverPermanentEntersBattlefield {
                            controller,
                            permanent_type,
                            exclude_self,
                            creature_filter,
                        } = tc
                        else {
                            return false;
                        };
                        if *exclude_self && src_id == entering_id {
                            return false;
                        }
                        let rel_ok = self.relative_player_matches(
                            *controller,
                            entering_controller,
                            src_ctrl,
                        );
                        if !rel_ok {
                            return false;
                        }
                        let type_matches = match permanent_type {
                            Some(tricerules_cards::PermanentTypeFilter::Creature) => {
                                entering_characteristics.is_creature()
                            }
                            Some(tricerules_cards::PermanentTypeFilter::Artifact) => {
                                entering_characteristics.is_artifact()
                            }
                            Some(tricerules_cards::PermanentTypeFilter::Enchantment) => {
                                entering_characteristics.has_type("Enchantment")
                            }
                            Some(tricerules_cards::PermanentTypeFilter::Land) => {
                                entering_characteristics.has_type("Land")
                            }
                            Some(tricerules_cards::PermanentTypeFilter::Planeswalker) => {
                                entering_characteristics.has_type("Planeswalker")
                            }
                            Some(tricerules_cards::PermanentTypeFilter::Battle) => {
                                entering_characteristics.has_type("Battle")
                            }
                            None => true,
                        };
                        type_matches
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
                                    controller: source.controller,
                                    ability_index: prior_abilities + ability_index,
                                    ability_origin: Some(TriggerAbilityOrigin::Printed(
                                        self.ability_definition(
                                            source.object_id,
                                            *face_index,
                                            ability_index,
                                        ),
                                    )),
                                    ability: ability.clone(),
                                    ability_text: ability.text.clone(),
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
            GameEvent::LeavesBattlefield { source } => self
                .matching_snapshot_abilities(source, |condition| {
                    *condition == TriggerCondition::WhenSelfLeavesBattlefield
                }),
            GameEvent::Sacrificed {
                source: sacrificed,
                player,
            } => {
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
                            exclude_self,
                        } = condition
                        else {
                            return false;
                        };
                        (!*exclude_self
                            || source.object_id != observed.object_id
                            || source.zone_change_generation != observed.zone_change_generation)
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
                let mut out = self
                    .matching_snapshot_abilities(dying, |tc| *tc == TriggerCondition::WhenSelfDies);
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
                        let TriggerCondition::WheneverCreatureDies {
                            controller,
                            exclude_self,
                        } = tc
                        else {
                            return false;
                        };
                        if *exclude_self && source.object_id == dying.object_id {
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
                // trigger per matching ability, while distinct watched permanents each produce
                // their own trigger and retain their own event identity.
                let mut seen = HashSet::new();
                let distinct_targets: Vec<ObjectId> = targets
                    .iter()
                    .copied()
                    .filter(|target| seen.insert(*target))
                    .collect();
                let mut out = Vec::new();
                for source in sources {
                    for target_id in &distinct_targets {
                        if !self
                            .state
                            .objects
                            .get(target_id)
                            .is_some_and(|object| object.zone == Zone::Battlefield)
                        {
                            continue;
                        }
                        let Some(target_characteristics) = self.characteristics(*target_id) else {
                            continue;
                        };
                        let target_controller = target_characteristics.controller;
                        let mut matching = self.matching_snapshot_abilities(source, |condition| {
                            let Some(permanent_type) = self.target_trigger_permanent_filter(
                                condition,
                                *targeting_source,
                                *targeting_controller,
                                source.object_id,
                                source.controller,
                                *target_id,
                                target_controller,
                            ) else {
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
                                self.trigger_object_ref(*target_id);
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

    fn relative_player_matches(
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
                controller,
                ability_index: idx,
                ability_origin: Some(origin),
                ability: ta.clone(),
                ability_text: ta.text.clone(),
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
        ability_index: usize,
    ) -> AbilityDefinitionId {
        let object = &self.state.objects[&source_id];
        let values = object
            .copiable_values
            .as_ref()
            .or(object.token_origin.as_ref());
        AbilityDefinitionId {
            card_id: values
                .filter(|v| !v.source_card_id.is_empty())
                .map(|v| v.source_card_id.clone())
                .unwrap_or_else(|| object.card_id.clone()),
            face_index: values
                .filter(|v| v.room_faces.is_none())
                .map(|v| v.source_face_index)
                .unwrap_or(face_index),
            ability_index,
        }
    }

    fn effective_triggered_abilities(
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
                    for (slot, ability) in faces[door].triggered_abilities.iter().enumerate() {
                        printed.push((
                            ability.clone(),
                            self.ability_definition(source_id, door, slot),
                        ));
                    }
                }
            } else if let Some(face) = self.effective_face(source_id) {
                for (slot, ability) in face.triggered_abilities.iter().enumerate() {
                    printed.push((
                        ability.clone(),
                        self.ability_definition(source_id, face_index, slot),
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

    fn matching_snapshot_abilities(
        &self,
        source: &TriggerSourceSnapshot,
        filter: impl Fn(&TriggerCondition) -> bool,
    ) -> Vec<CollectedTrigger> {
        source
            .triggered_abilities
            .iter()
            .filter(|(_, ability, _)| filter(&ability.trigger))
            .filter(|(_, ability, _)| {
                source.event_conditions_checked
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
                controller: source.controller,
                ability_index: *ability_index,
                ability_origin: Some(origin.clone()),
                ability: ability.clone(),
                ability_text: ability.text.clone(),
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
        clause: Option<&InterveningIf>,
    ) -> bool {
        self.intervening_if_holds_at_generation(source_id, controller, clause, None)
    }

    pub(super) fn intervening_if_holds_at_generation(
        &self,
        source_id: ObjectId,
        controller: PlayerId,
        clause: Option<&InterveningIf>,
        source_generation: Option<u64>,
    ) -> bool {
        match clause {
            None => true,
            Some(InterveningIf::SourceUntapped) | Some(InterveningIf::SourceTapped) => {
                let requires_tapped = matches!(clause, Some(InterveningIf::SourceTapped));
                let source_is_tapped = match self.state.objects.get(&source_id) {
                    Some(o)
                        if o.zone == Zone::Battlefield
                            && source_generation.is_none_or(|generation| {
                                self.state
                                    .zone_change_generation
                                    .get(&source_id)
                                    .copied()
                                    .unwrap_or(0)
                                    == generation
                            }) =>
                    {
                        o.tapped
                    }
                    // CR 608.2h / 113.7a: if the source left after triggering, evaluate the
                    // intervening condition from generation-scoped last known tap status. Reading
                    // the live object would confuse a returned object and CR 400.7's tap reset.
                    _ => source_generation
                        .and_then(|generation| {
                            self.state
                                .last_known_tapped_by_generation
                                .get(&(source_id, generation))
                                .copied()
                        })
                        .or_else(|| self.state.last_known_tapped.get(&source_id).copied())
                        .unwrap_or(false),
                };
                source_is_tapped == requires_tapped
            }
            Some(InterveningIf::SpellsCastLastTurn { min, max }) => {
                let count = self.state.turn_history.previous.spells_cast;
                min.is_none_or(|minimum| count >= minimum)
                    && max.is_none_or(|maximum| count <= maximum)
            }
            Some(InterveningIf::GameCondition(condition)) => self.condition_holds(
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
            ),
        }
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
            GameEvent::CrimeCommitted { player } => Some(*player),
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
            source_face_index,
            source_zone_change,
            source_face_change,
            card_id,
            card_name,
            controller,
            ability_index,
            ability,
            ability_text,
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
                        label: mode.label.clone(),
                        selectable,
                        needs_target: mode_needs_target,
                        targets: Some(targets),
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
                source_face_index,
                source_zone_change,
                source_face_change,
                ability_index,
                ability,
                ability_text: ability_text.clone(),
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
            self.state.stack.push(StackItem {
                id: virtual_id,
                controller,
                card_id,
                targets: vec![],
                ability_text: Some(ability_text.clone()),
                source_permanent_id: Some(source_id),
                source_zone_change,
                source_face_change,
                ability_index: Some(ability_index),
                activated_ability: None,
                triggered_ability: Some(ability),
                is_triggered: true,
                is_copy: false,
                chosen_x: 0,
                face_index: source_face_index,
                chosen_modes: vec![],
                cast_condition_results: Vec::new(),
                cast_occurrence: None,
                cast_cost_receipts: vec![],
                payment_result: CardResultCohort::default(),
                resolution_branch_choices: Default::default(),
                trigger_context,
                cast_method: SpellCastMethod::Normal,
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
                })),
            });
            events.push(ev_log(format!("Triggered: {card_name} — {ability_text}")));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_ability_origin(index: usize) -> TriggerAbilityOrigin {
        TriggerAbilityOrigin::Printed(AbilityDefinitionId {
            card_id: "test_trigger_source".into(),
            face_index: 0,
            ability_index: index,
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
        install_trigger_face(&mut engine, source, vec![ability.clone(), ability.clone()]);
        let values = engine.state.objects[&source]
            .copiable_values
            .clone()
            .unwrap();
        stage_limited_dies_grants(&mut engine, source);
        assert_eq!(
            take_staged_count(&mut engine),
            2,
            "identical printed slots are independent"
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
        let mut values = engine.copiable_values_for(source).unwrap();
        values.face.static_abilities = vec![StaticAbilityDef::ConditionalSelfModifier {
            condition: GameCondition::ActivePlayer {
                players: RelativePlayerSet::Controller,
            },
            delta_power: 0,
            delta_toughness: 0,
            keywords: vec![],
            triggered_abilities: vec![ability.clone(), ability],
            can_attack_as_though_without_defender: false,
        }];
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
            types: vec!["Creature".into()],
            power_toughness: (None, None),
            event_conditions_checked: false,
            object_id: 100,
            card_id: "wandertale_mentor".into(),
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
            targets: vec![giants[0], giants[0], giants[1]],
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
            types: vec!["Enchantment".into()],
            power_toughness: (None, None),
            event_conditions_checked: false,
            object_id: 100,
            card_id: "curse_of_disturbance".into(),
            controller: 0,
            face_index: 0,
            zone_change_generation: 0,
            face_change_generation: 0,
            attached_to: Some(AttachmentSnapshot::Player(1)),
            triggered_abilities: vec![(
                0,
                TriggeredAbilityDef {
                    trigger: TriggerCondition::WheneverAttachedPlayerIsAttacked,
                    effect: vec![SpellEffectKind::Draw {
                        who: PlayerRecipient::Controller,
                        count: Amount::Fixed(1),
                    }],
                    modal: None,
                    targeting: None,
                    text: "Whenever enchanted player is attacked, draw a card.".into(),
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
            },
            GameEvent::EntersBattlefield {
                object_id: wardens[1],
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
}
