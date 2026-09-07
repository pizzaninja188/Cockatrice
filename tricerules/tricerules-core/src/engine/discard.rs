//! Semantic discard operations (CR 701.9), shared by effects, costs, and cleanup.
use super::events::object_display_name;
use super::triggers::CollectedTrigger;
use super::*;
use crate::state::{DiscardCause, DiscardReceipt};

impl GameEngine {
    pub(super) fn commit_discard_to(
        &mut self,
        player: PlayerId,
        object_id: ObjectId,
        cause: DiscardCause,
        destination: Zone,
        revealed: bool,
    ) -> Result<(String, rv1::RuledEvent), EngineError> {
        let object = self
            .state
            .objects
            .get(&object_id)
            .ok_or(EngineError::Illegal("discarded card object not found"))?;
        if object.owner != player || object.zone != Zone::Hand {
            return Err(EngineError::Illegal(
                "discarded card is not in its owner's hand",
            ));
        }
        let madness = self.madness_ability(object_id);
        let card_id = object.card_id.clone();
        let name = object_display_name(&self.state, self.registry, object_id);
        let before_generation = self
            .state
            .zone_change_generation
            .get(&object_id)
            .copied()
            .unwrap_or(0);
        resolution::move_object_to_zone(
            &mut self.state,
            self.registry,
            object_id,
            destination,
            None,
        )?;
        let receipt = DiscardReceipt {
            player,
            object_id,
            before_generation,
            after_generation: self
                .state
                .zone_change_generation
                .get(&object_id)
                .copied()
                .unwrap_or(0),
            destination,
            cause,
            known_card_id: (revealed || matches!(destination, Zone::Graveyard | Zone::Exile))
                .then_some(card_id),
        };
        self.fire_triggers(&[GameEvent::Discarded(receipt)]);
        if destination == Zone::Exile {
            if let Some(cost) = madness {
                self.stage_madness_trigger(player, object_id, before_generation, cost);
            }
        }
        let destination = match destination {
            Zone::Graveyard => rv1::permanent_moved::Destination::Graveyard,
            Zone::Exile => rv1::permanent_moved::Destination::Exile,
            Zone::Library => rv1::permanent_moved::Destination::Library,
            _ => unreachable!("discard destination is engine-authored"),
        };
        Ok((
            name,
            resolution::permanent_moved_event(&self.state, object_id, player, destination),
        ))
    }
}

use super::events::{ev_log, finish_with_events};
use super::replacement::PendingReplacementEvent;
use crate::state::{PendingDiscardBatch, ProposedDiscard};

impl GameEngine {
    fn player_has_discard_static(
        &self,
        player: PlayerId,
        predicate: impl Fn(&StaticAbilityDef) -> bool,
    ) -> bool {
        self.state
            .players
            .iter()
            .flat_map(|p| p.battlefield.iter().copied())
            .any(|oid| {
                self.controller_of(oid) == Some(player)
                    && !self.state.objects[&oid].face_down
                    && super::characteristics::latest_remove_all_abilities_timestamp(
                        &self.state,
                        oid,
                    )
                    .is_none()
                    && self.effective_face(oid).is_some_and(|face| {
                        face.static_abilities
                            .iter()
                            .any(|a| predicate(&a.definition))
                    })
            })
    }

    pub(super) fn has_discard_library_replacement(&self, player: PlayerId) -> bool {
        self.player_has_discard_static(player, |a| matches!(a, StaticAbilityDef::DiscardToLibrary))
    }

    pub(super) fn maximum_hand_size(&self, player: PlayerId) -> usize {
        if self
            .player_has_discard_static(player, |a| matches!(a, StaticAbilityDef::NoMaximumHandSize))
        {
            usize::MAX
        } else {
            MAX_HAND_SIZE
        }
    }

