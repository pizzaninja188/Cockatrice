//! Chaos Warp (Oracle 07a0cba9-8768-4fd9-a3d5-b0f83b4bf8e8): the legal target's owner
//! shuffles, publicly reveals the first actual card, then performs normal permanent entry.
//! Reviewed CMD source/rulings are pinned in build/new-goal/chaos-warp-preflight.
//! CR 111.7/111.8 defer token cessation, 303.4f/g governs noncast Aura attachment,
//! 400.7 guards incarnations, 608.2 preserves instruction order, 614/616 governs entry,
//! 701.20/701.24 governs reveal/shuffle, and 903.9b permits the commander redirect.
use super::*;

pub(super) fn chaos_warp(cx: &mut EffectCx<'_>) -> Result<EffectOutcome, EngineError> {
    if let Some(owner) = cx
        .top
        .chaos_warp_owner_instructions
        .get(&cx.effect_index)
        .copied()
    {
        return match cx.engine.reveal_chaos_warp_owner_top(
            ParkedStackResolution::new(cx.top.clone()),
            owner,
            cx.events,
        )? {
            Some(_) => Ok(EffectOutcome::Continue),
            None => Ok(EffectOutcome::Suspended),
        };
    }
    let Some(target) = cx.targets.first().copied().filter(|target| {
        target_filter_legal_at_resolution(
            cx.engine,
            tricerules_cards::primitives::chaos_warp_target_filter(),
            *target,
            cx.controller,
            TargetSourceIdentity::for_stack_item(cx.engine, cx.top),
            cx.top.trigger_context,
        )
    }) else {
        return Ok(EffectOutcome::Continue);
    };
    let owner = cx.engine.state.objects[&target].owner;
    let owner_index = cx
        .engine
        .state
        .player_idx(owner)
        .ok_or(EngineError::UnknownPlayer(owner))?;
    if cx.engine.state.players[owner_index]
        .declared_commander_object_ids
        .contains(&target)
    {
        let prompt =
            format!("P{owner}: put this commander in the command zone or its owner's library?");
        let branches = ["Command zone", "Library"]
            .into_iter()
            .enumerate()
            .map(|(index, label)| rv1::ResolutionBranchOption {
                branch_index: index as u32,
                label: label.into(),
                selectable: true,
                ..Default::default()
            })
            .collect();
        cx.events.push(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                rv1::ResolutionChoiceRequired {
                    variable_mana_contribution: false,
                    deciding_player_id: owner,
                    source_object_id: cx.top.id,
                    prompt_text: prompt.clone(),
                    choice_kind: rv1::ChoiceKind::ResolutionBranch as i32,
                    min: 1,
                    max: 1,
                    resolution_branches: branches,
                    ..Default::default()
                },
            )),
        });
        cx.engine.state.pending_resolution = Some(PendingResolution {
            deciding_player: owner,
            presentation: PendingResolutionPresentation {
                source_object_id: cx.top.id,
                candidates: vec![],
                min: 1,
                max: 1,
                ordered: false,
                unique_names: false,
                prompt,
                choice_kind: custom::ChoiceKind::ResolutionBranch,
            },
            continuation: ResolutionContinuation::ChaosWarpCommander {
                stack: ParkedStackResolution::new(cx.top.clone()),
                owner,
                target,
                target_generation: cx
                    .engine
                    .state
                    .zone_change_generation
                    .get(&target)
                    .copied()
                    .unwrap_or(0),
                stack_generation: cx
                    .engine
                    .state
                    .zone_change_generation
                    .get(&cx.top.id)
                    .copied()
                    .unwrap_or(0),
                effect_index: cx.effect_index,
            },
        });
        return Ok(EffectOutcome::ChaosWarpCommanderChoice);
    }
    zones::move_permanent_to_owners_library(
        cx.engine,
        cx.events,
        target,
        LibraryPlacement::Shuffle,
        cx.spell_label,
    )?;
    Ok(EffectOutcome::ChaosWarpOwnerInstructions(owner))
}

