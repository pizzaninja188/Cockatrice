use super::events::ev_log;
use super::targeting::spell_effect_kind_needs_target;
use super::*;

impl GameEngine {
    /// Emit a game event and enqueue all matching triggered abilities (CR 603.3b APNAP order).
    pub(super) fn fire_triggers(&mut self, event: GameEvent, events: &mut Vec<rv1::RuledEvent>) {
        if let GameEvent::EntersBattlefield { object_id } = &event {
            self.emit_static_abilities_on_enter(*object_id);
        }
        let triggers = self.collect_triggers(&event);
        for (source_id, card_id, controller, ability_index, ability_text) in triggers {
            self.push_trigger(
                source_id,
                &card_id,
                controller,
                ability_index,
                ability_text,
                events,
            );
        }
    }

    /// Collect `(source_id, card_id, controller, ability_index, ability_text)` for every triggered
    /// ability whose condition matches `event`. Results are ordered APNAP (CR 603.3b).
    pub(super) fn collect_triggers(
        &self,
        event: &GameEvent,
    ) -> Vec<(ObjectId, String, PlayerId, usize, String)> {
        let ap = self.state.active_player_id();
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

                let mut ordered: Vec<usize> = (0..self.state.players.len()).collect();
                ordered.sort_by_key(|&i| (self.state.players[i].id != ap) as u8);
                let mut sources: Vec<(ObjectId, String, PlayerId)> = Vec::new();
                for pi in ordered {
                    for &sid in &self.state.players[pi].battlefield {
                        if let Some(o) = self.state.objects.get(&sid) {
                            sources.push((sid, o.card_id.clone(), o.owner));
                        }
                    }
                }
                for (src_id, src_card, src_ctrl) in sources {
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
            GameEvent::Dies {
                object_id: dying_id,
                card_id: dying_card_id,
                controller: dying_controller,
                was_creature,
            } => {
                let mut out = self.matching_triggered_abilities(
                    dying_card_id,
                    *dying_id,
                    *dying_controller,
                    |tc| *tc == TriggerCondition::WhenSelfDies,
                );
                // Observer triggers: check all battlefield permanents for WheneverCreatureDies.
                let ap = self.state.active_player_id();
                let mut ordered: Vec<usize> = (0..self.state.players.len()).collect();
                ordered.sort_by_key(|&i| (self.state.players[i].id != ap) as u8);
                let mut sources: Vec<(ObjectId, String, PlayerId)> = Vec::new();
                for pi in ordered {
                    for &sid in &self.state.players[pi].battlefield {
                        if let Some(o) = self.state.objects.get(&sid) {
                            sources.push((sid, o.card_id.clone(), o.owner));
                        }
                    }
                }
                if *was_creature {
                    // Check the dying creature itself for WheneverCreatureDies (exclude_self: false).
                    // It has already left the battlefield, so it won't appear in `sources`, but its
                    // card definition is still in the registry — same path as WhenSelfDies.
                    out.extend(self.matching_triggered_abilities(
                        dying_card_id,
                        *dying_id,
                        *dying_controller,
                        |tc| {
                            matches!(
                                tc,
                                TriggerCondition::WheneverCreatureDies {
                                    exclude_self: false,
                                    ..
                                }
                            )
                        },
                    ));
                    // Check all remaining battlefield permanents (observer triggers).
                    for (src_id, src_card, src_ctrl) in sources {
                        out.extend(self.matching_triggered_abilities(
                            &src_card,
                            src_id,
                            src_ctrl,
                            |tc| {
                                let TriggerCondition::WheneverCreatureDies {
                                    controller,
                                    exclude_self,
                                } = tc
                                else {
                                    return false;
                                };
                                if *exclude_self && src_id == *dying_id {
                                    return false;
                                }
                                match controller {
                                    CastTriggerPlayer::Controller => *dying_controller == src_ctrl,
                                    CastTriggerPlayer::Opponent => *dying_controller != src_ctrl,
                                    CastTriggerPlayer::AnyPlayer => true,
                                }
                            },
                        ));
                    }
                }
                out
            }
            GameEvent::Attacks { attacker_ids } => {
                let mut sorted = attacker_ids.clone();
                sorted.sort_by_key(|&oid| {
                    self.state
                        .objects
                        .get(&oid)
                        .map(|o| (o.owner != ap) as u8)
                        .unwrap_or(1)
                });
                sorted
                    .iter()
                    .flat_map(|&att| {
                        let Some(obj) = self.state.objects.get(&att) else {
                            return vec![];
                        };
                        let card_id = obj.card_id.clone();
                        let controller = obj.owner;
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
                let controller = obj.owner;
                let defender = *defender_id;
                self.matching_triggered_abilities(&card_id, *attacker_id, controller, |tc| match tc
                {
                    TriggerCondition::WheneverSelfDealsCombatDamageToPlayer => true,
                    TriggerCondition::WheneverSelfDealsDamageToOpponent => defender != controller,
                    _ => false,
                })
            }
            GameEvent::UpkeepBegin => {
                let ap_idx = self.state.player_idx(ap).unwrap_or(0);
                let bf: Vec<ObjectId> = self.state.players[ap_idx].battlefield.clone();
                bf.iter()
                    .flat_map(|&oid| {
                        let Some(obj) = self.state.objects.get(&oid) else {
                            return vec![];
                        };
                        let card_id = obj.card_id.clone();
                        let controller = obj.owner;
                        self.matching_triggered_abilities(&card_id, oid, controller, |tc| {
                            *tc == TriggerCondition::AtBeginningOfControllerUpkeep
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

                let mut ordered: Vec<usize> = (0..self.state.players.len()).collect();
                ordered.sort_by_key(|&i| (self.state.players[i].id != ap) as u8);
                let mut sources: Vec<(ObjectId, String, PlayerId)> = Vec::new();
                for pi in ordered {
                    for &sid in &self.state.players[pi].battlefield {
                        if let Some(o) = self.state.objects.get(&sid) {
                            sources.push((sid, o.card_id.clone(), o.owner));
                        }
                    }
                }
                sources
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
    ) -> Vec<(ObjectId, String, PlayerId, usize, String)> {
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
            .map(|(idx, ta)| {
                (
                    source_id,
                    card_id.to_string(),
                    controller,
                    idx,
                    ta.text.clone(),
                )
            })
            .collect()
    }

    pub(super) fn push_trigger(
        &mut self,
        source_id: ObjectId,
        card_id: &str,
        controller: PlayerId,
        ability_index: usize,
        ability_text: String,
        events: &mut Vec<rv1::RuledEvent>,
    ) {
        let def = match self.registry.get(card_id) {
            Some(d) => d.clone(),
            None => return,
        };
        let needs_target = def
            .primary_face()
            .triggered_abilities
            .get(ability_index)
            .map(|ta| spell_effect_kind_needs_target(&ta.effect))
            .unwrap_or(false);

        let card_name = def.name.clone();
        let virtual_id = self.state.next_object_id;
        self.state.next_object_id += 1;

        if needs_target {
            let was_empty = self.state.pending_triggers.is_empty();
            self.state.pending_triggers.push_back(PendingTrigger {
                source_permanent_id: source_id,
                ability_index,
                ability_text: ability_text.clone(),
                card_id: card_id.to_string(),
                controller,
            });
            if was_empty {
                events.push(rv1::RuledEvent {
                    ev: Some(rv1::ruled_event::Ev::TriggerNeedsTarget(
                        rv1::TriggerNeedsTarget {
                            source_permanent_id: source_id,
                            ability_index: ability_index as u32,
                            ability_text: ability_text.clone(),
                            controller_player_id: controller,
                        },
                    )),
                });
                events.push(ev_log(format!(
                    "Triggered: {card_name} — choose a target for: {ability_text}"
                )));
            }
        } else {
            self.state.stack.push(StackItem {
                id: virtual_id,
                controller,
                card_id: card_id.to_string(),
                targets: vec![],
                ability_text: Some(ability_text.clone()),
                source_permanent_id: Some(source_id),
                ability_index: Some(ability_index),
                is_triggered: true,
                is_copy: false,
                chosen_x: 0,
                face_index: 0,
                target_damage: vec![],
                chosen_modes: vec![],
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
                    copy_source_object_id: 0,
                    chosen_mode_indices: vec![],
                    chosen_mode_labels: vec![],
                })),
            });
            events.push(ev_log(format!("Triggered: {card_name} — {ability_text}")));
        }
    }
}