    pub(super) fn start_discard_replacements(
        &mut self,
        stack: ParkedStackResolution,
        selected: Vec<(PlayerId, ObjectId)>,
        revealed: bool,
        draw_after: Option<(PlayerId, u32)>,
    ) -> Result<RuledEventBatch, EngineError> {
        let cards = selected
            .into_iter()
            .map(|(player, oid)| ProposedDiscard {
                player,
                object: TriggerObjectRef {
                    object_id: oid,
                    zone_change_generation: self
                        .state
                        .zone_change_generation
                        .get(&oid)
                        .copied()
                        .unwrap_or(0),
                    controller_at_event: player,
                },
                destination: None,
            })
            .collect();
        self.continue_discard_batch(
            stack,
            PendingDiscardBatch {
                cards,
                current: 0,
                revealed,
                draw_after,
                library_orders: BTreeMap::new(),
            },
        )
    }

    fn discard_batch_current(&self, batch: &PendingDiscardBatch) -> bool {
        batch.cards.iter().all(|card| {
            self.state
                .objects
                .get(&card.object.object_id)
                .is_some_and(|obj| obj.zone == Zone::Hand && obj.owner == card.player)
                && self
                    .state
                    .zone_change_generation
                    .get(&card.object.object_id)
                    .copied()
                    .unwrap_or(0)
                    == card.object.zone_change_generation
        })
    }

    fn discard_order_player(batch: &PendingDiscardBatch) -> Option<PlayerId> {
        batch.cards.iter().find_map(|card| {
            (card.destination == Some(Zone::Library)
                && !batch.library_orders.contains_key(&card.player)
                && batch
                    .cards
                    .iter()
                    .filter(|c| c.player == card.player && c.destination == Some(Zone::Library))
                    .count()
                    > 1)
            .then_some(card.player)
        })
    }