impl GameEngine {
    pub(in crate::engine) fn finish_chaos_warp_commander(
        &mut self,
        pending: PendingResolution,
        answer: &rv1::SubmitResolutionChoice,
        decision: rv1::ResolutionChoiceDecision,
    ) -> Result<RuledEventBatch, EngineError> {
        let ResolutionContinuation::ChaosWarpCommander {
            mut stack,
            owner,
            target,
            target_generation,
            stack_generation,
            effect_index,
        } = pending.continuation.clone()
        else {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "Chaos Warp commander continuation missing",
            ));
        };
        let correct_shape = decision == rv1::ResolutionChoiceDecision::SelectBranch
            && answer.selected_branch_index <= 1
            && answer.chosen_object_ids.is_empty()
            && answer.chosen_player_ids.is_empty()
            && answer.payment.is_none()
            && answer.restricted_mana.is_empty()
            && answer.cast_spell.is_none()
            && answer.spell_cast_announcement.is_none()
            && answer.chosen_combat_defender.is_none();
        let owner_live = self.state.player_idx(owner).is_some_and(|idx| {
            !self.state.players[idx].has_lost
                && self.state.players[idx]
                    .declared_commander_object_ids
                    .contains(&target)
        });
        let target_current = self
            .state
            .objects
            .get(&target)
            .is_some_and(|object| object.zone == Zone::Battlefield && object.owner == owner)
            && self
                .state
                .zone_change_generation
                .get(&target)
                .copied()
                .unwrap_or(0)
                == target_generation;
        let stack_current = if stack.item.is_copy || stack.item.ability_text.is_some() {
            true
        } else {
            self.state
                .objects
                .get(&stack.item.id)
                .is_some_and(|object| {
                    object.zone == Zone::Stack && object.card_id == stack.item.card_id
                })
                && self
                    .state
                    .zone_change_generation
                    .get(&stack.item.id)
                    .copied()
                    .unwrap_or(0)
                    == stack_generation
        };
        if !correct_shape
            || !owner_live
            || pending.deciding_player != owner
            || !target_current
            || !stack_current
            || stack.resume_effect_index != Some(effect_index)
            || stack
                .item
                .chaos_warp_owner_instructions
                .contains_key(&effect_index)
        {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "invalid or stale Chaos Warp commander choice",
            ));
        }
        let mut events = vec![];
        if answer.selected_branch_index == 0 {
            let snapshot = self.snapshot_zone_event();
            let leave = self.battlefield_leave_event(target);
            move_object_to_zone(&mut self.state, self.registry, target, Zone::Command, None)?;
            self.fire_zone_triggers(snapshot, leave.into_iter().collect(), &mut events);
            events.push(permanent_moved_event(
                &self.state,
                target,
                owner,
                rv1::permanent_moved::Destination::Command,
            ));
            shuffle_player_library_for_current_command(&mut self.state, owner);
            events.push(ev_log(format!("P{owner} shuffles their library.")));
        } else {
            zones::move_permanent_to_owners_library(
                self,
                &mut events,
                target,
                LibraryPlacement::Shuffle,
                "Chaos Warp",
            )?;
        }
        stack
            .item
            .chaos_warp_owner_instructions
            .insert(effect_index, owner);
        if self.drain_immediate_observer_actions(Some(stack.clone()), &mut events)? {
            return Ok(super::super::events::finish_with_events(self, events));
        }
        self.complete_parked_resolution_with_previous(
            stack.item,
            stack.resume_effect_index,
            stack.previous_result,
            events,
        )
    }

    pub(in crate::engine) fn refresh_chaos_warp_departure(
        &mut self,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        let Some(pending) = self.state.pending_resolution.as_ref() else {
            return Ok(());
        };
        let ResolutionContinuation::ChaosWarpCommander {
            stack,
            owner,
            target,
            target_generation,
            effect_index,
            ..
        } = &pending.continuation
        else {
            return Ok(());
        };
        let owner_live = self
            .state
            .player_idx(*owner)
            .is_some_and(|idx| !self.state.players[idx].has_lost);
        let target_current = self
            .state
            .objects
            .get(target)
            .is_some_and(|object| object.zone == Zone::Battlefield && object.owner == *owner)
            && self
                .state
                .zone_change_generation
                .get(target)
                .copied()
                .unwrap_or(0)
                == *target_generation;
        if owner_live && target_current {
            return Ok(());
        }
        let mut stack = stack.clone();
        let owner = *owner;
        let effect_index = *effect_index;
        if owner_live {
            // CR 800.4a may already have exiled a permanently foreign-controlled target.
            // Never move that new incarnation. CR 701.24c still requires the owner's shuffle.
            stack
                .item
                .chaos_warp_owner_instructions
                .insert(effect_index, owner);
            shuffle_player_library_for_current_command(&mut self.state, owner);
            events.push(ev_log(format!("P{owner} shuffles their library.")));
        } else {
            stack.resume_effect_index = Some(effect_index + 1);
            stack.previous_result = EffectResult::default();
        }
        self.state.pending_resolution = None;
        if self.drain_immediate_observer_actions(Some(stack.clone()), events)? {
            return Ok(());
        }
        let batch = self.complete_parked_resolution_with_previous(
            stack.item,
            stack.resume_effect_index,
            stack.previous_result,
            vec![],
        )?;
        events.extend(batch.events);
        Ok(())
    }

    /// The shuffle is already complete. Snapshot the first actual card, retaining tokens until
    /// the ordinary post-resolution SBA boundary (CR 111.7/111.8).
    pub(in crate::engine) fn reveal_chaos_warp_owner_top(
        &mut self,
        stack: ParkedStackResolution,
        owner: PlayerId,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<Option<ParkedStackResolution>, EngineError> {
        if self
            .state
            .player_idx(owner)
            .is_none_or(|idx| self.state.players[idx].has_lost)
        {
            return Ok(Some(stack));
        }
        let Some(object_id) = self.state.library_card_objects(owner).next() else {
            return Ok(Some(stack));
        };
        let generation = self
            .state
            .zone_change_generation
            .get(&object_id)
            .copied()
            .unwrap_or(0);
        let definition = self
            .registry
            .get(&self.state.objects[&object_id].card_id)
            .ok_or(EngineError::Illegal("revealed card definition missing"))?;
        let permanent = definition.primary_face().is_permanent();
        let mut receipt = super::super::reveals::reveal_choice(
            &self.state,
            self.registry,
            &[object_id],
            stack.item.id,
            "Chaos Warp",
        )
        .ok_or(EngineError::Illegal("Chaos Warp reveal missing"))?;
        // Stamp before cloning into an entry receipt: reconnect must retain this exact identity.
        receipt.reveal_id = format!("event:{}:{}", self.state.command_index, events.len());
        events.push(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::CardsRevealed(receipt.clone())),
        });
        if !permanent {
            return Ok(Some(stack));
        }
        let entry = BattlefieldEntryEvent {
            entry_reveal_receipts: vec![receipt],
            mana_colors_spent_to_cast: Default::default(),
            prepared: false,
            object_id,
            deciding_player: owner,
            destination_controller: owner,
            battle_protector: None,
            face_index: 0,
            unlock_room_door: None,
            chosen_x: 0,
            cast_by: None,
            cast_cost_receipts: Vec::new(),
            player_life_snapshot: self.player_life_snapshot(),
            tapped: false,
            set_types: None,
            chosen_basic_land_type: None,
            chosen_opponents: Vec::new(),
            entry_counters: BTreeMap::new(),
            entry_modifiers: Vec::new(),
            attached_to: None,
            pending_copy_candidate: None,
            pending_aura_recipient: None,
            accepted_aura_recipient: None,
            applied_effects: Vec::new(),
        };
        self.begin_zone_entry_batch(
            stack,
            vec![entry],
            Zone::Library,
            "Chaos Warp",
            Some(crate::state::ZoneEntryCompletion::ChaosWarpRevealedTop {
                library_owner: owner,
                object_id,
                generation,
            }),
            events,
        )
    }
}
