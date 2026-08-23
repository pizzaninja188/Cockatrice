use super::damage::{DamageClassification, DamageRecipient};
use super::events::{ev_log, ev_trigger_order_required};
use super::targeting::{compute_ability_targets_with_context, target_schema, TargetSourceIdentity};
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
    pub ability: TriggeredAbilityDef,
    pub ability_text: String,
    /// The event's affected player ("that player"), when distinct from the ability controller.
    pub trigger_context: TriggerContext,
}

impl GameEngine {
    /// Collect one simultaneous event set and enqueue all matching triggered abilities as one
    /// CR 603.3b group.
    pub(super) fn fire_triggers(&mut self, events: &[GameEvent]) {
        if events.is_empty() {
            return;
        }

        self.record_committed_events(events);

        // All permanents in a simultaneous ETB set exist before any trigger check. Register every
        // static ability first so characteristics and trigger conditions see the completed event.
        for event in events {
            if let GameEvent::EntersBattlefield { object_id } = event {
                if !self.state.room_states.contains_key(object_id) {
                    self.emit_static_abilities_on_enter(*object_id);
                }
            }
        }

        let mut delayed = if events.iter().any(|event| {
            matches!(
                event,
                GameEvent::PhaseBegan {
                    phase: rv1::PhaseId::EndStep,
                    ..
                }
            )
        }) {
            self.state.take_next_end_step_delayed()
        } else {
            Vec::new()
        };
        let mut waiting = std::mem::take(&mut self.state.active_delayed_triggers);
        for candidate in waiting.drain(..) {
            let matched = candidate.ability.trigger
                == TriggerCondition::WhenWatchedObjectDiesThisTurn
                && events.iter().any(|event| match event {
                    GameEvent::Dies { source, .. } => {
                        source.object_id == candidate.watched.object_id
                            && source.zone_change_generation
                                == candidate.watched.zone_change_generation
                    }
                    _ => false,
                });
            if matched {
                delayed.push(candidate);
            } else {
                self.state.active_delayed_triggers.push(candidate);
            }
        }
        let mut collected = self.collect_event_triggers(events);
        collected.extend(delayed.into_iter().map(|delayed| CollectedTrigger {
            source_id: delayed.watched.object_id,
            card_id: delayed.card_id,
            face_index: delayed.source_face_index,
            source_zone_change: delayed.watched.zone_change_generation,
            source_face_change: 0,
            controller: delayed.controller,
            ability_index: 0,
            ability_text: delayed.ability.text.clone(),
            trigger_context: TriggerContext {
                observed_object: Some(delayed.watched),
                ..TriggerContext::default()
            },
            ability: delayed.ability,
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
        sources.extend(events.iter().filter_map(|event| match event {
            GameEvent::Dies { source, .. } => Some(source.clone()),
            _ => None,
        }));

        let mut collected = Vec::new();
        for event in events {
            let trigger_player = Self::trigger_player_for(event);
            let mut event_triggers = self.collect_triggers(event, &sources);
            for trigger in &mut event_triggers {
                trigger.trigger_context.affected_player = trigger_player;
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
            !trigger.ability.triggers_only_once
                || self.state.triggered_once.insert(TriggeredOnceKey {
                    object_id: trigger.source_id,
                    zone_change_generation: trigger.source_zone_change,
                    card_id: trigger.card_id.clone(),
                    face_index: trigger.face_index,
                    ability_index: trigger.ability_index,
                })
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
    fn battlefield_sources_apnap(&self) -> Vec<TriggerSourceSnapshot> {
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
            GameEvent::SpellCast {
                caster,
                card_id: cast_card_id,
                ordinal,
                face_index,
            } => {
                // CR 709.4/712.4: a spell on the stack has the characteristics of the cast face.
                let cast_face = self
                    .registry
                    .get(cast_card_id)
                    .and_then(|d| d.face(*face_index));

                sources
                    .iter()
                    .flat_map(|source| {
                        self.matching_snapshot_abilities(source, |tc| {
                            let TriggerCondition::WheneverPlayerCastsSpell {
                                caster: caster_filter,
                                spell_type,
                                ordinal: trigger_ordinal,
                            } = tc
                            else {
                                return false;
                            };
                            let caster_ok = self.relative_player_matches(
                                *caster_filter,
                                *caster,
                                source.controller,
                            );
                            if !caster_ok {
                                return false;
                            }
                            if trigger_ordinal.is_some_and(|expected| expected != *ordinal) {
                                return false;
                            }
                            match spell_type {
                                None => true,
                                Some(filter) => {
                                    cast_face.is_some_and(|face| face.matches_card_type(*filter))
                                }
                            }
                        })
                    })
                    .collect()
            }
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
            .filter(|(_, ta)| filter(&ta.trigger))
            // CR 603.4, first of the two checks: an intervening-"if" clause that is false as the
            // ability would go on the stack means it never triggers at all.
            .filter(|(_, ta)| {
                self.intervening_if_holds(source_id, controller, ta.intervening_if.as_ref())
            })
            .map(|(idx, ta)| CollectedTrigger {
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
        let controller = self
            .characteristics(source_id)
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

    fn effective_triggered_abilities(
        &self,
        source_id: ObjectId,
        _card_id: &str,
        _face_index: usize,
    ) -> Vec<(usize, TriggeredAbilityDef)> {
        let face_down = self
            .state
            .objects
            .get(&source_id)
            .is_some_and(|object| object.face_down);
        let removed_at =
            super::characteristics::latest_remove_all_abilities_timestamp(&self.state, source_id);
        let mut abilities: Vec<(usize, TriggeredAbilityDef)> = (!face_down && removed_at.is_none())
            .then(|| self.effective_face(source_id))
            .flatten()
            .map(|face| {
                face.triggered_abilities
                    .iter()
                    .cloned()
                    .enumerate()
                    .collect()
            })
            .unwrap_or_default();
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
            ) {
                abilities.push((next_index, (**ability).clone()));
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
            .filter(|(_, ability)| filter(&ability.trigger))
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
                face_index: source.face_index,
                source_zone_change: source.zone_change_generation,
                source_face_change: source.face_change_generation,
                controller: source.controller,
                ability_index: *ability_index,
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
            GameEvent::Surveilled { player } => Some(*player),
            GameEvent::CardDrawn { drawer, .. } => Some(*drawer),
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
                        || targets.groups.iter().all(|group| {
                            let players = u32::from(group.can_target_self)
                                + u32::from(group.can_target_opponent);
                            group.min
                                <= group.valid_permanent_ids.len() as u32
                                    + group.valid_stack_ids.len() as u32
                                    + group.valid_graveyard_ids.len() as u32
                                    + players
                        });
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
            targets.groups.iter().all(|group| {
                let player_count =
                    u32::from(group.can_target_self) + u32::from(group.can_target_opponent);
                group.min
                    <= group.valid_permanent_ids.len() as u32
                        + group.valid_stack_ids.len() as u32
                        + group.valid_graveyard_ids.len() as u32
                        + player_count
            })
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
                resolution_branch_choices: Default::default(),
                trigger_context,
                flashback: false,
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
                })),
            });
            events.push(ev_log(format!("Triggered: {card_name} — {ability_text}")));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
                        count: Amount::Fixed(1),
                    }],
                    modal: None,
                    targeting: None,
                    text: "Whenever enchanted player is attacked, draw a card.".into(),
                    may: false,
                    intervening_if: None,
                    triggers_only_once: false,
                },
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
                    defending_player: 1,
                },
                AttackEdgeSnapshot {
                    attacker: attacker(201),
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