    fn park_discard_batch(
        &mut self,
        stack: ParkedStackResolution,
        batch: PendingDiscardBatch,
        player: PlayerId,
        candidates: Vec<u32>,
        names: Vec<String>,
        ordered: bool,
    ) -> RuledEventBatch {
        let count = if ordered { candidates.len() as u32 } else { 1 };
        let prompt = if ordered {
            "Order discarded cards, top of library first."
        } else {
            "Choose the discard destination."
        }
        .to_string();
        let event = rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                rv1::ResolutionChoiceRequired {
                    deciding_player_id: player,
                    source_object_id: stack.item.id,
                    prompt_text: prompt.clone(),
                    choice_kind: rv1::ChoiceKind::PrivateReplacement as i32,
                    candidate_object_ids: candidates.clone(),
                    candidate_names: names,
                    min: count,
                    max: count,
                    ordered,
                    ..Default::default()
                },
            )),
        };
        self.state.pending_replacement_event =
            Some(PendingReplacementEvent::Discard(Box::new(batch)));
        self.state.pending_resolution = Some(PendingResolution {
            deciding_player: player,
            presentation: PendingResolutionPresentation {
                source_object_id: stack.item.id,
                candidates,
                min: count,
                max: count,
                ordered,
                unique_names: false,
                prompt,
                choice_kind: rv1::ChoiceKind::PrivateReplacement,
            },
            continuation: ResolutionContinuation::DiscardReplacement { stack },
        });
        finish_with_events(self, vec![event])
    }

    fn continue_discard_batch(
        &mut self,
        stack: ParkedStackResolution,
        mut batch: PendingDiscardBatch,
    ) -> Result<RuledEventBatch, EngineError> {
        while batch.current < batch.cards.len() {
            let card = &batch.cards[batch.current];
            if self.has_discard_library_replacement(card.player) {
                let name = object_display_name(&self.state, self.registry, card.object.object_id);
                let player = card.player;
                let madness = self.madness_ability(card.object.object_id).is_some();
                return Ok(self.park_discard_batch(
                    stack,
                    batch,
                    player,
                    vec![if madness { 2 } else { 0 }, 1],
                    vec![
                        if madness {
                            format!("Exile {name} (madness)")
                        } else {
                            format!("Discard {name} to graveyard")
                        },
                        format!("Discard {name} to library (Library of Leng)"),
                    ],
                    false,
                ));
            }
            batch.cards[batch.current].destination =
                Some(if self.madness_ability(card.object.object_id).is_some() {
                    Zone::Exile
                } else {
                    Zone::Graveyard
                });
            batch.current += 1;
        }
        if let Some(player) = Self::discard_order_player(&batch) {
            let ids: Vec<_> = batch
                .cards
                .iter()
                .filter(|c| c.player == player && c.destination == Some(Zone::Library))
                .map(|c| c.object.object_id)
                .collect();
            let names = ids
                .iter()
                .map(|oid| object_display_name(&self.state, self.registry, *oid))
                .collect();
            return Ok(self.park_discard_batch(stack, batch, player, ids, names, true));
        }
        if !self.discard_batch_current(&batch) {
            return Err(EngineError::Illegal("discard batch became stale"));
        }
        let mut events = Vec::new();
        let mut result = CardResultCohort::default();
        for card in &batch.cards {
            let destination = card.destination.expect("all discard destinations chosen");
            let (name, moved) = self.commit_discard_to(
                card.player,
                card.object.object_id,
                DiscardCause::Effect,
                destination,
                batch.revealed,
            )?;
            events.push(moved);
            let mut receipt = super::payment::card_result_entry(
                &self.state,
                self.registry,
                tricerules_cards::primitives::CardResultAction::Discard,
                card.player,
                card.object.object_id,
            );
            if destination == Zone::Library && !batch.revealed {
                receipt.matched_card_types.clear();
            }
            result.cards.push(receipt);
            events.push(ev_log(if destination == Zone::Library && !batch.revealed {
                format!(
                    "P{} discards a card to the top of their library.",
                    card.player
                )
            } else {
                format!("P{} discards {name}.", card.player)
            }));
        }
        // Moves use the normal zone funnel, then apply the owner's explicit top-first order.
        for card in &batch.cards {
            if card.destination == Some(Zone::Library) {
                batch
                    .library_orders
                    .entry(card.player)
                    .or_insert_with(|| vec![card.object.object_id]);
            }
        }
        for (player, order) in batch.library_orders {
            let index = self.state.player_idx(player).expect("discard owner exists");
            let library = &mut self.state.players[index].library;
            for oid in order.iter().rev() {
                library.retain(|other| other != oid);
                library.push_front(*oid);
            }
        }
        if let Some((player, count)) = batch.draw_after {
            resolution::zones::draw_cards_for_player(
                self,
                &mut events,
                player,
                count,
                "discard continuation",
            )?;
        }
        self.complete_parked_resolution_with_previous(
            stack.item,
            stack.resume_effect_index,
            result.into(),
            events,
        )
    }

    pub(super) fn finish_discard_replacement(
        &mut self,
        pending: PendingResolution,
        chosen: &[u32],
    ) -> Result<RuledEventBatch, EngineError> {
        let ResolutionContinuation::DiscardReplacement { stack } = &pending.continuation else {
            unreachable!()
        };
        let stack = stack.clone();
        let Some(PendingReplacementEvent::Discard(batch)) =
            self.state.pending_replacement_event.as_ref()
        else {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("discard replacement missing"));
        };
        if !self.discard_batch_current(batch) {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("discard replacement became stale"));
        }
        let mut batch = (**batch).clone();
        if batch.current < batch.cards.len() {
            if !self.has_discard_library_replacement(batch.cards[batch.current].player) {
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal(
                    "discard replacement source became stale",
                ));
            }
            batch.cards[batch.current].destination = Some(match chosen[0] {
                1 => Zone::Library,
                2 => Zone::Exile,
                _ => Zone::Graveyard,
            });
            batch.current += 1;
        } else {
            batch
                .library_orders
                .insert(pending.deciding_player, chosen.to_vec());
        }
        self.state.pending_replacement_event = None;
        self.continue_discard_batch(stack, batch)
    }
}

impl GameEngine {
    pub(super) fn madness_ability(&self, oid: ObjectId) -> Option<ManaCost> {
        let object = self.state.objects.get(&oid)?;
        let face = self.registry.get(&object.card_id)?.face(0)?;
        face.static_abilities
            .iter()
            .find_map(|ability| match &ability.definition {
                StaticAbilityDef::Madness { cost } => Some(cost.clone()),
                _ => None,
            })
    }

