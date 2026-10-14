use super::*;

impl GameEngine {
    /// Queue target selection for copied spells one at a time. Every copy gets its own CR
    /// 707.10c choice, and the next prompt is installed before target-chosen triggers are fired so
    /// priority cannot appear between copies.
    pub(in crate::engine) fn begin_copy_target_choices(
        &mut self,
        copies: Vec<ParkedStackResolution>,
        copy_source_object_id: ObjectId,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        let mut copies = copies.into_iter();
        let Some(stack) = copies.next() else {
            return Ok(());
        };
        let occurrences = copy_target_occurrences(&stack.item);
        if occurrences.is_empty() {
            return Err(EngineError::ProgramInvariant(
                "copy target choice was queued without target occurrences",
            ));
        }
        self.publish_copy_target_occurrence(
            CopyTargetChoiceProgress {
                stack,
                copy_source_object_id,
                remaining: copies.collect(),
                occurrences,
                cursor: 0,
                selected_targets: Vec::new(),
            },
            events,
        )
    }

    fn publish_copy_target_occurrence(
        &mut self,
        progress: CopyTargetChoiceProgress,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        let occurrence =
            progress
                .occurrences
                .get(progress.cursor)
                .ok_or(EngineError::ProgramInvariant(
                    "copy target occurrence cursor is out of range",
                ))?;
        let group = self.copy_target_group(&progress.stack.item, occurrence)?;
        let mut object_offers = Vec::new();
        for (ids, kind) in [
            (&group.valid_permanent_ids, rv1::TargetRefKind::Permanent),
            (&group.valid_stack_ids, rv1::TargetRefKind::Stack),
            (&group.valid_graveyard_ids, rv1::TargetRefKind::Graveyard),
        ] {
            for &object_id in ids {
                object_offers.push(CopyTargetObjectOffer {
                    object_id,
                    kind: kind as i32,
                    zone_change_generation: self.state.objects.contains_key(&object_id).then(
                        || {
                            self.state
                                .zone_change_generation
                                .get(&object_id)
                                .copied()
                                .unwrap_or(0)
                        },
                    ),
                });
            }
        }
        object_offers.sort_by_key(|offer| offer.object_id);
        object_offers.dedup_by_key(|offer| offer.object_id);
        let mut offered_players = self
            .state
            .players
            .iter()
            .filter(|player| {
                !player.has_lost
                    && ((group.can_target_self && player.id == progress.stack.item.controller)
                        || (group.can_target_opponent
                            && self
                                .state
                                .are_opponents(player.id, progress.stack.item.controller)))
            })
            .map(|player| player.id)
            .collect::<Vec<_>>();
        offered_players.sort_unstable();

        let candidates = object_offers
            .iter()
            .map(|offer| offer.object_id)
            .collect::<Vec<_>>();
        let candidate_card_ids = candidates
            .iter()
            .map(|object_id| {
                self.state
                    .objects
                    .get(object_id)
                    .map(|object| object.card_id.clone())
                    .unwrap_or_default()
            })
            .collect();
        let candidate_names = candidates
            .iter()
            .map(|object_id| {
                self.state
                    .objects
                    .get(object_id)
                    .and_then(|object| self.registry.get(&object.card_id))
                    .map(|definition| definition.name.clone())
                    .unwrap_or_else(|| format!("[object {object_id}]"))
            })
            .collect();
        let copied_name = self
            .registry
            .get(&progress.stack.item.card_id)
            .and_then(|definition| definition.face(progress.stack.item.face_index))
            .map(|face| face.name.to_string())
            .unwrap_or_else(|| progress.stack.item.card_id.clone());
        let mode_label = progress
            .occurrences
            .get(progress.cursor)
            .and_then(|occurrence| occurrence.mode_id.as_ref())
            .and_then(|mode_id| {
                let face = self
                    .registry
                    .get(&progress.stack.item.card_id)?
                    .face(progress.stack.item.face_index)?;
                let modal = face.modal_spell.as_ref()?;
                let mode = modal.mode_by_id(mode_id)?;
                let mode_index = modal.mode_index(mode_id)?;
                let fallback = mode_fallback(&face.name, &mode.mode_id);
                self.state
                    .stack_presentations
                    .get(&progress.stack.item.id)?
                    .chosen_modes
                    .get(mode_index)
                    .map(|presentation| presentation.fallback_text.as_str())
                    .filter(|label| !label.trim().is_empty() && *label != fallback)
                    .map(str::to_owned)
            })
            .map(|label| format!(" for {label}"))
            .unwrap_or_default();
        let prompt = format!(
            "Target {} of {}{} for {copied_name} (copy): choose a new target or keep the original",
            progress.cursor + 1,
            progress.occurrences.len(),
            mode_label
        );
        events.push(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                rv1::ResolutionChoiceRequired {
                    deciding_player_id: progress.stack.item.controller,
                    source_object_id: progress.stack.item.id,
                    prompt_text: prompt.clone(),
                    choice_kind: custom::ChoiceKind::TargetObjects as i32,
                    candidate_object_ids: candidates.clone(),
                    candidate_card_ids,
                    candidate_names,
                    candidate_player_ids: offered_players.clone(),
                    min: 0,
                    max: 1,
                    ..Default::default()
                },
            )),
        });
        events.push(ev_log(prompt.clone()));
        self.state.pending_resolution = Some(PendingResolution {
            resolution_prompt_id: crate::state::ResolutionPromptId::UNASSIGNED,
            resolution_prompt_semantics: None,
            deciding_player: progress.stack.item.controller,
            presentation: PendingResolutionPresentation {
                source_object_id: progress.stack.item.id,
                candidates,
                min: 0,
                max: 1,
                ordered: false,
                prompt,
                choice_kind: custom::ChoiceKind::TargetObjects,
                unique_names: false,
            },
            continuation: ResolutionContinuation::CopyTargets {
                progress,
                offered_objects: object_offers,
                offered_players,
            },
        });
        Ok(())
    }

    fn copy_target_group(
        &self,
        item: &StackItem,
        occurrence: &CopyTargetOccurrence,
    ) -> Result<rv1::LegalTargetGroup, EngineError> {
        let face = self
            .registry
            .get(&item.card_id)
            .and_then(|definition| definition.face(item.face_index))
            .ok_or(EngineError::Illegal("copied spell face no longer exists"))?;
        let source = TargetSourceIdentity::for_stack_item(self, item);
        let targets = if let Some(mode_id) = &occurrence.mode_id {
            let mode = face
                .modal_spell
                .as_ref()
                .and_then(|modal| modal.mode_by_id(mode_id))
                .ok_or(EngineError::Illegal("copied modal mode no longer exists"))?;
            compute_spell_targets_with_program(
                self,
                item.controller,
                source,
                &mode.effects,
                mode.targeting.as_ref(),
                &[],
                face_mode_program(self, &item.card_id, &face.face_id, &mode.mode_id),
            )
        } else {
            compute_spell_targets_with_program(
                self,
                item.controller,
                source,
                &face.spell_effect,
                face.targeting.as_ref(),
                &[],
                face_program(self, &item.card_id, &face.face_id),
            )
        };
        targets
            .groups
            .into_iter()
            .find(|group| group.group_index == occurrence.group_index)
            .ok_or(EngineError::ProgramInvariant(
                "copy target occurrence references an unknown group",
            ))
    }

    /// Consume one typed choice for one occurrence. Empty selection is the explicit
    /// keep-the-original branch, so an illegal unchanged target remains available as a receipt.
    pub(super) fn finish_copy_target_occurrence_choice(
        &mut self,
        pending: PendingResolution,
        answer: &rv1::SubmitResolutionChoice,
    ) -> Result<RuledEventBatch, EngineError> {
        if answer.decision != rv1::ResolutionChoiceDecision::Unspecified as i32
            || answer.selected_branch_index != 0
            || answer.cast_spell.is_some()
            || answer.spell_cast_announcement.is_some()
            || answer.chosen_combat_defender.is_some()
            || answer.payment.is_some()
            || !answer.restricted_mana.is_empty()
            || answer.chosen_object_ids.len() + answer.chosen_player_ids.len() > 1
        {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "invalid copy-target occurrence answer",
            ));
        }
        let (mut progress, offered_objects, offered_players) = match &pending.continuation {
            ResolutionContinuation::CopyTargets {
                progress,
                offered_objects,
                offered_players,
            } => (
                progress.clone(),
                offered_objects.clone(),
                offered_players.clone(),
            ),
            _ => return Err(EngineError::Illegal("copy-target continuation missing")),
        };
        let Some(occurrence) = progress.occurrences.get(progress.cursor).cloned() else {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::ProgramInvariant(
                "copy target occurrence cursor is out of range",
            ));
        };
        if progress.selected_targets.len() != progress.cursor {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::ProgramInvariant(
                "copy target receipts do not match the occurrence cursor",
            ));
        }

        let mut events = Vec::new();
        let replacement = if let Some(&object_id) = answer.chosen_object_ids.first() {
            let Some(offer) = offered_objects
                .iter()
                .find(|offer| offer.object_id == object_id)
            else {
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal(
                    "chosen object was not offered for this copy-target occurrence",
                ));
            };
            let current_generation = self.state.objects.contains_key(&object_id).then(|| {
                self.state
                    .zone_change_generation
                    .get(&object_id)
                    .copied()
                    .unwrap_or(0)
            });
            if current_generation != offer.zone_change_generation {
                self.publish_copy_target_occurrence(progress, &mut events)?;
                return Ok(finish_with_events(self, events));
            }
            Some(StackTarget {
                object_id,
                group_index: occurrence.group_index,
                damage_amount: occurrence.original.damage_amount,
                kind: offer.kind,
                zone_change_generation: offer.zone_change_generation,
                required_controller: occurrence.original.required_controller,
            })
        } else if let Some(&player_id) = answer.chosen_player_ids.first() {
            if !offered_players.contains(&player_id)
                || self
                    .state
                    .players
                    .iter()
                    .find(|candidate| candidate.id == player_id)
                    .is_none_or(|candidate| candidate.has_lost)
            {
                self.publish_copy_target_occurrence(progress, &mut events)?;
                return Ok(finish_with_events(self, events));
            }
            Some(StackTarget {
                object_id: player_id as ObjectId,
                group_index: occurrence.group_index,
                damage_amount: occurrence.original.damage_amount,
                kind: rv1::TargetRefKind::Player as i32,
                zone_change_generation: None,
                required_controller: occurrence.original.required_controller,
            })
        } else {
            None
        };

        let selected = replacement.map_or_else(
            || occurrence.original,
            |candidate| {
                if same_copy_target_identity(&candidate, &occurrence.original) {
                    occurrence.original
                } else {
                    candidate
                }
            },
        );
        if !same_copy_target_identity(&selected, &occurrence.original)
            && self
                .validate_copy_target_occurrence(&progress.stack.item, &occurrence, &selected)
                .is_err()
        {
            self.publish_copy_target_occurrence(progress, &mut events)?;
            return Ok(finish_with_events(self, events));
        }
        progress.selected_targets.push(selected);

        if progress.cursor + 1 < progress.occurrences.len() {
            progress.cursor += 1;
            self.publish_copy_target_occurrence(progress, &mut events)?;
            return Ok(finish_with_events(self, events));
        }

        if self
            .validate_final_copy_target_vector(
                &progress.stack.item,
                &progress.occurrences,
                &progress.selected_targets,
            )
            .is_err()
        {
            // CR 115.7e evaluates the completed set. Reopen the occurrence sequence so an earlier
            // mode or group decision can be repaired instead of consuming the copy.
            events.push(ev_log(
                "Those targets cannot be chosen together; choose the copied spell's targets again"
                    .to_owned(),
            ));
            progress.cursor = 0;
            progress.selected_targets.clear();
            self.publish_copy_target_occurrence(progress, &mut events)?;
            return Ok(finish_with_events(self, events));
        }

        self.push_copy_with_target_receipts(
            progress.stack,
            progress.copy_source_object_id,
            progress.remaining,
            progress.selected_targets,
        )
    }

    fn validate_copy_target_occurrence(
        &self,
        item: &StackItem,
        occurrence: &CopyTargetOccurrence,
        target: &StackTarget,
    ) -> Result<(), EngineError> {
        let face = self
            .registry
            .get(&item.card_id)
            .and_then(|definition| definition.face(item.face_index))
            .ok_or(EngineError::Illegal("copied spell face no longer exists"))?;
        let schema = if let Some(mode_id) = &occurrence.mode_id {
            let mode = face
                .modal_spell
                .as_ref()
                .and_then(|modal| modal.mode_by_id(mode_id))
                .ok_or(EngineError::Illegal("copied modal mode no longer exists"))?;
            target_schema_with_program(
                &mode.effects,
                mode.targeting.as_ref(),
                face_mode_program(self, &item.card_id, &face.face_id, &mode.mode_id),
            )
        } else {
            target_schema_with_program(
                &face.spell_effect,
                face.targeting.as_ref(),
                face_program(self, &item.card_id, &face.face_id),
            )
        };
        let group = schema.groups.get(occurrence.group_index as usize).ok_or(
            EngineError::ProgramInvariant(
                "copy target occurrence references an unknown target group",
            ),
        )?;
        if target.zone_change_generation.is_some_and(|expected| {
            self.state
                .zone_change_generation
                .get(&target.object_id)
                .copied()
                .unwrap_or(0)
                != expected
        }) {
            return Err(EngineError::Illegal("copy target generation is stale"));
        }
        let source = TargetSourceIdentity::for_stack_item(self, item);
        for binding in &group.bindings {
            target_role_legality_error(
                self,
                binding.role,
                target.object_id,
                item.controller,
                source,
                TriggerContext::default(),
            )?;
        }
        Ok(())
    }

    fn validate_final_copy_target_vector(
        &self,
        item: &StackItem,
        occurrences: &[CopyTargetOccurrence],
        selected: &[StackTarget],
    ) -> Result<(), EngineError> {
        if occurrences.len() != selected.len() {
            return Err(EngineError::Illegal(
                "wrong number of copied target occurrences",
            ));
        }
        let face = self
            .registry
            .get(&item.card_id)
            .and_then(|definition| definition.face(item.face_index))
            .ok_or(EngineError::Illegal("copied spell face no longer exists"))?;
        let mut mode_ids = Vec::<Option<tricerules_card_model::ModeId>>::new();
        for occurrence in occurrences {
            if !mode_ids.contains(&occurrence.mode_id) {
                mode_ids.push(occurrence.mode_id.clone());
            }
        }
        for mode_id in mode_ids {
            let (effects, targeting, program) = if let Some(mode_id) = &mode_id {
                let mode = face
                    .modal_spell
                    .as_ref()
                    .and_then(|modal| modal.mode_by_id(mode_id))
                    .ok_or(EngineError::Illegal("copied modal mode no longer exists"))?;
                (
                    mode.effects.as_slice(),
                    mode.targeting.as_ref(),
                    face_mode_program(self, &item.card_id, &face.face_id, &mode.mode_id),
                )
            } else {
                (
                    face.spell_effect.as_slice(),
                    face.targeting.as_ref(),
                    face_program(self, &item.card_id, &face.face_id),
                )
            };
            let schema = target_schema_with_program(effects, targeting, program);
            let owner_occurrences = occurrences
                .iter()
                .enumerate()
                .filter(|(_, occurrence)| occurrence.mode_id == mode_id)
                .collect::<Vec<_>>();
            for (group_index, group) in schema.groups.iter().enumerate() {
                let group_targets = owner_occurrences
                    .iter()
                    .filter(|(_, occurrence)| occurrence.group_index as usize == group_index)
                    .map(|(index, _)| &selected[*index])
                    .collect::<Vec<_>>();
                if group_targets.len() < group.min as usize
                    || group_targets.len() > group.max as usize
                {
                    return Err(EngineError::Illegal(
                        "copy target group cardinality is invalid",
                    ));
                }
                let mut within_group = HashSet::new();
                for target in &group_targets {
                    if !within_group.insert(copy_target_identity(target)) {
                        return Err(EngineError::Illegal("duplicate target in target group"));
                    }
                }
                for &other_group in group.distinct_from.iter() {
                    let other_targets = owner_occurrences
                        .iter()
                        .filter(|(_, occurrence)| occurrence.group_index == other_group)
                        .map(|(index, _)| &selected[*index]);
                    let other_targets = other_targets.collect::<Vec<_>>();
                    if group_targets.iter().any(|target| {
                        other_targets.iter().any(|other| {
                            copy_target_identity(target) == copy_target_identity(other)
                        })
                    }) {
                        return Err(EngineError::Illegal(
                            "target must be distinct across target groups",
                        ));
                    }
                }
                if group.same_graveyard {
                    let owners = group_targets
                        .iter()
                        .filter(|target| copy_target_is_current(self, target))
                        .filter_map(|target| {
                            self.state
                                .objects
                                .get(&target.object_id)
                                .filter(|object| object.zone == Zone::Graveyard)
                                .map(|object| object.owner)
                        })
                        .collect::<HashSet<_>>();
                    if owners.len() > 1 {
                        return Err(EngineError::Illegal(
                            "targets must be cards from the same graveyard",
                        ));
                    }
                }
            }
            for (effect_index, effect) in effects.iter().enumerate() {
                if !matches!(effect, SpellEffectKind::ExchangeArtifactWithGraveyard) {
                    continue;
                }
                let (departure_group, incoming_group) = schema
                    .artifact_exchange_groups(effect_index)
                    .ok_or(EngineError::ProgramInvariant(
                        "artifact exchange target groups are missing",
                    ))?;
                let target_for_group = |group_index: usize| {
                    owner_occurrences
                        .iter()
                        .find(|(_, occurrence)| occurrence.group_index as usize == group_index)
                        .map(|(index, _)| &selected[*index])
                };
                if let (Some(departure), Some(incoming)) = (
                    target_for_group(departure_group),
                    target_for_group(incoming_group),
                ) {
                    if copy_target_is_current(self, departure)
                        && copy_target_is_current(self, incoming)
                        && artifact_exchange_player(self, departure.object_id, incoming.object_id)
                            .is_none()
                    {
                        return Err(EngineError::Illegal(
                            "artifact targets must belong to the same player",
                        ));
                    }
                }
            }
        }
        for (index, occurrence) in occurrences.iter().enumerate() {
            if same_copy_target_identity(&selected[index], &occurrence.original) {
                continue;
            }
            self.validate_copy_target_occurrence(item, occurrence, &selected[index])?;
        }
        Ok(())
    }

    pub(super) fn copy_target_choice_pending(&self, pending: &PendingResolution) -> bool {
        matches!(
            &pending.continuation,
            ResolutionContinuation::CopyTargets { .. }
        )
    }

    /// Push the copy only after every target occurrence and CR 115.7e relationship has passed.
    fn push_copy_with_target_receipts(
        &mut self,
        stack: ParkedStackResolution,
        copy_source_object_id: ObjectId,
        remaining: Vec<ParkedStackResolution>,
        selected_targets: Vec<StackTarget>,
    ) -> Result<RuledEventBatch, EngineError> {
        let copy_id = stack.item.id;
        let card_id = stack.item.card_id.clone();
        let face_index = stack.item.face_index;
        let controller = stack.item.controller;
        let prototyped = stack.item.prototyped;
        let mut copy_item = stack.item;
        if copy_item.chosen_modes.is_empty() {
            copy_item.targets = selected_targets.clone();
        } else {
            let mut selected_iter = selected_targets.iter().cloned();
            for mode in &mut copy_item.chosen_modes {
                for target in &mut mode.targets {
                    *target = selected_iter.next().ok_or(EngineError::ProgramInvariant(
                        "copied modal target receipt is missing",
                    ))?;
                }
            }
            if selected_iter.next().is_some() {
                return Err(EngineError::ProgramInvariant(
                    "copied spell has extra target receipts",
                ));
            }
        }

        let copied_name = self
            .registry
            .get(&card_id)
            .and_then(|d| d.face(face_index))
            .map(|f| f.name.to_string())
            .unwrap_or_else(|| card_id.clone());

        let selected_targets: Vec<_> = if copy_item.chosen_modes.is_empty() {
            copy_item.targets.to_vec()
        } else {
            copy_item
                .chosen_modes
                .iter()
                .flat_map(|mode| mode.targets.iter().cloned())
                .collect()
        };
        let published_targets: Vec<_> = selected_targets
            .iter()
            .map(|target| rv1::TargetRef {
                object_id: target.object_id,
                damage_amount: target.damage_amount,
                group_index: target.group_index,
                kind: target.kind,
                expected_zone_change_generation: target.zone_change_generation,
            })
            .collect();
        let event_targets = selected_targets;
        let chosen_cast_cost_labels = copy_item
            .cast_cost_receipts
            .iter()
            .map(|receipt| receipt.label.clone())
            .collect();
        let copy_presentation = self
            .state
            .stack_presentations
            .get(&copy_id)
            .cloned()
            .unwrap_or_default();
        let (chosen_mode_indices, chosen_mode_labels) = if copy_item.chosen_modes.is_empty() {
            (Vec::new(), Vec::new())
        } else {
            let modal = self
                .registry
                .get(&card_id)
                .and_then(|definition| definition.face(face_index))
                .and_then(|face| face.modal_spell.as_ref())
                .ok_or(EngineError::Illegal(
                    "copied modal spell has no mode definition",
                ))?;
            let indices = copy_item
                .chosen_modes
                .iter()
                .map(|chosen| {
                    modal
                        .mode_index(&chosen.mode_id)
                        .map(|index| index as u32)
                        .ok_or(EngineError::Illegal("copied modal mode no longer exists"))
                })
                .collect::<Result<Vec<_>, _>>()?;
            let labels = copy_item
                .chosen_modes
                .iter()
                .map(|chosen| {
                    modal
                        .mode_by_id(&chosen.mode_id)
                        .map(|mode| mode_fallback(&copied_name, &mode.mode_id))
                        .ok_or(EngineError::Illegal("copied modal mode no longer exists"))
                })
                .collect::<Result<Vec<_>, _>>()?;
            (indices, labels)
        };
        self.state.stack.push(copy_item);
        self.state.passes_since_stack_change = 0;

        let target_ids = event_targets
            .iter()
            .map(|target| target.object_id)
            .collect::<Vec<_>>();
        let tgt_log = format_spell_targets_log(&self.state, self.registry, &target_ids);
        let mut ev = vec![
            rv1::RuledEvent {
                ev: Some(rv1::ruled_event::Ev::StackPushed(rv1::StackPushed {
                    object_id: copy_id,
                    description: copied_name.clone(),
                    targets: published_targets,
                    ability_annotation: "(copy)".to_string(),
                    card_id: card_id.clone(),
                    card_display_name: self
                        .registry
                        .get(&card_id)
                        .and_then(|definition| definition.face_display_name(face_index))
                        .unwrap_or(&copied_name)
                        .to_string(),
                    is_prepare_spell: self
                        .registry
                        .get(&card_id)
                        .is_some_and(|d| d.layout == Layout::Preparation && face_index == 1),
                    is_copy: true,
                    is_triggered: false,
                    copy_source_object_id,
                    chosen_mode_indices,
                    chosen_mode_labels,
                    chosen_cast_cost_labels,
                    source_token_identity: None,
                    primary_presentation: copy_presentation.primary,
                    chosen_mode_presentations: copy_presentation.chosen_modes,
                    chosen_cast_cost_presentations: copy_presentation.chosen_cast_costs,
                    prototyped,
                })),
            },
            ev_log(format!(
                "{copied_name} copy targeting{tgt_log} (P{controller})"
            )),
        ];

        let has_remaining = !remaining.is_empty();
        if has_remaining {
            self.begin_copy_target_choices(remaining, copy_source_object_id, &mut ev)?;
        }
        self.fire_triggers(
            &[GameEvent::TargetsChosen {
                controller,
                source: TargetingSourceKind::SpellCopy,
                stack_object: StackObjectRef {
                    object_id: copy_id,
                    zone_change_generation: None,
                },
                targets: event_targets,
            }],
            &mut ev,
        );

        if !has_remaining {
            if let Some(i) = self.state.player_idx(self.state.active_player_id()) {
                self.state.priority_idx = i;
            }
            ev.push(ev_priority_changed(self));
        }

        Ok(finish_with_events(self, ev))
    }
}

