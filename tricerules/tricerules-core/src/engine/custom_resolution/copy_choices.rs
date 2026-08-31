use super::*;

impl GameEngine {
    /// CR 707.10c: check the targets chosen for a copy without mutating anything, so a rejection
    /// can leave the pending choice intact for the player to try again.
    ///
    /// Returns one target vector per chosen mode (empty for a nonmodal copy), so the caller
    /// re-slices nothing and the mode boundaries are computed exactly once.
    fn validated_copy_targets(
        &self,
        pending: &PendingResolution,
        chosen: &[u32],
    ) -> Result<Vec<Vec<ObjectId>>, EngineError> {
        let item = &pending
            .continuation
            .stack()
            .ok_or(EngineError::Illegal("copy-target continuation missing"))?
            .item;
        let controller = item.controller;
        let target_source = TargetSourceIdentity::for_stack_item(self, item);
        let face = self
            .registry
            .get(&item.card_id)
            .and_then(|definition| definition.face(item.face_index));

        if item.chosen_modes.is_empty() {
            let effects = face.map(|f| f.spell_effect.to_vec()).unwrap_or_default();
            self.validate_copy_target_group(
                controller,
                target_source,
                &effects,
                chosen,
                &item.targets,
            )?;
            return Ok(vec![]);
        }

        let modal = face
            .and_then(|f| f.modal_spell.as_ref())
            .ok_or(EngineError::Illegal(
                "copied modal spell has no mode definition",
            ))?;
        let mut per_mode = Vec::with_capacity(item.chosen_modes.len());
        let mut offset = 0;
        for chosen_mode in &item.chosen_modes {
            let end = offset + chosen_mode.targets.len();
            let mode_targets = chosen
                .get(offset..end)
                .ok_or(EngineError::Illegal("wrong number of copied modal targets"))?;
            let mode = modal
                .mode_by_id(&chosen_mode.mode_id)
                .ok_or(EngineError::Illegal("copied modal mode no longer exists"))?;
            self.validate_copy_target_group(
                controller,
                target_source,
                &mode.effects,
                mode_targets,
                &chosen_mode.targets,
            )?;
            per_mode.push(mode_targets.to_vec());
            offset = end;
        }
        if offset != chosen.len() {
            return Err(EngineError::Illegal("wrong number of copied modal targets"));
        }
        Ok(per_mode)
    }

    /// CR 707.10c: a target the copy's controller left *unchanged* stays legal even if it would
    /// not be legal to choose now — which is exactly why [`copy_target_spell`] offers the
    /// original targets as candidates. Only a target that was actually changed has to be legal.
    ///
    /// Count, candidate membership and global uniqueness are already enforced by
    /// [`GameEngine::submit_resolution_choice`], so this validates legality per target.
    fn validate_copy_target_group(
        &self,
        controller: PlayerId,
        target_source: TargetSourceIdentity,
        effects: &[SpellEffectKind],
        chosen: &[ObjectId],
        original: &[StackTarget],
    ) -> Result<(), EngineError> {
        for (index, &object_id) in chosen.iter().enumerate() {
            if original.get(index).map(|target| target.object_id) == Some(object_id) {
                continue;
            }
            let original_target = original
                .get(index)
                .ok_or(EngineError::Illegal("wrong number of copied targets"))?;
            let target_ref = rv1::TargetRef {
                object_id,
                damage_amount: original_target.damage_amount,
                group_index: original_target.group_index,
                kind: original_target.kind,
            };
            validate_spell_targets(
                self,
                controller,
                target_source,
                effects,
                None,
                std::slice::from_ref(&target_ref),
            )?;
        }
        Ok(())
    }

    /// CR 707.10c: the copy's controller has chosen new targets for the copy. Push the copy to
    /// the stack with the chosen targets and hand priority back to the active player.
    pub(super) fn finish_copy_target_choice(
        &mut self,
        pending: PendingResolution,
        chosen: &[u32],
    ) -> Result<RuledEventBatch, EngineError> {
        let (stack, copy_source_object_id) = match &pending.continuation {
            ResolutionContinuation::CopyTargets {
                stack,
                copy_source_object_id,
            } => (stack.clone(), *copy_source_object_id),
            _ => return Err(EngineError::Illegal("copy-target continuation missing")),
        };
        let copy_id = stack.item.id;
        let card_id = stack.item.card_id.clone();
        let face_index = stack.item.face_index;
        let controller = stack.item.controller;

        // Validate before consuming `pending`: this choice was already taken out of
        // `state.pending_resolution`, so any early return has to put it back or the copy is lost
        // and the client waits forever on a prompt the engine has forgotten.
        let per_mode_targets = match self.validated_copy_targets(&pending, chosen) {
            Ok(targets) => targets,
            Err(e) => {
                self.state.pending_resolution = Some(pending);
                return Err(e);
            }
        };

        // Everything below this point is infallible.
        let mut copy_item = stack.item;
        for (chosen_mode, mode_targets) in copy_item.chosen_modes.iter_mut().zip(per_mode_targets) {
            for (target, object_id) in chosen_mode.targets.iter_mut().zip(mode_targets) {
                target.object_id = object_id;
            }
        }
        for (target, object_id) in copy_item.targets.iter_mut().zip(chosen) {
            target.object_id = *object_id;
        }

        let copied_name = self
            .registry
            .get(&card_id)
            .and_then(|d| d.face(face_index))
            .map(|f| f.name.to_string())
            .unwrap_or_else(|| card_id.clone());

        let published_targets = copy_item
            .targets
            .iter()
            .map(|target| rv1::TargetRef {
                object_id: target.object_id,
                damage_amount: target.damage_amount,
                group_index: target.group_index,
                kind: target.kind,
            })
            .collect();
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
        self.state.stack.push(copy_item);
        self.state.passes_since_stack_change = 0;

        self.fire_triggers(&[GameEvent::TargetsChosen {
            controller,
            source: TargetingSourceKind::SpellCopy,
            stack_object: StackObjectRef {
                object_id: copy_id,
                zone_change_generation: None,
            },
            targets: chosen.to_vec(),
        }]);

        let tgt_log = format_spell_targets_log(&self.state, self.registry, chosen);
        let mut ev = vec![
            rv1::RuledEvent {
                ev: Some(rv1::ruled_event::Ev::StackPushed(rv1::StackPushed {
                    object_id: copy_id,
                    description: copied_name.clone(),
                    targets: published_targets,
                    ability_annotation: "(copy)".to_string(),
                    card_id: card_id.clone(),
                    is_copy: true,
                    is_triggered: false,
                    copy_source_object_id,
                    chosen_mode_indices: vec![],
                    chosen_mode_labels: vec![],
                    chosen_cast_cost_labels,
                    source_token_identity: None,
                    primary_presentation: copy_presentation.primary,
                    chosen_mode_presentations: copy_presentation.chosen_modes,
                    chosen_cast_cost_presentations: copy_presentation.chosen_cast_costs,
                })),
            },
            ev_log(format!(
                "{copied_name} copy targeting{tgt_log} (P{controller})"
            )),
        ];

        if let Some(i) = self.state.player_idx(self.state.active_player_id()) {
            self.state.priority_idx = i;
        }
        ev.push(ev_priority_changed(self));

        Ok(finish_with_events(self, ev))
    }
}
