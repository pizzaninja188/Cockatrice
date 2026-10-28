//! CR 404.3: arrange actual simultaneous graveyard arrivals before resuming resolution.
use super::*;

#[derive(serde::Serialize, Debug, Clone)]
pub(super) struct SacrificeDeparture {
    pub source: TriggerSourceSnapshot,
    pub was_creature: bool,
    pub died: bool,
}

/// The committed sacrifice event survives its owners' sequential ordering choices.
#[derive(serde::Serialize, Debug, Clone)]
pub struct PendingMassSacrifice {
    pub(super) zone: zone_events::ZoneEventBatch,
    pub(super) departures: Vec<SacrificeDeparture>,
    /// Validation is reduced by player departure; the committed rules event is immutable.
    pub(super) live_validation: Vec<zone_events::ZoneChangeReceipt>,
    pub(super) remaining: VecDeque<(PlayerId, Vec<ObjectId>)>,
}

impl GameEngine {
    pub(super) fn begin_mass_sacrifice_order(
        &mut self,
        item: StackItem,
        zone: zone_events::ZoneEventBatch,
        departures: Vec<SacrificeDeparture>,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> bool {
        self.begin_mass_sacrifice_order_with_stack(
            ParkedStackResolution::new(item),
            zone,
            departures,
            events,
        )
    }

    pub(super) fn begin_mass_sacrifice_order_with_stack(
        &mut self,
        stack: ParkedStackResolution,
        zone: zone_events::ZoneEventBatch,
        departures: Vec<SacrificeDeparture>,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> bool {
        let mut groups = self
            .state
            .players
            .iter()
            .map(|player| {
                let arrivals = zone
                    .moves
                    .iter()
                    .filter(|receipt| {
                        receipt.destination == Zone::Graveyard
                            && receipt.before.owner == player.id
                            && self.state.is_card_object(receipt.before.object_id)
                    })
                    .map(|receipt| receipt.before.object_id)
                    .collect::<Vec<_>>();
                (player.id, arrivals)
            })
            .filter(|(_, arrivals)| arrivals.len() > 1)
            .collect::<Vec<_>>();
        groups.sort_by_key(|(owner, _)| self.state.apnap_rank(*owner));
        self.advance_mass_sacrifice_order(
            Some(stack),
            PendingMassSacrifice {
                live_validation: zone.moves.clone(),
                zone,
                departures,
                remaining: groups.into(),
            },
            events,
        )
    }

    /// Commit one generation-validated simultaneous sacrifice cohort and retain its card-result
    /// receipt across the graveyard-order continuation.
    pub(super) fn commit_player_set_sacrifice(
        &mut self,
        mut stack: ParkedStackResolution,
        cohort: &[ObjectId],
        effect_result: &mut EffectResult,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<bool, EngineError> {
        if cohort.is_empty() {
            return Ok(false);
        }
        let snapshot = self.snapshot_zone_event();
        let before = cohort
            .iter()
            .map(|&oid| {
                let source = snapshot
                    .source(oid)
                    .ok_or(EngineError::Illegal("sacrifice source missing"))?;
                let name = super::events::object_display_name(&self.state, self.registry, oid);
                let was_creature = source.types.iter().any(|kind| kind == "Creature");
                Ok((oid, source, name, was_creature))
            })
            .collect::<Result<Vec<_>, EngineError>>()?;
        let died = resolution::sacrifice_permanents(&mut self.state, self.registry, cohort)?;
        let mut departures = Vec::new();
        for ((oid, source, name, was_creature), died) in before.into_iter().zip(died) {
            events.push(super::events::ev_log(format!(
                "P{} sacrifices {name}.",
                source.controller
            )));
            events.push(resolution::permanent_moved_event(
                &self.state,
                oid,
                source.owner,
                rv1::permanent_moved::Destination::Graveyard,
            ));
            effect_result.cards.push(super::payment::card_result_entry(
                &self.state,
                self.registry,
                tricerules_card_model::primitives::CardResultAction::Sacrifice,
                source.controller,
                oid,
            ));
            departures.push(SacrificeDeparture {
                source,
                was_creature,
                died,
            });
        }
        let GameEvent::ZoneChanges(zone) = self.finish_zone_event(snapshot) else {
            unreachable!()
        };
        stack.previous_result = effect_result.clone();
        Ok(self.begin_mass_sacrifice_order_with_stack(stack, zone, departures, events))
    }

    fn current_player_sacrifice_candidates(
        &self,
        player: PlayerId,
        filter: &tricerules_card_model::primitives::TargetFilter,
        excluded: &HashSet<ObjectId>,
    ) -> Vec<(ObjectId, u64)> {
        let Some(index) = self.state.player_idx(player) else {
            return Vec::new();
        };
        let mut candidates = self.state.players[index]
            .battlefield
            .iter()
            .copied()
            .filter(|oid| !excluded.contains(oid))
            .filter(|oid| {
                self.characteristics(*oid).is_some_and(|characteristics| {
                    characteristics.controller == player
                        && super::targeting::object_matches_mass_filter(self, *oid, filter)
                })
            })
            .map(|oid| {
                (
                    oid,
                    self.state
                        .zone_change_generation
                        .get(&oid)
                        .copied()
                        .unwrap_or(0),
                )
            })
            .collect::<Vec<_>>();
        candidates.sort_unstable_by_key(|(oid, _)| *oid);
        candidates
    }

    /// Auto-select every forced slot in APNAP order, stopping at the next player with a real
    /// choice. Each candidate set is refreshed only after a player departure has changed control.
    pub(super) fn advance_player_set_sacrifice_choices(
        &self,
        sacrifice: &mut crate::state::PendingPlayerSetSacrifice,
    ) -> bool {
        let live_players: HashSet<_> = self
            .state
            .players
            .iter()
            .filter(|player| !player.has_lost)
            .map(|player| player.id)
            .collect();
        sacrifice.accepted.retain(|selection| {
            live_players.contains(&selection.player)
                && self
                    .state
                    .zone_change_generation
                    .get(&selection.object_id)
                    .copied()
                    .unwrap_or(0)
                    == selection.zone_change_generation
                && self
                    .state
                    .objects
                    .get(&selection.object_id)
                    .is_some_and(|object| object.zone == Zone::Battlefield)
                && self
                    .characteristics(selection.object_id)
                    .is_some_and(|characteristics| {
                        characteristics.controller == selection.player
                            && super::targeting::object_matches_mass_filter(
                                self,
                                selection.object_id,
                                &sacrifice.filter,
                            )
                    })
        });
        let mut excluded: HashSet<_> = sacrifice
            .accepted
            .iter()
            .map(|selection| selection.object_id)
            .collect();
        while sacrifice.current < sacrifice.choices.len() {
            let choice = &mut sacrifice.choices[sacrifice.current];
            if !live_players.contains(&choice.player) {
                sacrifice.current += 1;
                continue;
            }
            choice.candidate_generations = self.current_player_sacrifice_candidates(
                choice.player,
                &sacrifice.filter,
                &excluded,
            );
            choice.required = sacrifice
                .count
                .min(choice.candidate_generations.len() as u32);
            if choice.candidate_generations.len() as u32 > sacrifice.count {
                return true;
            }
            for (object_id, zone_change_generation) in &choice.candidate_generations {
                sacrifice
                    .accepted
                    .push(crate::state::AcceptedPermanentSelection {
                        player: choice.player,
                        object_id: *object_id,
                        zone_change_generation: *zone_change_generation,
                    });
                excluded.insert(*object_id);
            }
            sacrifice.current += 1;
        }
        false
    }

    pub(super) fn park_player_set_sacrifice_choice(
        &mut self,
        stack: ParkedStackResolution,
        sacrifice: crate::state::PendingPlayerSetSacrifice,
        events: &mut Vec<rv1::RuledEvent>,
    ) {
        let choice = &sacrifice.choices[sacrifice.current];
        let candidates = choice
            .candidate_generations
            .iter()
            .map(|(object_id, _)| *object_id)
            .collect::<Vec<_>>();
        let candidate_card_ids = candidates
            .iter()
            .map(|oid| {
                self.state
                    .objects
                    .get(oid)
                    .filter(|object| !object.face_down)
                    .map(|object| object.card_id.clone())
                    .unwrap_or_default()
            })
            .collect::<Vec<_>>();
        let candidate_names = candidates
            .iter()
            .map(|oid| {
                self.state
                    .objects
                    .get(oid)
                    .filter(|object| object.face_down)
                    .map(|_| "Face-down creature".to_string())
                    .unwrap_or_else(|| {
                        super::events::object_display_name(&self.state, self.registry, *oid)
                    })
            })
            .collect::<Vec<_>>();
        let selection_prompt = resolution::permanent_choice_prompt(
            &sacrifice.filter,
            choice.required,
            choice.required,
        );
        let prompt = format!(
            "P{}: {} to sacrifice.",
            choice.player,
            selection_prompt.trim_end_matches('.')
        );
        let accepted_permanent_selections = sacrifice
            .accepted
            .iter()
            .map(|selection| rv1::AcceptedPermanentSelection {
                selecting_player_id: selection.player,
                object_id: selection.object_id,
                zone_change_generation: selection.zone_change_generation,
            })
            .collect();
        events.push(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                rv1::ResolutionChoiceRequired {
                    resolution_prompt_id: 0,
                    deciding_player_id: choice.player,
                    source_object_id: stack.item.id,
                    prompt_text: prompt.clone(),
                    choice_kind: rv1::ChoiceKind::PermanentObjects as i32,
                    candidate_object_ids: candidates.clone(),
                    candidate_card_ids,
                    candidate_names,
                    min: choice.required,
                    max: choice.required,
                    candidate_selectable: vec![true; candidates.len()],
                    accepted_permanent_selections,
                    ..Default::default()
                },
            )),
        });
        events.push(super::events::ev_log(prompt.clone()));
        self.state.pending_resolution = Some(PendingResolution {
            resolution_prompt_id: crate::state::ResolutionPromptId::UNASSIGNED,
            resolution_prompt_semantics: None,

            deciding_player: choice.player,
            presentation: PendingResolutionPresentation {
                source_object_id: stack.item.id,
                candidates,
                min: choice.required,
                max: choice.required,
                ordered: false,
                prompt,
                choice_kind: rv1::ChoiceKind::PermanentObjects,
                unique_names: false,
            },
            continuation: ResolutionContinuation::PlayerSetSacrifice { stack, sacrifice },
        });
    }

