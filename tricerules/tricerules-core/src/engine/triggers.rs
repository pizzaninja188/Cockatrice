use super::events::{ev_log, ev_trigger_order_required};
use super::targeting::{compute_spell_targets, spell_effect_kind_needs_target};
use super::*;

/// One triggered ability that matched an event and is about to go on the stack (or be parked for
/// target selection) — the unit a trigger scan yields and [`GameEngine::push_trigger`] consumes.
///
/// Identifies the ability by *source permanent + index*, never by a resolved effect: the effect is
/// looked up from the registry at push and again at resolution, so a trigger surviving on the
/// stack after its source leaves still knows what it does.
pub(super) struct CollectedTrigger {
    pub source_id: ObjectId,
    pub card_id: String,
    /// CR 603.3d: the ability's controller — the controller of its source permanent.
    pub controller: PlayerId,
    pub ability_index: usize,
    pub ability_text: String,
}

impl GameEngine {
    /// Emit a game event and enqueue all matching triggered abilities (CR 603.3b APNAP order).
    pub(super) fn fire_triggers(&mut self, event: GameEvent, events: &mut Vec<rv1::RuledEvent>) {
        if let GameEvent::EntersBattlefield { object_id } = &event {
            self.emit_static_abilities_on_enter(*object_id);
        }
        // Every death goes through the simultaneous-death path, so a single death and a board
        // wipe share one implementation of who observes it (CR 603.6/603.10). Keeping a second
        // copy here is how the two drifted apart before.
        if let GameEvent::Dies {
            object_id,
            card_id,
            controller,
            was_creature,
        } = event
        {
            self.fire_dies_batch(&[(object_id, card_id, controller, was_creature)], events);
            return;
        }
        // The beneficiary is a property of the *event* ("that player"), not of the ability that
        // matched it, which is why it rides alongside the trigger rather than inside it.
        let trigger_player = Self::trigger_player_for(&event);
        let collected = self.collect_triggers(&event);
        self.stage_triggers(collected, trigger_player);
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
    pub(super) fn stage_triggers(
        &mut self,
        mut collected: Vec<CollectedTrigger>,
        trigger_player: Option<PlayerId>,
    ) {
        if collected.is_empty() {
            return;
        }
        collected.sort_by_key(|trigger| self.state.apnap_rank(trigger.controller));
        let triggers = collected
            .into_iter()
            .map(|trigger| {
                let object_id = self.state.next_object_id;
                self.state.next_object_id += 1;
                let def = self.registry.get(&trigger.card_id);
                let card_name = def.map(|d| d.name.clone()).unwrap_or_default();
                let may = def
                    .and_then(|d| {
                        d.primary_face()
                            .triggered_abilities
                            .get(trigger.ability_index)
                    })
                    .map(|ta| ta.may)
                    .unwrap_or(false);
                StagedTrigger {
                    object_id,
                    source_permanent_id: trigger.source_id,
                    card_id: trigger.card_id,
                    card_name,
                    controller: trigger.controller,
                    ability_index: trigger.ability_index,
                    ability_text: trigger.ability_text,
                    trigger_player,
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

    /// Emit a simultaneous group of creature deaths using the battlefield snapshot immediately
    /// before the group left. This is required for CR 603.6/603.10: a Blood Artist that dies in a
    /// wipe still observes every other creature dying in that same event, and all those triggers
    /// are ordered together under APNAP rather than one event at a time after the battlefield has
    /// already been cleared.
    pub(super) fn fire_dies_batch(
        &mut self,
        destroyed: &[(ObjectId, String, PlayerId, bool)],
        events: &mut Vec<rv1::RuledEvent>,
    ) {
        let mut sources = self.battlefield_sources_apnap();
        // The destroyed objects are no longer in the battlefield index, but their last-known
        // triggered abilities still participate in the simultaneous event.
        sources.extend(
            destroyed
                .iter()
                .map(|(id, card_id, controller, _)| (*id, card_id.clone(), *controller)),
        );

        let mut triggers = Vec::new();
        for (dying_id, dying_card_id, dying_controller, was_creature) in destroyed {
            triggers.extend(self.matching_triggered_abilities(
                dying_card_id,
                *dying_id,
                *dying_controller,
                |tc| *tc == TriggerCondition::WhenSelfDies,
            ));
            if !was_creature {
                continue;
            }
            for (source_id, source_card, source_controller) in &sources {
                triggers.extend(self.matching_triggered_abilities(
                    source_card,
                    *source_id,
                    *source_controller,
                    |tc| {
                        let TriggerCondition::WheneverCreatureDies {
                            controller,
                            exclude_self,
                        } = tc
                        else {
                            return false;
                        };
                        if *exclude_self && *source_id == *dying_id {
                            return false;
                        }
                        match controller {
                            CastTriggerPlayer::Controller => {
                                *dying_controller == *source_controller
                            }
                            CastTriggerPlayer::Opponent => *dying_controller != *source_controller,
                            CastTriggerPlayer::AnyPlayer => true,
                        }
                    },
                ));
            }
        }

        // `events` is retained on the signature (and on `fire_triggers`) only so the ~15 call sites
        // keep their shape: since staging replaced immediate pushing, no trigger event is emitted
        // until `flush_staged_triggers` runs. Dropping the parameter is part of issue #41's merge
        // of the two firing paths, not of this change.
        let _ = events;
        self.stage_triggers(triggers, None);
    }

    /// Every permanent on every battlefield as `(object_id, card_id, controller)`, in APNAP order
    /// (CR 603.3b / 101.4): the active player's permanents first, then each nonactive player's.
    /// Triggers collected in this order go on the stack in the order the rules require.
    ///
    /// The single source of this ordering. Five event arms need it, and the one that rolled its
    /// own — `UpkeepBegin` — scanned only the active player's battlefield, so a nonactive
    /// player's upkeep trigger never fired at all (CR 603.2: *every* permanent observes the
    /// event). `sort_by_key` is stable, so each player's battlefield keeps its own order.
    fn battlefield_sources_apnap(&self) -> Vec<(ObjectId, String, PlayerId)> {
        let mut ordered: Vec<usize> = (0..self.state.players.len()).collect();
        ordered.sort_by_key(|&i| self.state.apnap_rank(self.state.players[i].id));
        let mut sources = Vec::new();
        for pi in ordered {
            for &source_id in &self.state.players[pi].battlefield {
                if let Some(object) = self.state.objects.get(&source_id) {
                    sources.push((source_id, object.card_id.clone(), object.controller));
                }
            }
        }
        sources
    }

    /// Collect every triggered ability whose condition matches `event`, ordered APNAP (CR 603.3b).
    pub(super) fn collect_triggers(&self, event: &GameEvent) -> Vec<CollectedTrigger> {
        match event {
            GameEvent::EntersBattlefield { object_id } => {
                let Some(obj) = self.state.objects.get(object_id) else {
                    return vec![];
                };
                let entering_id = *object_id;
                let entering_card_id = obj.card_id.clone();
                let Some(entering_characteristics) = self.characteristics(entering_id) else {
                    return vec![];
                };
                let entering_controller = entering_characteristics.controller;

                let mut out = Vec::new();
                out.extend(self.matching_triggered_abilities(
                    &entering_card_id,
                    entering_id,
                    entering_controller,
                    |tc| *tc == TriggerCondition::WhenSelfEntersBattlefield,
                ));

                for (src_id, src_card, src_ctrl) in self.battlefield_sources_apnap() {
                    out.extend(self.matching_triggered_abilities(
                        &src_card,
                        src_id,
                        src_ctrl,
                        |tc| {
                            let TriggerCondition::WheneverPermanentEntersBattlefield {
                                controller,
                                permanent_type,
                                exclude_self,
                            } = tc
                            else {
                                return false;
                            };
                            if *exclude_self && src_id == entering_id {
                                return false;
                            }
                            let rel_ok = match controller {
                                tricerules_cards::CastTriggerPlayer::Controller => {
                                    entering_controller == src_ctrl
                                }
                                tricerules_cards::CastTriggerPlayer::Opponent => {
                                    entering_controller != src_ctrl
                                }
                                tricerules_cards::CastTriggerPlayer::AnyPlayer => true,
                            };
                            if !rel_ok {
                                return false;
                            }
                            match permanent_type {
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
                            }
                        },
                    ));
                }
                out
            }
            // Deaths never reach here: `fire_triggers` routes them to `fire_dies_batch`, which is
            // the only place that knows how to treat a group of deaths as simultaneous.
            GameEvent::Dies { .. } => vec![],
            GameEvent::Attacks { attacker_ids } => {
                let mut sorted = attacker_ids.clone();
                // CR 603.3b APNAP: the active player's triggers go on the stack first. A trigger
                // belongs to the seat that *controls* its source, not the seat that owns the card.
                // `stage_triggers` sorts by the same rank, but attacker order within one controller
                // is decided here, so the two must agree.
                sorted.sort_by_key(|&oid| {
                    self.state
                        .objects
                        .get(&oid)
                        .map(|o| self.state.apnap_rank(o.controller))
                        .unwrap_or(usize::MAX)
                });
                sorted
                    .iter()
                    .flat_map(|&att| {
                        let Some(obj) = self.state.objects.get(&att) else {
                            return vec![];
                        };
                        let card_id = obj.card_id.clone();
                        let controller = obj.controller;
                        self.matching_triggered_abilities(&card_id, att, controller, |tc| {
                            *tc == TriggerCondition::WheneverSelfAttacks
                        })
                    })
                    .collect()
            }
            GameEvent::CombatDamageToPlayer {
                attacker_id,
                defender_id,
            } => {
                let Some(obj) = self.state.objects.get(attacker_id) else {
                    return vec![];
                };
                let card_id = obj.card_id.clone();
                let controller = obj.controller;
                let defender = *defender_id;
                self.matching_triggered_abilities(&card_id, *attacker_id, controller, |tc| match tc
                {
                    TriggerCondition::WheneverSelfDealsCombatDamageToPlayer => true,
                    TriggerCondition::WheneverSelfDealsDamageToOpponent => defender != controller,
                    _ => false,
                })
            }
            GameEvent::UpkeepBegin { player: upkeep } => {
                // CR 603.2: every player's permanents watch, not just the active player's — a
                // Sulfuric Vortex under a nonactive player's control still triggers on the
                // active player's upkeep. Walked in APNAP order (CR 603.3b); all of them land on
                // the stack before the active player gets priority (CR 503.1a).
                self.battlefield_sources_apnap()
                    .into_iter()
                    .flat_map(|(oid, card_id, source_controller)| {
                        self.matching_triggered_abilities(&card_id, oid, source_controller, |tc| {
                            let TriggerCondition::AtBeginningOfUpkeep {
                                player: player_filter,
                            } = tc
                            else {
                                return false;
                            };
                            match player_filter {
                                CastTriggerPlayer::Controller => *upkeep == source_controller,
                                CastTriggerPlayer::Opponent => *upkeep != source_controller,
                                CastTriggerPlayer::AnyPlayer => true,
                            }
                        })
                    })
                    .collect()
            }
            GameEvent::DrawStepBegin { player: drawing } => {
                // Every player's permanents watch (CR 603.2) — Howling Mine triggers on the
                // opponent's draw step too — walked in APNAP order (CR 603.3b) via
                // `battlefield_sources_apnap`, as for `LifeGained` and `UpkeepBegin`.
                self.battlefield_sources_apnap()
                    .into_iter()
                    .flat_map(|(oid, card_id, source_controller)| {
                        self.matching_triggered_abilities(&card_id, oid, source_controller, |tc| {
                            let TriggerCondition::AtBeginningOfDrawStep {
                                player: player_filter,
                            } = tc
                            else {
                                return false;
                            };
                            match player_filter {
                                CastTriggerPlayer::Controller => *drawing == source_controller,
                                CastTriggerPlayer::Opponent => *drawing != source_controller,
                                CastTriggerPlayer::AnyPlayer => true,
                            }
                        })
                    })
                    .collect()
            }
            GameEvent::LifeGained { player: gaining } => {
                // Every player's permanents watch, in APNAP order (CR 603.3b) — the amount is
                // irrelevant, one gain event fires each matching ability once.
                self.battlefield_sources_apnap()
                    .into_iter()
                    .flat_map(|(oid, card_id, source_controller)| {
                        self.matching_triggered_abilities(&card_id, oid, source_controller, |tc| {
                            let TriggerCondition::WheneverPlayerGainsLife {
                                player: player_filter,
                            } = tc
                            else {
                                return false;
                            };
                            match player_filter {
                                CastTriggerPlayer::Controller => *gaining == source_controller,
                                CastTriggerPlayer::Opponent => *gaining != source_controller,
                                CastTriggerPlayer::AnyPlayer => true,
                            }
                        })
                    })
                    .collect()
            }
            GameEvent::SpellCast {
                caster,
                card_id: cast_card_id,
                face_index,
            } => {
                // CR 709.4/712.4: a spell on the stack has the characteristics of the cast face.
                let cast_face = self
                    .registry
                    .get(cast_card_id)
                    .and_then(|d| d.face(*face_index));
                let is_enchantment = cast_face.is_some_and(|f| f.is_enchantment);
                let is_instant = cast_face.is_some_and(|f| f.is_instant);
                let is_sorcery = cast_face.is_some_and(|f| f.is_sorcery);
                let is_creature = cast_face.is_some_and(|f| f.is_creature);
                let is_artifact = cast_face.is_some_and(|f| f.is_artifact);

                self.battlefield_sources_apnap()
                    .into_iter()
                    .flat_map(|(oid, card_id, source_controller)| {
                        self.matching_triggered_abilities(&card_id, oid, source_controller, |tc| {
                            let TriggerCondition::WheneverPlayerCastsSpell {
                                caster: caster_filter,
                                spell_type,
                            } = tc
                            else {
                                return false;
                            };
                            let caster_ok = match caster_filter {
                                CastTriggerPlayer::Controller => *caster == source_controller,
                                CastTriggerPlayer::Opponent => *caster != source_controller,
                                CastTriggerPlayer::AnyPlayer => true,
                            };
                            if !caster_ok {
                                return false;
                            }
                            match spell_type {
                                None => true,
                                Some(SpellTypeFilter::Enchantment) => is_enchantment,
                                Some(SpellTypeFilter::Instant) => is_instant,
                                Some(SpellTypeFilter::Sorcery) => is_sorcery,
                                Some(SpellTypeFilter::InstantOrSorcery) => is_instant || is_sorcery,
                                Some(SpellTypeFilter::Creature) => is_creature,
                                Some(SpellTypeFilter::Artifact) => is_artifact,
                                Some(SpellTypeFilter::Noncreature) => !is_creature,
                            }
                        })
                    })
                    .collect()
            }
        }
    }

    pub(super) fn matching_triggered_abilities(
        &self,
        card_id: &str,
        source_id: ObjectId,
        controller: PlayerId,
        filter: impl Fn(&TriggerCondition) -> bool,
    ) -> Vec<CollectedTrigger> {
        let Some(def) = self.registry.get(card_id) else {
            return vec![];
        };
        // Triggered-ability indices are face-0-relative everywhere (`StackItem::face_index` is `0`
        // for abilities); the back face of a transforming permanent granting its own triggers is
        // the point at which this must read the source object's active face instead.
        def.primary_face()
            .triggered_abilities
            .iter()
            .enumerate()
            .filter(|(_, ta)| filter(&ta.trigger))
            // CR 603.4, first of the two checks: an intervening-"if" clause that is false as the
            // ability would go on the stack means it never triggers at all.
            .filter(|(_, ta)| self.intervening_if_holds(source_id, ta.intervening_if))
            .map(|(idx, ta)| CollectedTrigger {
                source_id,
                card_id: card_id.to_string(),
                controller,
                ability_index: idx,
                ability_text: ta.text.clone(),
            })
            .collect()
    }

    /// CR 603.4: evaluate a triggered ability's intervening-"if" clause against the current state.
    /// `None` (no clause) always holds. Called once when the trigger would go on the stack and
    /// again when it resolves — both checks read live state, which is the whole point of the rule.
    pub(super) fn intervening_if_holds(
        &self,
        source_id: ObjectId,
        clause: Option<InterveningIf>,
    ) -> bool {
        self.intervening_if_holds_at_generation(source_id, clause, None)
    }

    pub(super) fn intervening_if_holds_at_generation(
        &self,
        source_id: ObjectId,
        clause: Option<InterveningIf>,
        source_generation: Option<u64>,
    ) -> bool {
        match clause {
            None => true,
            Some(InterveningIf::SourceUntapped) => match self.state.objects.get(&source_id) {
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
                    // "if {this} is untapped" (Howling Mine), read live while the same object
                    // is still in play.
                    !o.tapped
                }
                // CR 608.2h / 113.7a: the source has left the battlefield since it triggered, so
                // the check uses its *last known* tap status — bounced or destroyed while untapped,
                // the ability still resolves (Howling Mine ruling, 2004-10-04). Reading the live
                // object would be wrong twice over: CR 400.7 resets `tapped` on the way out, so a
                // permanent that left tapped would read as untapped.
                _ => {
                    let lki = source_generation
                        .and_then(|generation| {
                            self.state
                                .last_known_tapped_by_generation
                                .get(&(source_id, generation))
                                .copied()
                        })
                        .or_else(|| self.state.last_known_tapped.get(&source_id).copied())
                        .unwrap_or(false);
                    !lki
                }
            },
        }
    }

    /// The player a trigger's effects act on when the trigger names a player other than its
    /// controller ("**that player** draws an additional card"). `None` means "the ability's
    /// controller", which is every other trigger today. Stored on the stack item so the
    /// beneficiary survives the trip through the stack and any responses.
    fn trigger_player_for(event: &GameEvent) -> Option<PlayerId> {
        match event {
            GameEvent::UpkeepBegin { player } | GameEvent::DrawStepBegin { player } => {
                Some(*player)
            }
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
            card_id,
            card_name,
            controller,
            ability_index,
            ability_text,
            trigger_player,
            may,
        } = trigger;
        let def = match self.registry.get(&card_id) {
            Some(d) => d.clone(),
            None => return,
        };
        let trigger_def = def.primary_face().triggered_abilities.get(ability_index);
        let needs_target = trigger_def
            .map(|ta| ta.effect.iter().any(spell_effect_kind_needs_target))
            .unwrap_or(false);
        let has_legal_target = if needs_target {
            trigger_def.is_some_and(|ta| {
                let targets = compute_spell_targets(self, controller, &ta.effect);
                !targets.valid_permanent_ids.is_empty()
                    || !targets.valid_stack_ids.is_empty()
                    || !targets.valid_graveyard_ids.is_empty()
                    || targets.can_target_self
                    || targets.can_target_opponent
            })
        } else {
            true
        };

        if needs_target && (has_legal_target || may) {
            let was_empty = self.state.pending_triggers.is_empty();
            self.state.pending_triggers.push_back(PendingTrigger {
                object_id: virtual_id,
                source_permanent_id: source_id,
                ability_index,
                ability_text: ability_text.clone(),
                card_id,
                controller,
                trigger_player,
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
                        },
                    )),
                });
                events.push(ev_log(format!(
                    "Triggered: {card_name} — choose a target for: {ability_text}"
                )));
            }
        } else if needs_target {
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
                source_zone_change: self
                    .state
                    .zone_change_generation
                    .get(&source_id)
                    .copied()
                    .unwrap_or(0),
                ability_index: Some(ability_index),
                is_triggered: true,
                is_copy: false,
                chosen_x: 0,
                face_index: 0,
                target_damage: vec![],
                chosen_modes: vec![],
                trigger_player,
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
