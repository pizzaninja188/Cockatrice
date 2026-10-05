//! Shared CR 614/616 replacement ordering and battlefield-entry preprocessing.

use super::characteristics::creature_matches_scope;
use super::events::{ev_log, ev_priority_changed, finish_with_events};
use super::history::player_life_aggregate_value;
use super::presentation::{
    stack_child_presentation_ref, PresentationPath, StackPresentationSource,
};
use super::resolution::{
    move_object_to_zone, permanent_moved_event, permanent_moved_event_with_library_position,
};
use super::targeting::{battlefield_objects_matching, object_matches_mass_filter};
use super::*;

#[cfg(test)]
mod entry_layers_tests;
use crate::state::{
    ChosenOpponentKey, ChosenOpponentRecord, PendingAuraEntryRecipient,
    ReplacementSourcePresentation,
};

fn materialize_entry_modifiers(
    event: &BattlefieldEntryEvent,
    timestamp: u64,
) -> Vec<ContinuousEffect> {
    let mut effects = Vec::new();
    if let Some(land_type) = event.chosen_basic_land_type {
        effects.push(ContinuousEffect {
            trigger_grant_origin: None,
            source_id: Some(event.object_id),
            affected: AffectedScope::Single(event.object_id),
            kind: ContinuousEffectKind::Layer4SetBasicLandType(land_type),
            condition: None,
            duration: EffectDuration::WhileSourceOnBattlefield,
            timestamp,
        });
    }
    let kinds = event
        .set_types
        .iter()
        .cloned()
        .map(ContinuousEffectKind::Layer4SetTypeLine)
        .chain(
            event
                .entry_modifiers
                .iter()
                .cloned()
                .flat_map(super::resolution::materialize_resolving_modifier),
        );
    effects.extend(kinds.map(|kind| ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(event.object_id),
        kind,
        condition: None,
        duration: EffectDuration::Indefinite,
        timestamp,
    }));
    effects
}

impl ReplacementSourcePresentation {
    pub(super) fn option(
        &self,
        application_id: u32,
        effect_summary: String,
    ) -> rv1::ReplacementEffectOption {
        rv1::ReplacementEffectOption {
            application_id,
            source_card_name: self.card_name.clone(),
            effect_summary,
            source_object_id: self.object_id,
            source_zone_change_generation: self.zone_change_generation,
        }
    }
}

fn accumulate_entry_counters(
    counters: &mut BTreeMap<CounterKind, u32>,
    counter: CounterKind,
    count: u32,
) {
    if count == 0 {
        return;
    }
    let total = counters.entry(counter).or_insert(0);
    *total = total.saturating_add(count);
}

