//! Kaya's bounded resolution-time legendary cast; source zones retain their own identity.
use super::events::ev_log;
use super::*;

impl GameEngine {
    fn current_legendary_cast_cohort(
        &self,
        player: PlayerId,
    ) -> Vec<crate::state::LegendaryCastCandidate> {
        let Some(index) = self.state.player_idx(player) else {
            return vec![];
        };
        let mut seen = BTreeSet::new();
        legal_actions::legendary_resolution_candidates(self, player)
            .into_iter()
            .filter(|(id, _, _)| seen.insert(*id))
            .map(|(id, zone, _)| crate::state::LegendaryCastCandidate {
                reference: crate::state::CardChoiceCandidateRef {
                    object_id: id,
                    zone_change_generation: self
                        .state
                        .zone_change_generation
                        .get(&id)
                        .copied()
                        .unwrap_or(0),
                    source_zone: zone,
                },
                hand_index: (zone == Zone::Hand).then(|| {
                    self.state.players[index]
                        .hand
                        .iter()
                        .position(|candidate| *candidate == id)
                        .expect("offered hand card") as u32
                }),
            })
            .collect()
    }

    pub(super) fn refresh_legendary_cast_cohort(&mut self) {
        let Some(player) = self
            .state
            .pending_resolution
            .as_ref()
            .filter(|pending| self.legendary_resolution_cast_for(pending.deciding_player))
            .map(|pending| pending.deciding_player)
        else {
            return;
        };
        if self.state.pending_spell_cast.is_some() {
            return;
        }
        let current = self.current_legendary_cast_cohort(player);
        let pending = self
            .state
            .pending_resolution
            .as_mut()
            .expect("legendary parent");
        let ResolutionContinuation::LegendarySpellCast { candidates, .. } =
            &mut pending.continuation
        else {
            unreachable!()
        };
        if *candidates != current {
            *candidates = current;
            pending.resolution_prompt_id = crate::state::ResolutionPromptId::UNASSIGNED;
            pending.resolution_prompt_semantics = None;
        }
    }

    pub(super) fn legendary_cast_candidate_is_current(
        &self,
        player: PlayerId,
        oid: ObjectId,
    ) -> bool {
        let Some(pending) = self.state.pending_resolution.as_ref() else {
            return false;
        };
        let ResolutionContinuation::LegendarySpellCast { candidates, .. } = &pending.continuation
        else {
            return false;
        };
        *candidates == self.current_legendary_cast_cohort(player)
            && candidates
                .iter()
                .any(|candidate| candidate.reference.object_id == oid)
    }

    pub(super) fn legendary_resolution_cast_for(&self, player: PlayerId) -> bool {
        self.state
            .pending_resolution
            .as_ref()
            .is_some_and(|pending| {
                pending.deciding_player == player
                    && matches!(
                        pending.continuation,
                        ResolutionContinuation::LegendarySpellCast { .. }
                    )
            })
    }

    pub(super) fn legendary_cast_source_zone(
        &self,
        player: PlayerId,
        oid: ObjectId,
    ) -> Option<Zone> {
        if !self.legendary_resolution_cast_for(player) || !self.state.is_card_object(oid) {
            return None;
        }
        let object = self.state.objects.get(&oid)?;
        let index = self.state.player_idx(player)?;
        match object.zone {
            Zone::Hand if self.state.players[index].hand.contains(&oid) => Some(Zone::Hand),
            Zone::Graveyard if self.state.players[index].graveyard.contains(&oid) => {
                Some(Zone::Graveyard)
            }
            Zone::Exile
                if object.owner == player
                    && self.state.players.iter().any(|p| p.exile.contains(&oid)) =>
            {
                let generation = self
                    .state
                    .zone_change_generation
                    .get(&oid)
                    .copied()
                    .unwrap_or(0);
                if object.face_down
                    && !self.state.hideaway_look_permissions.iter().any(
                        |((_, id, gen), viewers)| {
                            *id == oid && *gen == generation && viewers.contains(&player)
                        },
                    )
                {
                    return None;
                }
                Some(Zone::Exile)
            }
            _ => None,
        }
    }

    pub(super) fn legendary_cast_face_allowed(
        &self,
        player: PlayerId,
        oid: ObjectId,
        face_index: usize,
    ) -> bool {
        if self.legendary_cast_source_zone(player, oid).is_none() {
            return false;
        }
        let Some(definition) = self
            .state
            .objects
            .get(&oid)
            .and_then(|o| self.registry.get(&o.card_id))
        else {
            return false;
        };
        definition.face_available_from_hand(face_index)
            && definition.face(face_index).is_some_and(|face| {
                !face.is_land
                    && face.supertypes.iter().any(|s| s == "Legendary")
                    && self.legendary_spell_cast_allowed(player, face)
                    && self.spell_cast_permitted_by_emblems(player, face)
            })
    }