    pub(super) fn finish_player_set_sacrifice_choice(
        &mut self,
        pending: PendingResolution,
        chosen: &[ObjectId],
    ) -> Result<RuledEventBatch, EngineError> {
        let (stack, mut sacrifice) = match &pending.continuation {
            ResolutionContinuation::PlayerSetSacrifice { stack, sacrifice } => {
                (stack.clone(), sacrifice.clone())
            }
            _ => {
                return Err(EngineError::Illegal(
                    "player-set sacrifice continuation missing",
                ))
            }
        };
        let Some(choice) = sacrifice.choices.get(sacrifice.current) else {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("player-set sacrifice chooser missing"));
        };
        let legal = choice.player == pending.deciding_player
            && chosen.len() == choice.required as usize
            && chosen.iter().all(|object_id| {
                let Some(generation) =
                    choice
                        .candidate_generations
                        .iter()
                        .find_map(|(candidate, generation)| {
                            (candidate == object_id).then_some(*generation)
                        })
                else {
                    return false;
                };
                self.state
                    .zone_change_generation
                    .get(object_id)
                    .copied()
                    .unwrap_or(0)
                    == generation
                    && self.state.objects.get(object_id).is_some_and(|object| {
                        object.zone == Zone::Battlefield
                            && self
                                .characteristics(*object_id)
                                .is_some_and(|characteristics| {
                                    characteristics.controller == choice.player
                                        && super::targeting::object_matches_mass_filter(
                                            self,
                                            *object_id,
                                            &sacrifice.filter,
                                        )
                                })
                    })
            });
        if !legal || chosen.iter().copied().collect::<HashSet<_>>().len() != chosen.len() {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "chosen sacrifice permanent is stale or illegal",
            ));
        }
        for object_id in chosen {
            let generation = choice
                .candidate_generations
                .iter()
                .find_map(|(candidate, generation)| (candidate == object_id).then_some(*generation))
                .expect("validated player-set sacrifice candidate");
            sacrifice
                .accepted
                .push(crate::state::AcceptedPermanentSelection {
                    player: choice.player,
                    object_id: *object_id,
                    zone_change_generation: generation,
                });
        }
        sacrifice.current += 1;
        self.state.pending_resolution = None;
        let mut events = Vec::new();
        if self.advance_player_set_sacrifice_choices(&mut sacrifice) {
            self.park_player_set_sacrifice_choice(stack, sacrifice, &mut events);
            return Ok(events::finish_with_events(self, events));
        }
        events.push(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::ResolutionChoiceProgress(
                rv1::ResolutionChoiceProgress {
                    source_object_id: stack.item.id,
                    accepted_permanent_selections: Vec::new(),
                },
            )),
        });
        let cohort = sacrifice
            .accepted
            .iter()
            .map(|selection| selection.object_id)
            .collect::<Vec<_>>();
        let mut effect_result = EffectResult::default();
        if self.commit_player_set_sacrifice(
            stack.clone(),
            &cohort,
            &mut effect_result,
            &mut events,
        )? {
            return Ok(events::finish_with_events(self, events));
        }
        self.complete_parked_resolution_with_previous(
            stack.item,
            stack.resume_effect_index,
            effect_result,
            events,
        )
    }

    pub(super) fn refresh_player_set_sacrifice_departure(
        &mut self,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        let Some(pending) = self.state.pending_resolution.clone() else {
            return Ok(());
        };
        let ResolutionContinuation::PlayerSetSacrifice {
            stack,
            mut sacrifice,
        } = pending.continuation
        else {
            return Ok(());
        };
        self.state.pending_resolution = None;
        if self.advance_player_set_sacrifice_choices(&mut sacrifice) {
            self.park_player_set_sacrifice_choice(stack, sacrifice, events);
            return Ok(());
        }
        events.push(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::ResolutionChoiceProgress(
                rv1::ResolutionChoiceProgress {
                    source_object_id: stack.item.id,
                    accepted_permanent_selections: Vec::new(),
                },
            )),
        });
        let cohort = sacrifice
            .accepted
            .iter()
            .map(|selection| selection.object_id)
            .collect::<Vec<_>>();
        let mut effect_result = EffectResult::default();
        if self.commit_player_set_sacrifice(stack.clone(), &cohort, &mut effect_result, events)? {
            return Ok(());
        }
        let completed = self.complete_parked_resolution_with_previous(
            stack.item,
            stack.resume_effect_index,
            effect_result,
            std::mem::take(events),
        )?;
        events.extend(completed.events);
        Ok(())
    }

    fn type_choice_subject_is_live(&self, subject: PlayerId) -> bool {
        self.state
            .player_idx(subject)
            .is_some_and(|index| !self.state.players[index].has_lost)
    }

    fn type_choice_sacrifice_candidates(
        &self,
        subject: &crate::state::PendingTypeChoiceSacrificeSubject,
        permanent_type: PermanentTypeFilter,
        scope: tricerules_card_model::primitives::PermanentSacrificeScope,
    ) -> Result<Vec<(ObjectId, u64)>, EngineError> {
        let subject_live = self.type_choice_subject_is_live(subject.player);
        let objects = if subject_live {
            self.state
                .players
                .iter()
                .flat_map(|player| player.battlefield.iter().copied())
                .map(|object_id| {
                    (
                        object_id,
                        self.state
                            .zone_change_generation
                            .get(&object_id)
                            .copied()
                            .unwrap_or(0),
                    )
                })
                .collect::<Vec<_>>()
        } else {
            subject
                .departure_cohort
                .as_deref()
                .ok_or(EngineError::ProgramInvariant(
                    "departed type-choice subject is missing its control cohort",
                ))?
                .to_vec()
        };
        let mut candidates = objects
            .into_iter()
            .filter(|(object_id, expected_generation)| {
                self.state
                    .zone_change_generation
                    .get(object_id)
                    .copied()
                    .unwrap_or(0)
                    == *expected_generation
                    && self
                        .state
                        .objects
                        .get(object_id)
                        .is_some_and(|object| object.zone == Zone::Battlefield)
            })
            .filter(|(object_id, _)| {
                self.characteristics(*object_id).is_some_and(|characteristics| {
                    (!subject_live || characteristics.controller == subject.player)
                        && characteristics
                            .types
                            .iter()
                            .any(|kind| kind == permanent_type.as_str())
                        && (scope
                            != tricerules_card_model::primitives::PermanentSacrificeScope::NonlandPermanents
                            || !characteristics.types.iter().any(|kind| kind == "Land"))
                })
            })
            .collect::<Vec<_>>();
        candidates.sort_unstable_by_key(|(object_id, _)| *object_id);
        candidates.dedup_by_key(|(object_id, generation)| (*object_id, *generation));
        Ok(candidates)
    }

    fn accept_type_choice_keep(
        sacrifice: &mut crate::state::PendingTypeChoiceSacrifice,
        subject: PlayerId,
        object_id: ObjectId,
        zone_change_generation: u64,
    ) {
        if sacrifice.accepted.iter().any(|selection| {
            selection.object_id == object_id
                && selection.zone_change_generation == zone_change_generation
        }) {
            return;
        }
        sacrifice
            .accepted
            .push(crate::state::AcceptedPermanentSelection {
                player: subject,
                object_id,
                zone_change_generation,
            });
    }

    /// Skip missing types, auto-select a sole candidate, and stop at the next actual choice.
    /// Delegation is only requested when a departed subject has a real multi-option choice.
    pub(super) fn advance_type_choice_sacrifice(
        &self,
        stack: &ParkedStackResolution,
        sacrifice: &mut crate::state::PendingTypeChoiceSacrifice,
    ) -> Result<bool, EngineError> {
        let mut seen = HashSet::new();
        sacrifice.accepted.retain(|selection| {
            let key = (selection.object_id, selection.zone_change_generation);
            seen.insert(key)
                && self
                    .state
                    .zone_change_generation
                    .get(&selection.object_id)
                    .copied()
                    .unwrap_or(0)
                    == selection.zone_change_generation
                && self
                    .state
                    .objects
                    .get(&selection.object_id)
                    .is_some_and(|object| object.zone == Zone::Battlefield)
        });

        loop {
            if sacrifice.current_subject >= sacrifice.subjects.len() {
                sacrifice.candidate_generations.clear();
                sacrifice.stage = crate::state::PendingTypeChoiceSacrificeStage::ChoosingPermanent;
                return Ok(false);
            }
            if sacrifice.current_type >= sacrifice.types.len() {
                sacrifice.current_subject += 1;
                sacrifice.current_type = 0;
                sacrifice.candidate_generations.clear();
                sacrifice.stage = crate::state::PendingTypeChoiceSacrificeStage::ChoosingPermanent;
                continue;
            }

            let subject_index = sacrifice.current_subject;
            let subject_id = sacrifice.subjects[subject_index].player;
            let subject_live = self.type_choice_subject_is_live(subject_id);
            if !subject_live && sacrifice.subjects[subject_index].departure_cohort.is_none() {
                let cohort = self
                    .state
                    .departed_targeted_player_control_cohorts
                    .get(&subject_id)
                    .filter(|cohort| cohort.player_id == subject_id)
                    .ok_or(EngineError::ProgramInvariant(
                        "departed type-choice subject is missing its control cohort",
                    ))?;
                sacrifice.subjects[subject_index].departure_cohort = Some(
                    cohort
                        .permanents
                        .iter()
                        .map(|permanent| (permanent.object_id, permanent.zone_change_generation))
                        .collect(),
                );
            }

            let permanent_type = sacrifice.types[sacrifice.current_type];
            let candidates = self.type_choice_sacrifice_candidates(
                &sacrifice.subjects[subject_index],
                permanent_type,
                sacrifice.scope,
            )?;
            match candidates.as_slice() {
                [] => {
                    sacrifice.current_type += 1;
                    sacrifice.candidate_generations.clear();
                    sacrifice.stage =
                        crate::state::PendingTypeChoiceSacrificeStage::ChoosingPermanent;
                }
                [(object_id, generation)] => {
                    Self::accept_type_choice_keep(sacrifice, subject_id, *object_id, *generation);
                    sacrifice.current_type += 1;
                    sacrifice.candidate_generations.clear();
                    sacrifice.stage =
                        crate::state::PendingTypeChoiceSacrificeStage::ChoosingPermanent;
                }
                _ => {
                    sacrifice.candidate_generations = candidates;
                    if subject_live {
                        sacrifice.subjects[subject_index].delegate = None;
                        sacrifice.stage =
                            crate::state::PendingTypeChoiceSacrificeStage::ChoosingPermanent;
                        return Ok(true);
                    }

                    let controller = stack.item.controller;
                    let eligible = resolution::resolution_choice_delegate_candidates(
                        &self.state,
                        controller,
                        subject_id,
                    );
                    let current_delegate = sacrifice.subjects[subject_index].delegate;
                    let delegate_is_eligible =
                        current_delegate.is_some_and(|delegate| eligible.contains(&delegate));
                    let current_delegate_prompt = matches!(
                        &sacrifice.stage,
                        crate::state::PendingTypeChoiceSacrificeStage::ChoosingDelegate { .. }
                    );
                    if delegate_is_eligible && !current_delegate_prompt {
                        sacrifice.stage =
                            crate::state::PendingTypeChoiceSacrificeStage::ChoosingPermanent;
                        return Ok(true);
                    }
                    if eligible.is_empty() {
                        return Err(EngineError::ProgramInvariant(
                            "no live player can make a departed type-choice subject's choice",
                        ));
                    }
                    let previous_slots = match &sacrifice.stage {
                        crate::state::PendingTypeChoiceSacrificeStage::ChoosingDelegate {
                            candidate_slots,
                        } => candidate_slots.clone(),
                        crate::state::PendingTypeChoiceSacrificeStage::ChoosingPermanent => {
                            sacrifice.subjects[subject_index]
                                .delegate_candidate_slots
                                .clone()
                        }
                    };
                    let candidate_slots = resolution::reconcile_resolution_choice_delegate_slots(
                        &previous_slots,
                        &eligible,
                    );
                    sacrifice.subjects[subject_index].delegate = None;
                    sacrifice.subjects[subject_index].delegate_candidate_slots =
                        candidate_slots.clone();
                    let was_delegate_prompt = matches!(
                        &sacrifice.stage,
                        crate::state::PendingTypeChoiceSacrificeStage::ChoosingDelegate { .. }
                    );
                    if !was_delegate_prompt && eligible.len() == 1 {
                        sacrifice.subjects[subject_index].delegate = eligible.first().copied();
                        sacrifice.stage =
                            crate::state::PendingTypeChoiceSacrificeStage::ChoosingPermanent;
                    } else {
                        resolution::resolution_choice_selector(&self.state, controller).ok_or(
                            EngineError::ProgramInvariant(
                                "no live player can select a type-choice delegate",
                            ),
                        )?;
                        sacrifice.stage =
                            crate::state::PendingTypeChoiceSacrificeStage::ChoosingDelegate {
                                candidate_slots,
                            };
                        return Ok(true);
                    }
                }
            }
        }
    }

    pub(super) fn park_type_choice_sacrifice_choice(
        &mut self,
        stack: ParkedStackResolution,
        sacrifice: crate::state::PendingTypeChoiceSacrifice,
        events: &mut Vec<rv1::RuledEvent>,
    ) {
        let subject = &sacrifice.subjects[sacrifice.current_subject];
        let permanent_type = sacrifice.types[sacrifice.current_type];
        let subject_live = self.type_choice_subject_is_live(subject.player);
        let (
            deciding_player,
            choice_kind,
            prompt,
            candidates,
            candidate_card_ids,
            candidate_names,
            candidate_selectable,
            resolution_branches,
        ) = match &sacrifice.stage {
            crate::state::PendingTypeChoiceSacrificeStage::ChoosingPermanent => {
                let deciding_player = if subject_live {
                    subject.player
                } else {
                    subject
                        .delegate
                        .expect("departed subject's live delegate is selected before parking")
                };
                let candidates = sacrifice
                    .candidate_generations
                    .iter()
                    .map(|(object_id, _)| *object_id)
                    .collect::<Vec<_>>();
                let candidate_card_ids = candidates
                    .iter()
                    .map(|object_id| {
                        self.state
                            .objects
                            .get(object_id)
                            .filter(|object| !object.face_down)
                            .map(|object| object.card_id.clone())
                            .unwrap_or_default()
                    })
                    .collect::<Vec<_>>();
                let candidate_names = candidates
                    .iter()
                    .map(|object_id| {
                        self.state
                            .objects
                            .get(object_id)
                            .filter(|object| object.face_down)
                            .map(|_| "Face-down permanent".to_string())
                            .unwrap_or_else(|| {
                                super::events::object_display_name(
                                    &self.state,
                                    self.registry,
                                    *object_id,
                                )
                            })
                    })
                    .collect::<Vec<_>>();
                let article = match permanent_type {
                    PermanentTypeFilter::Artifact | PermanentTypeFilter::Enchantment => "an",
                    _ => "a",
                };
                let prompt = format!(
                    "P{deciding_player}: choose {article} {} permanent to keep for P{}.",
                    permanent_type.as_str().to_ascii_lowercase(),
                    subject.player
                );
                (
                    deciding_player,
                    rv1::ChoiceKind::PermanentObjects,
                    prompt,
                    candidates.clone(),
                    candidate_card_ids,
                    candidate_names,
                    vec![true; candidates.len()],
                    Vec::new(),
                )
            }
            crate::state::PendingTypeChoiceSacrificeStage::ChoosingDelegate { candidate_slots } => {
                let selector =
                    resolution::resolution_choice_selector(&self.state, stack.item.controller)
                        .expect("nonterminal game has a live type-choice delegate selector");
                let eligible = resolution::resolution_choice_delegate_candidates(
                    &self.state,
                    stack.item.controller,
                    subject.player,
                );
                let options = candidate_slots
                    .iter()
                    .enumerate()
                    .map(|(index, candidate)| {
                        let selectable = candidate.is_some_and(|candidate| {
                            eligible.contains(&candidate)
                                && self
                                    .state
                                    .player_idx(candidate)
                                    .is_some_and(|player| !self.state.players[player].has_lost)
                        });
                        let label = candidate.map_or_else(
                            || "Unavailable player".into(),
                            |candidate| format!("P{candidate} makes P{}'s choice", subject.player),
                        );
                        rv1::ResolutionBranchOption {
                            branch_index: index as u32,
                            label: label.clone(),
                            selectable,
                            presentation: Some(rv1::PresentationRef {
                                fallback_text: label,
                                ..Default::default()
                            }),
                            ..Default::default()
                        }
                    })
                    .collect::<Vec<_>>();
                (
                    selector,
                    rv1::ChoiceKind::ResolutionBranch,
                    format!(
                        "P{selector}: choose a player to make P{}'s remaining type choices.",
                        subject.player
                    ),
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                    options,
                )
            }
        };
        let accepted_permanent_selections = sacrifice
            .accepted
            .iter()
            .map(|selection| rv1::AcceptedPermanentSelection {
                selecting_player_id: selection.player,
                object_id: selection.object_id,
                zone_change_generation: selection.zone_change_generation,
            })
            .collect();
        let mut choice = rv1::ResolutionChoiceRequired {
            resolution_prompt_id: 0,
            deciding_player_id: deciding_player,
            source_object_id: stack.item.id,
            prompt_text: prompt.clone(),
            choice_kind: choice_kind as i32,
            candidate_object_ids: candidates.clone(),
            candidate_card_ids,
            candidate_names,
            min: 1,
            max: 1,
            candidate_selectable,
            accepted_permanent_selections,
            resolution_branches,
            ..Default::default()
        };
        if choice_kind == rv1::ChoiceKind::ResolutionBranch {
            choice.candidate_object_ids.clear();
        }
        events.push(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice)),
        });
        events.push(super::events::ev_log(prompt.clone()));
        self.state.pending_resolution = Some(PendingResolution {
            resolution_prompt_id: crate::state::ResolutionPromptId::UNASSIGNED,
            resolution_prompt_semantics: None,
            deciding_player,
            presentation: PendingResolutionPresentation {
                source_object_id: stack.item.id,
                candidates,
                min: 1,
                max: 1,
                ordered: false,
                prompt,
                choice_kind,
                unique_names: false,
            },
            continuation: ResolutionContinuation::TypeChoiceSacrifice { stack, sacrifice },
        });
    }

    pub(super) fn type_choice_sacrifice_victims(
        &self,
        sacrifice: &crate::state::PendingTypeChoiceSacrifice,
    ) -> Vec<ObjectId> {
        let live_affected: HashSet<_> = sacrifice
            .affected_players
            .iter()
            .copied()
            .filter(|player| self.type_choice_subject_is_live(*player))
            .collect();
        let kept: HashSet<_> = sacrifice
            .accepted
            .iter()
            .map(|selection| (selection.object_id, selection.zone_change_generation))
            .collect();
        let mut victims = self
            .state
            .players
            .iter()
            .flat_map(|player| player.battlefield.iter().copied())
            .filter(|object_id| {
                self.state
                    .objects
                    .get(object_id)
                    .is_some_and(|object| object.zone == Zone::Battlefield)
            })
            .filter(|object_id| {
                let generation = self
                    .state
                    .zone_change_generation
                    .get(object_id)
                    .copied()
                    .unwrap_or(0);
                !kept.contains(&(*object_id, generation))
                    && self
                        .characteristics(*object_id)
                        .is_some_and(|characteristics| {
                            live_affected.contains(&characteristics.controller)
                                && (sacrifice.scope
                                    != tricerules_card_model::primitives::PermanentSacrificeScope::NonlandPermanents
                                    || !characteristics.types.iter().any(|kind| kind == "Land"))
                        })
            })
            .collect::<Vec<_>>();
        victims.sort_unstable();
        victims.dedup();
        victims
    }

    fn finish_type_choice_sacrifice_after_choices(
        &mut self,
        stack: ParkedStackResolution,
        sacrifice: &crate::state::PendingTypeChoiceSacrifice,
        mut events: Vec<rv1::RuledEvent>,
    ) -> Result<RuledEventBatch, EngineError> {
        events.push(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::ResolutionChoiceProgress(
                rv1::ResolutionChoiceProgress {
                    source_object_id: stack.item.id,
                    accepted_permanent_selections: Vec::new(),
                },
            )),
        });
        let victims = self.type_choice_sacrifice_victims(sacrifice);
        let mut effect_result = EffectResult::default();
        if self.commit_player_set_sacrifice(
            stack.clone(),
            &victims,
            &mut effect_result,
            &mut events,
        )? {
            return Ok(events::finish_with_events(self, events));
        }
        self.complete_parked_resolution_with_previous(
            stack.item,
            stack.resume_effect_index,
            effect_result,
            events,
        )
    }

    pub(super) fn finish_type_choice_sacrifice_choice(
        &mut self,
        pending: PendingResolution,
        answer: &rv1::SubmitResolutionChoice,
        decision: rv1::ResolutionChoiceDecision,
        deciding_player: PlayerId,
    ) -> Result<RuledEventBatch, EngineError> {
        let (stack, mut sacrifice) = match &pending.continuation {
            ResolutionContinuation::TypeChoiceSacrifice { stack, sacrifice } => {
                (stack.clone(), sacrifice.clone())
            }
            _ => {
                return Err(EngineError::Illegal(
                    "type-choice sacrifice continuation missing",
                ))
            }
        };
        let Some(subject) = sacrifice.subjects.get(sacrifice.current_subject).cloned() else {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "type-choice sacrifice subject missing",
            ));
        };
        if sacrifice.current_type >= sacrifice.types.len() {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("type-choice sacrifice type missing"));
        }
        let subject_id = subject.player;
        let permanent_type = sacrifice.types[sacrifice.current_type];
        let subject_live = self.type_choice_subject_is_live(subject_id);
        let no_unrelated_data = answer.chosen_player_ids.is_empty()
            && answer.cast_spell.is_none()
            && answer.spell_cast_announcement.is_none()
            && answer.chosen_combat_defender.is_none()
            && answer.payment.is_none()
            && answer.restricted_mana.is_empty();
        match &sacrifice.stage {
            crate::state::PendingTypeChoiceSacrificeStage::ChoosingDelegate { candidate_slots } => {
                if decision != rv1::ResolutionChoiceDecision::SelectBranch
                    || !answer.chosen_object_ids.is_empty()
                    || !no_unrelated_data
                {
                    self.state.pending_resolution = Some(pending);
                    return Err(EngineError::Illegal(
                        "replacement chooser must select one available player",
                    ));
                }
                let delegate = candidate_slots
                    .get(answer.selected_branch_index as usize)
                    .copied()
                    .flatten();
                let eligible = resolution::resolution_choice_delegate_candidates(
                    &self.state,
                    stack.item.controller,
                    subject_id,
                );
                let Some(delegate) = delegate.filter(|delegate| eligible.contains(delegate)) else {
                    self.state.pending_resolution = Some(pending);
                    return Err(EngineError::Illegal(
                        "that replacement chooser is no longer available",
                    ));
                };
                if !self.type_choice_subject_is_live(delegate) {
                    self.state.pending_resolution = Some(pending);
                    return Err(EngineError::Illegal(
                        "that replacement chooser is no longer available",
                    ));
                }
                sacrifice.subjects[sacrifice.current_subject].delegate = Some(delegate);
                sacrifice.stage = crate::state::PendingTypeChoiceSacrificeStage::ChoosingPermanent;
                self.state.pending_resolution = None;
                let mut events = vec![super::events::ev_log(format!(
                    "P{deciding_player} chooses P{delegate} to make P{subject_id}'s remaining type choices."
                ))];
                if self.advance_type_choice_sacrifice(&stack, &mut sacrifice)? {
                    self.park_type_choice_sacrifice_choice(stack, sacrifice, &mut events);
                    return Ok(events::finish_with_events(self, events));
                }
                self.finish_type_choice_sacrifice_after_choices(stack, &sacrifice, events)
            }
            crate::state::PendingTypeChoiceSacrificeStage::ChoosingPermanent => {
                let expected_decider = if subject_live {
                    Some(subject_id)
                } else {
                    subject
                        .delegate
                        .filter(|delegate| self.type_choice_subject_is_live(*delegate))
                };
                if decision != rv1::ResolutionChoiceDecision::Unspecified
                    || answer.selected_branch_index != 0
                    || !no_unrelated_data
                    || expected_decider != Some(deciding_player)
                {
                    self.state.pending_resolution = Some(pending);
                    return Err(EngineError::Illegal(
                        "type-choice permanent selection is stale or malformed",
                    ));
                }
                let [object_id] = answer.chosen_object_ids.as_slice() else {
                    self.state.pending_resolution = Some(pending);
                    return Err(EngineError::Illegal(
                        "choose exactly one permanent for this type",
                    ));
                };
                let expected_generation =
                    sacrifice
                        .candidate_generations
                        .iter()
                        .find_map(|(candidate, generation)| {
                            (candidate == object_id).then_some(*generation)
                        });
                let legal_candidates = self.type_choice_sacrifice_candidates(
                    &subject,
                    permanent_type,
                    sacrifice.scope,
                )?;
                let Some(expected_generation) = expected_generation.filter(|generation| {
                    legal_candidates
                        .iter()
                        .any(|(candidate, candidate_generation)| {
                            candidate == object_id && candidate_generation == generation
                        })
                }) else {
                    self.state.pending_resolution = Some(pending);
                    return Err(EngineError::Illegal(
                        "chosen permanent is stale or illegal for this type",
                    ));
                };
                Self::accept_type_choice_keep(
                    &mut sacrifice,
                    subject_id,
                    *object_id,
                    expected_generation,
                );
                let name = self
                    .state
                    .objects
                    .get(object_id)
                    .filter(|object| !object.face_down)
                    .map(|_| {
                        super::events::object_display_name(&self.state, self.registry, *object_id)
                    })
                    .unwrap_or_else(|| "a face-down permanent".into());
                let mut events = vec![super::events::ev_log(format!(
                    "P{deciding_player} selects {name} for P{subject_id}'s {} keep.",
                    permanent_type.as_str()
                ))];
                sacrifice.current_type += 1;
                sacrifice.candidate_generations.clear();
                sacrifice.stage = crate::state::PendingTypeChoiceSacrificeStage::ChoosingPermanent;
                self.state.pending_resolution = None;
                if self.advance_type_choice_sacrifice(&stack, &mut sacrifice)? {
                    self.park_type_choice_sacrifice_choice(stack, sacrifice, &mut events);
                    return Ok(events::finish_with_events(self, events));
                }
                self.finish_type_choice_sacrifice_after_choices(stack, &sacrifice, events)
            }
        }
    }

    pub(super) fn refresh_type_choice_sacrifice_departure(
        &mut self,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        let Some(pending) = self.state.pending_resolution.take() else {
            return Ok(());
        };
        let (stack, mut sacrifice) = match &pending.continuation {
            ResolutionContinuation::TypeChoiceSacrifice { stack, sacrifice } => {
                (stack.clone(), sacrifice.clone())
            }
            _ => {
                self.state.pending_resolution = Some(pending);
                return Ok(());
            }
        };
        for subject in &mut sacrifice.subjects {
            if self.type_choice_subject_is_live(subject.player)
                || subject.departure_cohort.is_some()
            {
                continue;
            }
            let cohort = self
                .state
                .departed_targeted_player_control_cohorts
                .get(&subject.player)
                .filter(|cohort| cohort.player_id == subject.player)
                .ok_or(EngineError::ProgramInvariant(
                    "departed type-choice subject is missing its control cohort",
                ))?;
            subject.departure_cohort = Some(
                cohort
                    .permanents
                    .iter()
                    .map(|permanent| (permanent.object_id, permanent.zone_change_generation))
                    .collect(),
            );
        }
        if self.advance_type_choice_sacrifice(&stack, &mut sacrifice)? {
            self.park_type_choice_sacrifice_choice(stack, sacrifice, events);
            return Ok(());
        }
        events.push(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::ResolutionChoiceProgress(
                rv1::ResolutionChoiceProgress {
                    source_object_id: stack.item.id,
                    accepted_permanent_selections: Vec::new(),
                },
            )),
        });
        let victims = self.type_choice_sacrifice_victims(&sacrifice);
        let mut effect_result = EffectResult::default();
        if self.commit_player_set_sacrifice(stack.clone(), &victims, &mut effect_result, events)? {
            return Ok(());
        }
        let completed = self.complete_parked_resolution_with_previous(
            stack.item,
            stack.resume_effect_index,
            effect_result,
            std::mem::take(events),
        )?;
        events.extend(completed.events);
        Ok(())
    }

    fn advance_mass_sacrifice_order(
        &mut self,
        stack: Option<ParkedStackResolution>,
        mut sacrifice: PendingMassSacrifice,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> bool {
        self.reconcile_combat_characteristics(events);
        sacrifice
            .remaining
            .retain(|(_, candidates)| candidates.len() > 1);
        let Some((owner, candidates)) = sacrifice.remaining.pop_front() else {
            self.finish_mass_sacrifice_events(sacrifice, events);
            return false;
        };
        let count = candidates.len() as u32;
        let source_object_id = stack.as_ref().map_or(0, |stack| stack.item.id);
        let prompt =
            "Order the cards entering your graveyard, oldest first; the last card is on top."
                .to_string();
        events.push(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                rv1::ResolutionChoiceRequired {
                    resolution_prompt_id: 0,
                    variable_mana_contribution: false,
                    deciding_player_id: owner,
                    source_object_id,
                    prompt_text: prompt.clone(),
                    choice_kind: custom::ChoiceKind::GraveyardCards as i32,
                    candidate_object_ids: candidates.clone(),
                    candidate_card_ids: candidates
                        .iter()
                        .map(|oid| self.state.objects[oid].card_id.clone())
                        .collect(),
                    candidate_names: candidates
                        .iter()
                        .map(|&oid| events::object_display_name(&self.state, self.registry, oid))
                        .collect(),
                    min: count,
                    max: count,
                    ordered: true,
                    candidate_source_zones: vec![
                        rv1::ChoiceCandidateSourceZone::Graveyard as i32;
                        candidates.len()
                    ],
                    ..Default::default()
                },
            )),
        });
        self.state.pending_resolution = Some(PendingResolution {
            resolution_prompt_id: crate::state::ResolutionPromptId::UNASSIGNED,
            resolution_prompt_semantics: None,

            deciding_player: owner,
            presentation: PendingResolutionPresentation {
                source_object_id,
                candidates,
                min: count,
                max: count,
                ordered: true,
                prompt,
                choice_kind: custom::ChoiceKind::GraveyardCards,
                unique_names: false,
            },
            continuation: ResolutionContinuation::MassSacrificeGraveyardOrder {
                stack,
                sacrifice: Box::new(sacrifice),
            },
        });
        true
    }

    fn finish_mass_sacrifice_events(
        &mut self,
        sacrifice: PendingMassSacrifice,
        out: &mut Vec<rv1::RuledEvent>,
    ) {
        let mut triggers = sacrifice
            .departures
            .into_iter()
            .flat_map(|departure| {
                let player = departure.source.controller;
                sacrifice_events(
                    departure.source,
                    departure.was_creature,
                    player,
                    departure.died,
                )
            })
            .collect::<Vec<_>>();
        triggers.push(GameEvent::ZoneChanges(sacrifice.zone));
        self.fire_triggers(&triggers, out);
    }

    pub(super) fn finish_mass_sacrifice_graveyard_order(
        &mut self,
        pending: PendingResolution,
        chosen: &[ObjectId],
    ) -> Result<RuledEventBatch, EngineError> {
        let ResolutionContinuation::MassSacrificeGraveyardOrder { stack, sacrifice } =
            &pending.continuation
        else {
            return Err(EngineError::Illegal("mass sacrifice continuation missing"));
        };
        // Revalidate all surviving arrivals, including replacement exits, without erasing history.
        let current = sacrifice.live_validation.iter().all(|receipt| {
            self.state
                .objects
                .get(&receipt.before.object_id)
                .is_some_and(|object| {
                    object.zone == receipt.destination
                        && object.owner == receipt.before.owner
                        && self.state.zone_change_generation.get(&object.id).copied()
                            == Some(receipt.destination_generation)
                })
        });
        let Some(owner_index) = self.state.player_idx(pending.deciding_player) else {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("graveyard owner missing"));
        };
        let positions = self.state.players[owner_index]
            .graveyard
            .iter()
            .enumerate()
            .filter_map(|(position, oid)| {
                pending
                    .presentation
                    .candidates
                    .contains(oid)
                    .then_some(position)
            })
            .collect::<Vec<_>>();
        if !current || positions.len() != chosen.len() {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "simultaneous graveyard arrivals became stale",
            ));
        }
        let stack = stack.clone();
        let sacrifice = *sacrifice.clone();
        for (position, &oid) in positions.into_iter().zip(chosen) {
            self.state.players[owner_index].graveyard[position] = oid;
        }
        let mut events = Vec::new();
        if self.advance_mass_sacrifice_order(stack.clone(), sacrifice, &mut events) {
            return Ok(events::finish_with_events(self, events));
        }
        self.complete_mass_sacrifice_order(stack, events)
    }

    fn complete_mass_sacrifice_order(
        &mut self,
        stack: Option<ParkedStackResolution>,
        mut events: Vec<rv1::RuledEvent>,
    ) -> Result<RuledEventBatch, EngineError> {
        if self.drain_immediate_observer_actions(stack.clone(), &mut events)? {
            return Ok(events::finish_with_events(self, events));
        }
        if let Some(stack) = stack {
            self.complete_parked_resolution_with_previous(
                stack.item,
                stack.resume_effect_index,
                stack.previous_result,
                events,
            )
        } else {
            Ok(events::finish_with_events(self, events))
        }
    }

    pub(super) fn refresh_mass_sacrifice_departure(
        &mut self,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        let Some(mut pending) = self.state.pending_resolution.clone() else {
            return Ok(());
        };
        let ResolutionContinuation::MassSacrificeGraveyardOrder { stack, sacrifice } =
            &mut pending.continuation
        else {
            return Ok(());
        };
        let live_owners: HashSet<_> = self
            .state
            .players
            .iter()
            .filter(|player| !player.has_lost)
            .map(|player| player.id)
            .collect();
        sacrifice
            .live_validation
            .retain(|receipt| live_owners.contains(&receipt.before.owner));
        let live_ids: HashSet<_> = sacrifice
            .live_validation
            .iter()
            .map(|receipt| receipt.before.object_id)
            .collect();
        sacrifice.remaining.retain_mut(|(owner, ids)| {
            ids.retain(|oid| live_ids.contains(oid));
            live_owners.contains(owner) && ids.len() > 1
        });
        let current = sacrifice.live_validation.iter().all(|receipt| {
            self.state
                .objects
                .get(&receipt.before.object_id)
                .is_some_and(|object| {
                    object.zone == receipt.destination
                        && object.owner == receipt.before.owner
                        && self.state.zone_change_generation.get(&object.id).copied()
                            == Some(receipt.destination_generation)
                })
        });
        if !current {
            let stack = stack.clone();
            let sacrifice = *sacrifice.clone();
            self.state.pending_resolution = None;
            self.finish_mass_sacrifice_events(sacrifice, events);
            return self.abandon_participating_resolution(stack, events);
        }
        if live_owners.contains(&pending.deciding_player) {
            self.state.pending_resolution = Some(pending);
            return Ok(());
        }
        let stack = stack.clone();
        let sacrifice = *sacrifice.clone();
        self.state.pending_resolution = None;
        let mut resumed = Vec::new();
        if !self.advance_mass_sacrifice_order(stack.clone(), sacrifice, &mut resumed) {
            resumed = self.complete_mass_sacrifice_order(stack, resumed)?.events;
        }
        events.extend(resumed);
        Ok(())
    }
}