fn saga_chapter_label(mut chapter: u32) -> String {
    let mut label = String::new();
    for (value, numeral) in [
        (1000, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ] {
        while chapter >= value {
            chapter -= value;
            label.push_str(numeral);
        }
    }
    label
}

/// The event domain currently parked behind the one shared CR 616 choice channel.
#[derive(serde::Serialize, Debug, Clone)]
pub(crate) enum PendingReplacementEvent {
    Discard(Box<crate::state::PendingDiscardBatch>),
    Draw(Box<super::draw::PendingDrawTransaction>),
    Damage(super::damage::PendingDamageBatch),
    BattlefieldEntry(Box<PendingBattlefieldEntry>),
}

pub(super) enum BattlefieldEntryProgress {
    Ready(Box<BattlefieldEntryEvent>),
    /// CR 303.4g: an unattached Aura with no legal recipient remains in its origin.
    Skipped(Box<BattlefieldEntryEvent>),
    Parked,
}

impl GameEngine {
    fn replacement_object_display_name(
        &self,
        object_id: ObjectId,
        entry_face: Option<usize>,
    ) -> String {
        let display_name =
            self.effective_card_identity(object_id)
                .and_then(|(card_id, face_index)| {
                    let object = self.state.objects.get(&object_id)?;
                    let face_index =
                        if object.copiable_values.is_some() || object.token_origin.is_some() {
                            face_index
                        } else {
                            entry_face.unwrap_or(face_index)
                        };
                    self.registry.get(card_id)?.face_display_name(face_index)
                });
        display_name.map(str::to_string).unwrap_or_else(|| {
            self.effective_face(object_id)
                .map(|face| face.name.clone())
                .unwrap_or_default()
        })
    }

    pub(super) fn replacement_source_presentation(
        &self,
        object_id: ObjectId,
    ) -> ReplacementSourcePresentation {
        let Some(object) = self.state.objects.get(&object_id) else {
            return ReplacementSourcePresentation::default();
        };
        if object.face_down || matches!(object.zone, Zone::Hand | Zone::Library) {
            return ReplacementSourcePresentation::default();
        }
        ReplacementSourcePresentation {
            card_name: self.replacement_object_display_name(object_id, None),
            object_id,
            zone_change_generation: self
                .state
                .zone_change_generation
                .get(&object_id)
                .copied()
                .unwrap_or(0),
        }
    }

    pub(super) fn replacement_stack_source_presentation(
        &self,
        item: &StackItem,
    ) -> ReplacementSourcePresentation {
        // StackItem retains the face/card identity of spells, copies, and abilities even after
        // their physical source has moved. Do not look up a new incarnation of that source.
        let object_id = item.source_permanent_id.unwrap_or(item.id);
        if item.source_permanent_id.is_some()
            && self.state.objects.get(&object_id).is_none_or(|object| {
                object.face_down || matches!(object.zone, Zone::Hand | Zone::Library)
            })
        {
            return ReplacementSourcePresentation::default();
        }
        ReplacementSourcePresentation {
            card_name: self
                .registry
                .get(&item.card_id)
                .and_then(|card| card.face_display_name(item.face_index))
                .map(str::to_string)
                .unwrap_or_default(),
            object_id,
            zone_change_generation: if item.source_permanent_id.is_some() {
                item.source_zone_change
            } else {
                self.state
                    .zone_change_generation
                    .get(&item.id)
                    .copied()
                    .unwrap_or(0)
            },
        }
    }

    /// The face proposed by this entry event. Objects outside the battlefield normally expose
    /// their front face through `effective_face`, but a transformed Siege spell is entering on
    /// its back face and its intrinsic entry replacements must come from that face instead.
    fn battlefield_entry_face<'a>(
        &'a self,
        event: &BattlefieldEntryEvent,
    ) -> Option<Cow<'a, CardFace>> {
        let object = self.state.objects.get(&event.object_id)?;
        if let Some(values) = object
            .copiable_values
            .as_ref()
            .or(object.token_origin.as_ref())
        {
            return Some(Cow::Borrowed(&values.face));
        }
        self.registry
            .get(&object.card_id)?
            .face(event.face_index)
            .map(Cow::Borrowed)
    }

    fn battlefield_entry_cast_cost_origin(
        &self,
        event: &BattlefieldEntryEvent,
    ) -> Option<crate::state::CastCostAbilityOrigin> {
        let object = self.state.objects.get(&event.object_id)?;
        let (original_card_id, face_id) = if let Some(values) = object
            .copiable_values
            .as_ref()
            .or(object.token_origin.as_ref())
        {
            (values.source_card_id.clone(), values.face.face_id.clone())
        } else {
            (
                object.card_id.clone(),
                self.registry
                    .get(&object.card_id)?
                    .face(event.face_index)?
                    .face_id
                    .clone(),
            )
        };
        Some(crate::state::CastCostAbilityOrigin {
            original_card_id,
            face_id,
            copy_revision: object.copy_revision,
        })
    }

    fn battlefield_entry_characteristics_through_layer_5(
        &self,
        event: &BattlefieldEntryEvent,
    ) -> Option<Characteristics> {
        self.battlefield_entry_early_characteristics(event)
            .map(|(characteristics, _)| characteristics)
    }

    fn battlefield_entry_early_characteristics(
        &self,
        event: &BattlefieldEntryEvent,
    ) -> Option<(Characteristics, bool)> {
        let object = self.state.objects.get(&event.object_id)?;
        let copied = object
            .copiable_values
            .as_ref()
            .or(object.token_origin.as_ref());
        let selected = self.battlefield_entry_face(event)?;
        let room_faces = self.room_faces(event.object_id);
        let unlocked = if copied.is_none() {
            event.unlock_room_door.into_iter().collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let room_face = room_faces
            .and_then(|faces| CardDefinition::synthesize_room_permanent_face(faces, &unlocked));
        let face = room_face.as_ref().unwrap_or(&selected);
        let raw_faces = if let Some(faces) = room_faces {
            unlocked
                .iter()
                .filter_map(|&index| faces.get(index).map(|face| (index, face)))
                .collect::<Vec<_>>()
        } else {
            vec![(event.face_index, selected.as_ref())]
        };
        let statics = raw_faces
            .into_iter()
            .flat_map(|(index, face)| {
                face.static_abilities.iter().map(move |ability| {
                    (
                        self.ability_definition(
                            event.object_id,
                            index,
                            vec![ability.ability_id.clone()],
                        ),
                        &ability.definition,
                    )
                })
            })
            .collect::<Vec<_>>();
        let effects = materialize_entry_modifiers(event, self.state.command_index);
        super::characteristics::entry_characteristics_through_layer_5(
            &self.state,
            self.registry,
            event,
            face,
            &statics,
            &effects,
        )
    }

    fn battlefield_entry_is_battle(&self, event: &BattlefieldEntryEvent) -> bool {
        let mut is_battle = if let Some(replacement) = &event.set_types {
            replacement
                .card_types
                .contains(&PermanentTypeFilter::Battle)
        } else {
            self.battlefield_entry_face(event)
                .is_some_and(|face| face.types.iter().any(|card_type| card_type == "Battle"))
        };
        for modifier in &event.entry_modifiers {
            match modifier {
                ResolvingPermanentModifier::SetTypeLine(replacement) => {
                    is_battle = replacement
                        .card_types
                        .contains(&PermanentTypeFilter::Battle);
                }
                ResolvingPermanentModifier::AddTypes(addition) => {
                    is_battle |= addition.card_types.contains(&PermanentTypeFilter::Battle);
                }
                ResolvingPermanentModifier::SetBasePowerToughness { .. }
                | ResolvingPermanentModifier::GrantKeywords(_)
                | ResolvingPermanentModifier::GrantActivatedAbility(_) => {}
            }
        }
        is_battle
    }

    pub(super) fn player_life_snapshot(&self) -> BTreeMap<PlayerId, i32> {
        self.state
            .players
            .iter()
            .map(|player| (player.id, player.life))
            .collect()
    }

    fn entry_condition_holds(
        &self,
        condition: &GameCondition,
        event: &BattlefieldEntryEvent,
    ) -> bool {
        match condition {
            GameCondition::PlayerLifeAggregate {
                players, aggregate, ..
            } => player_life_aggregate_value(
                &self.state,
                *players,
                *aggregate,
                event.destination_controller,
                |player_id| event.player_life_snapshot.get(&player_id).copied(),
            )
            .is_some_and(|value| condition.matches_life_value(value)),
            _ => self.condition_holds(
                condition,
                ConditionContext {
                    controller: event.destination_controller,
                    source_object_id: event.object_id,
                    source_zone_change: self
                        .state
                        .zone_change_generation
                        .get(&event.object_id)
                        .copied()
                        .unwrap_or(0),
                    resolving_spell_id: None,
                    stack_item: None,
                    previous_effect_result: None,
                },
            ),
        }
    }

    fn battlefield_counter_replacement_affects(
        &self,
        source_id: ObjectId,
        filter: &CreatureScopeFilter,
        event: &BattlefieldEntryEvent,
    ) -> bool {
        let Some(source_controller) = self.controller_of(source_id) else {
            return false;
        };
        let Some(characteristics) = self.battlefield_entry_characteristics_through_layer_5(event)
        else {
            return false;
        };
        creature_matches_scope(
            &self.state,
            self.registry,
            filter,
            source_controller,
            filter.exclude_self.then_some(source_id),
            event.object_id,
            &characteristics,
        )
    }

    fn battlefield_entry_candidates(
        &self,
        event: &BattlefieldEntryEvent,
    ) -> Vec<(EntryReplacementEffectId, ReplacementPriority, String)> {
        let Some(entering) = self.state.objects.get(&event.object_id) else {
            return Vec::new();
        };
        let mut candidates = Vec::new();
        if !entering.face_down
            && self
                .battlefield_entry_early_characteristics(event)
                .is_some_and(|(_, printed)| printed)
        {
            if let Some(face) = self.battlefield_entry_face(event) {
                for (ability_index, ability) in face.static_abilities.iter().enumerate() {
                    let (priority, label) = match &ability.definition {
                        StaticAbilityDef::EntersPrepared if !event.prepared => (
                            ReplacementPriority::Other,
                            Some(format!("{} — enters prepared", face.name)),
                        ),
                        StaticAbilityDef::EntersAsCopy { .. } => (
                            ReplacementPriority::EntryCopy,
                            Some(format!("{} — enters as a copy", face.name)),
                        ),
                        StaticAbilityDef::EntersWithChosenBasicLandType { .. } => (
                            ReplacementPriority::Other,
                            Some(format!("{} — choose a basic land type", face.name)),
                        ),
                        StaticAbilityDef::AsEntersChooseOpponent { .. } => (
                            ReplacementPriority::Other,
                            Some(format!("{} — choose an opponent", face.name)),
                        ),
                        StaticAbilityDef::EntersTapped {
                            affected: EntersTappedAffected::Self_,
                            condition,
                            unless_cost,
                        } if (unless_cost.is_some() || !event.tapped)
                            && condition.as_ref().is_none_or(|condition| {
                                self.entry_condition_holds(condition, event)
                            }) =>
                        {
                            (
                                ReplacementPriority::Other,
                                Some(format!("{} — enters tapped", face.name)),
                            )
                        }
                        StaticAbilityDef::EntersWithCounters {
                            affected: EntersWithCountersAffected::Self_,
                            cast_cost_condition,
                            ..
                        } if cast_cost_condition.as_ref().is_none_or(|condition| {
                            event.cast_cost_receipts.iter().any(|receipt| {
                                receipt.group_id.as_ref() == Some(&condition.group_id)
                                    && receipt.option_id.as_ref() == Some(&condition.option_id)
                            }) == condition.expected_selected
                        }) =>
                        {
                            (
                                ReplacementPriority::Other,
                                Some(format!("{} — enters with counters", face.name)),
                            )
                        }
                        _ => (ReplacementPriority::Other, None),
                    };
                    let Some(label) = label else {
                        continue;
                    };
                    let effect_id = EntryReplacementEffectId::Intrinsic {
                        object_id: event.object_id,
                        copy_revision: entering.copy_revision,
                        ability_index,
                    };
                    if !event.applied_effects.contains(&effect_id) {
                        candidates.push((effect_id, priority, label));
                    }
                }
                if face.keywords.contains(&Keyword::ReadAhead) {
                    let effect_id = EntryReplacementEffectId::ReadAhead {
                        object_id: event.object_id,
                        copy_revision: entering.copy_revision,
                    };
                    if !event.applied_effects.contains(&effect_id) {
                        candidates.push((
                            effect_id,
                            ReplacementPriority::Other,
                            format!("{} — read ahead", face.name),
                        ));
                    }
                }
            }
        }

        let mut battlefield_sources: Vec<_> = self
            .state
            .objects
            .values()
            .filter(|object| object.zone == Zone::Battlefield)
            .collect();
        battlefield_sources.sort_by_key(|object| object.id);
        for source in battlefield_sources {
            if !super::characteristics::printed_static_source_is_available(
                &self.state,
                self.registry,
                source.id,
            ) {
                continue;
            }
            let Some(face) = self.effective_face(source.id) else {
                continue;
            };
            for (ability_index, ability) in face.static_abilities.iter().enumerate() {
                let label = match &ability.definition {
                    StaticAbilityDef::EntersTapped {
                        affected: EntersTappedAffected::Permanents,
                        condition,
                        ..
                    } if !event.tapped
                        && condition.as_ref().is_none_or(|condition| {
                            self.entry_condition_holds(condition, event)
                        }) =>
                    {
                        Some(format!(
                            "{} (P{}, object {}) — permanents enter tapped",
                            face.name, source.controller, source.id
                        ))
                    }
                    StaticAbilityDef::EntersWithCounters {
                        affected: EntersWithCountersAffected::Creatures(filter),
                        ..
                    } if self.battlefield_counter_replacement_affects(source.id, filter, event) => {
                        Some(format!(
                            "{} (P{}, object {}) — enters with counters",
                            face.name, source.controller, source.id
                        ))
                    }
                    _ => None,
                };
                if let Some(label) = label {
                    let effect_id = EntryReplacementEffectId::Battlefield {
                        source_id: source.id,
                        source_generation: self
                            .state
                            .zone_change_generation
                            .get(&source.id)
                            .copied()
                            .unwrap_or(0),
                        ability_index,
                    };
                    if !event.applied_effects.contains(&effect_id) {
                        candidates.push((effect_id, ReplacementPriority::Other, label));
                    }
                }
            }
        }

        let Some(priority) = candidates.iter().map(|(_, priority, _)| *priority).min() else {
            return candidates;
        };
        candidates
            .into_iter()
            .filter(|(_, candidate_priority, _)| *candidate_priority == priority)
            .collect()
    }

    fn entry_copy_filter(
        &self,
        event: &BattlefieldEntryEvent,
        effect_id: &EntryReplacementEffectId,
    ) -> Option<TargetFilter> {
        self.entry_copy_definition(event, effect_id)
            .map(|(filter, _)| filter)
    }

    fn entry_copy_definition(
        &self,
        event: &BattlefieldEntryEvent,
        effect_id: &EntryReplacementEffectId,
    ) -> Option<(TargetFilter, bool)> {
        let EntryReplacementEffectId::Intrinsic {
            object_id,
            copy_revision,
            ability_index,
        } = effect_id
        else {
            return None;
        };
        if *object_id != event.object_id {
            return None;
        }
        let object = self.state.objects.get(object_id)?;
        if object.copy_revision != *copy_revision {
            return None;
        }
        match &self
            .effective_face(*object_id)?
            .static_abilities
            .get(*ability_index)?
            .definition
        {
            StaticAbilityDef::EntersAsCopy {
                filter,
                artifact_in_addition,
            } => Some((filter.clone(), *artifact_in_addition)),
            _ => None,
        }
    }

    fn entry_read_ahead_final_chapter(
        &self,
        event: &BattlefieldEntryEvent,
        effect_id: &EntryReplacementEffectId,
    ) -> Option<u32> {
        let EntryReplacementEffectId::ReadAhead {
            object_id,
            copy_revision,
        } = effect_id
        else {
            return None;
        };
        let object = self.state.objects.get(object_id)?;
        if *object_id != event.object_id || object.copy_revision != *copy_revision {
            return None;
        }
        let face = self.battlefield_entry_face(event)?;
        if !face.keywords.contains(&Keyword::ReadAhead) {
            return None;
        }
        face.triggered_abilities
            .iter()
            .filter_map(|ability| match &ability.trigger {
                TriggerCondition::SagaChapter { chapters } => chapters.iter().copied().max(),
                _ => None,
            })
            .max()
    }

    fn entry_unless_cost(
        &self,
        event: &BattlefieldEntryEvent,
        effect_id: &EntryReplacementEffectId,
    ) -> Option<EntryCost> {
        let EntryReplacementEffectId::Intrinsic {
            object_id,
            copy_revision,
            ability_index,
        } = effect_id
        else {
            return None;
        };
        let object = self.state.objects.get(object_id)?;
        if *object_id != event.object_id || object.copy_revision != *copy_revision {
            return None;
        }
        match &self
            .battlefield_entry_face(event)?
            .static_abilities
            .get(*ability_index)?
            .definition
        {
            StaticAbilityDef::EntersTapped {
                affected: EntersTappedAffected::Self_,
                unless_cost: Some(cost),
                ..
            } => Some(cost.clone()),
            StaticAbilityDef::EntersWithChosenBasicLandType { untapped_cost }
                if event.chosen_basic_land_type.is_some() =>
            {
                Some(untapped_cost.clone())
            }
            _ => None,
        }
    }

    fn entry_opponent_key(
        &self,
        event: &BattlefieldEntryEvent,
        effect_id: &EntryReplacementEffectId,
    ) -> Option<ChosenOpponentKey> {
        let EntryReplacementEffectId::Intrinsic {
            object_id,
            copy_revision,
            ability_index,
        } = effect_id
        else {
            return None;
        };
        let object = self.state.objects.get(object_id)?;
        if *object_id != event.object_id || object.copy_revision != *copy_revision {
            return None;
        }
        let face = self.battlefield_entry_face(event)?;
        let StaticAbilityDef::AsEntersChooseOpponent { link_id } =
            &face.static_abilities.get(*ability_index)?.definition
        else {
            return None;
        };
        self.chosen_opponent_key(*object_id, event.face_index, &face, link_id)
    }

    fn entry_opponent_players(&self, event: &BattlefieldEntryEvent) -> Vec<PlayerId> {
        self.state
            .players
            .iter()
            .filter(|player| {
                !player.has_lost
                    && self
                        .state
                        .are_opponents(event.destination_controller, player.id)
            })
            .map(|player| player.id)
            .collect()
    }

    fn entry_opponent_choice_event(
        &self,
        event: &BattlefieldEntryEvent,
        players: &[PlayerId],
    ) -> rv1::RuledEvent {
        rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                rv1::ResolutionChoiceRequired {
                    deciding_player_id: event.destination_controller,
                    source_object_id: event.object_id,
                    prompt_text: "Choose an opponent as this permanent enters.".into(),
                    choice_kind: rv1::ChoiceKind::ResolutionBranch as i32,
                    min: 1,
                    max: 1,
                    resolution_branches: players
                        .iter()
                        .enumerate()
                        .map(|(index, player)| rv1::ResolutionBranchOption {
                            branch_index: index as u32,
                            label: format!("P{player}"),
                            selectable: self
                                .state
                                .player_idx(*player)
                                .is_some_and(|index| !self.state.players[index].has_lost)
                                && self
                                    .state
                                    .are_opponents(event.destination_controller, *player),
                            ..Default::default()
                        })
                        .collect(),
                    ..Default::default()
                },
            )),
        }
    }

    fn park_entry_opponent_choice(
        &mut self,
        item: StackItem,
        event: BattlefieldEntryEvent,
        completion: BattlefieldEntryCompletion,
        effect_id: EntryReplacementEffectId,
        events: &mut Vec<rv1::RuledEvent>,
    ) {
        let key = self
            .entry_opponent_key(&event, &effect_id)
            .expect("current intrinsic opponent choice");
        let entering_zone = self.state.objects[&event.object_id].zone;
        let entering_generation = key.source_zone_change;
        let players = self.entry_opponent_players(&event);
        events.push(self.entry_opponent_choice_event(&event, &players));
        let deciding_player = event.destination_controller;
        let source_object_id = event.object_id;
        self.state.pending_replacement_event = Some(PendingReplacementEvent::BattlefieldEntry(
            Box::new(PendingBattlefieldEntry {
                event,
                applications: Vec::new(),
                copy_source_effect: None,
                copy_source_candidates: Vec::new(),
                completion,
            }),
        ));
        self.state.pending_resolution = Some(PendingResolution {
            deciding_player,
            presentation: PendingResolutionPresentation {
                source_object_id,
                candidates: Vec::new(),
                min: 1,
                max: 1,
                ordered: false,
                prompt: "Choose an opponent as this permanent enters.".into(),
                choice_kind: rv1::ChoiceKind::ResolutionBranch,
                unique_names: false,
            },
            continuation: ResolutionContinuation::EntryChooseOpponent {
                stack: ParkedStackResolution::new(item),
                effect_id,
                key,
                entering_zone,
                entering_generation,
                players,
            },
        });
    }

    pub(super) fn refresh_participating_entry_departure(
        &mut self,
        departed_objects: &HashSet<ObjectId>,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        if let Some(batch) = self.state.pending_observer_return_batch.as_mut() {
            batch
                .ready
                .retain(|entry| self.state.objects.contains_key(&entry.event.object_id));
            batch.remaining.retain(|action| {
                let ImmediateObserverAction::ReturnExiledObject { exiled } = action;
                self.state.objects.contains_key(&exiled.object_id)
            });
        }
        let Some(pending) = self.state.pending_resolution.clone() else {
            return Ok(());
        };
        let Some(PendingReplacementEvent::BattlefieldEntry(entry)) =
            self.state.pending_replacement_event.clone()
        else {
            return Ok(());
        };
        let mut entry = *entry;
        let decider_live = self
            .state
            .player_idx(pending.deciding_player)
            .is_some_and(|index| !self.state.players[index].has_lost);
        let spell_label = pending
            .continuation
            .stack()
            .map_or("entry", |stack| stack.item.card_id.as_str());
        match &mut entry.completion {
            BattlefieldEntryCompletion::ZoneEntryBatch(batch) => {
                self.reconcile_departed_zone_entry_members(batch, departed_objects)
            }
            BattlefieldEntryCompletion::ObserverReturn { .. } => {}
            BattlefieldEntryCompletion::TokenBatch(batch) => {
                self.reconcile_departed_token_entry_members(batch, departed_objects, spell_label);
            }
            _ => return Ok(()),
        }
        let stack = pending
            .continuation
            .stack()
            .cloned()
            .ok_or(EngineError::Illegal("entry continuation missing"))?;
        if let BattlefieldEntryCompletion::ZoneEntryBatch(batch) = &entry.completion {
            if !self.zone_entry_batch_current(batch) {
                self.restore_abandoned_battlefield_entry(&entry.event)?;
                for ready in &batch.ready {
                    self.restore_abandoned_battlefield_entry(ready)?;
                }
                self.state.pending_resolution = None;
                self.state.pending_replacement_event = None;
                return self.abandon_participating_resolution(Some(stack), events);
            }
        }
        let entrant_live = self.state.objects.contains_key(&entry.event.object_id) && decider_live;
        if !entrant_live {
            self.state.pending_resolution = None;
            self.state.pending_replacement_event = None;
            if !departed_objects.contains(&entry.event.object_id)
                && matches!(
                    entry.completion,
                    BattlefieldEntryCompletion::ZoneEntryBatch(_)
                )
            {
                return self.abandon_participating_resolution(Some(stack), events);
            }
            if departed_objects.contains(&entry.event.object_id) {
                // CR 800.4a removed this entrant. There is no provisional object to restore.
                // This exemption does not cover a stale surviving incarnation.
                entry.event.pending_copy_candidate = None;
                entry.event.pending_aura_recipient = None;
                entry.event.accepted_aura_recipient = None;
            }
            let resumed = self.finish_entry_copy_without_recipient(
                stack,
                entry.event,
                entry.completion,
                Vec::new(),
            )?;
            events.extend(resumed.events);
            return Ok(());
        }
        if matches!(
            pending.continuation,
            ResolutionContinuation::EntryCopySource { .. }
        ) {
            let retained: Vec<_> = entry
                .copy_source_candidates
                .iter()
                .copied()
                .filter(|(oid, _)| self.state.objects.contains_key(oid))
                .collect();
            if retained != entry.copy_source_candidates {
                let effect_id = entry
                    .copy_source_effect
                    .clone()
                    .ok_or(EngineError::Illegal("entry copy effect missing"))?;
                self.park_copy_source_choice(
                    stack.item.clone(),
                    entry.event,
                    entry.completion,
                    effect_id,
                    retained.iter().map(|(oid, _)| *oid).collect(),
                    events,
                );
                // Refreshing the prompt must not rebind any surviving source to a new incarnation.
                if let Some(PendingReplacementEvent::BattlefieldEntry(entry)) =
                    self.state.pending_replacement_event.as_mut()
                {
                    entry.copy_source_candidates = retained;
                }
                self.transfer_entry_choice_resume(&stack);
                return Ok(());
            }
        }
        if matches!(
            pending.continuation,
            ResolutionContinuation::EntryAuraRecipient { .. }
        ) {
            let choice = entry
                .event
                .pending_aura_recipient
                .as_ref()
                .ok_or(EngineError::Illegal("entry Aura recipient receipt missing"))?;
            let filter = choice.filter.clone();
            let values = choice
                .copy_candidate
                .as_ref()
                .map(|candidate| candidate.values.clone())
                .or_else(|| self.copiable_values_for(entry.event.object_id))
                .ok_or(EngineError::Illegal("entry Aura values missing"))?;
            let legal = self.entry_copy_aura_candidates(
                entry.event.object_id,
                entry.event.destination_controller,
                &values,
                &filter,
            );
            let retained: Vec<_> = pending
                .presentation
                .candidates
                .iter()
                .copied()
                .filter(|oid| {
                    legal.contains(oid)
                        && (filter.is_player()
                            || choice.recipient_generations.iter().any(
                                |(candidate, generation)| {
                                    candidate == oid
                                        && self
                                            .state
                                            .zone_change_generation
                                            .get(oid)
                                            .copied()
                                            .unwrap_or(0)
                                            == *generation
                                },
                            ))
                })
                .collect();
            if retained != pending.presentation.candidates {
                self.state.pending_resolution = None;
                self.state.pending_replacement_event = None;
                if retained.is_empty() {
                    events.extend(
                        self.finish_entry_copy_without_recipient(
                            stack,
                            entry.event,
                            entry.completion,
                            Vec::new(),
                        )?
                        .events,
                    );
                } else {
                    // Keep the original incarnation receipts; refreshing cannot bind new objects.
                    self.park_entry_copy_aura_recipient_choice(
                        stack,
                        entry.event,
                        entry.completion,
                        filter,
                        retained,
                        events,
                    );
                }
                return Ok(());
            }
        }
        let affected_application = match &pending.continuation {
            ResolutionContinuation::BattleProtector { .. } => pending.presentation.candidates.iter()
                .any(|&player| !self.entry_battle_protector_is_live(&entry.event, player as PlayerId)),
            ResolutionContinuation::EntryReplacement { .. } => entry.applications.iter().any(|application|
                matches!(application.effect_id, EntryReplacementEffectId::Battlefield { source_id, .. } if !self.state.objects.contains_key(&source_id))),
            ResolutionContinuation::EntryCost { effect_id, .. } | ResolutionContinuation::EntryReveal { effect_id, .. } =>
                matches!(effect_id, EntryReplacementEffectId::Battlefield { source_id, .. } if !self.state.objects.contains_key(source_id)),
            _ => false,
        };
        if affected_application {
            self.state.pending_resolution = None;
            self.state.pending_replacement_event = None;
            match self.advance_or_park_battlefield_entry(
                stack.item.clone(),
                entry.event,
                entry.completion.clone(),
                events,
            ) {
                BattlefieldEntryProgress::Parked => self.transfer_entry_choice_resume(&stack),
                BattlefieldEntryProgress::Skipped(event) => {
                    events.extend(
                        self.finish_entry_copy_without_recipient(
                            stack,
                            *event,
                            entry.completion,
                            Vec::new(),
                        )?
                        .events,
                    );
                }
                BattlefieldEntryProgress::Ready(event) => {
                    events.extend(
                        self.complete_pending_battlefield_entry(
                            pending,
                            *event,
                            entry.completion,
                            Vec::new(),
                        )?
                        .events,
                    );
                }
            }
        } else {
            self.state.pending_replacement_event =
                Some(PendingReplacementEvent::BattlefieldEntry(Box::new(entry)));
        }
        Ok(())
    }

    pub(super) fn refresh_entry_opponent_departure(
        &mut self,
        departed_objects: &HashSet<ObjectId>,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        let Some(pending) = self.state.pending_resolution.clone() else {
            return Ok(());
        };
        let ResolutionContinuation::EntryChooseOpponent {
            stack,
            effect_id,
            key,
            entering_zone,
            entering_generation,
            players,
        } = &pending.continuation
        else {
            return Ok(());
        };
        let Some(PendingReplacementEvent::BattlefieldEntry(entry)) =
            self.state.pending_replacement_event.as_ref()
        else {
            return Ok(());
        };
        let mut entry = (**entry).clone();
        if let BattlefieldEntryCompletion::ZoneEntryBatch(batch) = &mut entry.completion {
            self.reconcile_departed_zone_entry_members(batch, departed_objects);
        }
        self.state.pending_replacement_event = Some(PendingReplacementEvent::BattlefieldEntry(
            Box::new(entry.clone()),
        ));
        let source_is_current = self
            .state
            .objects
            .get(&entry.event.object_id)
            .is_some_and(|object| object.zone == *entering_zone)
            && self
                .state
                .zone_change_generation
                .get(&entry.event.object_id)
                .copied()
                .unwrap_or(0)
                == *entering_generation
            && self.entry_opponent_key(&entry.event, effect_id).as_ref() == Some(key);
        let decider_is_live = self
            .state
            .player_idx(pending.deciding_player)
            .is_some_and(|index| !self.state.players[index].has_lost);
        if !source_is_current || !decider_is_live {
            // CR 800.4a/609.3: owner departure can remove an entrant while its surviving
            // controller's outer spell is resolving. Skip that entry, preserving its tail.
            let stack = stack.clone();
            self.state.pending_resolution = None;
            self.state.pending_replacement_event = None;
            let batch = match entry.completion {
                BattlefieldEntryCompletion::PermanentSpell { .. } => self
                    .complete_parked_resolution_with_previous(
                        stack.item,
                        Some(0),
                        stack.previous_result,
                        Vec::new(),
                    )?,
                BattlefieldEntryCompletion::ResolutionEffect { .. }
                | BattlefieldEntryCompletion::Ninjutsu { .. } => self
                    .complete_parked_resolution_with_previous(
                        stack.item,
                        stack.resume_effect_index,
                        stack.previous_result,
                        Vec::new(),
                    )?,
                BattlefieldEntryCompletion::ZoneEntryBatch(mut batch) => {
                    self.reconcile_departed_zone_entry_members(&mut batch, departed_objects);
                    let mut resumed_events = Vec::new();
                    if let Some(stack) =
                        self.continue_zone_entry_batch(stack, *batch, &mut resumed_events)?
                    {
                        self.complete_parked_resolution_with_previous(
                            stack.item,
                            stack.resume_effect_index,
                            stack.previous_result,
                            resumed_events,
                        )?
                    } else {
                        finish_with_events(self, resumed_events)
                    }
                }
                completion => self.finish_entry_copy_without_recipient(
                    stack,
                    entry.event,
                    completion,
                    Vec::new(),
                )?,
            };
            events.extend(batch.events);
            return Ok(());
        }
        if players
            .iter()
            .any(|player| self.entry_opponent_players(&entry.event).contains(player))
        {
            events.push(self.entry_opponent_choice_event(&entry.event, players));
            return Ok(());
        }
        // CR 609.3: impossible mandatory designation does not strand an entry transaction.
        let stack = stack.clone();
        let effect_id = effect_id.clone();
        let pending = self.state.pending_resolution.take().unwrap();
        self.state.pending_replacement_event = None;
        self.apply_entry_replacement(&mut entry.event, effect_id);
        match self.advance_or_park_battlefield_entry(
            stack.item.clone(),
            entry.event,
            entry.completion.clone(),
            events,
        ) {
            BattlefieldEntryProgress::Parked => self.transfer_entry_choice_resume(&stack),
            BattlefieldEntryProgress::Skipped(event) => {
                events.extend(
                    self.finish_entry_copy_without_recipient(
                        stack,
                        *event,
                        entry.completion,
                        Vec::new(),
                    )?
                    .events,
                );
            }
            BattlefieldEntryProgress::Ready(event) => {
                let batch = self.complete_pending_battlefield_entry(
                    pending,
                    *event,
                    entry.completion,
                    Vec::new(),
                )?;
                events.extend(batch.events);
            }
        }
        Ok(())
    }

    pub(super) fn reconcile_departed_zone_entry_members(
        &mut self,
        batch: &mut crate::state::PendingZoneEntryBatch,
        departed_objects: &HashSet<ObjectId>,
    ) {
        if matches!(&batch.completion,
            Some(crate::state::ZoneEntryCompletion::ChaosWarpRevealedTop { object_id, .. })
                if departed_objects.contains(object_id))
        {
            // CR 800.4: stop the departed owner's instruction without losing the real tail.
            batch.completion = None;
        }
        for (oid, generation, mana_value) in &batch.origin_mana_values {
            if !self.state.objects.contains_key(oid) {
                self.state
                    .last_known_mana_value_by_generation
                    .entry((*oid, *generation))
                    .or_insert(*mana_value);
            }
        }
        // A missing live-owner member remains in the frozen cohort so validation fails closed.
        batch
            .generations
            .retain(|(oid, _)| !departed_objects.contains(oid));
        let survivors: HashSet<_> = batch.generations.iter().map(|(oid, _)| *oid).collect();
        batch
            .ready
            .retain(|event| survivors.contains(&event.object_id));
        batch
            .remaining
            .retain(|event| survivors.contains(&event.object_id));
    }

    fn entry_needs_basic_land_type_choice(
        &self,
        event: &BattlefieldEntryEvent,
        effect_id: &EntryReplacementEffectId,
    ) -> bool {
        if event.chosen_basic_land_type.is_some() {
            return false;
        }
        let EntryReplacementEffectId::Intrinsic {
            object_id,
            copy_revision,
            ability_index,
        } = effect_id
        else {
            return false;
        };
        self.state
            .objects
            .get(object_id)
            .filter(|object| {
                *object_id == event.object_id && object.copy_revision == *copy_revision
            })
            .and_then(|_| self.battlefield_entry_face(event))
            .and_then(|face| face.static_abilities.get(*ability_index).cloned())
            .is_some_and(|ability| {
                matches!(
                    ability.definition,
                    StaticAbilityDef::EntersWithChosenBasicLandType { .. }
                )
            })
    }

    fn park_basic_land_type_choice(
        &mut self,
        item: StackItem,
        event: BattlefieldEntryEvent,
        completion: BattlefieldEntryCompletion,
        effect_id: EntryReplacementEffectId,
        events: &mut Vec<rv1::RuledEvent>,
    ) {
        let options = BasicLandType::ALL
            .into_iter()
            .enumerate()
            .map(|(index, land_type)| rv1::ResolutionBranchOption {
                branch_index: index as u32,
                label: land_type.as_str().to_string(),
                cost_kind: rv1::ResolutionBranchCostKind::Unspecified as i32,
                cost_text: String::new(),
                selectable: true,
                search_zones: Vec::new(),
                presentation: None,
            })
            .collect();
        let prompt = "Choose a basic land type for Multiversal Passage.".to_string();
        events.push(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                rv1::ResolutionChoiceRequired {
                    candidate_token_identities: Vec::new(),
                    candidate_player_ids: Vec::new(),
                    deciding_player_id: event.deciding_player,
                    source_object_id: event.object_id,
                    prompt_text: prompt.clone(),
                    choice_kind: rv1::ChoiceKind::ResolutionBranch as i32,
                    candidate_object_ids: Vec::new(),
                    candidate_card_ids: Vec::new(),
                    min: 1,
                    max: 1,
                    ordered: false,
                    candidate_names: Vec::new(),
                    candidate_server_card_ids: Vec::new(),
                    candidate_selectable: Vec::new(),
                    unique_names: false,
                    generic_mana_cost: 0,
                    payment_currently_legal: false,
                    resolution_branches: options,
                    mana_cost: String::new(),
                    public_reveal: None,
                    candidate_source_zones: Vec::new(),
                    combat_defender_options: Vec::new(),
                    waterbend: false,
                    selection_slots: Vec::new(),
                    replacement_options: Vec::new(),
                    selection_alternatives: Vec::new(),
                },
            )),
        });
        let deciding_player = event.deciding_player;
        self.state.pending_replacement_event = Some(PendingReplacementEvent::BattlefieldEntry(
            Box::new(PendingBattlefieldEntry {
                event,
                applications: Vec::new(),
                copy_source_effect: None,
                copy_source_candidates: Vec::new(),
                completion,
            }),
        ));
        self.state.pending_resolution = Some(PendingResolution {
            deciding_player,
            presentation: PendingResolutionPresentation {
                source_object_id: item.id,
                candidates: Vec::new(),
                min: 1,
                max: 1,
                ordered: false,
                prompt,
                choice_kind: rv1::ChoiceKind::ResolutionBranch,
                unique_names: false,
            },
            continuation: ResolutionContinuation::EntryBasicLandType {
                stack: ParkedStackResolution::new(item),
                effect_id,
            },
        });
    }

    fn entry_reveal_candidates(
        &self,
        event: &BattlefieldEntryEvent,
        filter: &ZoneCardFilter,
    ) -> Vec<(ObjectId, u64)> {
        self.state
            .player_idx(event.destination_controller)
            .map_or_else(Vec::new, |index| {
                self.state.players[index]
                    .hand
                    .iter()
                    .copied()
                    .filter(|oid| {
                        *oid != event.object_id
                            && self.state.objects.get(oid).is_some_and(|object| {
                                object.owner == event.destination_controller
                                    && object.zone == Zone::Hand
                                    && zone_card_matches_filter(
                                        &self.state,
                                        self.registry,
                                        *oid,
                                        Some(filter),
                                    )
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
                    .collect()
            })
    }

    fn park_entry_reveal_choice(
        &mut self,
        item: StackItem,
        event: BattlefieldEntryEvent,
        completion: BattlefieldEntryCompletion,
        effect_id: EntryReplacementEffectId,
        filter: ZoneCardFilter,
        events: &mut Vec<rv1::RuledEvent>,
    ) {
        let candidate_generations = self.entry_reveal_candidates(&event, &filter);
        let candidates: Vec<_> = candidate_generations.iter().map(|(oid, _)| *oid).collect();
        let candidate_card_ids = candidates
            .iter()
            .map(|oid| self.state.objects[oid].card_id.clone())
            .collect();
        let candidate_names = candidates
            .iter()
            .map(|oid| {
                self.registry
                    .get(&self.state.objects[oid].card_id)
                    .map_or_else(
                        || self.state.objects[oid].card_id.clone(),
                        |card| card.name.clone(),
                    )
            })
            .collect();
        let name = self
            .battlefield_entry_face(&event)
            .map_or_else(|| "this permanent".to_owned(), |face| face.name.clone());
        let prompt = format!("As {name} enters, you may reveal one qualifying card from your hand. Choose none to enter tapped.");
        events.push(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                rv1::ResolutionChoiceRequired {
                    deciding_player_id: event.destination_controller,
                    source_object_id: event.object_id,
                    prompt_text: prompt.clone(),
                    choice_kind: rv1::ChoiceKind::HandCards as i32,
                    candidate_object_ids: candidates.clone(),
                    candidate_card_ids,
                    candidate_names,
                    candidate_selectable: vec![true; candidates.len()],
                    min: 0,
                    max: 1,
                    ..Default::default()
                },
            )),
        });
        let entering_zone = self.state.objects[&event.object_id].zone;
        let entering_generation = self
            .state
            .zone_change_generation
            .get(&event.object_id)
            .copied()
            .unwrap_or(0);
        self.state.pending_resolution = Some(PendingResolution {
            deciding_player: event.destination_controller,
            presentation: PendingResolutionPresentation {
                source_object_id: event.object_id,
                candidates,
                min: 0,
                max: 1,
                ordered: false,
                prompt,
                choice_kind: rv1::ChoiceKind::HandCards,
                unique_names: false,
            },
            continuation: ResolutionContinuation::EntryReveal {
                stack: ParkedStackResolution::new(item),
                effect_id,
                entering_zone,
                entering_generation,
                candidate_generations,
            },
        });
        self.state.pending_replacement_event = Some(PendingReplacementEvent::BattlefieldEntry(
            Box::new(PendingBattlefieldEntry {
                event,
                applications: Vec::new(),
                copy_source_effect: None,
                copy_source_candidates: Vec::new(),
                completion,
            }),
        ));
    }

    fn park_entry_cost_choice(
        &mut self,
        item: StackItem,
        event: BattlefieldEntryEvent,
        completion: BattlefieldEntryCompletion,
        effect_id: EntryReplacementEffectId,
        cost: EntryCost,
        events: &mut Vec<rv1::RuledEvent>,
    ) {
        let (label, cost_text, selectable) = match cost {
            EntryCost::RevealFromHand { filter } => {
                self.park_entry_reveal_choice(item, event, completion, effect_id, filter, events);
                return;
            }
            EntryCost::PayLife { amount } => {
                let can_pay = self
                    .state
                    .player_idx(event.destination_controller)
                    .is_some_and(|index| self.state.players[index].life >= amount as i32);
                (
                    format!("Pay {amount} life"),
                    format!("{amount} life"),
                    can_pay,
                )
            }
        };
        let name = self
            .battlefield_entry_face(&event)
            .map(|face| face.name.clone())
            .unwrap_or_else(|| "this permanent".to_string());
        let prompt = format!("As {name} enters, you may {label}.");
        events.push(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                rv1::ResolutionChoiceRequired {
                    candidate_token_identities: Vec::new(),
                    candidate_player_ids: Vec::new(),
                    deciding_player_id: event.destination_controller,
                    source_object_id: event.object_id,
                    prompt_text: prompt.clone(),
                    choice_kind: rv1::ChoiceKind::ResolutionBranch as i32,
                    candidate_object_ids: Vec::new(),
                    candidate_card_ids: Vec::new(),
                    min: 0,
                    max: 1,
                    ordered: false,
                    candidate_names: Vec::new(),
                    candidate_server_card_ids: Vec::new(),
                    candidate_selectable: Vec::new(),
                    unique_names: false,
                    generic_mana_cost: 0,
                    payment_currently_legal: false,
                    resolution_branches: vec![rv1::ResolutionBranchOption {
                        branch_index: 0,
                        label,
                        cost_kind: rv1::ResolutionBranchCostKind::Unspecified as i32,
                        cost_text,
                        selectable,
                        search_zones: Vec::new(),
                        presentation: None,
                    }],
                    mana_cost: String::new(),
                    public_reveal: None,
                    candidate_source_zones: Vec::new(),
                    combat_defender_options: Vec::new(),
                    waterbend: false,
                    selection_slots: Vec::new(),
                    replacement_options: Vec::new(),
                    selection_alternatives: Vec::new(),
                },
            )),
        });
        let deciding_player = event.destination_controller;
        self.state.pending_replacement_event = Some(PendingReplacementEvent::BattlefieldEntry(
            Box::new(PendingBattlefieldEntry {
                event,
                applications: Vec::new(),
                copy_source_effect: None,
                copy_source_candidates: Vec::new(),
                completion,
            }),
        ));
        self.state.pending_resolution = Some(PendingResolution {
            deciding_player,
            presentation: PendingResolutionPresentation {
                source_object_id: item.id,
                candidates: Vec::new(),
                min: 0,
                max: 1,
                ordered: false,
                prompt,
                choice_kind: rv1::ChoiceKind::ResolutionBranch,
                unique_names: false,
            },
            continuation: ResolutionContinuation::EntryCost {
                stack: ParkedStackResolution::new(item),
                effect_id,
            },
        });
    }

    fn park_read_ahead_choice(
        &mut self,
        item: StackItem,
        event: BattlefieldEntryEvent,
        completion: BattlefieldEntryCompletion,
        effect_id: EntryReplacementEffectId,
        final_chapter: u32,
        events: &mut Vec<rv1::RuledEvent>,
    ) {
        let options = (1..=final_chapter)
            .map(|chapter| rv1::ResolutionBranchOption {
                branch_index: chapter - 1,
                label: saga_chapter_label(chapter),
                cost_kind: rv1::ResolutionBranchCostKind::Unspecified as i32,
                cost_text: String::new(),
                selectable: true,
                search_zones: Vec::new(),
                presentation: None,
            })
            .collect();
        let prompt = "Choose this Saga's starting chapter (read ahead).".to_string();
        events.push(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                rv1::ResolutionChoiceRequired {
                    candidate_token_identities: Vec::new(),
                    candidate_player_ids: Vec::new(),
                    deciding_player_id: event.deciding_player,
                    source_object_id: event.object_id,
                    prompt_text: prompt.clone(),
                    choice_kind: rv1::ChoiceKind::ResolutionBranch as i32,
                    candidate_object_ids: Vec::new(),
                    candidate_card_ids: Vec::new(),
                    min: 1,
                    max: 1,
                    ordered: false,
                    candidate_names: Vec::new(),
                    candidate_server_card_ids: Vec::new(),
                    candidate_selectable: Vec::new(),
                    unique_names: false,
                    generic_mana_cost: 0,
                    payment_currently_legal: false,
                    resolution_branches: options,
                    mana_cost: String::new(),
                    public_reveal: None,
                    candidate_source_zones: Vec::new(),
                    combat_defender_options: Vec::new(),
                    waterbend: false,
                    selection_slots: Vec::new(),
                    replacement_options: Vec::new(),
                    selection_alternatives: Vec::new(),
                },
            )),
        });
        events.push(ev_log(prompt.clone()));
        let deciding_player = event.deciding_player;
        self.state.pending_replacement_event = Some(PendingReplacementEvent::BattlefieldEntry(
            Box::new(PendingBattlefieldEntry {
                event,
                applications: Vec::new(),
                copy_source_effect: None,
                copy_source_candidates: Vec::new(),
                completion,
            }),
        ));
        self.state.pending_resolution = Some(PendingResolution {
            deciding_player,
            presentation: PendingResolutionPresentation {
                source_object_id: item.id,
                candidates: Vec::new(),
                min: 1,
                max: 1,
                ordered: false,
                prompt,
                choice_kind: rv1::ChoiceKind::ResolutionBranch,
                unique_names: false,
            },
            continuation: ResolutionContinuation::SagaReadAhead {
                stack: ParkedStackResolution::new(item),
                effect_id,
            },
        });
    }

    fn copy_source_candidates(
        &self,
        event: &BattlefieldEntryEvent,
        filter: &TargetFilter,
    ) -> Vec<ObjectId> {
        let candidates = battlefield_objects_matching(self, filter);
        candidates
            .into_iter()
            .filter(|oid| *oid != event.object_id)
            .collect()
    }

    fn copy_source_description(filter: &TargetFilter) -> String {
        use tricerules_cards::primitives::{PermanentTypeFilter, TargetKind};

        let noun = match filter.permanent_types.as_slice() {
            [PermanentTypeFilter::Artifact] => "artifact".to_string(),
            [PermanentTypeFilter::Enchantment] => "enchantment".to_string(),
            [PermanentTypeFilter::Artifact, PermanentTypeFilter::Enchantment] => {
                "artifact or enchantment".to_string()
            }
            [] if filter
                .excluded_permanent_types
                .contains(&PermanentTypeFilter::Land) =>
            {
                "nonland permanent".to_string()
            }
            [] if filter.kind == TargetKind::Creature => "creature".to_string(),
            [] => "permanent".to_string(),
            _ => "matching permanent".to_string(),
        };
        let article = if matches!(noun.chars().next(), Some('a' | 'e' | 'i' | 'o' | 'u')) {
            "an"
        } else {
            "a"
        };
        format!("{article} {noun}")
    }

    pub(super) fn restore_skipped_battlefield_entry(
        &mut self,
        event: &BattlefieldEntryEvent,
    ) -> Result<(), EngineError> {
        if let Some(candidate) = Self::entry_copy_rollback_candidate(event) {
            self.restore_entry_copy_candidate(event.object_id, candidate)?;
        }
        Ok(())
    }

    fn entry_copy_rollback_candidate(
        event: &BattlefieldEntryEvent,
    ) -> Option<&PendingCopyCandidate> {
        event
            .pending_copy_candidate
            .as_ref()
            .or_else(|| {
                event
                    .pending_aura_recipient
                    .as_ref()
                    .and_then(|choice| choice.copy_candidate.as_ref())
            })
            .or_else(|| {
                event
                    .accepted_aura_recipient
                    .as_ref()
                    .and_then(|receipt| receipt.copy_candidate.as_ref())
            })
    }

    fn restore_abandoned_battlefield_entry(
        &mut self,
        event: &BattlefieldEntryEvent,
    ) -> Result<(), EngineError> {
        let candidate = Self::entry_copy_rollback_candidate(event);
        if let Some(candidate) = candidate {
            // Concession can remove the entrant or replace another member. Only restore the
            // still-present incarnation carrying precisely this provisional installation.
            if self
                .state
                .zone_change_generation
                .get(&event.object_id)
                .copied()
                .unwrap_or(0)
                == candidate.entering_zone_generation
                && self
                    .state
                    .objects
                    .get(&event.object_id)
                    .is_some_and(|object| {
                        object.copy_revision == candidate.entering_copy_revision.saturating_add(1)
                    })
            {
                self.restore_entry_copy_candidate(event.object_id, candidate)?;
            }
        }
        Ok(())
    }

    pub(super) fn accepted_entry_aura_entrant_current(
        &self,
        event: &BattlefieldEntryEvent,
    ) -> bool {
        event
            .accepted_aura_recipient
            .as_ref()
            .is_none_or(|receipt| {
                self.state
                    .zone_change_generation
                    .get(&event.object_id)
                    .copied()
                    .unwrap_or(0)
                    == receipt.entering_zone_generation
                    && self
                        .state
                        .objects
                        .get(&event.object_id)
                        .is_some_and(|object| {
                            object.copy_revision == receipt.entering_copy_revision
                        })
            })
    }

    pub(super) fn accepted_entry_aura_recipient_current(
        &mut self,
        event: &BattlefieldEntryEvent,
    ) -> Result<bool, EngineError> {
        let Some(receipt) = &event.accepted_aura_recipient else {
            return Ok(true);
        };
        if !self.accepted_entry_aura_entrant_current(event) {
            return Err(EngineError::Illegal("accepted Aura entry became stale"));
        }
        let Some(recipient) = event.attached_to else {
            return Err(EngineError::Illegal("accepted Aura entry has no recipient"));
        };
        if let AttachmentRecipient::Object(oid) = recipient {
            if self
                .state
                .objects
                .get(&oid)
                .is_none_or(|object| object.zone != Zone::Battlefield)
                || receipt.recipient_generation
                    != Some(
                        self.state
                            .zone_change_generation
                            .get(&oid)
                            .copied()
                            .unwrap_or(0),
                    )
            {
                return Ok(false);
            }
        }
        let Some(face) = self.battlefield_entry_face(event).map(Cow::into_owned) else {
            return Err(EngineError::Illegal("accepted Aura entry face disappeared"));
        };
        let Some(mut values) = self.copiable_values_for(event.object_id) else {
            return Err(EngineError::Illegal(
                "accepted Aura entry values disappeared",
            ));
        };
        values.face = face;
        let recipient_id = match recipient {
            AttachmentRecipient::Object(oid) => oid,
            AttachmentRecipient::Player(pid) => pid as ObjectId,
        };
        Ok(self
            .entry_copy_aura_candidates(
                event.object_id,
                event.destination_controller,
                &values,
                &receipt.filter,
            )
            .contains(&recipient_id))
    }

    pub(super) fn prune_invalid_zone_entry_auras(
        &mut self,
        batch: &mut crate::state::PendingZoneEntryBatch,
    ) -> Result<(), EngineError> {
        let mut skipped = HashSet::new();
        for event in &batch.ready {
            if !self.accepted_entry_aura_recipient_current(event)? {
                skipped.insert(event.object_id);
            }
        }
        for event in batch
            .ready
            .iter()
            .filter(|event| skipped.contains(&event.object_id))
        {
            self.restore_skipped_battlefield_entry(event)?;
        }
        batch
            .ready
            .retain(|event| !skipped.contains(&event.object_id));
        Ok(())
    }

    pub(super) fn prune_invalid_observer_entry_auras(
        &mut self,
        entries: &mut Vec<ObserverReturnEntry>,
    ) -> Result<(), EngineError> {
        let mut skipped = HashSet::new();
        for entry in entries.iter() {
            if !self.accepted_entry_aura_recipient_current(&entry.event)? {
                skipped.insert(entry.event.object_id);
            }
        }
        for entry in entries
            .iter()
            .filter(|entry| skipped.contains(&entry.event.object_id))
        {
            self.restore_skipped_battlefield_entry(&entry.event)?;
        }
        entries.retain(|entry| !skipped.contains(&entry.event.object_id));
        Ok(())
    }

    pub(super) fn prune_invalid_token_entry_auras(
        &mut self,
        entries: &mut Vec<TokenBattlefieldEntry>,
    ) -> Result<HashSet<ObjectId>, EngineError> {
        let mut skipped = HashSet::new();
        for entry in entries.iter() {
            if !self.accepted_entry_aura_recipient_current(&entry.event)? {
                skipped.insert(entry.event.object_id);
            }
        }
        for entry in entries
            .iter()
            .filter(|entry| skipped.contains(&entry.event.object_id))
        {
            self.restore_skipped_battlefield_entry(&entry.event)?;
            self.state.objects.remove(&entry.event.object_id);
        }
        entries.retain(|entry| !skipped.contains(&entry.event.object_id));
        Ok(skipped)
    }

    pub(super) fn reconcile_departed_token_entry_members(
        &self,
        batch: &mut PendingTokenEntryBatch,
        departed_objects: &HashSet<ObjectId>,
        spell_label: &str,
    ) {
        let prior_count = batch.ready.len() + batch.remaining.len();
        batch
            .ready
            .retain(|entry| !departed_objects.contains(&entry.event.object_id));
        batch
            .remaining
            .retain(|entry| !departed_objects.contains(&entry.event.object_id));
        batch
            .result_object_ids
            .retain(|oid| !departed_objects.contains(oid));
        if prior_count != batch.ready.len() + batch.remaining.len() {
            batch.logs = Self::token_creation_logs(
                batch.ready.iter().chain(batch.remaining.iter()),
                &batch.logs,
                spell_label,
            );
        }
    }

    pub(super) fn prune_invalid_simultaneous_entry_auras(
        &mut self,
        batch: &mut SimultaneousEntryBatch,
        spell_label: &str,
    ) -> Result<(), EngineError> {
        match batch {
            SimultaneousEntryBatch::Zone(batch) => self.prune_invalid_zone_entry_auras(batch)?,
            SimultaneousEntryBatch::Observer(batch) => {
                self.prune_invalid_observer_entry_auras(&mut batch.ready)?
            }
            SimultaneousEntryBatch::Token(batch) => {
                let skipped = self.prune_invalid_token_entry_auras(&mut batch.ready)?;
                if !skipped.is_empty() {
                    batch.result_object_ids.retain(|oid| !skipped.contains(oid));
                    batch.logs = Self::token_creation_logs(
                        batch.ready.iter().chain(batch.remaining.iter()),
                        &batch.logs,
                        spell_label,
                    );
                }
            }
        }
        Ok(())
    }

    fn prepare_entry_aura_recipient(
        &mut self,
        item: StackItem,
        mut event: BattlefieldEntryEvent,
        completion: BattlefieldEntryCompletion,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> BattlefieldEntryProgress {
        let explicitly_attached = matches!(
            &completion,
            BattlefieldEntryCompletion::PermanentSpell {
                attached_to: Some(_)
            } | BattlefieldEntryCompletion::ObserverReturn {
                attached_to: Some(_),
                ..
            }
        );
        let prospective_copy = event.pending_copy_candidate.is_some();
        if ((event.attached_to.is_some() || explicitly_attached) && !prospective_copy)
            || event.pending_aura_recipient.is_some()
            || self
                .state
                .objects
                .get(&event.object_id)
                .is_some_and(|object| object.face_down)
        {
            return BattlefieldEntryProgress::Ready(Box::new(event));
        }
        let Some(face) = self.battlefield_entry_face(&event).map(Cow::into_owned) else {
            return BattlefieldEntryProgress::Ready(Box::new(event));
        };
        let filter = face.spell_effect.iter().find_map(|effect| match effect {
            SpellEffectKind::AuraAttach { target } => Some(target.clone()),
            _ => None,
        });
        let Some(filter) = filter else {
            return BattlefieldEntryProgress::Ready(Box::new(event));
        };
        let Some(mut values) = self.copiable_values_for(event.object_id) else {
            return BattlefieldEntryProgress::Ready(Box::new(event));
        };
        values.face = face;
        let recipients = self.entry_copy_aura_candidates(
            event.object_id,
            event.destination_controller,
            &values,
            &filter,
        );
        if recipients.is_empty() {
            return BattlefieldEntryProgress::Skipped(Box::new(event));
        }
        let recipient_generations = if filter.is_player() {
            Vec::new()
        } else {
            recipients
                .iter()
                .map(|oid| {
                    (
                        *oid,
                        self.state
                            .zone_change_generation
                            .get(oid)
                            .copied()
                            .unwrap_or(0),
                    )
                })
                .collect()
        };
        event.pending_aura_recipient = Some(PendingAuraEntryRecipient {
            filter: filter.clone(),
            entering_zone_generation: self
                .state
                .zone_change_generation
                .get(&event.object_id)
                .copied()
                .unwrap_or(0),
            entering_copy_revision: self.state.objects[&event.object_id].copy_revision,
            recipient_generations,
            copy_candidate: event.pending_copy_candidate.take(),
        });
        self.park_entry_copy_aura_recipient_choice(
            ParkedStackResolution::new(item),
            event,
            completion,
            filter,
            recipients,
            events,
        );
        BattlefieldEntryProgress::Parked
    }

    /// Evaluate legal recipients against the prospective copy's characteristics, without
    /// exposing or committing those values while the second entry choice is pending.
    fn entry_copy_aura_candidates(
        &mut self,
        object_id: ObjectId,
        controller: PlayerId,
        values: &CopiableValues,
        filter: &TargetFilter,
    ) -> Vec<ObjectId> {
        let Some(object) = self.state.objects.get_mut(&object_id) else {
            return Vec::new();
        };
        let saved = (
            object.copiable_values.clone(),
            object.copy_revision,
            object.must_attack_if_able,
            object.must_block_if_able,
        );
        object.copiable_values = Some(values.clone());
        object.copy_revision = object.copy_revision.saturating_add(1);
        object.must_attack_if_able = values.face.must_attack_if_able;
        object.must_block_if_able = values.face.must_block_if_able;

        let mut candidates = if filter.is_player() {
            self.state
                .players
                .iter()
                .map(|player| player.id as ObjectId)
                .filter(|player_id| {
                    super::targeting::attachment_filter_legal(
                        self,
                        filter,
                        AttachmentRecipient::Player(*player_id as PlayerId),
                        object_id,
                        controller,
                    )
                })
                .collect::<Vec<_>>()
        } else {
            let mut object_ids = self
                .state
                .objects
                .values()
                .filter(|object| object.zone == Zone::Battlefield)
                .map(|object| object.id)
                .collect::<Vec<_>>();
            object_ids.sort_unstable();
            object_ids
                .into_iter()
                .filter(|recipient_id| {
                    super::targeting::attachment_filter_legal(
                        self,
                        filter,
                        AttachmentRecipient::Object(*recipient_id),
                        object_id,
                        controller,
                    )
                })
                .collect()
        };
        candidates.sort_unstable();

        if let Some(object) = self.state.objects.get_mut(&object_id) {
            object.copiable_values = saved.0;
            object.copy_revision = saved.1;
            object.must_attack_if_able = saved.2;
            object.must_block_if_able = saved.3;
        }
        candidates
    }

    fn park_entry_copy_aura_recipient_choice(
        &mut self,
        stack: ParkedStackResolution,
        event: BattlefieldEntryEvent,
        completion: BattlefieldEntryCompletion,
        filter: TargetFilter,
        recipients: Vec<ObjectId>,
        events: &mut Vec<rv1::RuledEvent>,
    ) {
        let choice_kind = if filter.is_player() {
            rv1::ChoiceKind::AuraPlayer
        } else {
            rv1::ChoiceKind::AuraPermanent
        };
        let name = event
            .pending_aura_recipient
            .as_ref()
            .and_then(|recipient| recipient.copy_candidate.as_ref())
            .map(|candidate| candidate.values.display_name.clone())
            .or_else(|| {
                self.state
                    .objects
                    .get(&event.object_id)
                    .and_then(|object| {
                        object
                            .copiable_values
                            .as_ref()
                            .or(object.token_origin.as_ref())
                    })
                    .map(|values| values.display_name.clone())
            })
            .or_else(|| {
                self.registry
                    .get(&self.state.objects.get(&event.object_id)?.card_id)
                    .map(|definition| definition.name.clone())
            })
            .unwrap_or_else(|| "this Aura".to_string());
        let prompt = format!("Choose what {name} will enchant as it enters.");
        let candidate_card_ids =
            if filter.is_player() {
                vec![String::new(); recipients.len()]
            } else {
                recipients
                    .iter()
                    .map(|object_id| {
                        if self.state.objects.get(object_id).is_some_and(|object| {
                            object.zone == Zone::Battlefield && object.face_down
                        }) {
                            return String::new();
                        }
                        self.effective_card_identity(*object_id)
                            .map(|(card_id, _)| card_id.to_string())
                            .unwrap_or_default()
                    })
                    .collect()
            };
        let candidate_names = recipients
            .iter()
            .map(|recipient| {
                if filter.is_player() {
                    format!("P{recipient}")
                } else {
                    super::events::object_display_name(&self.state, self.registry, *recipient)
                }
            })
            .collect();
        events.push(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                rv1::ResolutionChoiceRequired {
                    deciding_player_id: event.deciding_player,
                    source_object_id: event.object_id,
                    prompt_text: prompt.clone(),
                    choice_kind: choice_kind as i32,
                    candidate_object_ids: recipients.clone(),
                    candidate_card_ids,
                    candidate_names,
                    candidate_selectable: vec![true; recipients.len()],
                    min: 1,
                    max: 1,
                    ..Default::default()
                },
            )),
        });
        events.push(ev_log(prompt.clone()));
        let deciding_player = event.deciding_player;
        self.state.pending_replacement_event = Some(PendingReplacementEvent::BattlefieldEntry(
            Box::new(PendingBattlefieldEntry {
                event,
                applications: Vec::new(),
                copy_source_effect: None,
                copy_source_candidates: Vec::new(),
                completion,
            }),
        ));
        self.state.pending_resolution = Some(PendingResolution {
            deciding_player,
            presentation: PendingResolutionPresentation {
                source_object_id: stack.item.id,
                candidates: recipients,
                min: 1,
                max: 1,
                ordered: false,
                prompt,
                choice_kind,
                unique_names: false,
            },
            continuation: ResolutionContinuation::EntryAuraRecipient { stack },
        });
    }

    pub(super) fn transfer_entry_choice_resume(&mut self, stack: &ParkedStackResolution) {
        if let Some(pending) = self.state.pending_resolution.as_mut() {
            if let Some(parked) = pending.continuation.stack_mut() {
                parked.resume_effect_index = stack.resume_effect_index;
                parked.previous_result = stack.previous_result.clone();
            }
        }
    }

    pub(super) fn finish_entry_copy_without_recipient(
        &mut self,
        stack: ParkedStackResolution,
        event: BattlefieldEntryEvent,
        completion: BattlefieldEntryCompletion,
        mut events: Vec<rv1::RuledEvent>,
    ) -> Result<RuledEventBatch, EngineError> {
        self.restore_skipped_battlefield_entry(&event)?;
        match completion {
            BattlefieldEntryCompletion::PermanentSpell { .. } => {
                let owner = self
                    .state
                    .objects
                    .get(&event.object_id)
                    .map(|object| object.owner);
                events.push(rv1::RuledEvent {
                    ev: Some(rv1::ruled_event::Ev::StackResolved(rv1::StackResolved {
                        object_id: event.object_id,
                        destination: rv1::StackResolveDestination::Graveyard as i32,
                        owner_player_id: owner,
                    })),
                });
                move_object_to_zone(
                    &mut self.state,
                    self.registry,
                    event.object_id,
                    Zone::Graveyard,
                    None,
                )?;
                self.complete_parked_resolution(stack.item, Some(0), events)
            }
            BattlefieldEntryCompletion::ResolutionEffect { .. }
            | BattlefieldEntryCompletion::Ninjutsu { .. } => self
                .complete_parked_resolution_with_previous(
                    stack.item,
                    stack.resume_effect_index,
                    stack.previous_result,
                    events,
                ),
            BattlefieldEntryCompletion::LibrarySearch { progress, .. } => {
                self.continue_library_search_battlefield_entries(stack, progress, events)
            }
            BattlefieldEntryCompletion::ZoneEntryBatch(batch) => {
                if let Some(stack) = self.continue_zone_entry_batch(stack, *batch, &mut events)? {
                    self.complete_parked_resolution_with_previous(
                        stack.item,
                        stack.resume_effect_index,
                        stack.previous_result,
                        events,
                    )
                } else {
                    Ok(finish_with_events(self, events))
                }
            }
            BattlefieldEntryCompletion::TokenBatch(batch) => self
                .continue_token_entry_batch_without_current(stack, *batch, event.object_id, events),
            BattlefieldEntryCompletion::ObserverReturn {
                resume_original_stack,
                ..
            } => {
                if self.drain_immediate_observer_actions(
                    resume_original_stack.then_some(stack.clone()),
                    &mut events,
                )? {
                    return Ok(finish_with_events(self, events));
                }
                if resume_original_stack {
                    self.complete_parked_resolution_with_previous(
                        stack.item,
                        stack.resume_effect_index,
                        stack.previous_result,
                        events,
                    )
                } else {
                    self.apply_sbas(&mut events)?;
                    if let Some(index) = self.state.player_idx(self.state.active_player_id()) {
                        self.state.priority_idx = index;
                    }
                    events.push(ev_priority_changed(self));
                    Ok(finish_with_events(self, events))
                }
            }
            BattlefieldEntryCompletion::ManifestDread {
                owner,
                other_object_id,
                ..
            } => {
                if let Some(object) = self.state.objects.get_mut(&event.object_id) {
                    object.face_down = false;
                }
                if let Some(other) = other_object_id {
                    move_object_to_zone(
                        &mut self.state,
                        self.registry,
                        other,
                        Zone::Graveyard,
                        None,
                    )?;
                    events.push(permanent_moved_event_with_library_position(
                        &self.state,
                        other,
                        owner,
                        rv1::permanent_moved::Destination::Graveyard,
                        0,
                    ));
                }
                self.complete_parked_resolution_with_previous(
                    stack.item,
                    stack.resume_effect_index,
                    stack.previous_result,
                    events,
                )
            }
            BattlefieldEntryCompletion::LandPlay { .. } => {
                self.state.passes_since_stack_change = 0;
                Ok(finish_with_events(self, events))
            }
            BattlefieldEntryCompletion::DevPlacement {
                deferred_events,
                announce_move,
                ..
            } => {
                self.complete_skipped_dev_placement(
                    &event,
                    deferred_events,
                    announce_move,
                    &mut events,
                );
                Ok(finish_with_events(self, events))
            }
        }
    }

    fn continue_token_entry_batch_without_current(
        &mut self,
        stack: ParkedStackResolution,
        mut batch: PendingTokenEntryBatch,
        failed_object_id: ObjectId,
        mut events: Vec<rv1::RuledEvent>,
    ) -> Result<RuledEventBatch, EngineError> {
        self.state.objects.remove(&failed_object_id);
        batch
            .result_object_ids
            .retain(|object_id| *object_id != failed_object_id);
        batch.logs = Self::token_creation_logs(
            batch.ready.iter().chain(batch.remaining.iter()),
            &batch.logs,
            &stack.item.card_id,
        );

        while !batch.remaining.is_empty() {
            let next = batch.remaining.remove(0);
            let completion =
                BattlefieldEntryCompletion::TokenBatch(Box::new(PendingTokenEntryBatch {
                    current_created: next.created.clone(),
                    result_object_ids: batch.result_object_ids.clone(),
                    ready: batch.ready.clone(),
                    remaining: batch.remaining.clone(),
                    logs: batch.logs.clone(),
                    options: batch.options.clone(),
                }));
            match self.advance_or_park_battlefield_entry(
                stack.item.clone(),
                next.event,
                completion,
                &mut events,
            ) {
                BattlefieldEntryProgress::Parked => {
                    self.transfer_entry_choice_resume(&stack);
                    return Ok(finish_with_events(self, events));
                }
                BattlefieldEntryProgress::Ready(event) => {
                    batch.ready.push(TokenBattlefieldEntry {
                        event: *event,
                        created: next.created,
                    });
                }
                BattlefieldEntryProgress::Skipped(event) => {
                    self.restore_skipped_battlefield_entry(&event)?;
                    self.state.objects.remove(&event.object_id);
                    batch
                        .result_object_ids
                        .retain(|oid| *oid != event.object_id);
                    batch.logs = Self::token_creation_logs(
                        batch.ready.iter().chain(batch.remaining.iter()),
                        &batch.logs,
                        &stack.item.card_id,
                    );
                }
            }
        }
        let amass = batch.options.amass;
        let created_ids = batch.result_object_ids.clone();
        if self.finish_prepared_token_batch(
            stack.clone(),
            batch.ready,
            batch.result_object_ids,
            batch.logs,
            batch.options,
            &mut events,
        )? {
            return Ok(finish_with_events(self, events));
        }
        if let Some(amass) = amass {
            return self.finish_amass_after_token_entry(stack, amass, events);
        }
        self.complete_parked_resolution_with_previous(
            stack.item,
            stack.resume_effect_index,
            EffectResult {
                produced_objects: self.token_entry_object_refs(&created_ids),
                ..EffectResult::default()
            },
            events,
        )
    }

    fn park_copy_source_choice(
        &mut self,
        item: StackItem,
        event: BattlefieldEntryEvent,
        completion: BattlefieldEntryCompletion,
        effect_id: EntryReplacementEffectId,
        candidates: Vec<ObjectId>,
        events: &mut Vec<rv1::RuledEvent>,
    ) {
        let physical_name = self
            .state
            .objects
            .get(&event.object_id)
            .and_then(|object| self.registry.get(&object.card_id))
            .map(|definition| definition.name.clone())
            .unwrap_or_else(|| "this permanent".to_string());
        let source_filter = self
            .entry_copy_filter(&event, &effect_id)
            .expect("copy-source prompt requires an active copy filter");
        let source_description = Self::copy_source_description(&source_filter);
        let prompt = format!(
            "Choose {source_description} for {physical_name} to copy, or Decline to enter as {physical_name}."
        );
        let candidate_card_ids = candidates
            .iter()
            .map(|oid| {
                if self
                    .state
                    .objects
                    .get(oid)
                    .is_some_and(|object| object.zone == Zone::Battlefield && object.face_down)
                {
                    return String::new();
                }
                self.effective_card_identity(*oid)
                    .map(|(card_id, _)| card_id.to_string())
                    .unwrap_or_default()
            })
            .collect();
        let candidate_names = candidates
            .iter()
            .map(|oid| super::events::object_display_name(&self.state, self.registry, *oid))
            .collect();
        events.push(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                rv1::ResolutionChoiceRequired {
                    candidate_token_identities: Vec::new(),
                    candidate_player_ids: Vec::new(),
                    deciding_player_id: event.deciding_player,
                    source_object_id: event.object_id,
                    prompt_text: prompt.clone(),
                    choice_kind: rv1::ChoiceKind::CopySource as i32,
                    candidate_object_ids: candidates.clone(),
                    candidate_card_ids,
                    min: 0,
                    max: 1,
                    ordered: false,
                    candidate_names,
                    candidate_server_card_ids: Vec::new(),
                    candidate_selectable: Vec::new(),
                    resolution_branches: Vec::new(),
                    mana_cost: String::new(),
                    unique_names: false,
                    generic_mana_cost: 0,
                    payment_currently_legal: false,
                    public_reveal: None,
                    candidate_source_zones: Vec::new(),
                    combat_defender_options: Vec::new(),
                    waterbend: false,
                    selection_slots: Vec::new(),
                    replacement_options: Vec::new(),
                    selection_alternatives: Vec::new(),
                },
            )),
        });
        events.push(ev_log(prompt.clone()));
        let deciding_player = event.deciding_player;
        let entering_zone = self.state.objects[&event.object_id].zone;
        let entering_generation = self
            .state
            .zone_change_generation
            .get(&event.object_id)
            .copied()
            .unwrap_or(0);
        self.state.pending_replacement_event = Some(PendingReplacementEvent::BattlefieldEntry(
            Box::new(PendingBattlefieldEntry {
                event,
                applications: Vec::new(),
                copy_source_effect: Some(effect_id),
                copy_source_candidates: candidates
                    .iter()
                    .map(|candidate| {
                        (
                            *candidate,
                            self.state
                                .zone_change_generation
                                .get(candidate)
                                .copied()
                                .unwrap_or(0),
                        )
                    })
                    .collect(),
                completion,
            }),
        ));
        self.state.pending_resolution = Some(PendingResolution {
            deciding_player,
            presentation: PendingResolutionPresentation {
                source_object_id: item.id,
                candidates,
                min: 0,
                max: 1,
                ordered: false,
                prompt,
                choice_kind: rv1::ChoiceKind::CopySource,
                unique_names: false,
            },
            continuation: ResolutionContinuation::EntryCopySource {
                stack: ParkedStackResolution::new(item),
                entering_zone,
                entering_generation,
            },
        });
    }

    fn apply_entry_replacement(
        &self,
        event: &mut BattlefieldEntryEvent,
        effect_id: EntryReplacementEffectId,
    ) {
        match &effect_id {
            EntryReplacementEffectId::Intrinsic {
                object_id,
                copy_revision,
                ability_index,
            } => {
                debug_assert_eq!(*object_id, event.object_id);
                let effective_face = self
                    .state
                    .objects
                    .get(object_id)
                    .filter(|object| object.copy_revision == *copy_revision)
                    .and_then(|_| self.battlefield_entry_face(event));
                let ability = effective_face
                    .as_deref()
                    .and_then(|face| face.static_abilities.get(*ability_index))
                    .map(|ability| &ability.definition);
                match ability {
                    Some(StaticAbilityDef::EntersPrepared) => event.prepared = true,
                    Some(StaticAbilityDef::EntersAsCopy { .. }) => {
                        debug_assert!(false, "copy source choice must be completed before apply")
                    }
                    Some(StaticAbilityDef::EntersTapped {
                        affected: EntersTappedAffected::Self_,
                        ..
                    }) => event.tapped = true,
                    Some(StaticAbilityDef::EntersWithChosenBasicLandType { .. }) => {
                        debug_assert!(event.chosen_basic_land_type.is_some());
                        event.tapped = true;
                    }
                    Some(StaticAbilityDef::AsEntersChooseOpponent { .. }) => {}
                    Some(StaticAbilityDef::EntersWithCounters {
                        affected: EntersWithCountersAffected::Self_,
                        counter,
                        amount,
                        ..
                    }) => {
                        let origin = self.battlefield_entry_cast_cost_origin(event);
                        let count = self.resolve_amount(
                            amount,
                            AmountContext {
                                entry_cast_cost_receipts: &event.cast_cost_receipts,
                                entry_cast_cost_origin: origin.as_ref(),
                                entry_mana_colors_spent: event.mana_colors_spent_to_cast,
                                stack_item: None,
                                controller: event.destination_controller,
                                source_object_id: event.object_id,
                                source_zone_change: self
                                    .state
                                    .zone_change_generation
                                    .get(&event.object_id)
                                    .copied()
                                    .unwrap_or(0),
                                resolving_spell_id: None,
                                chosen_x: event.chosen_x,
                                previous_effect_result: None,
                            },
                        );
                        accumulate_entry_counters(&mut event.entry_counters, *counter, count);
                    }
                    _ => debug_assert!(false, "stale intrinsic entry replacement"),
                }
            }
            EntryReplacementEffectId::Battlefield {
                source_id,
                source_generation,
                ability_index,
            } => {
                let effective_face = self
                    .state
                    .objects
                    .get(source_id)
                    .filter(|source| source.zone == Zone::Battlefield)
                    .filter(|_| {
                        self.state
                            .zone_change_generation
                            .get(source_id)
                            .copied()
                            .unwrap_or(0)
                            == *source_generation
                    })
                    .and_then(|_| self.effective_face(*source_id));
                let ability = effective_face
                    .as_deref()
                    .and_then(|face| face.static_abilities.get(*ability_index))
                    .map(|ability| &ability.definition);
                match ability {
                    Some(StaticAbilityDef::EntersTapped {
                        affected: EntersTappedAffected::Permanents,
                        ..
                    }) => event.tapped = true,
                    Some(StaticAbilityDef::EntersWithCounters {
                        affected: EntersWithCountersAffected::Creatures(_),
                        counter,
                        amount,
                        ..
                    }) => {
                        let controller = self
                            .controller_of(*source_id)
                            .unwrap_or(event.destination_controller);
                        let count = self.resolve_amount(
                            amount,
                            AmountContext {
                                entry_cast_cost_receipts: &[],
                                entry_cast_cost_origin: None,
                                entry_mana_colors_spent: Default::default(),
                                stack_item: None,
                                controller,
                                source_object_id: *source_id,
                                source_zone_change: *source_generation,
                                resolving_spell_id: None,
                                chosen_x: event.chosen_x,
                                previous_effect_result: None,
                            },
                        );
                        accumulate_entry_counters(&mut event.entry_counters, *counter, count);
                    }
                    _ => debug_assert!(false, "stale battlefield entry replacement"),
                }
            }
            EntryReplacementEffectId::ReadAhead { .. } => {
                debug_assert!(false, "read-ahead choice must be completed before apply")
            }
        }
        event.applied_effects.push(effect_id);
    }

    pub(super) fn begin_battlefield_entry(
        &mut self,
        item: StackItem,
        event: BattlefieldEntryEvent,
        completion: BattlefieldEntryCompletion,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> BattlefieldEntryProgress {
        self.advance_or_park_battlefield_entry(item, event, completion, events)
    }

    fn advance_or_park_battlefield_entry(
        &mut self,
        item: StackItem,
        mut event: BattlefieldEntryEvent,
        completion: BattlefieldEntryCompletion,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> BattlefieldEntryProgress {
        loop {
            let candidates = self.battlefield_entry_candidates(&event);
            match candidates.as_slice() {
                [] => {
                    let is_battle = self.battlefield_entry_is_battle(&event);
                    if is_battle && event.battle_protector.is_none() {
                        let protectors: Vec<_> = self
                            .state
                            .players
                            .iter()
                            .filter(|player| self.entry_battle_protector_is_live(&event, player.id))
                            .map(|player| player.id)
                            .collect();
                        if !protectors.is_empty() {
                            let name = self
                                .state
                                .objects
                                .get(&event.object_id)
                                .and_then(|object| self.registry.get(&object.card_id))
                                .map(|definition| definition.name.as_str())
                                .unwrap_or("this Battle");
                            let prompt = format!("Choose a player to protect {name}.");
                            let candidate_ids: Vec<_> =
                                protectors.iter().map(|player| *player as u32).collect();
                            events.push(rv1::RuledEvent {
                                ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                                    rv1::ResolutionChoiceRequired {
                                        candidate_token_identities: Vec::new(),
                                        candidate_player_ids: Vec::new(),
                                        deciding_player_id: event.deciding_player,
                                        source_object_id: event.object_id,
                                        prompt_text: prompt.clone(),
                                        choice_kind: rv1::ChoiceKind::BattleProtector as i32,
                                        candidate_object_ids: candidate_ids.clone(),
                                        candidate_card_ids: vec![String::new(); protectors.len()],
                                        min: 1,
                                        max: 1,
                                        ordered: false,
                                        candidate_names: protectors
                                            .iter()
                                            .map(|player| format!("P{player}"))
                                            .collect(),
                                        candidate_server_card_ids: Vec::new(),
                                        unique_names: false,
                                        generic_mana_cost: 0,
                                        payment_currently_legal: false,
                                        resolution_branches: Vec::new(),
                                        mana_cost: String::new(),
                                        candidate_selectable: Vec::new(),
                                        public_reveal: None,
                                        candidate_source_zones: Vec::new(),
                                        combat_defender_options: Vec::new(),
                                        waterbend: false,
                                        selection_slots: Vec::new(),
                                        replacement_options: Vec::new(),
                                        selection_alternatives: Vec::new(),
                                    },
                                )),
                            });
                            events.push(ev_log(prompt.clone()));
                            let deciding_player = event.deciding_player;
                            self.state.pending_replacement_event =
                                Some(PendingReplacementEvent::BattlefieldEntry(Box::new(
                                    PendingBattlefieldEntry {
                                        event,
                                        applications: Vec::new(),
                                        copy_source_effect: None,
                                        copy_source_candidates: Vec::new(),
                                        completion,
                                    },
                                )));
                            self.state.pending_resolution = Some(PendingResolution {
                                deciding_player,
                                presentation: PendingResolutionPresentation {
                                    source_object_id: item.id,
                                    candidates: candidate_ids,
                                    min: 1,
                                    max: 1,
                                    ordered: false,
                                    prompt,
                                    choice_kind: rv1::ChoiceKind::BattleProtector,
                                    unique_names: false,
                                },
                                continuation: ResolutionContinuation::BattleProtector {
                                    stack: ParkedStackResolution::new(item),
                                },
                            });
                            return BattlefieldEntryProgress::Parked;
                        }
                    }
                    return self.prepare_entry_aura_recipient(item, event, completion, events);
                }
                [(effect_id, _, _)] => {
                    if let Some(filter) = self.entry_copy_filter(&event, effect_id) {
                        let sources = self.copy_source_candidates(&event, &filter);
                        if sources.is_empty() {
                            event.applied_effects.push(effect_id.clone());
                            continue;
                        }
                        self.park_copy_source_choice(
                            item,
                            event,
                            completion,
                            effect_id.clone(),
                            sources,
                            events,
                        );
                        return BattlefieldEntryProgress::Parked;
                    }
                    if let Some(final_chapter) =
                        self.entry_read_ahead_final_chapter(&event, effect_id)
                    {
                        self.park_read_ahead_choice(
                            item,
                            event,
                            completion,
                            effect_id.clone(),
                            final_chapter,
                            events,
                        );
                        return BattlefieldEntryProgress::Parked;
                    }
                    if self.entry_needs_basic_land_type_choice(&event, effect_id) {
                        self.park_basic_land_type_choice(
                            item,
                            event,
                            completion,
                            effect_id.clone(),
                            events,
                        );
                        return BattlefieldEntryProgress::Parked;
                    }
                    if self.entry_opponent_key(&event, effect_id).is_some() {
                        if self.entry_opponent_players(&event).is_empty() {
                            event.applied_effects.push(effect_id.clone());
                            continue;
                        }
                        self.park_entry_opponent_choice(
                            item,
                            event,
                            completion,
                            effect_id.clone(),
                            events,
                        );
                        return BattlefieldEntryProgress::Parked;
                    }
                    if let Some(cost) = self.entry_unless_cost(&event, effect_id) {
                        self.park_entry_cost_choice(
                            item,
                            event,
                            completion,
                            effect_id.clone(),
                            cost,
                            events,
                        );
                        return BattlefieldEntryProgress::Parked;
                    }
                    self.apply_entry_replacement(&mut event, effect_id.clone());
                }
                _ => {
                    let mut applications = Vec::new();
                    let mut application_ids = Vec::new();
                    let mut candidate_names = Vec::new();
                    let mut replacement_options = Vec::new();
                    for (effect_id, _, label) in candidates {
                        let application_id = self.state.next_replacement_application_id;
                        self.state.next_replacement_application_id =
                            application_id.saturating_add(1);
                        let source = match &effect_id {
                            EntryReplacementEffectId::Battlefield { source_id, .. } => {
                                self.replacement_source_presentation(*source_id)
                            }
                            EntryReplacementEffectId::Intrinsic { object_id, .. }
                            | EntryReplacementEffectId::ReadAhead { object_id, .. } => {
                                ReplacementSourcePresentation {
                                    card_name: self.replacement_object_display_name(
                                        *object_id,
                                        Some(event.face_index),
                                    ),
                                    object_id: *object_id,
                                    zone_change_generation: self
                                        .state
                                        .zone_change_generation
                                        .get(object_id)
                                        .copied()
                                        .unwrap_or(0),
                                }
                            }
                        };
                        replacement_options.push(source.option(application_id, label.clone()));
                        applications.push(EntryReplacementApplication {
                            application_id,
                            effect_id,
                        });
                        application_ids.push(application_id);
                        candidate_names.push(label);
                    }
                    let prompt = format!(
                        "Choose the next replacement effect for {} entering the battlefield.",
                        self.state
                            .objects
                            .get(&event.object_id)
                            .and_then(|object| self.registry.get(&object.card_id))
                            .map(|definition| definition.name.as_str())
                            .unwrap_or("this permanent")
                    );
                    events.push(rv1::RuledEvent {
                        ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                            rv1::ResolutionChoiceRequired {
                                candidate_token_identities: Vec::new(),
                                candidate_player_ids: Vec::new(),
                                deciding_player_id: event.deciding_player,
                                source_object_id: event.object_id,
                                prompt_text: prompt.clone(),
                                choice_kind: rv1::ChoiceKind::ReplacementEffect as i32,
                                candidate_object_ids: application_ids.clone(),
                                candidate_card_ids: vec![String::new(); application_ids.len()],
                                min: 1,
                                max: 1,
                                ordered: false,
                                candidate_names,
                                candidate_server_card_ids: Vec::new(),
                                candidate_selectable: Vec::new(),
                                resolution_branches: Vec::new(),
                                mana_cost: String::new(),
                                unique_names: false,
                                generic_mana_cost: 0,
                                payment_currently_legal: false,
                                public_reveal: None,
                                candidate_source_zones: Vec::new(),
                                combat_defender_options: Vec::new(),
                                waterbend: false,
                                selection_slots: Vec::new(),
                                replacement_options,
                                selection_alternatives: Vec::new(),
                            },
                        )),
                    });
                    events.push(ev_log(prompt.clone()));
                    let deciding_player = event.deciding_player;
                    self.state.pending_replacement_event =
                        Some(PendingReplacementEvent::BattlefieldEntry(Box::new(
                            PendingBattlefieldEntry {
                                event,
                                applications,
                                copy_source_effect: None,
                                copy_source_candidates: Vec::new(),
                                completion,
                            },
                        )));
                    self.state.pending_resolution = Some(PendingResolution {
                        deciding_player,
                        presentation: PendingResolutionPresentation {
                            source_object_id: item.id,
                            candidates: application_ids,
                            min: 1,
                            max: 1,
                            ordered: false,
                            prompt,
                            choice_kind: rv1::ChoiceKind::ReplacementEffect,
                            unique_names: false,
                        },
                        continuation: ResolutionContinuation::EntryReplacement {
                            stack: ParkedStackResolution::new(item),
                        },
                    });
                    return BattlefieldEntryProgress::Parked;
                }
            }
        }
    }

    pub(super) fn commit_battlefield_entry(
        &mut self,
        event: BattlefieldEntryEvent,
        attached_to: Option<AttachmentRecipient>,
        out: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        let attached_to = attached_to.or(event.attached_to);
        let object_id = event.object_id;
        let chosen_x = event.chosen_x;
        let door_event = self.commit_battlefield_entry_state(event, attached_to)?;
        let mut trigger_events = vec![GameEvent::EntersBattlefield {
            object_id,
            chosen_x,
        }];
        trigger_events.extend(door_event);
        self.fire_triggers(&trigger_events, out);
        Ok(())
    }

    pub(super) fn entry_battle_protector_is_live(
        &self,
        event: &BattlefieldEntryEvent,
        protector: PlayerId,
    ) -> bool {
        self.state
            .are_opponents(event.destination_controller, protector)
            && [event.destination_controller, protector]
                .iter()
                .all(|&player| {
                    self.state
                        .player_idx(player)
                        .is_some_and(|index| !self.state.players[index].has_lost)
                })
    }

    pub(super) fn validate_battlefield_entry_commit(
        &self,
        event: &BattlefieldEntryEvent,
    ) -> Result<(), EngineError> {
        let object = self
            .state
            .objects
            .get(&event.object_id)
            .ok_or(EngineError::Illegal("no object"))?;
        if self.state.player_idx(object.owner).is_none()
            || self
                .state
                .player_idx(event.destination_controller)
                .is_none()
        {
            return Err(EngineError::Illegal("no such entry player"));
        }
        let is_battle = self.battlefield_entry_is_battle(event);
        if is_battle
            && !event
                .battle_protector
                .is_some_and(|protector| self.entry_battle_protector_is_live(event, protector))
        {
            return Err(EngineError::Illegal(
                "Battle entry requires a valid protector",
            ));
        }
        if !is_battle && event.battle_protector.is_some() {
            return Err(EngineError::Illegal(
                "non-Battle entry cannot carry a protector",
            ));
        }
        Ok(())
    }

    pub(super) fn commit_battlefield_entry_state(
        &mut self,
        event: BattlefieldEntryEvent,
        attached_to: Option<AttachmentRecipient>,
    ) -> Result<Vec<GameEvent>, EngineError> {
        self.validate_battlefield_entry_commit(&event)?;
        // CR 400.7a: the marked characteristic-changing mana effects on a permanent spell
        // continue to apply to the permanent it becomes. Do not carry unrelated stack effects.
        let carries_spell_effects = self
            .state
            .objects
            .get(&event.object_id)
            .is_some_and(|object| object.zone == Zone::Stack)
            && self
                .state
                .spell_effects_carry_to_permanent
                .contains(&event.object_id);
        let spell_effects = if carries_spell_effects {
            self.state
                .continuous_effects
                .iter()
                .filter(|effect| {
                    matches!(effect.affected, AffectedScope::Single(id) if id == event.object_id)
                        && matches!(effect.kind, ContinuousEffectKind::Layer6AddKeyword(_))
                })
                .cloned()
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let zone_snapshot = self.snapshot_zone_event();
        let battle_protector = event.battle_protector;
        let read_ahead_entry = event
            .applied_effects
            .iter()
            .any(|effect| matches!(effect, EntryReplacementEffectId::ReadAhead { .. }));
        let (is_room, enters_as_copy) = self
            .state
            .objects
            .get(&event.object_id)
            .map(|object| {
                let snapshot = object
                    .copiable_values
                    .as_ref()
                    .or(object.token_origin.as_ref());
                let is_room = snapshot
                    .map(|values| values.room_faces.is_some())
                    .unwrap_or_else(|| {
                        self.registry
                            .get(&object.card_id)
                            .is_some_and(|definition| definition.layout == Layout::Room)
                    });
                (is_room, snapshot.is_some())
            })
            .ok_or(EngineError::Illegal("no object"))?;
        move_object_to_zone(
            &mut self.state,
            self.registry,
            event.object_id,
            Zone::Battlefield,
            Some(event.destination_controller),
        )?;
        for mut record in event.chosen_opponents.iter().cloned() {
            record.key.source_zone_change = self.state.zone_change_generation[&event.object_id];
            self.state.chosen_opponents.push(record);
        }
        self.state.continuous_effects.extend(spell_effects);
        let bargained = event.cast_cost_receipts.iter().any(|receipt| {
            receipt.object_cost_kind == Some(tricerules_cards::ObjectCastCostKind::Bargain)
        });
        if event.cast_by.is_some() || bargained {
            self.state.spell_entry_facts.insert(
                event.object_id,
                crate::state::SpellEntryFact {
                    object_id: event.object_id,
                    zone_change_generation: self.state.zone_change_generation[&event.object_id],
                    caster: event.cast_by,
                    bargained,
                },
            );
        }
        self.state
            .continuous_effects
            .extend(materialize_entry_modifiers(
                &event,
                self.state.command_index,
            ));
        if let Some(object) = self.state.objects.get_mut(&event.object_id) {
            object.face_up_index = event.face_index;
            object.tapped = event.tapped;
            object.counters.clear();
            object.counter_timestamps.clear();
            object.attached_to = attached_to;
        }
        let mut trigger_events = Vec::new();
        for (counter, count) in event.entry_counters {
            if let Some(placed) = self.place_counters_with_event(
                event.object_id,
                counter,
                count,
                read_ahead_entry && counter == CounterKind::Lore,
                super::continuous::CounterPlacementOrigin::Entry,
            ) {
                trigger_events.push(placed);
            }
        }
        if event.prepared {
            self.prepare_permanent(event.object_id);
        }
        if let Some(protector) = battle_protector {
            self.state
                .battle_protectors
                .insert(event.object_id, protector);
        }
        trigger_events.insert(0, self.finish_zone_event(zone_snapshot));
        if !is_room {
            return Ok(trigger_events);
        }
        self.state
            .room_states
            .insert(event.object_id, RoomState::default());
        if let Some(face_index) = event.unlock_room_door.filter(|_| !enters_as_copy) {
            let static_start = self.state.continuous_effects.len();
            trigger_events.push(self.transition_room_door(event.object_id, face_index)?);
            self.order_new_entry_statics_before_modifiers(event.object_id, static_start);
        }
        Ok(trigger_events)
    }

    pub(super) fn begin_token_entry_batch(
        &mut self,
        item: StackItem,
        mut entries: Vec<TokenBattlefieldEntry>,
        mut logs: Vec<String>,
        options: TokenEntryBatchOptions,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<bool, EngineError> {
        let mut result_object_ids = entries
            .iter()
            .map(|entry| entry.event.object_id)
            .collect::<Vec<_>>();
        // CR 616.1: when one simultaneous event requires choices from multiple players, those
        // players make them in APNAP order. Stable sorting preserves mint order within one seat.
        entries.sort_by_key(|entry| self.state.apnap_rank(entry.event.deciding_player));
        let mut ready = Vec::new();
        while !entries.is_empty() {
            let current = entries.remove(0);
            let completion =
                BattlefieldEntryCompletion::TokenBatch(Box::new(PendingTokenEntryBatch {
                    current_created: current.created.clone(),
                    result_object_ids: result_object_ids.clone(),
                    ready: ready.clone(),
                    remaining: entries.clone(),
                    logs: logs.clone(),
                    options: options.clone(),
                }));
            match self.advance_or_park_battlefield_entry(
                item.clone(),
                current.event,
                completion,
                events,
            ) {
                BattlefieldEntryProgress::Parked => return Ok(true),
                BattlefieldEntryProgress::Skipped(event) => {
                    self.restore_skipped_battlefield_entry(&event)?;
                    self.state.objects.remove(&event.object_id);
                    result_object_ids.retain(|oid| *oid != event.object_id);
                    logs = Self::token_creation_logs(
                        ready.iter().chain(entries.iter()),
                        &logs,
                        &item.card_id,
                    );
                }
                BattlefieldEntryProgress::Ready(event) => ready.push(TokenBattlefieldEntry {
                    event: *event,
                    created: current.created,
                }),
            }
        }
        self.finish_prepared_token_batch(
            ParkedStackResolution::new(item),
            ready,
            result_object_ids,
            logs,
            options,
            events,
        )
    }

    fn continue_token_entry_batch(
        &mut self,
        stack: ParkedStackResolution,
        current: TokenBattlefieldEntry,
        mut batch: PendingTokenEntryBatch,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<bool, EngineError> {
        batch.ready.push(current);
        while !batch.remaining.is_empty() {
            let next = batch.remaining.remove(0);
            let completion =
                BattlefieldEntryCompletion::TokenBatch(Box::new(PendingTokenEntryBatch {
                    current_created: next.created.clone(),
                    result_object_ids: batch.result_object_ids.clone(),
                    ready: batch.ready.clone(),
                    remaining: batch.remaining.clone(),
                    logs: batch.logs.clone(),
                    options: batch.options.clone(),
                }));
            match self.advance_or_park_battlefield_entry(
                stack.item.clone(),
                next.event,
                completion,
                events,
            ) {
                BattlefieldEntryProgress::Parked => {
                    self.transfer_entry_choice_resume(&stack);
                    return Ok(true);
                }
                BattlefieldEntryProgress::Skipped(event) => {
                    self.restore_skipped_battlefield_entry(&event)?;
                    self.state.objects.remove(&event.object_id);
                    batch
                        .result_object_ids
                        .retain(|oid| *oid != event.object_id);
                    batch.logs = Self::token_creation_logs(
                        batch.ready.iter().chain(batch.remaining.iter()),
                        &batch.logs,
                        &stack.item.card_id,
                    );
                }
                BattlefieldEntryProgress::Ready(event) => batch.ready.push(TokenBattlefieldEntry {
                    event: *event,
                    created: next.created,
                }),
            }
        }
        self.finish_prepared_token_batch(
            stack,
            batch.ready,
            batch.result_object_ids,
            batch.logs,
            batch.options,
            events,
        )
    }

    fn token_creation_logs<'a>(
        entries: impl Iterator<Item = &'a TokenBattlefieldEntry>,
        previous_logs: &[String],
        fallback: &str,
    ) -> Vec<String> {
        let spell_label = previous_logs
            .first()
            .and_then(|log| log.rsplit_once(" ("))
            .map(|(_, suffix)| suffix.trim_end_matches(")."))
            .unwrap_or(fallback);
        let mut counts = BTreeMap::<(PlayerId, String), usize>::new();
        for entry in entries {
            let name = entry
                .created
                .identity
                .as_ref()
                .map(|identity| identity.name.clone())
                .unwrap_or_else(|| entry.created.card_id.clone());
            *counts
                .entry((entry.event.destination_controller, name))
                .or_default() += 1;
        }
        counts
            .into_iter()
            .map(|((player, name), count)| {
                let noun = if count == 1 { "token" } else { "tokens" };
                format!("P{player} creates {count} {name} {noun} ({spell_label}).")
            })
            .collect()
    }

    fn finish_prepared_token_batch(
        &mut self,
        stack: ParkedStackResolution,
        ready: Vec<TokenBattlefieldEntry>,
        result_object_ids: Vec<ObjectId>,
        logs: Vec<String>,
        options: TokenEntryBatchOptions,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<bool, EngineError> {
        if ready.is_empty() {
            self.commit_token_entry_batch(
                &stack.item,
                ready,
                logs,
                options.attacking,
                options.delayed_sacrifice,
                events,
            )?;
            return Ok(false);
        }
        let Some(SimultaneousEntryBatch::Token(batch)) = self.begin_entry_timestamp_order(
            SimultaneousEntryBatch::Token(Box::new(PendingTokenEntryBatch {
                current_created: ready[0].created.clone(),
                result_object_ids,
                ready,
                remaining: Vec::new(),
                logs,
                options,
            })),
            Some(stack.clone()),
            events,
        )?
        else {
            return Ok(true);
        };
        self.commit_token_entry_batch(
            &stack.item,
            batch.ready,
            batch.logs,
            batch.options.attacking,
            batch.options.delayed_sacrifice,
            events,
        )?;
        Ok(false)
    }

    pub(super) fn commit_token_entry_batch(
        &mut self,
        item: &StackItem,
        mut entries: Vec<TokenBattlefieldEntry>,
        mut logs: Vec<String>,
        attacking: Option<AttackingTokenBatch>,
        delayed_sacrifice: Option<DelayedTokenSacrificeTiming>,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        if !self
            .prune_invalid_token_entry_auras(&mut entries)?
            .is_empty()
        {
            logs = Self::token_creation_logs(entries.iter(), &logs, &item.card_id);
        }
        let object_ids = entries
            .iter()
            .map(|entry| entry.event.object_id)
            .collect::<Vec<_>>();
        let mut trigger_events = Vec::new();
        for entry in &entries {
            trigger_events.push(GameEvent::EntersBattlefield {
                object_id: entry.event.object_id,
                chosen_x: entry.event.chosen_x,
            });
            trigger_events.extend(
                self.commit_battlefield_entry_state(entry.event.clone(), entry.event.attached_to)?,
            );
        }
        let added_assignments = if let Some(attacking) = &attacking {
            self.add_attacking_objects(&object_ids, &attacking.defenders)?
        } else {
            Vec::new()
        };
        for mut entry in entries {
            // Replacement choices may have changed both characteristics and entry status.
            // The physical token must be minted with the final, public identity.
            entry.created.enters_tapped = entry.event.tapped;
            entry.created.controller_player_id = entry.event.destination_controller;
            if let Some(values) = self.copiable_values_for(entry.event.object_id) {
                entry.created.identity = Some(super::resolution::token_identity(&values));
            }
            events.push(rv1::RuledEvent {
                ev: Some(rv1::ruled_event::Ev::TokenCreated(entry.created)),
            });
        }
        if !added_assignments.is_empty() {
            events.push(rv1::RuledEvent {
                ev: Some(rv1::ruled_event::Ev::AttackersAdded(rv1::AttackersAdded {
                    assignments: added_assignments,
                })),
            });
        }
        if let (Some(delayed_sacrifice), false) = (delayed_sacrifice, object_ids.is_empty()) {
            let observed_objects = object_ids
                .iter()
                .filter_map(|object_id| {
                    self.state
                        .objects
                        .get(object_id)
                        .map(|object| TriggerObjectRef {
                            object_id: *object_id,
                            zone_change_generation: self
                                .state
                                .zone_change_generation
                                .get(object_id)
                                .copied()
                                .unwrap_or(0),
                            controller_at_event: object.controller,
                        })
                })
                .collect::<Vec<_>>();
            if let Some(&watched) = observed_objects.first() {
                self.state.observed_object_cohorts.insert(
                    (watched.object_id, watched.zone_change_generation),
                    observed_objects.clone(),
                );
                let card_name = self
                    .registry
                    .get(&item.card_id)
                    .map(|definition| definition.name.clone())
                    .unwrap_or_else(|| item.card_id.clone());
                let (matcher, trigger, _text) = match delayed_sacrifice {
                    DelayedTokenSacrificeTiming::NextEndStep => (
                        EventObserverMatcher::AtBeginningOfNextEndStep,
                        TriggerCondition::AtBeginningOfNextEndStep,
                        "At the beginning of the next end step, sacrifice those tokens.",
                    ),
                    DelayedTokenSacrificeTiming::ControllerNextTurnEndStep => (
                        EventObserverMatcher::AtBeginningOfControllerNextTurnEndStep {
                            controller: item.controller,
                            created_turn_instance: self.state.turn_instance,
                            target_turn_instance: None,
                        },
                        TriggerCondition::AtBeginningOfControllerNextTurnEndStep,
                        "At the beginning of the end step on your next turn, sacrifice those tokens.",
                    ),
                };
                let ability = TriggeredAbilityDef {
                    ability_id: tricerules_cards::AbilityId::new("delayed_sacrifice")
                        .expect("intrinsic ability id"),
                    presentation: tricerules_cards::AbilityPresentation::Fallback,
                    trigger,
                    effect: vec![SpellEffectKind::SacrificeObservedObjects],
                    modal: None,
                    targeting: None,
                    may: false,
                    intervening_if: None,
                    max_triggers_per_turn: None,
                    triggers_only_once: false,
                };
                let ability_text = ability.fallback_text(&card_name);
                let parent = self
                    .state
                    .stack_presentations
                    .get(&item.id)
                    .and_then(|stack| stack.primary.as_ref());
                let presentation = stack_child_presentation_ref(
                    self.registry,
                    &item.card_id,
                    item.face_index,
                    StackPresentationSource::for_stack(parent, item.ability_text.is_none()),
                    PresentationPath::Ability(&ability.ability_id),
                    &ability.presentation,
                    ability_text,
                );
                self.state.active_event_observers.push(ActiveEventObserver {
                    watched,
                    matcher,
                    payload: EventObserverPayload::StageDelayedTrigger(Box::new(
                        DelayedTriggerPayload {
                            source: TriggerObjectRef {
                                object_id: item.source_permanent_id.unwrap_or(item.id),
                                zone_change_generation: item
                                    .cast_occurrence
                                    .and_then(|cast| cast.zone_change_generation)
                                    .unwrap_or(item.source_zone_change),
                                controller_at_event: item.controller,
                            },
                            controller: item.controller,
                            card_id: item.card_id.clone(),
                            card_name,
                            source_face_index: item.face_index,
                            presentation,
                            ability,
                        },
                    )),
                });
            }
        }
        self.fire_triggers(&trigger_events, events);
        events.extend(logs.into_iter().map(ev_log));
        Ok(())
    }

    pub(super) fn complete_pending_battlefield_entry(
        &mut self,
        pending: PendingResolution,
        event: BattlefieldEntryEvent,
        completion: BattlefieldEntryCompletion,
        mut events: Vec<rv1::RuledEvent>,
    ) -> Result<RuledEventBatch, EngineError> {
        let mut stack = pending
            .continuation
            .stack()
            .ok_or(EngineError::Illegal(
                "battlefield-entry continuation missing",
            ))?
            .clone();
        if !self.accepted_entry_aura_recipient_current(&event)? {
            return self.finish_entry_copy_without_recipient(stack, event, completion, events);
        }
        match completion {
            BattlefieldEntryCompletion::LandPlay { player, land_name } => {
                let object_id = event.object_id;
                self.commit_battlefield_entry(event, None, &mut events)?;
                events.push(permanent_moved_event(
                    &self.state,
                    object_id,
                    player,
                    rv1::permanent_moved::Destination::Battlefield,
                ));
                self.state.passes_since_stack_change = 0;
                events.push(ev_log(format!("P{player} played {land_name}")));
                Ok(finish_with_events(self, events))
            }
            BattlefieldEntryCompletion::PermanentSpell { attached_to } => {
                events.push(rv1::RuledEvent {
                    ev: Some(rv1::ruled_event::Ev::StackResolved(rv1::StackResolved {
                        object_id: event.object_id,
                        destination: rv1::StackResolveDestination::Battlefield as i32,
                        owner_player_id: self
                            .state
                            .objects
                            .get(&event.object_id)
                            .map(|object| object.owner),
                    })),
                });
                self.commit_battlefield_entry(event, attached_to, &mut events)?;
                self.finish_permanent_spell_entry(&stack.item, &mut events);
                self.complete_parked_resolution(stack.item, Some(0), events)
            }
            BattlefieldEntryCompletion::ResolutionEffect {
                owner,
                spell_label,
                object_label,
                from_zone,
            } => {
                let object_id = event.object_id;
                self.commit_battlefield_entry(event, None, &mut events)?;
                events.push(ev_log(format!(
                    "{spell_label} returns {object_label} from {} to battlefield.",
                    match from_zone {
                        Zone::Graveyard => "graveyard",
                        Zone::Exile => "exile",
                        Zone::Hand => "hand",
                        Zone::Library => "library",
                        Zone::Stack => "the stack",
                        Zone::Battlefield => "the battlefield",
                        Zone::Command => "the command zone",
                    }
                )));
                events.push(permanent_moved_event(
                    &self.state,
                    object_id,
                    owner,
                    rv1::permanent_moved::Destination::Battlefield,
                ));
                self.complete_parked_resolution(stack.item, stack.resume_effect_index, events)
            }
            BattlefieldEntryCompletion::Ninjutsu {
                owner,
                object_label,
                assignment,
            } => {
                let object_id = event.object_id;
                self.commit_battlefield_entry(event, None, &mut events)?;
                events.push(ev_log(format!(
                    "{} puts {object_label} onto the battlefield tapped and attacking.",
                    stack.item.ability_text.as_deref().unwrap_or("Ninjutsu")
                )));
                events.push(permanent_moved_event(
                    &self.state,
                    object_id,
                    owner,
                    rv1::permanent_moved::Destination::Battlefield,
                ));
                if let Some(assignment) = self.add_returned_attacker(object_id, assignment) {
                    events.push(rv1::RuledEvent {
                        ev: Some(rv1::ruled_event::Ev::AttackersAdded(rv1::AttackersAdded {
                            assignments: vec![assignment],
                        })),
                    });
                }
                self.complete_parked_resolution(stack.item, stack.resume_effect_index, events)
            }
            BattlefieldEntryCompletion::ObserverReturn {
                owner,
                object_label,
                attached_to,
                resume_original_stack,
            } => {
                let observer_stack = resume_original_stack.then_some(stack.clone());
                self.state
                    .pending_observer_return_batch
                    .get_or_insert_with(|| PendingObserverReturnBatch {
                        ready: Vec::new(),
                        remaining: VecDeque::new(),
                        resume_stack: observer_stack.clone(),
                    })
                    .ready
                    .push(ObserverReturnEntry {
                        event,
                        owner,
                        label: object_label,
                        attached_to,
                    });
                if self.drain_immediate_observer_actions(observer_stack, &mut events)? {
                    return Ok(finish_with_events(self, events));
                }
                if resume_original_stack {
                    self.complete_parked_resolution_with_previous(
                        stack.item,
                        stack.resume_effect_index,
                        stack.previous_result,
                        events,
                    )
                } else {
                    self.apply_sbas(&mut events)?;
                    if let Some(index) = self.state.player_idx(self.state.active_player_id()) {
                        self.state.priority_idx = index;
                    }
                    events.push(ev_priority_changed(self));
                    Ok(finish_with_events(self, events))
                }
            }
            BattlefieldEntryCompletion::LibrarySearch {
                owner,
                card_label,
                mut progress,
            } => {
                let object_id = event.object_id;
                let controller = event.destination_controller;
                self.commit_battlefield_entry(event, None, &mut events)?;
                events.push(ev_log(format!(
                    "P{controller} puts {card_label} onto the battlefield."
                )));
                events.push(permanent_moved_event(
                    &self.state,
                    object_id,
                    owner,
                    rv1::permanent_moved::Destination::Battlefield,
                ));
                if let Some(result_id) = progress.result_id.take() {
                    stack.item.search_results.insert(
                        result_id,
                        TriggerObjectRef {
                            object_id,
                            zone_change_generation: self
                                .state
                                .zone_change_generation
                                .get(&object_id)
                                .copied()
                                .unwrap_or(0),
                            controller_at_event: controller,
                        },
                    );
                }
                self.continue_library_search_battlefield_entries(stack, progress, events)
            }
            BattlefieldEntryCompletion::ManifestDread {
                owner,
                other_object_id,
                chosen_library_position,
            } => {
                let object_id = event.object_id;
                self.commit_battlefield_entry(event, None, &mut events)?;
                events.push(permanent_moved_event_with_library_position(
                    &self.state,
                    object_id,
                    owner,
                    rv1::permanent_moved::Destination::Battlefield,
                    chosen_library_position,
                ));
                if let Some(other) = other_object_id {
                    move_object_to_zone(
                        &mut self.state,
                        self.registry,
                        other,
                        Zone::Graveyard,
                        None,
                    )?;
                    events.push(permanent_moved_event_with_library_position(
                        &self.state,
                        other,
                        owner,
                        rv1::permanent_moved::Destination::Graveyard,
                        0,
                    ));
                }
                events.push(ev_log(format!("P{owner} manifests dread.")));
                let previous_result = EffectResult {
                    produced_objects: vec![TriggerObjectRef {
                        object_id,
                        zone_change_generation: self
                            .state
                            .zone_change_generation
                            .get(&object_id)
                            .copied()
                            .unwrap_or(0),
                        controller_at_event: owner,
                    }],
                    ..EffectResult::default()
                };
                self.complete_parked_resolution_with_previous(
                    stack.item,
                    stack.resume_effect_index,
                    previous_result,
                    events,
                )
            }
            BattlefieldEntryCompletion::TokenBatch(batch) => {
                let amass = batch.options.amass;
                // CR 608.2: once a parked token batch commits, publish every created object so the
                // resumed effect list can name "the token it created" via `PreviousEffectObject`.
                let created_ids = batch.result_object_ids.clone();
                let current = TokenBattlefieldEntry {
                    event,
                    created: batch.current_created.clone(),
                };
                if self.continue_token_entry_batch(stack.clone(), current, *batch, &mut events)? {
                    return Ok(finish_with_events(self, events));
                }
                if let Some(amass) = amass {
                    return self.finish_amass_after_token_entry(stack, amass, events);
                }
                let previous_result = EffectResult {
                    produced_objects: self.token_entry_object_refs(&created_ids),
                    ..EffectResult::default()
                };
                self.complete_parked_resolution_with_previous(
                    stack.item,
                    stack.resume_effect_index,
                    previous_result,
                    events,
                )
            }
            BattlefieldEntryCompletion::ZoneEntryBatch(mut batch) => {
                batch.ready.push(event);
                let Some(stack) = self.continue_zone_entry_batch(stack, *batch, &mut events)?
                else {
                    return Ok(finish_with_events(self, events));
                };
                self.complete_parked_resolution_with_previous(
                    stack.item,
                    stack.resume_effect_index,
                    stack.previous_result,
                    events,
                )
            }
            BattlefieldEntryCompletion::DevPlacement {
                target,
                ready,
                name,
                verb,
                deferred_events,
                announce_move,
            } => {
                self.complete_dev_battlefield_placement(
                    event,
                    target,
                    ready,
                    &name,
                    &verb,
                    deferred_events,
                    announce_move,
                    &mut events,
                )?;
                Ok(finish_with_events(self, events))
            }
        }
    }

    pub(super) fn finish_entry_copy_source_choice(
        &mut self,
        pending: PendingResolution,
        chosen: &[ObjectId],
    ) -> Result<RuledEventBatch, EngineError> {
        let (stack, entering_zone, entering_generation) = match &pending.continuation {
            ResolutionContinuation::EntryCopySource {
                stack,
                entering_zone,
                entering_generation,
            } => (stack.clone(), *entering_zone, *entering_generation),
            _ => return Err(EngineError::Illegal("copy-source continuation missing")),
        };
        let Some(pending_event) = self.state.pending_replacement_event.take() else {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("copy source choice is stale"));
        };
        let mut entry = match pending_event {
            PendingReplacementEvent::BattlefieldEntry(entry) => *entry,
            other => {
                self.state.pending_replacement_event = Some(other);
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal("copy source choice is stale"));
            }
        };
        if !self
            .state
            .objects
            .get(&entry.event.object_id)
            .is_some_and(|object| object.zone == entering_zone)
            || self
                .state
                .zone_change_generation
                .get(&entry.event.object_id)
                .copied()
                .unwrap_or(0)
                != entering_generation
        {
            self.state.pending_replacement_event =
                Some(PendingReplacementEvent::BattlefieldEntry(Box::new(entry)));
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("entering copy object is stale"));
        }
        let Some(effect_id) = entry.copy_source_effect.take() else {
            self.state.pending_replacement_event =
                Some(PendingReplacementEvent::BattlefieldEntry(Box::new(entry)));
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("copy source choice is stale"));
        };
        let Some((filter, artifact_in_addition)) =
            self.entry_copy_definition(&entry.event, &effect_id)
        else {
            entry.copy_source_effect = Some(effect_id);
            self.state.pending_replacement_event =
                Some(PendingReplacementEvent::BattlefieldEntry(Box::new(entry)));
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("copy replacement is stale"));
        };

        let mut events = Vec::new();
        if let Some(&source_id) = chosen.first() {
            let Some(source_generation) =
                entry
                    .copy_source_candidates
                    .iter()
                    .find_map(|(candidate, generation)| {
                        (*candidate == source_id).then_some(*generation)
                    })
            else {
                entry.copy_source_effect = Some(effect_id);
                self.state.pending_replacement_event =
                    Some(PendingReplacementEvent::BattlefieldEntry(Box::new(entry)));
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal("copy source is stale"));
            };
            let current_generation = self
                .state
                .zone_change_generation
                .get(&source_id)
                .copied()
                .unwrap_or(0);
            if source_id == entry.event.object_id
                || source_generation != current_generation
                || !object_matches_mass_filter(self, source_id, &filter)
            {
                entry.copy_source_effect = Some(effect_id);
                self.state.pending_replacement_event =
                    Some(PendingReplacementEvent::BattlefieldEntry(Box::new(entry)));
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal("copy source is stale"));
            }
            let Some(mut values) = self.copiable_values_for(source_id) else {
                entry.copy_source_effect = Some(effect_id);
                self.state.pending_replacement_event =
                    Some(PendingReplacementEvent::BattlefieldEntry(Box::new(entry)));
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal("copy source is stale"));
            };
            if artifact_in_addition {
                fn add_artifact(face: &mut CardFace) {
                    if !face.types.iter().any(|kind| kind == "Artifact") {
                        face.types.push("Artifact".into());
                    }
                    face.is_artifact = true;
                }
                add_artifact(&mut values.face);
                if let Some(faces) = &mut values.room_faces {
                    for face in faces {
                        add_artifact(face);
                    }
                }
            }
            let Some(entering) = self.state.objects.get(&entry.event.object_id) else {
                entry.copy_source_effect = Some(effect_id);
                self.state.pending_replacement_event =
                    Some(PendingReplacementEvent::BattlefieldEntry(Box::new(entry)));
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal("entering copy object is stale"));
            };
            let previous_copy = Self::entry_copy_rollback_candidate(&entry.event);
            if previous_copy.is_some_and(|candidate| {
                candidate.entering_copy_revision.saturating_add(1) != entering.copy_revision
            }) {
                entry.copy_source_effect = Some(effect_id);
                self.state.pending_replacement_event =
                    Some(PendingReplacementEvent::BattlefieldEntry(Box::new(entry)));
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal("provisional copy chain became stale"));
            }
            let candidate = PendingCopyCandidate {
                source_id,
                source_generation,
                source_filter: filter,
                entering_zone_generation: self
                    .state
                    .zone_change_generation
                    .get(&entry.event.object_id)
                    .copied()
                    .unwrap_or(0),
                entering_copy_revision: entering.copy_revision,
                rollback_copy_revision: previous_copy.map_or(entering.copy_revision, |candidate| {
                    candidate.rollback_copy_revision
                }),
                entering_copiable_values: previous_copy.map_or_else(
                    || entering.copiable_values.clone(),
                    |candidate| candidate.entering_copiable_values.clone(),
                ),
                entering_must_attack_if_able: previous_copy
                    .map_or(entering.must_attack_if_able, |candidate| {
                        candidate.entering_must_attack_if_able
                    }),
                entering_must_block_if_able: previous_copy
                    .map_or(entering.must_block_if_able, |candidate| {
                        candidate.entering_must_block_if_able
                    }),
                values,
            };
            self.install_entry_copy_candidate(entry.event.object_id, &candidate)?;
            entry.event.pending_copy_candidate = Some(candidate);
            if let BattlefieldEntryCompletion::PermanentSpell { attached_to }
            | BattlefieldEntryCompletion::ObserverReturn { attached_to, .. } =
                &mut entry.completion
            {
                // The target chosen to cast an Aura spell applies only if its printed Aura
                // identity remains. A copied source supplies a different Aura restriction (or
                // may cease to be an Aura entirely), so the original target cannot be reused.
                *attached_to = None;
            }
            entry.event.attached_to = None;
            entry.event.accepted_aura_recipient = None;
        }
        entry.event.applied_effects.push(effect_id);
        let event = match self.advance_or_park_battlefield_entry(
            stack.item.clone(),
            entry.event,
            entry.completion.clone(),
            &mut events,
        ) {
            BattlefieldEntryProgress::Parked => {
                self.transfer_entry_choice_resume(&stack);
                return Ok(finish_with_events(self, events));
            }
            BattlefieldEntryProgress::Skipped(event) => {
                return self.finish_entry_copy_without_recipient(
                    stack,
                    *event,
                    entry.completion,
                    events,
                );
            }
            BattlefieldEntryProgress::Ready(event) => *event,
        };
        self.complete_pending_battlefield_entry(pending, event, entry.completion, events)
    }

    fn install_entry_copy_candidate(
        &mut self,
        object_id: ObjectId,
        candidate: &PendingCopyCandidate,
    ) -> Result<(), EngineError> {
        let current_source_generation = self
            .state
            .zone_change_generation
            .get(&candidate.source_id)
            .copied()
            .unwrap_or(0);
        let current_object_generation = self
            .state
            .zone_change_generation
            .get(&object_id)
            .copied()
            .unwrap_or(0);
        let Some(object) = self.state.objects.get(&object_id) else {
            return Err(EngineError::Illegal("entering copy object is stale"));
        };
        if current_source_generation != candidate.source_generation
            || !object_matches_mass_filter(self, candidate.source_id, &candidate.source_filter)
            || current_object_generation != candidate.entering_zone_generation
            || object.copy_revision != candidate.entering_copy_revision
        {
            return Err(EngineError::Illegal("copy source choice is stale"));
        }
        let Some(object) = self.state.objects.get_mut(&object_id) else {
            return Err(EngineError::Illegal("entering copy object is stale"));
        };
        object.must_attack_if_able = candidate.values.face.must_attack_if_able;
        object.must_block_if_able = candidate.values.face.must_block_if_able;
        object.copiable_values = Some(candidate.values.clone());
        object.copy_revision = object.copy_revision.saturating_add(1);
        Ok(())
    }

    fn restore_entry_copy_candidate(
        &mut self,
        object_id: ObjectId,
        candidate: &PendingCopyCandidate,
    ) -> Result<(), EngineError> {
        let generation = self
            .state
            .zone_change_generation
            .get(&object_id)
            .copied()
            .unwrap_or(0);
        let Some(object) = self.state.objects.get_mut(&object_id) else {
            return Err(EngineError::Illegal("entering copy object is stale"));
        };
        if generation != candidate.entering_zone_generation
            || object.copy_revision != candidate.entering_copy_revision.saturating_add(1)
        {
            return Err(EngineError::Illegal("provisional copy state is stale"));
        }
        object.copiable_values = candidate.entering_copiable_values.clone();
        object.copy_revision = candidate.rollback_copy_revision;
        object.must_attack_if_able = candidate.entering_must_attack_if_able;
        object.must_block_if_able = candidate.entering_must_block_if_able;
        Ok(())
    }

    pub(super) fn finish_entry_aura_recipient_choice(
        &mut self,
        pending: PendingResolution,
        chosen: ObjectId,
    ) -> Result<RuledEventBatch, EngineError> {
        let stack = match &pending.continuation {
            ResolutionContinuation::EntryAuraRecipient { stack } => stack.clone(),
            _ => return Err(EngineError::Illegal("Aura-copy continuation missing")),
        };
        let Some(pending_event) = self.state.pending_replacement_event.take() else {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("Aura-copy recipient choice is stale"));
        };
        let mut entry = match pending_event {
            PendingReplacementEvent::BattlefieldEntry(entry) => *entry,
            other => {
                self.state.pending_replacement_event = Some(other);
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal("Aura-copy recipient choice is stale"));
            }
        };
        let Some(aura_choice) = entry.event.pending_aura_recipient.take() else {
            self.state.pending_replacement_event =
                Some(PendingReplacementEvent::BattlefieldEntry(Box::new(entry)));
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("Aura-copy recipient choice is stale"));
        };
        let filter = aura_choice.filter.clone();
        if !filter.is_player()
            && !aura_choice
                .recipient_generations
                .iter()
                .any(|(oid, generation)| {
                    *oid == chosen
                        && self
                            .state
                            .zone_change_generation
                            .get(oid)
                            .copied()
                            .unwrap_or(0)
                            == *generation
                })
        {
            entry.event.pending_aura_recipient = Some(aura_choice);
            self.state.pending_replacement_event =
                Some(PendingReplacementEvent::BattlefieldEntry(Box::new(entry)));
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("Aura recipient choice is stale"));
        }
        let recipient = if filter.is_player() {
            AttachmentRecipient::Player(chosen as PlayerId)
        } else {
            AttachmentRecipient::Object(chosen)
        };
        let entering_generation = self
            .state
            .zone_change_generation
            .get(&entry.event.object_id)
            .copied()
            .unwrap_or(0);
        let entering_revision = self
            .state
            .objects
            .get(&entry.event.object_id)
            .map(|object| object.copy_revision);
        let source_is_valid = aura_choice.copy_candidate.as_ref().is_none_or(|candidate| {
            self.state
                .zone_change_generation
                .get(&candidate.source_id)
                .copied()
                .unwrap_or(0)
                == candidate.source_generation
                && object_matches_mass_filter(self, candidate.source_id, &candidate.source_filter)
        });
        if entering_generation != aura_choice.entering_zone_generation
            || entering_revision != Some(aura_choice.entering_copy_revision)
            || !source_is_valid
            || !super::targeting::attachment_filter_legal(
                self,
                &filter,
                recipient,
                entry.event.object_id,
                entry.event.destination_controller,
            )
        {
            entry.event.pending_aura_recipient = Some(aura_choice);
            self.state.pending_replacement_event =
                Some(PendingReplacementEvent::BattlefieldEntry(Box::new(entry)));
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("Aura recipient is no longer legal"));
        }
        if let AttachmentRecipient::Object(object_id) = recipient {
            let generation = self
                .state
                .zone_change_generation
                .get(&object_id)
                .copied()
                .unwrap_or(0);
            if aura_choice
                .recipient_generations
                .iter()
                .find(|(candidate_id, _)| *candidate_id == object_id)
                .is_none_or(|(_, expected)| *expected != generation)
            {
                entry.event.pending_aura_recipient = Some(aura_choice);
                self.state.pending_replacement_event =
                    Some(PendingReplacementEvent::BattlefieldEntry(Box::new(entry)));
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal("Aura recipient choice is stale"));
            }
        }
        entry.event.attached_to = Some(recipient);
        entry.event.accepted_aura_recipient = Some(crate::state::AcceptedAuraEntryRecipient {
            filter: aura_choice.filter,
            entering_zone_generation: aura_choice.entering_zone_generation,
            entering_copy_revision: aura_choice.entering_copy_revision,
            recipient_generation: match recipient {
                AttachmentRecipient::Object(oid) => Some(
                    self.state
                        .zone_change_generation
                        .get(&oid)
                        .copied()
                        .unwrap_or(0),
                ),
                AttachmentRecipient::Player(_) => None,
            },
            copy_candidate: aura_choice.copy_candidate,
        });
        let mut events = Vec::new();
        let event = match self.advance_or_park_battlefield_entry(
            stack.item.clone(),
            entry.event,
            entry.completion.clone(),
            &mut events,
        ) {
            BattlefieldEntryProgress::Parked => {
                self.transfer_entry_choice_resume(&stack);
                return Ok(finish_with_events(self, events));
            }
            BattlefieldEntryProgress::Skipped(event) => {
                return self.finish_entry_copy_without_recipient(
                    stack,
                    *event,
                    entry.completion,
                    events,
                );
            }
            BattlefieldEntryProgress::Ready(event) => *event,
        };
        self.complete_pending_battlefield_entry(pending, event, entry.completion, events)
    }

    pub(super) fn finish_battlefield_entry_replacement_choice(
        &mut self,
        pending: PendingResolution,
        application_id: u32,
    ) -> Result<RuledEventBatch, EngineError> {
        let stack = match &pending.continuation {
            ResolutionContinuation::EntryReplacement { stack } => stack.clone(),
            _ => {
                return Err(EngineError::Illegal(
                    "entry-replacement continuation missing",
                ))
            }
        };
        let Some(pending_event) = self.state.pending_replacement_event.take() else {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "battlefield-entry replacement choice is stale",
            ));
        };
        let mut entry = match pending_event {
            PendingReplacementEvent::BattlefieldEntry(entry) => *entry,
            other => {
                self.state.pending_replacement_event = Some(other);
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal(
                    "battlefield-entry replacement choice is stale",
                ));
            }
        };
        let Some(application) = entry
            .applications
            .iter()
            .find(|application| application.application_id == application_id)
            .cloned()
        else {
            self.state.pending_replacement_event =
                Some(PendingReplacementEvent::BattlefieldEntry(Box::new(entry)));
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("replacement application is stale"));
        };

        let mut events = Vec::new();
        if let Some(filter) = self.entry_copy_filter(&entry.event, &application.effect_id) {
            let sources = self.copy_source_candidates(&entry.event, &filter);
            if !sources.is_empty() {
                self.park_copy_source_choice(
                    stack.item.clone(),
                    entry.event,
                    entry.completion,
                    application.effect_id,
                    sources,
                    &mut events,
                );
                self.transfer_entry_choice_resume(&stack);
                return Ok(finish_with_events(self, events));
            }
            entry.event.applied_effects.push(application.effect_id);
        } else if let Some(final_chapter) =
            self.entry_read_ahead_final_chapter(&entry.event, &application.effect_id)
        {
            self.park_read_ahead_choice(
                stack.item.clone(),
                entry.event,
                entry.completion,
                application.effect_id,
                final_chapter,
                &mut events,
            );
            self.transfer_entry_choice_resume(&stack);
            return Ok(finish_with_events(self, events));
        } else if self.entry_needs_basic_land_type_choice(&entry.event, &application.effect_id) {
            self.park_basic_land_type_choice(
                stack.item.clone(),
                entry.event,
                entry.completion,
                application.effect_id,
                &mut events,
            );
            self.transfer_entry_choice_resume(&stack);
            return Ok(finish_with_events(self, events));
        } else if self
            .entry_opponent_key(&entry.event, &application.effect_id)
            .is_some()
            && !self.entry_opponent_players(&entry.event).is_empty()
        {
            self.park_entry_opponent_choice(
                stack.item.clone(),
                entry.event,
                entry.completion,
                application.effect_id,
                &mut events,
            );
            self.transfer_entry_choice_resume(&stack);
            return Ok(finish_with_events(self, events));
        } else if let Some(cost) = self.entry_unless_cost(&entry.event, &application.effect_id) {
            self.park_entry_cost_choice(
                stack.item.clone(),
                entry.event,
                entry.completion,
                application.effect_id,
                cost,
                &mut events,
            );
            self.transfer_entry_choice_resume(&stack);
            return Ok(finish_with_events(self, events));
        } else {
            self.apply_entry_replacement(&mut entry.event, application.effect_id);
        }
        let event = match self.advance_or_park_battlefield_entry(
            stack.item.clone(),
            entry.event,
            entry.completion.clone(),
            &mut events,
        ) {
            BattlefieldEntryProgress::Parked => {
                self.transfer_entry_choice_resume(&stack);
                return Ok(finish_with_events(self, events));
            }
            BattlefieldEntryProgress::Skipped(event) => {
                return self.finish_entry_copy_without_recipient(
                    stack,
                    *event,
                    entry.completion,
                    events,
                );
            }
            BattlefieldEntryProgress::Ready(event) => *event,
        };

        self.complete_pending_battlefield_entry(pending, event, entry.completion, events)
    }

    pub(super) fn finish_entry_opponent_choice(
        &mut self,
        pending: PendingResolution,
        answer: &rv1::SubmitResolutionChoice,
        decision: rv1::ResolutionChoiceDecision,
    ) -> Result<RuledEventBatch, EngineError> {
        let (stack, effect_id, key, entering_zone, entering_generation, players) =
            match &pending.continuation {
                ResolutionContinuation::EntryChooseOpponent {
                    stack,
                    effect_id,
                    key,
                    entering_zone,
                    entering_generation,
                    players,
                } => (
                    stack.clone(),
                    effect_id.clone(),
                    key.clone(),
                    *entering_zone,
                    *entering_generation,
                    players.clone(),
                ),
                _ => return Err(EngineError::Illegal("opponent continuation missing")),
            };
        let entry = match &self.state.pending_replacement_event {
            Some(PendingReplacementEvent::BattlefieldEntry(entry)) => Some((**entry).clone()),
            _ => None,
        };
        let selected = players.get(answer.selected_branch_index as usize).copied();
        let valid = decision == rv1::ResolutionChoiceDecision::SelectBranch
            && answer.chosen_object_ids.is_empty()
            && answer.chosen_player_ids.is_empty()
            && answer.payment.is_none()
            && answer.restricted_mana.is_empty()
            && answer.cast_spell.is_none()
            && answer.spell_cast_announcement.is_none()
            && answer.chosen_combat_defender.is_none()
            && entry.as_ref().is_some_and(|entry| {
                self.state
                    .objects
                    .get(&entry.event.object_id)
                    .is_some_and(|object| object.zone == entering_zone)
                    && self
                        .state
                        .zone_change_generation
                        .get(&entry.event.object_id)
                        .copied()
                        .unwrap_or(0)
                        == entering_generation
                    && self.entry_opponent_key(&entry.event, &effect_id).as_ref() == Some(&key)
                    && selected.is_some_and(|player| {
                        self.entry_opponent_players(&entry.event).contains(&player)
                    })
            });
        if !valid {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "opponent choice is malformed or stale",
            ));
        }
        let mut entry = entry.unwrap();
        self.state.pending_replacement_event = None;
        entry.event.chosen_opponents.push(ChosenOpponentRecord {
            key,
            player: selected.unwrap(),
        });
        self.apply_entry_replacement(&mut entry.event, effect_id);
        let mut events = Vec::new();
        match self.advance_or_park_battlefield_entry(
            stack.item.clone(),
            entry.event,
            entry.completion.clone(),
            &mut events,
        ) {
            BattlefieldEntryProgress::Parked => {
                self.transfer_entry_choice_resume(&stack);
                Ok(finish_with_events(self, events))
            }
            BattlefieldEntryProgress::Skipped(event) => {
                self.finish_entry_copy_without_recipient(stack, *event, entry.completion, events)
            }
            BattlefieldEntryProgress::Ready(event) => {
                self.complete_pending_battlefield_entry(pending, *event, entry.completion, events)
            }
        }
    }

    pub(super) fn finish_basic_land_type_choice(
        &mut self,
        pending: PendingResolution,
        answer: &rv1::SubmitResolutionChoice,
        decision: rv1::ResolutionChoiceDecision,
    ) -> Result<RuledEventBatch, EngineError> {
        let (stack, effect_id) = match &pending.continuation {
            ResolutionContinuation::EntryBasicLandType { stack, effect_id } => {
                (stack.clone(), effect_id.clone())
            }
            _ => return Err(EngineError::Illegal("basic-land-type continuation missing")),
        };
        let Some(pending_event) = self.state.pending_replacement_event.take() else {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("basic land type choice is stale"));
        };
        let mut entry = match pending_event {
            PendingReplacementEvent::BattlefieldEntry(entry) => *entry,
            other => {
                self.state.pending_replacement_event = Some(other);
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal("basic land type choice is stale"));
            }
        };
        let restore = |engine: &mut Self,
                       pending: PendingResolution,
                       entry: PendingBattlefieldEntry,
                       message| {
            engine.state.pending_replacement_event =
                Some(PendingReplacementEvent::BattlefieldEntry(Box::new(entry)));
            engine.state.pending_resolution = Some(pending);
            Err(EngineError::Illegal(message))
        };
        if decision != rv1::ResolutionChoiceDecision::SelectBranch
            || !answer.chosen_object_ids.is_empty()
            || answer.payment.is_some()
            || !answer.restricted_mana.is_empty()
            || answer.cast_spell.is_some()
            || answer.spell_cast_announcement.is_some()
            || answer.chosen_combat_defender.is_some()
        {
            return restore(
                self,
                pending,
                entry,
                "basic land type requires exactly one branch",
            );
        }
        if !self.entry_needs_basic_land_type_choice(&entry.event, &effect_id) {
            return restore(self, pending, entry, "basic land type choice is stale");
        }
        let Some(chosen) = BasicLandType::ALL
            .get(answer.selected_branch_index as usize)
            .copied()
        else {
            return restore(self, pending, entry, "unknown basic land type branch");
        };
        entry.event.chosen_basic_land_type = Some(chosen);
        let Some(cost) = self.entry_unless_cost(&entry.event, &effect_id) else {
            return restore(self, pending, entry, "basic land type choice is stale");
        };
        let mut events = vec![ev_log(format!(
            "P{} chooses {} for Multiversal Passage.",
            entry.event.deciding_player,
            chosen.as_str()
        ))];
        self.park_entry_cost_choice(
            stack.item.clone(),
            entry.event,
            entry.completion,
            effect_id,
            cost,
            &mut events,
        );
        self.transfer_entry_choice_resume(&stack);
        Ok(finish_with_events(self, events))
    }

    pub(super) fn finish_entry_reveal_choice(
        &mut self,
        pending: PendingResolution,
        answer: &rv1::SubmitResolutionChoice,
        decision: rv1::ResolutionChoiceDecision,
    ) -> Result<RuledEventBatch, EngineError> {
        let (stack, effect_id, entering_zone, entering_generation, candidates) =
            match &pending.continuation {
                ResolutionContinuation::EntryReveal {
                    stack,
                    effect_id,
                    entering_zone,
                    entering_generation,
                    candidate_generations,
                } => (
                    stack.clone(),
                    effect_id.clone(),
                    *entering_zone,
                    *entering_generation,
                    candidate_generations.clone(),
                ),
                _ => return Err(EngineError::Illegal("entry reveal continuation missing")),
            };
        let Some(pending_event) = self.state.pending_replacement_event.take() else {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("entry reveal is stale"));
        };
        let mut entry = match pending_event {
            PendingReplacementEvent::BattlefieldEntry(entry) => *entry,
            other => {
                self.state.pending_replacement_event = Some(other);
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal("entry reveal is stale"));
            }
        };
        let restore = |engine: &mut Self,
                       pending: PendingResolution,
                       entry: PendingBattlefieldEntry,
                       message| {
            engine.state.pending_replacement_event =
                Some(PendingReplacementEvent::BattlefieldEntry(Box::new(entry)));
            engine.state.pending_resolution = Some(pending);
            Err(EngineError::Illegal(message))
        };
        if decision != rv1::ResolutionChoiceDecision::Unspecified
            || answer.chosen_object_ids.len() > 1
            || answer.selected_branch_index != 0
            || answer.payment.is_some()
            || !answer.restricted_mana.is_empty()
            || answer.cast_spell.is_some()
            || answer.spell_cast_announcement.is_some()
            || answer.chosen_combat_defender.is_some()
            || !answer.chosen_player_ids.is_empty()
        {
            return restore(
                self,
                pending,
                entry,
                "entry reveal requires zero or one hand card only",
            );
        }
        let payer = entry.event.destination_controller;
        let source_current = self
            .state
            .objects
            .get(&entry.event.object_id)
            .is_some_and(|object| object.zone == entering_zone)
            && self
                .state
                .zone_change_generation
                .get(&entry.event.object_id)
                .copied()
                .unwrap_or(0)
                == entering_generation;
        let Some(EntryCost::RevealFromHand { filter }) =
            self.entry_unless_cost(&entry.event, &effect_id)
        else {
            return restore(self, pending, entry, "entry reveal effect is stale");
        };
        if !source_current
            || pending.deciding_player != payer
            || entry.event.applied_effects.contains(&effect_id)
        {
            return restore(
                self,
                pending,
                entry,
                "entry reveal source or chooser is stale",
            );
        }
        let mut events = Vec::new();
        if let Some(&chosen) = answer.chosen_object_ids.first() {
            let legal = self
                .state
                .player_idx(payer)
                .is_some_and(|index| self.state.players[index].hand.contains(&chosen))
                && self
                    .state
                    .objects
                    .get(&chosen)
                    .is_some_and(|object| object.owner == payer && object.zone == Zone::Hand)
                && candidates.iter().any(|(oid, generation)| {
                    *oid == chosen
                        && self
                            .state
                            .zone_change_generation
                            .get(oid)
                            .copied()
                            .unwrap_or(0)
                            == *generation
                })
                && chosen != entry.event.object_id
                && zone_card_matches_filter(&self.state, self.registry, chosen, Some(&filter));
            if !legal {
                return restore(
                    self,
                    pending,
                    entry,
                    "revealed hand card is stale or illegal",
                );
            }
            let name = self
                .battlefield_entry_face(&entry.event)
                .map_or_else(|| "entry".to_owned(), |face| face.name.clone());
            let reveal_id = format!(
                "entry:{}:{entering_generation}:{effect_id:?}",
                entry.event.object_id
            );
            for mut event in super::reveals::reveal_cards(
                &self.state,
                self.registry,
                &[chosen],
                entry.event.object_id,
                &name,
            ) {
                if let Some(rv1::ruled_event::Ev::CardsRevealed(reveal)) = &mut event.ev {
                    reveal.reveal_id = reveal_id.clone();
                    entry.event.entry_reveal_receipts.push(reveal.clone());
                }
                events.push(event);
            }
            entry.event.applied_effects.push(effect_id);
        } else {
            self.apply_entry_replacement(&mut entry.event, effect_id);
        }
        let event = match self.advance_or_park_battlefield_entry(
            stack.item.clone(),
            entry.event,
            entry.completion.clone(),
            &mut events,
        ) {
            BattlefieldEntryProgress::Parked => {
                self.transfer_entry_choice_resume(&stack);
                return Ok(finish_with_events(self, events));
            }
            BattlefieldEntryProgress::Skipped(event) => {
                return self.finish_entry_copy_without_recipient(
                    stack,
                    *event,
                    entry.completion,
                    events,
                );
            }
            BattlefieldEntryProgress::Ready(event) => *event,
        };
        self.complete_pending_battlefield_entry(pending, event, entry.completion, events)
    }

    pub(super) fn finish_entry_cost_choice(
        &mut self,
        pending: PendingResolution,
        answer: &rv1::SubmitResolutionChoice,
        decision: rv1::ResolutionChoiceDecision,
    ) -> Result<RuledEventBatch, EngineError> {
        let (stack, effect_id) = match &pending.continuation {
            ResolutionContinuation::EntryCost { stack, effect_id } => {
                (stack.clone(), effect_id.clone())
            }
            _ => return Err(EngineError::Illegal("entry-cost continuation missing")),
        };
        let Some(pending_event) = self.state.pending_replacement_event.take() else {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("entry cost choice is stale"));
        };
        let mut entry = match pending_event {
            PendingReplacementEvent::BattlefieldEntry(entry) => *entry,
            other => {
                self.state.pending_replacement_event = Some(other);
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal("entry cost choice is stale"));
            }
        };
        let restore = |engine: &mut Self,
                       pending: PendingResolution,
                       entry: PendingBattlefieldEntry,
                       message| {
            engine.state.pending_replacement_event =
                Some(PendingReplacementEvent::BattlefieldEntry(Box::new(entry)));
            engine.state.pending_resolution = Some(pending);
            Err(EngineError::Illegal(message))
        };
        if !answer.chosen_object_ids.is_empty()
            || answer.selected_branch_index != 0
            || answer.payment.is_some()
            || !answer.restricted_mana.is_empty()
            || answer.cast_spell.is_some()
            || answer.spell_cast_announcement.is_some()
            || answer.chosen_combat_defender.is_some()
            || !matches!(
                decision,
                rv1::ResolutionChoiceDecision::SelectBranch
                    | rv1::ResolutionChoiceDecision::Decline
            )
        {
            return restore(
                self,
                pending,
                entry,
                "entry cost requires its payment branch or decline",
            );
        }
        let Some(cost @ EntryCost::PayLife { .. }) =
            self.entry_unless_cost(&entry.event, &effect_id)
        else {
            return restore(self, pending, entry, "entry cost choice is stale");
        };
        let payer = entry.event.destination_controller;
        if pending.deciding_player != payer {
            return restore(self, pending, entry, "entry cost payer is stale");
        }

        let mut events = Vec::new();
        match (decision, cost) {
            (rv1::ResolutionChoiceDecision::SelectBranch, EntryCost::PayLife { amount }) => {
                let Some(player_index) = self.state.player_idx(payer) else {
                    return restore(self, pending, entry, "entry cost payer is stale");
                };
                if self.state.players[player_index].life < amount as i32 {
                    return restore(self, pending, entry, "entry life payment is unaffordable");
                }
                super::history::commit_life_change(&mut self.state, player_index, -(amount as i32));
                events.push(rv1::RuledEvent {
                    ev: Some(rv1::ruled_event::Ev::LifeChanged(rv1::LifeChanged {
                        player_id: payer,
                        new_total: self.state.players[player_index].life,
                        delta: -(amount as i32),
                    })),
                });
                events.push(ev_log(format!("P{payer} pays {amount} life.")));
                entry.event.applied_effects.push(effect_id);
            }
            (rv1::ResolutionChoiceDecision::Decline, _) => {
                self.apply_entry_replacement(&mut entry.event, effect_id);
            }
            _ => unreachable!("entry cost decision validated above"),
        }

        let event = match self.advance_or_park_battlefield_entry(
            stack.item.clone(),
            entry.event,
            entry.completion.clone(),
            &mut events,
        ) {
            BattlefieldEntryProgress::Parked => {
                self.transfer_entry_choice_resume(&stack);
                return Ok(finish_with_events(self, events));
            }
            BattlefieldEntryProgress::Skipped(event) => {
                return self.finish_entry_copy_without_recipient(
                    stack,
                    *event,
                    entry.completion,
                    events,
                );
            }
            BattlefieldEntryProgress::Ready(event) => *event,
        };
        self.complete_pending_battlefield_entry(pending, event, entry.completion, events)
    }

    pub(super) fn finish_saga_read_ahead_choice(
        &mut self,
        pending: PendingResolution,
        answer: &rv1::SubmitResolutionChoice,
        decision: rv1::ResolutionChoiceDecision,
    ) -> Result<RuledEventBatch, EngineError> {
        let (stack, effect_id) = match &pending.continuation {
            ResolutionContinuation::SagaReadAhead { stack, effect_id } => {
                (stack.clone(), effect_id.clone())
            }
            _ => return Err(EngineError::Illegal("read-ahead continuation missing")),
        };
        let Some(pending_event) = self.state.pending_replacement_event.take() else {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("read-ahead choice is stale"));
        };
        let mut entry = match pending_event {
            PendingReplacementEvent::BattlefieldEntry(entry) => *entry,
            other => {
                self.state.pending_replacement_event = Some(other);
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal("read-ahead choice is stale"));
            }
        };
        let restore = |engine: &mut Self,
                       pending: PendingResolution,
                       entry: PendingBattlefieldEntry,
                       message| {
            engine.state.pending_replacement_event =
                Some(PendingReplacementEvent::BattlefieldEntry(Box::new(entry)));
            engine.state.pending_resolution = Some(pending);
            Err(EngineError::Illegal(message))
        };
        if decision != rv1::ResolutionChoiceDecision::SelectBranch
            || !answer.chosen_object_ids.is_empty()
            || answer.payment.is_some()
            || !answer.restricted_mana.is_empty()
            || answer.cast_spell.is_some()
            || answer.spell_cast_announcement.is_some()
            || answer.chosen_combat_defender.is_some()
        {
            return restore(
                self,
                pending,
                entry,
                "read ahead requires one chapter branch",
            );
        }
        let Some(final_chapter) = self.entry_read_ahead_final_chapter(&entry.event, &effect_id)
        else {
            return restore(self, pending, entry, "read-ahead choice is stale");
        };
        let chapter = answer.selected_branch_index.saturating_add(1);
        if chapter == 0 || chapter > final_chapter {
            return restore(self, pending, entry, "read-ahead chapter is out of range");
        }
        accumulate_entry_counters(&mut entry.event.entry_counters, CounterKind::Lore, chapter);
        entry.event.applied_effects.push(effect_id);

        let mut events = vec![ev_log(format!(
            "P{} chooses chapter {} for read ahead.",
            pending.deciding_player,
            saga_chapter_label(chapter)
        ))];
        let event = match self.advance_or_park_battlefield_entry(
            stack.item.clone(),
            entry.event,
            entry.completion.clone(),
            &mut events,
        ) {
            BattlefieldEntryProgress::Parked => {
                self.transfer_entry_choice_resume(&stack);
                return Ok(finish_with_events(self, events));
            }
            BattlefieldEntryProgress::Skipped(event) => {
                return self.finish_entry_copy_without_recipient(
                    stack,
                    *event,
                    entry.completion,
                    events,
                );
            }
            BattlefieldEntryProgress::Ready(event) => *event,
        };
        self.complete_pending_battlefield_entry(pending, event, entry.completion, events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deploy_private_skip_fixture_bottoms_selected_card_that_never_entered() {
        // Characterize the shared copy/Aura skip seam, not a natural printed planeswalker copy.
        let mut engine = GameEngine::new(90_220, &[0, 1], 20, None, true).unwrap();
        engine.state.opening = None;
        engine.state.turn_step = TurnStep::Main1;
        engine.state.priority_idx = 0;
        let spell = engine.state.players[0].hand[0];
        engine.state.objects.get_mut(&spell).unwrap().card_id = "deploy_the_gatewatch".into();
        let mut looked = Vec::new();
        for card in ["jace_beleren", "chandra,_novice_pyromancer", "forest"] {
            let oid = engine.state.players[0].hand.pop().unwrap();
            let object = engine.state.objects.get_mut(&oid).unwrap();
            object.card_id = card.into();
            object.zone = Zone::Library;
            looked.push(oid);
        }
        for &oid in looked.iter().rev() {
            engine.state.players[0].library.push_front(oid);
        }
        let original = engine.state.players[0]
            .library
            .iter()
            .copied()
            .collect::<Vec<_>>();
        let window = original.iter().take(7).copied().collect::<Vec<_>>();
        for _ in 0..2 {
            let oid = engine.state.players[0].hand.pop().unwrap();
            let object = engine.state.objects.get_mut(&oid).unwrap();
            object.card_id = "orb_of_dreams".into();
            object.zone = Zone::Battlefield;
            engine.state.players[0].battlefield.push(oid);
        }
        engine.state.players[0].mana_pool.white = 2;
        engine.state.players[0].mana_pool.colorless = 4;
        engine
            .cast_spell(
                0,
                &rv1::CastSpell {
                    cast_method: rv1::CastMethod::Normal as i32,
                    source: Some(rv1::CastSource {
                        location: Some(rv1::cast_source::Location::HandIndex(0)),
                        expected_zone_change_generation: None,
                    }),
                    ..Default::default()
                },
            )
            .unwrap();
        engine.resolve_top_of_stack(&mut Vec::new()).unwrap();
        engine
            .submit_resolution_choice(
                0,
                &rv1::SubmitResolutionChoice {
                    chosen_object_ids: vec![looked[0], looked[1]],
                    ..Default::default()
                },
            )
            .unwrap();
        let pending = engine.state.pending_resolution.take().unwrap();
        let stack = pending.continuation.stack().unwrap().clone();
        let Some(PendingReplacementEvent::BattlefieldEntry(entry)) =
            engine.state.pending_replacement_event.take()
        else {
            panic!("replacement parks first proposed entry");
        };
        assert_eq!(entry.event.object_id, looked[0]);
        engine
            .finish_entry_copy_without_recipient(stack, entry.event, entry.completion, Vec::new())
            .unwrap();
        for _ in 0..10 {
            let Some(pending) = engine.state.pending_resolution.as_ref() else {
                break;
            };
            let chosen = if pending.presentation.choice_kind == rv1::ChoiceKind::ReplacementEffect {
                vec![pending.presentation.candidates[0]]
            } else {
                pending.presentation.candidates.clone()
            };
            engine
                .submit_resolution_choice(
                    0,
                    &rv1::SubmitResolutionChoice {
                        chosen_object_ids: chosen,
                        ..Default::default()
                    },
                )
                .unwrap();
        }
        assert!(engine.state.pending_resolution.is_none());
        assert_eq!(engine.state.objects[&looked[0]].zone, Zone::Library);
        assert_eq!(
            engine
                .state
                .zone_change_generation
                .get(&looked[0])
                .copied()
                .unwrap_or(0),
            0
        );
        assert_eq!(engine.state.objects[&looked[1]].zone, Zone::Battlefield);
        let library = engine.state.players[0]
            .library
            .iter()
            .copied()
            .collect::<Vec<_>>();
        assert_eq!(
            &library[..original.len() - window.len()],
            &original[window.len()..]
        );
        let mut rest = library[original.len() - window.len()..].to_vec();
        rest.sort_unstable();
        let mut expected = window
            .into_iter()
            .filter(|oid| *oid != looked[1])
            .collect::<Vec<_>>();
        expected.sort_unstable();
        assert_eq!(
            rest, expected,
            "skipped selected incarnation is part of the random bottom cohort"
        );
    }

    #[test]
    fn issue_153_tatterkite_enters_without_proposed_counters() {
        let mut engine = GameEngine::new(153_008, &[0, 1], 20, None, true).unwrap();
        let object_id = engine.state.players[0].hand[0];
        engine.state.objects.get_mut(&object_id).unwrap().card_id = "tatterkite".into();
        let event = BattlefieldEntryEvent {
            entry_reveal_receipts: Vec::new(),
            mana_colors_spent_to_cast: Default::default(),
            prepared: false,
            object_id,
            deciding_player: 0,
            destination_controller: 0,
            battle_protector: None,
            face_index: 0,
            unlock_room_door: None,
            chosen_x: 0,
            cast_by: None,
            cast_cost_receipts: Vec::new(),
            player_life_snapshot: engine.player_life_snapshot(),
            tapped: false,
            set_types: None,
            chosen_basic_land_type: None,
            chosen_opponents: Vec::new(),
            entry_counters: BTreeMap::from([
                (CounterKind::PlusOnePlusOne, 3),
                (CounterKind::Stun, 2),
            ]),
            entry_modifiers: Vec::new(),
            attached_to: None,
            pending_copy_candidate: None,
            pending_aura_recipient: None,
            accepted_aura_recipient: None,
            applied_effects: Vec::new(),
        };
        engine
            .commit_battlefield_entry(event, None, &mut Vec::new())
            .unwrap();
        assert_eq!(engine.state.objects[&object_id].zone, Zone::Battlefield);
        assert!(engine.state.objects[&object_id].counters.is_empty());
    }

    #[test]
    fn entry_counter_accumulation_saturates_and_omits_zero_entries() {
        let mut counters = BTreeMap::new();
        accumulate_entry_counters(&mut counters, CounterKind::PlusOnePlusOne, 0);
        assert!(counters.is_empty());

        accumulate_entry_counters(&mut counters, CounterKind::PlusOnePlusOne, u32::MAX - 1);
        accumulate_entry_counters(&mut counters, CounterKind::PlusOnePlusOne, 4);
        assert_eq!(counters[&CounterKind::PlusOnePlusOne], u32::MAX);
    }

    #[test]
    fn conditional_entry_uses_the_captured_life_snapshot() {
        let mut engine = GameEngine::new(97_007, &[0, 1], 20, None, true).expect("engine");
        let snapshot = engine.player_life_snapshot();
        engine.state.players[1].life = 1;
        let event = BattlefieldEntryEvent {
            entry_reveal_receipts: Vec::new(),
            mana_colors_spent_to_cast: Default::default(),
            prepared: false,
            object_id: 999,
            deciding_player: 0,
            destination_controller: 0,
            battle_protector: None,
            face_index: 0,
            unlock_room_door: None,
            chosen_x: 0,
            cast_by: None,
            cast_cost_receipts: Vec::new(),
            player_life_snapshot: snapshot,
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
        let condition = GameCondition::PlayerLifeAggregate {
            players: RelativePlayerSet::All,
            aggregate: PlayerLifeAggregate::Minimum,
            min: Some(14),
            max: None,
        };

        assert!(engine.entry_condition_holds(&condition, &event));
        assert!(!engine.condition_holds(
            &condition,
            ConditionContext {
                controller: 0,
                source_object_id: 999,
                source_zone_change: 0,
                resolving_spell_id: None,
                stack_item: None,
                previous_effect_result: None,
            }
        ));
    }

    #[test]
    fn a_globe_in_the_same_proposed_batch_is_not_a_battlefield_source() {
        let decks = Some(vec![
            vec![
                "dragonstorm_globe".into(),
                "sparktongue_dragon".into(),
                "mountain".into(),
                "mountain".into(),
                "mountain".into(),
                "mountain".into(),
                "mountain".into(),
            ],
            vec!["forest".into(); 7],
        ]);
        let mut engine = GameEngine::new(97_008, &[0, 1], 20, decks, true).expect("engine");
        let globe = engine
            .state
            .objects
            .values()
            .find(|object| object.card_id == "dragonstorm_globe")
            .expect("Globe")
            .id;
        let dragon = engine
            .state
            .objects
            .values()
            .find(|object| object.card_id == "sparktongue_dragon")
            .expect("Dragon")
            .id;
        engine.state.objects.get_mut(&globe).unwrap().zone = Zone::Stack;
        engine.state.objects.get_mut(&dragon).unwrap().zone = Zone::Stack;
        let event = BattlefieldEntryEvent {
            entry_reveal_receipts: Vec::new(),
            mana_colors_spent_to_cast: Default::default(),
            prepared: false,
            object_id: dragon,
            deciding_player: 0,
            destination_controller: 0,
            battle_protector: None,
            face_index: 0,
            unlock_room_door: None,
            chosen_x: 0,
            cast_by: None,
            cast_cost_receipts: Vec::new(),
            player_life_snapshot: engine.player_life_snapshot(),
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

        assert!(engine.battlefield_entry_candidates(&event).is_empty());
        engine.state.objects.get_mut(&globe).unwrap().zone = Zone::Battlefield;
        assert_eq!(engine.battlefield_entry_candidates(&event).len(), 1);
    }

    #[test]
    fn entry_replacements_see_types_added_by_the_resolving_effect() {
        let decks = Some(vec![
            vec![
                "dragonstorm_globe".into(),
                "hill_giant".into(),
                "mountain".into(),
                "mountain".into(),
                "mountain".into(),
                "mountain".into(),
                "mountain".into(),
            ],
            vec!["forest".into(); 7],
        ]);
        let mut engine = GameEngine::new(234_002, &[0, 1], 20, decks, true).expect("engine");
        let globe = engine
            .state
            .objects
            .values()
            .find(|object| object.card_id == "dragonstorm_globe")
            .expect("Globe")
            .id;
        let giant = engine
            .state
            .objects
            .values()
            .find(|object| object.card_id == "hill_giant")
            .expect("Giant")
            .id;
        engine.state.objects.get_mut(&globe).unwrap().zone = Zone::Battlefield;
        engine.state.objects.get_mut(&giant).unwrap().zone = Zone::Stack;
        let event = BattlefieldEntryEvent {
            entry_reveal_receipts: Vec::new(),
            mana_colors_spent_to_cast: Default::default(),
            prepared: false,
            object_id: giant,
            deciding_player: 0,
            destination_controller: 0,
            battle_protector: None,
            face_index: 0,
            unlock_room_door: None,
            chosen_x: 0,
            cast_by: None,
            cast_cost_receipts: Vec::new(),
            player_life_snapshot: engine.player_life_snapshot(),
            tapped: false,
            set_types: None,
            chosen_basic_land_type: None,
            chosen_opponents: Vec::new(),
            entry_counters: BTreeMap::new(),
            entry_modifiers: vec![ResolvingPermanentModifier::AddTypes(
                tricerules_cards::TypeLineAddition {
                    land_types: Vec::new(),
                    card_types: Vec::new(),
                    creature_types: vec!["Dragon".into()],
                },
            )],
            attached_to: None,
            pending_copy_candidate: None,
            pending_aura_recipient: None,
            accepted_aura_recipient: None,
            applied_effects: Vec::new(),
        };

        assert_eq!(engine.battlefield_entry_candidates(&event).len(), 1);
        engine.state.continuous_effects.push(ContinuousEffect {
            trigger_grant_origin: None,
            source_id: None,
            affected: AffectedScope::Single(globe),
            kind: ContinuousEffectKind::Layer4SetTypeLine(tricerules_cards::TypeLineReplacement {
                card_types: vec![PermanentTypeFilter::Land],
                creature_types: Vec::new(),
                land_types: vec![BasicLandType::Forest],
            }),
            condition: None,
            duration: EffectDuration::UntilEndOfTurn,
            timestamp: 5,
        });
        assert!(engine.battlefield_entry_candidates(&event).is_empty(),
            "a printed battlefield entry replacement loses its source under type-setting suppression");
        engine.state.continuous_effects.clear();
        assert_eq!(engine.battlefield_entry_candidates(&event).len(), 1);
    }
}

#[cfg(test)]
mod presentation_tests {
    use super::*;

    #[test]
    fn replacement_images_never_resolve_concealed_source_identity() {
        let mut engine = GameEngine::new(
            220_002,
            &[0, 1],
            20,
            Some(vec![vec!["forest".into(); 7], vec!["plains".into(); 7]]),
            true,
        )
        .unwrap();
        let id = *engine.state.objects.keys().next().unwrap();
        for (zone, face_down) in [
            (Zone::Hand, false),
            (Zone::Library, false),
            (Zone::Battlefield, true),
            (Zone::Exile, true),
        ] {
            let object = engine.state.objects.get_mut(&id).unwrap();
            object.zone = zone;
            object.face_down = face_down;
            let source = engine.replacement_source_presentation(id);
            assert_eq!(source, ReplacementSourcePresentation::default());
        }
        let object = engine.state.objects.get_mut(&id).unwrap();
        object.zone = Zone::Battlefield;
        object.face_down = false;
        let source = engine.replacement_source_presentation(id);
        assert!(!source.card_name.is_empty());
        assert_eq!(source.object_id, id);
        let object = engine.state.objects.get_mut(&id).unwrap();
        object.card_id = "bonecrusher_giant_stomp".into();
        object.zone = Zone::Stack;
        object.face_up_index = 1;
        assert_eq!(
            engine.replacement_source_presentation(id).card_name,
            "Bonecrusher Giant // Stomp"
        );
    }
}
