//! Pure early-layer snapshots and CR 613.8 ordering for type-changing effects.
use super::*;

#[derive(Clone)]
pub(super) struct EarlyObject {
    pub(super) characteristics: Characteristics,
    pub(super) printed_rules_text_present: bool,
}

#[derive(Clone, Default)]
pub(super) struct EarlyLayerView {
    pub(super) projected_entrant: Option<ObjectId>,
    pub(super) objects: BTreeMap<ObjectId, EarlyObject>,
    pub(super) started: BTreeMap<ObjectId, Vec<TriggerAbilityOrigin>>,
}

pub(super) struct EarlyEntryProjection<'a> {
    pub(super) event: &'a BattlefieldEntryEvent,
    pub(super) base: EarlyObject,
    pub(super) own_origins: Vec<TriggerAbilityOrigin>,
    old_generation: u64,
    copy_revision: u64,
}

struct EarlyLayerInputs<'a> {
    effects: Vec<&'a ContinuousEffect>,
    entry: Option<&'a EarlyEntryProjection<'a>>,
}

#[derive(Clone, PartialEq, Eq)]
enum TypeInstruction {
    RemoveCreature,
    Add(tricerules_cards::TypeLineAddition),
    Set(tricerules_cards::TypeLineReplacement),
    Basic(BasicLandType),
    CreatureTypes(Vec<String>),
    AllCreatureTypes,
}

#[derive(PartialEq, Eq)]
struct TypeProbe {
    exists: bool,
    instructions: Vec<(ObjectId, TypeInstruction)>,
}

fn is_type_effect(kind: &ContinuousEffectKind) -> bool {
    matches!(
        kind,
        ContinuousEffectKind::Layer4AddTypes(_)
            | ContinuousEffectKind::Layer4RemoveCreature
            | ContinuousEffectKind::Layer4SetTypeLine(_)
            | ContinuousEffectKind::Layer4SetBasicLandType(_)
            | ContinuousEffectKind::Layer4SetCreatureTypes(_)
            | ContinuousEffectKind::Layer4SetAllCreatureTypes
    )
}

fn next_dependency_candidate(depends: &[Vec<bool>]) -> usize {
    let size = depends.len();
    let mut reaches = depends.to_vec();
    for bridge in 0..size {
        for from in 0..size {
            for to in 0..size {
                reaches[from][to] |= reaches[from][bridge] && reaches[bridge][to];
            }
        }
    }
    // An external prerequisite of any member blocks the whole strongly connected
    // component. Input order already represents timestamp, then insertion order.
    (0..size)
        .find(|&index| {
            (0..size).all(|member| {
                let same_component =
                    member == index || (reaches[index][member] && reaches[member][index]);
                !same_component
                    || (0..size).all(|other| {
                        !depends[member][other] || (reaches[index][other] && reaches[other][index])
                    })
            })
        })
        .expect("finite dependency graph has an available source component")
}