fn copy_target_occurrences(item: &StackItem) -> Vec<CopyTargetOccurrence> {
    let mut occurrences = Vec::new();
    let mut append = |mode_id: Option<tricerules_card_model::ModeId>, targets: &[StackTarget]| {
        let mut slots_by_group = HashMap::<u32, u32>::new();
        for target in targets {
            let slot_index = slots_by_group.entry(target.group_index).or_default();
            occurrences.push(CopyTargetOccurrence {
                mode_id: mode_id.clone(),
                group_index: target.group_index,
                slot_index: *slot_index,
                original: *target,
            });
            *slot_index += 1;
        }
    };
    if item.chosen_modes.is_empty() {
        append(None, &item.targets);
    } else {
        for mode in &item.chosen_modes {
            append(Some(mode.mode_id.clone()), &mode.targets);
        }
    }
    occurrences
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum CopyTargetIdentity {
    Player(PlayerId),
    Object(ObjectId, Option<u64>),
}

fn copy_target_identity(target: &StackTarget) -> CopyTargetIdentity {
    match rv1::TargetRefKind::try_from(target.kind) {
        Ok(rv1::TargetRefKind::Player) => CopyTargetIdentity::Player(target.object_id as PlayerId),
        Ok(
            rv1::TargetRefKind::Permanent
            | rv1::TargetRefKind::Stack
            | rv1::TargetRefKind::Graveyard,
        ) => CopyTargetIdentity::Object(target.object_id, target.zone_change_generation),
        Ok(rv1::TargetRefKind::Unspecified) | Err(_) => {
            if target.zone_change_generation.is_some() {
                CopyTargetIdentity::Object(target.object_id, target.zone_change_generation)
            } else {
                CopyTargetIdentity::Player(target.object_id as PlayerId)
            }
        }
    }
}

fn same_copy_target_identity(left: &StackTarget, right: &StackTarget) -> bool {
    copy_target_identity(left) == copy_target_identity(right)
}

fn copy_target_is_current(engine: &GameEngine, target: &StackTarget) -> bool {
    match rv1::TargetRefKind::try_from(target.kind) {
        Ok(rv1::TargetRefKind::Player) => engine
            .state
            .players
            .iter()
            .any(|player| player.id as ObjectId == target.object_id && !player.has_lost),
        Ok(rv1::TargetRefKind::Stack) if target.zone_change_generation.is_none() => engine
            .state
            .stack
            .iter()
            .any(|item| item.id == target.object_id),
        Ok(rv1::TargetRefKind::Permanent | rv1::TargetRefKind::Graveyard)
        | Ok(rv1::TargetRefKind::Stack)
        | Ok(rv1::TargetRefKind::Unspecified)
        | Err(_) => target.zone_change_generation.is_some_and(|generation| {
            engine.state.objects.contains_key(&target.object_id)
                && engine
                    .state
                    .zone_change_generation
                    .get(&target.object_id)
                    .copied()
                    .unwrap_or(0)
                    == generation
        }),
    }
}