    fn stage_madness_trigger(
        &mut self,
        player: PlayerId,
        oid: ObjectId,
        before_generation: u64,
        cost: ManaCost,
    ) {
        let object = &self.state.objects[&oid];
        let card_id = object.card_id.clone();
        let static_ability = self
            .registry
            .get(&card_id)
            .unwrap()
            .face(0)
            .unwrap()
            .static_abilities
            .iter()
            .find(|a| matches!(a.definition, StaticAbilityDef::Madness { .. }))
            .unwrap();
        let ability = TriggeredAbilityDef {
            ability_id: static_ability.ability_id.clone(),
            presentation: static_ability.presentation.clone(),
            trigger: TriggerCondition::WhenSelfDies,
            effect: vec![SpellEffectKind::CastMadness { cost }],
            modal: None,
            targeting: None,
            may: false,
            intervening_if: None,
            max_triggers_per_turn: None,
            triggers_only_once: false,
        };
        let presentation = super::presentation::presentation_ref(
            self.registry,
            &card_id,
            &self
                .registry
                .get(&card_id)
                .unwrap()
                .face(0)
                .unwrap()
                .face_id,
            [super::presentation::PresentationPath::Ability(
                &static_ability.ability_id,
            )],
            &static_ability.presentation,
            "Madness".into(),
        );
        let trigger = CollectedTrigger {
            source_id: oid,
            card_id,
            face_index: 0,
            source_zone_change: before_generation,
            source_face_change: 0,
            source_fact: None,
            additional_instances: 0,
            controller: player,
            ability_index: usize::MAX,
            ability_origin: None,
            presentation: Some(presentation),
            ability_text:
                "Madness - cast this card for its madness cost or put it into your graveyard."
                    .into(),
            ability,
            trigger_context: TriggerContext {
                source_after_zone_change: Some(TriggerObjectRef {
                    object_id: oid,
                    zone_change_generation: self.state.zone_change_generation[&oid],
                    controller_at_event: player,
                }),
                ..Default::default()
            },
        };
        self.stage_triggers(vec![trigger]);
    }

    /// A cast permission exists only while this exact resolution continuation is parked.
    pub(super) fn special_cast_permission(
        &self,
        player: PlayerId,
    ) -> Option<crate::state::ActiveExilePlayPermission> {
        let pending = self.state.pending_resolution.as_ref()?;
        if pending.deciding_player != player {
            return None;
        }
        let ResolutionContinuation::SpecialCast {
            stack,
            exiled,
            face_index,
            cost,
            ..
        } = &pending.continuation
        else {
            return None;
        };
        Some(crate::state::ActiveExilePlayPermission {
            group_id: stack.item.id as u64,
            player_id: player,
            source_label: pending.presentation.prompt.clone(),
            object_id: exiled.object_id,
            zone_change_generation: exiled.zone_change_generation,
            scope: ExilePlayPermissionScope::CastFace(*face_index),
            cast_cost: crate::state::ExilePermissionCastCost::AlternativeManaCost(cost.clone()),
            origin: crate::state::ExilePlayPermissionOrigin::Effect,
            available_after_turn_instance: None,
            expires_at_cleanup_turn_instance: None,
        })
    }

    pub(super) fn special_cast_method(&self, player: PlayerId) -> Option<SpellCastMethod> {
        let pending = self.state.pending_resolution.as_ref()?;
        if pending.deciding_player != player {
            return None;
        }
        match pending.continuation {
            ResolutionContinuation::SpecialCast { method, .. } => Some(method),
            _ => None,
        }
    }
}