    pub(super) fn begin_legendary_resolution_cast(
        &mut self,
        player: PlayerId,
        stack: ParkedStackResolution,
        events: &mut Vec<rv1::RuledEvent>,
    ) {
        let presentation = PendingResolutionPresentation {
            source_object_id: stack.item.id,
            candidates: vec![],
            min: 0,
            max: 1,
            ordered: false,
            unique_names: false,
            prompt: "You may cast a legendary spell from your hand, graveyard, or a card you own in exile without paying its mana cost.".into(),
            choice_kind: rv1::ChoiceKind::SpecialCast,
        };
        let continuation = ResolutionContinuation::LegendarySpellCast {
            stack,
            candidates: vec![],
            undo_history_start: self.state.undoable_mana_abilities.len(),
        };
        self.state.pending_resolution = Some(PendingResolution::new_choice(
            player,
            presentation,
            continuation,
        ));
        self.refresh_legendary_cast_cohort();
        // Always wait for the private optional decision, including an empty cohort.
        // Skipping only empty hands would disclose their legendary-spell eligibility.
        events.push(
            self.legendary_resolution_choice_event()
                .expect("legendary parent exists"),
        );
    }

    pub(super) fn legendary_resolution_choice_event(&self) -> Option<rv1::RuledEvent> {
        let pending = self.state.pending_resolution.as_ref()?;
        if !self.legendary_resolution_cast_for(pending.deciding_player)
            || self.state.pending_spell_cast.is_some()
        {
            return None;
        }
        let actions = legal_actions::legendary_resolution_candidates(self, pending.deciding_player);
        let mut seen = BTreeSet::new();
        let actions: Vec<_> = actions.into_iter().filter(|a| seen.insert(a.0)).collect();
        Some(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                rv1::ResolutionChoiceRequired {
                    resolution_prompt_id: pending.resolution_prompt_id.get(),
                    deciding_player_id: pending.deciding_player,
                    source_object_id: pending.presentation.source_object_id,
                    prompt_text: pending.presentation.prompt.clone(),
                    choice_kind: rv1::ChoiceKind::SpecialCast as i32,
                    min: 0,
                    max: 1,
                    candidate_object_ids: actions.iter().map(|a| a.0).collect(),
                    candidate_names: actions.iter().map(|a| a.2.clone()).collect(),
                    candidate_card_ids: actions
                        .iter()
                        .map(|a| self.state.objects[&a.0].card_id.clone())
                        .collect(),
                    candidate_source_zones: actions
                        .iter()
                        .map(|a| match a.1 {
                            Zone::Hand => rv1::ChoiceCandidateSourceZone::Hand,
                            Zone::Graveyard => rv1::ChoiceCandidateSourceZone::Graveyard,
                            Zone::Exile => rv1::ChoiceCandidateSourceZone::Exile,
                            _ => unreachable!(),
                        } as i32)
                        .collect(),
                    candidate_selectable: vec![true; actions.len()],
                    ..Default::default()
                },
            )),
        })
    }

    pub(super) fn finish_legendary_cast_choice(
        &mut self,
        pending: PendingResolution,
        answer: &rv1::SubmitResolutionChoice,
        decision: rv1::ResolutionChoiceDecision,
    ) -> Result<RuledEventBatch, EngineError> {
        if !answer.chosen_object_ids.is_empty()
            || !answer.chosen_player_ids.is_empty()
            || answer.selected_branch_index != 0
            || answer.chosen_combat_defender.is_some()
            || answer.payment.is_some()
            || !answer.restricted_mana.is_empty()
            || answer.cast_spell.is_some()
            || !matches!(
                decision,
                rv1::ResolutionChoiceDecision::Decline | rv1::ResolutionChoiceDecision::CastSpell
            )
            || (decision == rv1::ResolutionChoiceDecision::Decline
                && answer.spell_cast_announcement.is_some())
        {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("invalid legendary cast choice"));
        }
        if decision == rv1::ResolutionChoiceDecision::CastSpell {
            let player = pending.deciding_player;
            self.state.pending_resolution = Some(pending);
            let announcement =
                answer
                    .spell_cast_announcement
                    .as_ref()
                    .ok_or(EngineError::Illegal(
                        "legendary cast requires an announcement",
                    ))?;
            let batch = self.begin_spell_cast(
                player,
                &rv1::BeginSpellCast {
                    announcement: Some(announcement.clone()),
                },
            )?;
            self.retire_resolution_prompt_during_child_cast();
            return Ok(batch);
        }
        let ResolutionContinuation::LegendarySpellCast { stack, .. } = pending.continuation else {
            unreachable!()
        };
        let (item, index, previous, _) = stack.into_parts();
        self.complete_parked_resolution_with_previous(
            item,
            index,
            previous,
            vec![ev_log(format!(
                "P{} declines the legendary cast.",
                pending.deciding_player
            ))],
        )
    }
}
