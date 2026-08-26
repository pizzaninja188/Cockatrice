use super::*;

fn defender_choice_event(
    deciding_player: PlayerId,
    source_object_id: ObjectId,
    ordinal: usize,
    total: usize,
    spell_label: &str,
    token_label: &str,
    options: Vec<rv1::CombatDefenderOption>,
) -> rv1::RuledEvent {
    rv1::RuledEvent {
        ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
            rv1::ResolutionChoiceRequired {
                deciding_player_id: deciding_player,
                source_object_id,
                prompt_text: format!(
                    "Choose what {token_label} {ordinal} of {total} enters attacking ({spell_label})."
                ),
                choice_kind: rv1::ChoiceKind::AttackingTokenDefender as i32,
                candidate_object_ids: Vec::new(),
                candidate_card_ids: Vec::new(),
                min: 1,
                max: 1,
                ordered: false,
                candidate_names: Vec::new(),
                candidate_server_card_ids: Vec::new(),
                unique_names: false,
                generic_mana_cost: 0,
                payment_currently_legal: false,
                resolution_branches: Vec::new(),
                mana_cost: String::new(),
                candidate_selectable: Vec::new(),
                reveal_audience: rv1::ResolutionRevealAudience::None as i32,
                revealed_zone_owner_player_id: None,
                candidate_source_zones: Vec::new(),
                combat_defender_options: options,
            },
        )),
    }
}

impl GameEngine {
    pub(super) fn finish_attacking_token_defender_choice(
        &mut self,
        mut pending: PendingResolution,
        answer: &rv1::SubmitResolutionChoice,
        decision: rv1::ResolutionChoiceDecision,
    ) -> Result<RuledEventBatch, EngineError> {
        let ResolutionContinuation::AttackingTokenDefenders {
            stack,
            entries,
            logs,
            chosen_defenders,
            current_options,
            delayed_sacrifice,
        } = &pending.continuation
        else {
            unreachable!("attacking-token choice routed by caller")
        };
        if decision != rv1::ResolutionChoiceDecision::Unspecified
            || !answer.chosen_object_ids.is_empty()
            || answer.cast_spell.is_some()
        {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "attacking-token defender requires one structured choice",
            ));
        }
        let Some(choice) = answer.chosen_combat_defender else {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("attacking-token defender missing"));
        };
        let live_options = self.legal_combat_defender_options();
        if !current_options.contains(&choice) || !live_options.contains(&choice) {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "illegal or stale attacking-token defender",
            ));
        }

        let stack = stack.clone();
        let entries = entries.clone();
        let logs = logs.clone();
        let mut chosen_defenders = chosen_defenders.clone();
        let delayed_sacrifice = *delayed_sacrifice;
        chosen_defenders.push(choice);
        if chosen_defenders.len() < entries.len() {
            let token_label = entries
                .first()
                .and_then(|entry| entry.created.identity.as_ref())
                .map(|identity| identity.name.clone())
                .unwrap_or_else(|| "token".to_string());
            let spell_label = object_display_name(
                &self.state,
                self.registry,
                stack.item.source_permanent_id.unwrap_or(stack.item.id),
            );
            let prompt = format!(
                "Choose what {token_label} {} of {} enters attacking ({}).",
                chosen_defenders.len() + 1,
                entries.len(),
                spell_label
            );
            pending.presentation.prompt = prompt;
            pending.continuation = ResolutionContinuation::AttackingTokenDefenders {
                stack: stack.clone(),
                entries,
                logs,
                chosen_defenders,
                current_options: live_options.clone(),
                delayed_sacrifice,
            };
            self.state.pending_resolution = Some(pending);
            return Ok(finish_with_events(
                self,
                vec![defender_choice_event(
                    stack.item.controller,
                    stack.item.id,
                    self.state
                        .pending_resolution
                        .as_ref()
                        .and_then(|pending| match &pending.continuation {
                            ResolutionContinuation::AttackingTokenDefenders {
                                chosen_defenders,
                                ..
                            } => Some(chosen_defenders.len() + 1),
                            _ => None,
                        })
                        .unwrap_or(1),
                    self.state
                        .pending_resolution
                        .as_ref()
                        .and_then(|pending| match &pending.continuation {
                            ResolutionContinuation::AttackingTokenDefenders { entries, .. } => {
                                Some(entries.len())
                            }
                            _ => None,
                        })
                        .unwrap_or(1),
                    &spell_label,
                    &token_label,
                    live_options,
                )],
            ));
        }

        let mut events = Vec::new();
        if self.begin_token_entry_batch(
            stack.item.clone(),
            entries,
            logs,
            Some(AttackingTokenBatch {
                defenders: chosen_defenders,
            }),
            delayed_sacrifice,
            &mut events,
        )? {
            return Ok(finish_with_events(self, events));
        }
        self.complete_parked_resolution_with_previous(
            stack.item,
            stack.resume_effect_index,
            stack.previous_result,
            events,
        )
    }
}