impl GameEngine {
    pub(super) fn card_result_generation(&self, entry: &CardResultEntry) -> u64 {
        if entry.action == tricerules_cards::primitives::CardResultAction::Discard {
            self.state
                .discard_reference_successors
                .get(&(entry.object_id, entry.zone_change_generation))
                .copied()
                .unwrap_or(entry.zone_change_generation)
        } else {
            entry.zone_change_generation
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn issue_197_uncast_madness_preserves_only_the_discard_reference() {
        let mut e = GameEngine::new(
            19710,
            &[0, 1],
            20,
            Some(vec![
                vec!["fiery_temper".into(); 20],
                vec!["forest".into(); 20],
            ]),
            true,
        )
        .unwrap();
        e.state.turn_step = TurnStep::Main1;
        e.state.active_player_idx = 0;
        e.state.priority_idx = 0;
        let oid = e.state.players[0].hand[0];
        resolution::perform_discard(&mut e, 0, oid, DiscardCause::Cost).unwrap();
        let receipt = super::super::payment::card_result_entry(
            &e.state,
            e.registry,
            tricerules_cards::primitives::CardResultAction::Discard,
            0,
            oid,
        );
        let exile_generation = receipt.zone_change_generation;
        e.flush_staged_triggers(&mut vec![]);
        let pass = rv1::RuledCommand {
            cmd: Some(rv1::ruled_command::Cmd::PassPriority(rv1::PassPriority {})),
        };
        e.apply_command(0, &pass).unwrap();
        e.apply_command(1, &pass).unwrap();
        e.submit_resolution_choice(
            0,
            &rv1::SubmitResolutionChoice {
                decision: rv1::ResolutionChoiceDecision::Decline as i32,
                ..Default::default()
            },
        )
        .unwrap();
        let grave_generation = e.state.zone_change_generation[&oid];
        assert_eq!(grave_generation, exile_generation + 1);
        assert_eq!(
            e.card_result_generation(&receipt),
            grave_generation,
            "CR 400.7k follows an uncast madness card to its public destination"
        );
        let mut ordinary = receipt.clone();
        ordinary.action = tricerules_cards::primitives::CardResultAction::Exile;
        assert_eq!(
            e.card_result_generation(&ordinary),
            exile_generation,
            "ordinary references are not rebound"
        );
        resolution::move_object_to_zone(&mut e.state, e.registry, oid, Zone::Exile, None).unwrap();
        resolution::move_object_to_zone(&mut e.state, e.registry, oid, Zone::Graveyard, None)
            .unwrap();
        assert_ne!(
            e.card_result_generation(&receipt),
            e.state.zone_change_generation[&oid],
            "another zone change ends the exception"
        );
    }
    #[test]
    fn issue_197_madness_countering_and_new_incarnations_leave_the_card_exiled() {
        for countered in [true, false] {
            let mut e = GameEngine::new(
                19711,
                &[0, 1],
                20,
                Some(vec![
                    vec!["fiery_temper".into(); 20],
                    vec!["forest".into(); 20],
                ]),
                true,
            )
            .unwrap();
            e.state.turn_step = TurnStep::Main1;
            e.state.active_player_idx = 0;
            e.state.priority_idx = 0;
            let oid = e.state.players[0].hand[0];
            resolution::perform_discard(&mut e, 0, oid, DiscardCause::Effect).unwrap();
            e.flush_staged_triggers(&mut vec![]);
            assert_eq!(e.state.stack.len(), 1);
            let trigger = e.state.stack[0].id;
            if countered {
                resolution::counter_stack_object(&mut e, trigger, "counter madness", &mut vec![])
                    .unwrap();
            } else {
                resolution::move_object_to_zone(
                    &mut e.state,
                    e.registry,
                    oid,
                    Zone::Graveyard,
                    None,
                )
                .unwrap();
                resolution::move_object_to_zone(&mut e.state, e.registry, oid, Zone::Exile, None)
                    .unwrap();
                let pass = rv1::RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::PassPriority(rv1::PassPriority {})),
                };
                e.apply_command(0, &pass).unwrap();
                e.apply_command(1, &pass).unwrap();
            }
            assert!(e.state.pending_resolution.is_none());
            assert!(e.state.stack.is_empty());
            assert_eq!(e.state.objects[&oid].zone, Zone::Exile);
            assert!(e.state.active_exile_play_permissions.is_empty());
            assert!(e.state.discard_reference_successors.is_empty());
        }
    }

    #[test]
    fn issue_197_direct_hand_moves_are_not_discards() {
        let mut e = GameEngine::new(
            19712,
            &[0, 1],
            20,
            Some(vec![
                vec!["fiery_temper".into(); 20],
                vec!["forest".into(); 20],
            ]),
            true,
        )
        .unwrap();
        for destination in [Zone::Exile, Zone::Graveyard] {
            let oid = e.state.players[0].hand[0];
            resolution::move_object_to_zone(&mut e.state, e.registry, oid, destination, None)
                .unwrap();
            e.flush_staged_triggers(&mut vec![]);
            assert!(e.state.stack.is_empty());
            assert!(e.state.pending_resolution.is_none());
        }
    }
}