impl CharacteristicsEvaluator<'_> {
    pub(super) fn evaluate_entry_layers(
        &self,
        event: &BattlefieldEntryEvent,
        face: &CardFace,
        statics: &[(AbilityDefinitionId, &StaticAbilityDef)],
        entry_effects: &[ContinuousEffect],
    ) -> Option<(Characteristics, bool)> {
        let object = self.state.objects.get(&event.object_id)?;
        let old_generation = self
            .state
            .zone_change_generation
            .get(&event.object_id)
            .copied()
            .unwrap_or(0);
        let generation = old_generation.checked_add(1)?;
        let base = EarlyObject {
            characteristics: self.base_characteristics_from_face(
                event.object_id,
                face,
                event.face_index,
                event.destination_controller,
                object.face_down,
            )?,
            printed_rules_text_present: !object.face_down,
        };
        let own_effects = if object.face_down {
            Vec::new()
        } else {
            statics
                .iter()
                .flat_map(|(definition, ability)| {
                    super::super::continuous::materialize_early_static_components(
                        event.object_id,
                        event.destination_controller,
                        generation,
                        self.state.command_index,
                        definition,
                        ability,
                    )
                })
                .collect::<Vec<_>>()
        };
        let projection = EarlyEntryProjection {
            event,
            base,
            old_generation,
            copy_revision: object.copy_revision,
            own_origins: own_effects
                .iter()
                .filter_map(|effect| effect.trigger_grant_origin.clone())
                .collect(),
        };
        let inputs = EarlyLayerInputs {
            effects: self
                .state
                .continuous_effects
                .iter()
                .filter(|effect| {
                    !(matches!(effect.affected, AffectedScope::Single(oid) if oid == event.object_id)
                        || effect.source_id == Some(event.object_id)
                            && effect.duration == EffectDuration::WhileSourceOnBattlefield)
                })
                .chain(&own_effects)
                .chain(entry_effects)
                .collect(),
            entry: Some(&projection),
        };
        self.evaluate_early_inputs(event.object_id, true, &inputs)?
            .objects
            .remove(&event.object_id)
            .map(|object| (object.characteristics, object.printed_rules_text_present))
    }

    fn early_component_group(
        &self,
        inputs: &EarlyLayerInputs<'_>,
        effect: &ContinuousEffect,
    ) -> Option<TriggerAbilityOrigin> {
        if let Some(entry) = inputs
            .entry
            .filter(|entry| effect.source_id == Some(entry.event.object_id))
        {
            let object = self.state.objects.get(&entry.event.object_id)?;
            if object.face_down
                || object.copy_revision != entry.copy_revision
                || self
                    .state
                    .zone_change_generation
                    .get(&entry.event.object_id)
                    .copied()
                    .unwrap_or(0)
                    != entry.old_generation
            {
                return None;
            }
            let origin = effect.trigger_grant_origin.as_ref()?;
            // Exact origins freshly materialized from the selected/copied raw face in this
            // immutable preview. Old source records are excluded from the inputs.
            return entry.own_origins.contains(origin).then(|| origin.clone());
        }
        component_group(self.state, self.registry, effect)
    }

    fn early_condition_holds(
        &self,
        inputs: &EarlyLayerInputs<'_>,
        condition: &GameCondition,
        source: ObjectId,
        oid: ObjectId,
        view: &EarlyLayerView,
    ) -> bool {
        if let Some(entry) = inputs.entry.filter(|entry| entry.event.object_id == source) {
            match condition {
                GameCondition::AllOf(conditions) => {
                    return conditions.iter().all(|condition| {
                        self.early_condition_holds(inputs, condition, source, oid, view)
                    })
                }
                GameCondition::AnyOf(conditions) => {
                    return conditions.iter().any(|condition| {
                        self.early_condition_holds(inputs, condition, source, oid, view)
                    })
                }
                GameCondition::SourceCounterCount { counter, .. } => {
                    return condition.matches_value(
                        entry
                            .event
                            .entry_counters
                            .get(counter)
                            .copied()
                            .unwrap_or(0),
                    )
                }
                GameCondition::SourceTotalCounterCount { .. } => {
                    return condition.matches_value(
                        entry
                            .event
                            .entry_counters
                            .values()
                            .copied()
                            .fold(0, u32::saturating_add),
                    )
                }
                GameCondition::ObjectTapped {
                    object: ConditionObjectRef::Source,
                    tapped,
                } => return entry.event.tapped == *tapped,
                _ => {}
            }
        }
        let Some(object) = view.objects.get(&oid) else {
            return false;
        };
        let controller = view
            .objects
            .get(&source)
            .map(|source| source.characteristics.controller)
            .unwrap_or_else(|| self.layer_2_controller(source, &mut Vec::new()));
        self.characteristic_condition_holds_in_view(
            condition,
            source,
            controller,
            oid,
            &object.characteristics,
            Some(view),
        )
    }

    pub(super) fn evaluate_early_layers(
        &self,
        queried: ObjectId,
        world_required: bool,
    ) -> Option<EarlyLayerView> {
        let inputs = EarlyLayerInputs {
            effects: self.state.continuous_effects.iter().collect(),
            entry: None,
        };
        self.evaluate_early_inputs(queried, world_required, &inputs)
    }

    fn evaluate_early_inputs(
        &self,
        queried: ObjectId,
        world_required: bool,
        inputs: &EarlyLayerInputs<'_>,
    ) -> Option<EarlyLayerView> {
        let queried_object = self.state.objects.get(&queried)?;
        // A world view is necessary for inter-object early-layer dependencies and counts.
        // Ordinary queries with only later-layer effects need just their own base object.
        let world_required = world_required
            || inputs
                .effects
                .iter()
                .any(|effect| is_earlier_characteristic_component(&effect.kind));
        let mut view = EarlyLayerView {
            projected_entrant: inputs.entry.map(|entry| entry.event.object_id),
            ..Default::default()
        };
        let others = world_required
            .then_some(&self.state.objects)
            .into_iter()
            .flat_map(|objects| objects.iter())
            .filter(|(&oid, object)| oid != queried && object.zone == Zone::Battlefield);
        for (&oid, object) in std::iter::once((&queried, queried_object)).chain(others) {
            if let Some(entry) = inputs.entry.filter(|entry| entry.event.object_id == oid) {
                view.objects.insert(oid, entry.base.clone());
            } else if let Some(characteristics) = self.base_characteristics_through_layer_2(oid) {
                view.objects.insert(
                    oid,
                    EarlyObject {
                        characteristics,
                        printed_rules_text_present: !(object.face_down
                            && object.zone == Zone::Battlefield),
                    },
                );
            }
        }
        self.apply_early_timestamp_layer(inputs, &mut view, 3);
        self.apply_dependency_ordered_types(inputs, &mut view);
        self.apply_early_timestamp_layer(inputs, &mut view, 5);
        for object in view.objects.values_mut() {
            object.characteristics.signed_power = object.characteristics.power.map(i64::from);
            object.characteristics.signed_toughness =
                object.characteristics.toughness.map(i64::from);
        }
        Some(view)
    }

    fn early_effect_exists(
        &self,
        inputs: &EarlyLayerInputs<'_>,
        effect: &ContinuousEffect,
        view: &EarlyLayerView,
    ) -> bool {
        if inputs
            .entry
            .is_some_and(|entry| effect.source_id == Some(entry.event.object_id))
        {
            if effect.trigger_grant_origin.is_some()
                && self.early_component_group(inputs, effect).is_none()
            {
                return false;
            }
        } else if !static_source_identity_is_current(self.state, self.registry, effect) {
            return false;
        }
        let Some(origin) = self.early_component_group(inputs, effect) else {
            return true;
        };
        view.started
            .values()
            .any(|started| started.contains(&origin))
            || effect
                .source_id
                .and_then(|source| view.objects.get(&source))
                .is_some_and(|source| source.printed_rules_text_present)
    }

    fn early_recipient_matches(
        &self,
        inputs: &EarlyLayerInputs<'_>,
        effect: &ContinuousEffect,
        oid: ObjectId,
        view: &EarlyLayerView,
    ) -> bool {
        let Some(object) = view.objects.get(&oid) else {
            return false;
        };
        if inputs.entry.is_some_and(|entry| {
            effect.source_id == Some(entry.event.object_id)
                && effect.trigger_grant_origin.is_some()
                && oid != entry.event.object_id
        }) {
            return false;
        }
        if let Some(origin) = self.early_component_group(inputs, effect) {
            if view
                .started
                .values()
                .any(|started| started.contains(&origin))
            {
                return view
                    .started
                    .get(&oid)
                    .is_some_and(|started| started.contains(&origin));
            }
        }
        if !matches!(effect.affected, AffectedScope::Single(_))
            && view.projected_entrant != Some(oid)
            && !self
                .state
                .objects
                .get(&oid)
                .is_some_and(|object| object.zone == Zone::Battlefield)
        {
            return false;
        }
        if !effect_scope_affects_with_reference(
            self.state,
            self.registry,
            effect,
            oid,
            &object.characteristics,
            effect
                .source_id
                .and_then(|source| view.objects.get(&source))
                .map(|source| source.characteristics.controller),
        ) {
            return false;
        }
        let Some(condition) = effect.condition.as_ref() else {
            return true;
        };
        let Some(source) = effect.source_id else {
            return false;
        };
        self.early_condition_holds(inputs, condition, source, oid, view)
    }

    fn record_early_start(
        &self,
        inputs: &EarlyLayerInputs<'_>,
        effect: &ContinuousEffect,
        recipients: &[ObjectId],
        view: &mut EarlyLayerView,
    ) {
        if let Some(origin) = self.early_component_group(inputs, effect) {
            for &oid in recipients {
                let started = view.started.entry(oid).or_default();
                if !started.contains(&origin) {
                    started.push(origin.clone());
                }
            }
        }
    }

    fn apply_early_timestamp_layer(
        &self,
        inputs: &EarlyLayerInputs<'_>,
        view: &mut EarlyLayerView,
        layer: u8,
    ) {
        let mut effects = inputs
            .effects
            .iter()
            .copied()
            .enumerate()
            .filter(|(_, effect)| {
                matches!(
                    (&effect.kind, layer),
                    (ContinuousEffectKind::Layer3SetName(_), 3)
                        | (ContinuousEffectKind::Layer5SetColors(_), 5)
                )
            })
            .collect::<Vec<_>>();
        effects.sort_by_key(|(index, effect)| (effect.timestamp, *index));
        for (_, effect) in effects {
            if !self.early_effect_exists(inputs, effect, view) {
                continue;
            }
            let recipients = view
                .objects
                .keys()
                .copied()
                .filter(|&oid| self.early_recipient_matches(inputs, effect, oid, view))
                .collect::<Vec<_>>();
            self.record_early_start(inputs, effect, &recipients, view);
            for oid in recipients {
                let result = &mut view
                    .objects
                    .get_mut(&oid)
                    .expect("probed recipient")
                    .characteristics;
                match &effect.kind {
                    ContinuousEffectKind::Layer3SetName(name) => result.names = vec![name.clone()],
                    ContinuousEffectKind::Layer5SetColors(colors) => {
                        result.colors.clone_from(colors)
                    }
                    _ => unreachable!("selected early layer"),
                }
            }
        }
    }

    fn probe_type_effect(
        &self,
        inputs: &EarlyLayerInputs<'_>,
        index: usize,
        view: &EarlyLayerView,
    ) -> TypeProbe {
        let effect = inputs.effects[index];
        let exists = self.early_effect_exists(inputs, effect, view);
        let mut instructions = Vec::new();
        if exists {
            for (&oid, object) in &view.objects {
                if !self.early_recipient_matches(inputs, effect, oid, view) {
                    continue;
                }
                let result = &object.characteristics;
                let instruction = match &effect.kind {
                    ContinuousEffectKind::Layer4RemoveCreature => TypeInstruction::RemoveCreature,
                    ContinuousEffectKind::Layer4AddTypes(addition) => {
                        // Compare effective instructions, not whether an addition is redundant.
                        let mut addition = addition.clone();
                        if !(result.is_creature()
                            || result.has_type("Kindred")
                            || addition.card_types.contains(&PermanentTypeFilter::Creature))
                        {
                            addition.creature_types.clear();
                        }
                        if !(result.has_type("Land")
                            || addition.card_types.contains(&PermanentTypeFilter::Land))
                        {
                            addition.land_types.clear();
                        }
                        TypeInstruction::Add(addition)
                    }
                    ContinuousEffectKind::Layer4SetTypeLine(replacement) => {
                        TypeInstruction::Set(replacement.clone())
                    }
                    ContinuousEffectKind::Layer4SetBasicLandType(land_type) => {
                        if !result.has_type("Land") {
                            continue;
                        }
                        TypeInstruction::Basic(*land_type)
                    }
                    ContinuousEffectKind::Layer4SetCreatureTypes(types) => {
                        TypeInstruction::CreatureTypes(
                            if result.is_creature() || result.has_type("Kindred") {
                                types.clone()
                            } else {
                                Vec::new()
                            },
                        )
                    }
                    ContinuousEffectKind::Layer4SetAllCreatureTypes => {
                        TypeInstruction::AllCreatureTypes
                    }
                    _ => unreachable!("selected type effect"),
                };
                instructions.push((oid, instruction));
            }
        }
        TypeProbe {
            exists,
            instructions,
        }
    }

    fn apply_type_probe(
        &self,
        inputs: &EarlyLayerInputs<'_>,
        index: usize,
        probe: &TypeProbe,
        view: &mut EarlyLayerView,
    ) {
        let effect = inputs.effects[index];
        self.record_early_start(
            inputs,
            effect,
            &probe
                .instructions
                .iter()
                .map(|(oid, _)| *oid)
                .collect::<Vec<_>>(),
            view,
        );
        for (oid, instruction) in &probe.instructions {
            let object = view.objects.get_mut(oid).expect("probed recipient");
            let result = &mut object.characteristics;
            let removes_text = match instruction {
                TypeInstruction::RemoveCreature => {
                    apply_creature_type_removal(result);
                    false
                }
                TypeInstruction::Add(addition) => {
                    apply_type_line_addition(result, addition);
                    false
                }
                TypeInstruction::Set(replacement) => {
                    apply_type_line_replacement(result, replacement);
                    !replacement.land_types.is_empty()
                }
                TypeInstruction::Basic(land_type) => {
                    apply_basic_land_type(result, *land_type);
                    true
                }
                TypeInstruction::CreatureTypes(types) => {
                    result.all_creature_types = false;
                    result.types.retain(|value| !is_creature_type(value));
                    result.types.extend(types.iter().cloned());
                    false
                }
                TypeInstruction::AllCreatureTypes => {
                    result.all_creature_types = true;
                    result.types.retain(|value| !is_creature_type(value));
                    false
                }
            };
            if removes_text {
                object.printed_rules_text_present = false;
                result.keywords.clear();
                result.protections.clear();
                result.evasions.clear();
            }
        }
    }

    fn type_effect_depends_on(
        &self,
        inputs: &EarlyLayerInputs<'_>,
        dependent: usize,
        prerequisite: usize,
        view: &EarlyLayerView,
    ) -> bool {
        let left = inputs.effects[dependent];
        let right = inputs.effects[prerequisite];
        let left_group = self.early_component_group(inputs, left);
        if left_group.is_some() && left_group == self.early_component_group(inputs, right) {
            return false; // Components of one logical effect retain authored order.
        }
        let before = self.probe_type_effect(inputs, dependent, view);
        let mut temporary = view.clone();
        self.apply_type_probe(
            inputs,
            prerequisite,
            &self.probe_type_effect(inputs, prerequisite, view),
            &mut temporary,
        );
        before != self.probe_type_effect(inputs, dependent, &temporary)
    }

    fn apply_dependency_ordered_types(
        &self,
        inputs: &EarlyLayerInputs<'_>,
        view: &mut EarlyLayerView,
    ) {
        let mut remaining = inputs
            .effects
            .iter()
            .copied()
            .enumerate()
            .filter(|(_, effect)| is_type_effect(&effect.kind))
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        remaining.sort_by_key(|&index| (inputs.effects[index].timestamp, index));
        while !remaining.is_empty() {
            let size = remaining.len();
            let mut depends = vec![vec![false; size]; size];
            for dependent in 0..size {
                for prerequisite in 0..size {
                    if dependent != prerequisite {
                        depends[dependent][prerequisite] = self.type_effect_depends_on(
                            inputs,
                            remaining[dependent],
                            remaining[prerequisite],
                            view,
                        );
                    }
                }
            }
            let chosen = next_dependency_candidate(&depends);
            let index = remaining.remove(chosen);
            let probe = self.probe_type_effect(inputs, index, view);
            self.apply_type_probe(inputs, index, &probe, view);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn early_layer_loop_waits_for_external_prerequisite_of_another_member() {
        // Timestamp order is A, B, C, D. A/B form one loop, C/D another.
        // Only B depends on C: the entire A/B loop must wait for C/D.
        let depends = vec![
            vec![false, true, false, false],
            vec![true, false, true, false],
            vec![false, false, false, true],
            vec![false, false, true, false],
        ];
        assert_eq!(next_dependency_candidate(&depends), 2);
    }

    #[test]
    fn early_layer_available_loop_competes_with_independent_effect_by_timestamp() {
        let depends = vec![
            vec![false, true, false],
            vec![true, false, false],
            vec![false, false, false],
        ];
        assert_eq!(next_dependency_candidate(&depends), 0);
    }
}
