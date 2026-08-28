//! Rules-visible permanent characteristics.
//!
//! [`GameEngine::characteristics`] is the single engine entry point for derived power,
//! toughness, types, colors, keywords, and controller. It deliberately mirrors the CR 613
//! layer order:
//!
//! 1. copy effects;
//! 2. control-changing effects;
//! 3. text-changing effects;
//! 4. type-changing effects;
//! 5. color-changing effects;
//! 6. ability-adding/removing effects;
//! 7. power/toughness CDAs, setters, modifiers, counters, then switches.
//!
//! Unused layer subparts remain explicit identity stages.
//! CR 613.8 dependency ordering is intentionally deferred until the first effect that needs it;
//! the `ordered_effects` boundary is the insertion point. Replacement/prevention choice ordering
//! (CR 616) is the separate shared pipeline in `engine/replacement.rs`.
//!
//! The calculation is side-effect-free and depends only on `GameState`, the registry, and the
//! queried object id. Its owned result and single ordered-effect pass make it straightforward
//! to memoize later without changing callers.

use super::history::{
    graveyard_aggregate_value, player_life_aggregate_value, relative_player_set_contains,
};
use super::*;

/// The complete rules-visible characteristic snapshot currently modeled for a permanent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Characteristics {
    /// Rules-only CR 202.3 mana value; not a new wire field.
    pub mana_value: u32,
    pub controller: PlayerId,
    /// Rules-visible names after copy, face-down, and layer-3 text-changing effects. The vector
    /// preserves nameless and future multi-name objects without conflating names with card ids.
    pub names: Vec<String>,
    pub types: Vec<String>,
    /// CR 702.73 Changeling without materializing the entire CR 205.3m list per snapshot.
    pub all_creature_types: bool,
    pub supertypes: Vec<String>,
    pub colors: Vec<Color>,
    pub keywords: Vec<Keyword>,
    pub protections: Vec<ProtectionQuality>,
    pub evasions: Vec<Evasion>,
    pub power: Option<u32>,
    pub toughness: Option<u32>,
    /// Signed rules values for quantity arithmetic; the existing wire projection stays unsigned.
    pub(crate) signed_power: Option<i64>,
    pub(crate) signed_toughness: Option<i64>,
}

impl Characteristics {
    pub fn has_name(&self, name: &str) -> bool {
        self.names.iter().any(|candidate| candidate == name)
    }

    pub fn primary_name(&self) -> Option<&str> {
        self.names.first().map(String::as_str)
    }

    pub fn has_type(&self, card_type: &str) -> bool {
        self.types.iter().any(|t| t == card_type)
            || (self.all_creature_types && is_creature_type(card_type))
    }

    pub fn is_creature(&self) -> bool {
        self.has_type("Creature")
    }

    pub fn is_artifact(&self) -> bool {
        self.has_type("Artifact")
    }

    pub fn is_aura(&self) -> bool {
        self.has_type("Aura")
    }

    pub fn is_legendary(&self) -> bool {
        self.supertypes.iter().any(|t| t == "Legendary")
    }

    pub fn has_keyword(&self, keyword: Keyword) -> bool {
        self.keywords.contains(&keyword)
    }
}

struct CharacteristicsEvaluator<'a> {
    state: &'a GameState,
    registry: &'static CardRegistry,
}

pub(super) fn characteristics_from(
    state: &GameState,
    registry: &'static CardRegistry,
    oid: ObjectId,
) -> Option<Characteristics> {
    CharacteristicsEvaluator { state, registry }.characteristics(oid)
}

impl CharacteristicsEvaluator<'_> {
    fn characteristics(&self, oid: ObjectId) -> Option<Characteristics> {
        let object = self.state.objects.get(&oid)?;
        let mut result = self.characteristics_through_layer_5(oid)?;

        let ordered_effects = self.ordered_effects(oid, &result);
        self.apply_layer_6_abilities(object, &mut result, &ordered_effects);
        self.apply_layer_7_power_toughness(object, &mut result, &ordered_effects);
        Some(result)
    }

    /// Snapshot through CR 613 layer 5. Conditional layer-6/7 effects may inspect controller,
    /// type, and color through this boundary without recursively asking for their own result.
    fn characteristics_through_layer_5(&self, oid: ObjectId) -> Option<Characteristics> {
        let object = self.state.objects.get(&oid)?;
        let definition = self.registry.get(&object.card_id);
        let copied = object.copiable_values.as_ref();
        let face = effective_face_from(self.state, self.registry, oid)?;

        let mut result = Characteristics {
            // CR 202.3b/710.2: original transformed/flip cards retain front mana value.
            // A copy of a transforming back face has that face's (normally absent) mana cost.
            mana_value: if object.copiable_values.is_none()
                && object.token_origin.is_none()
                && definition
                    .is_some_and(|def| matches!(def.layout, Layout::Transform | Layout::Flip))
            {
                definition
                    .expect("checked definition")
                    .primary_face()
                    .mana_cost
                    .mana_value()
            } else {
                face.mana_cost.mana_value()
            },
            signed_power: None,
            signed_toughness: None,
            // CR 110.2 base value set by the instruction that put the object onto the battlefield.
            // Layer 2 below applies control-changing continuous effects on top.
            controller: object.base_controller,
            names: (!face.name.is_empty())
                .then(|| face.name.clone())
                .into_iter()
                .collect(),
            types: face.types.to_vec(),
            all_creature_types: face
                .characteristic_defining_abilities
                .contains(&CharacteristicDefiningAbility::Changeling),
            supertypes: face.supertypes.to_vec(),
            colors: if copied.is_none()
                && definition.is_some_and(|definition| definition.layout == Layout::Flip)
                && object.face_up_index > 0
            {
                definition
                    .expect("checked flip definition")
                    .primary_face()
                    .colors()
            } else {
                face.colors()
            },
            keywords: face.keywords.to_vec(),
            protections: face.protections.to_vec(),
            evasions: face.evasions.to_vec(),
            // Object snapshots take precedence for tokens and scenario overrides. Multi-face
            // objects leave these unset and read the active face.
            power: if copied.is_some() {
                face.power
            } else {
                object.power.or(face.power)
            },
            toughness: if copied.is_some() {
                face.toughness
            } else {
                object.toughness.or(face.toughness)
            },
        };

        self.apply_layer_1_copy(&mut result);
        self.apply_layer_1b_face_down(object, &mut result);
        self.apply_layer_2_control(oid, &mut result);
        self.apply_layer_3_text(oid, &mut result);
        self.apply_layer_4_type(oid, &mut result);
        self.apply_layer_5_color(oid, &mut result);
        result.signed_power = result.power.map(i64::from);
        result.signed_toughness = result.toughness.map(i64::from);
        Some(result)
    }

    // These identity stages are intentionally separate: adding the first effect in a layer must
    // fill its existing slot rather than creating another characteristics path.
    fn apply_layer_1_copy(&self, _result: &mut Characteristics) {
        // The owned snapshot was selected above as the base printed face. Keeping this named
        // stage makes the CR 613 order explicit while avoiding a second characteristics path.
    }

    /// CR 613.2b / 708.2: face-down values are applied after copy effects and before every later
    /// characteristic-changing layer. Later effects therefore modify the public 2/2 instead of
    /// exposing or replacing the underlying printed face.
    fn apply_layer_1b_face_down(&self, object: &GameObject, result: &mut Characteristics) {
        if !object.face_down || object.zone != Zone::Battlefield {
            return;
        }
        apply_face_down_values(result);
    }

    /// CR 613 layer 2 — control-changing continuous effects. This pass is deliberately earlier
    /// than `ordered_effects`: source-relative effects such as Mind Control may depend on the
    /// source Aura's own derived controller. Resolve that dependency recursively, then apply
    /// otherwise independent effects in stable `(timestamp, index)` order (CR 613.7–613.8).
    fn apply_layer_2_control(&self, oid: ObjectId, result: &mut Characteristics) {
        result.controller = self.layer_2_controller(oid, &mut Vec::new());
    }

    fn layer_2_controller(&self, oid: ObjectId, visiting: &mut Vec<ObjectId>) -> PlayerId {
        let Some(object) = self.state.objects.get(&oid) else {
            return 0;
        };
        let base = object.base_controller;
        if visiting.contains(&oid) {
            return base;
        }
        visiting.push(oid);
        let mut controller = base;
        let mut effects: Vec<(usize, &ContinuousEffect)> = self
            .state
            .continuous_effects
            .iter()
            .enumerate()
            .filter(|(_, effect)| {
                matches!(effect.kind, ContinuousEffectKind::Layer2Control { .. })
                    && match effect.affected {
                        AffectedScope::Single(id) => id == oid,
                        AffectedScope::AttachedTo(source_id) => {
                            self.state.objects.get(&source_id).is_some_and(|source| {
                                source.zone == Zone::Battlefield
                                    && source.attached_to == Some(AttachmentRecipient::Object(oid))
                            })
                        }
                        _ => false,
                    }
            })
            .collect();
        effects.sort_by_key(|(index, effect)| (effect.timestamp, *index));
        for (_, effect) in effects {
            if let ContinuousEffectKind::Layer2Control {
                controller: reference,
            } = effect.kind
            {
                controller = match reference {
                    ControllerReference::Fixed(player) => player,
                    ControllerReference::SourceController => effect
                        .source_id
                        .and_then(|source| {
                            self.state
                                .objects
                                .get(&source)
                                .filter(|object| object.zone == Zone::Battlefield)
                                .map(|_| self.layer_2_controller(source, visiting))
                        })
                        .unwrap_or(controller),
                };
            }
        }
        visiting.pop();
        controller
    }

    fn apply_layer_3_text(&self, oid: ObjectId, result: &mut Characteristics) {
        let mut effects: Vec<(usize, &ContinuousEffect)> = self
            .state
            .continuous_effects
            .iter()
            .enumerate()
            .filter(|(_, effect)| matches!(effect.kind, ContinuousEffectKind::Layer3SetName(_)))
            .filter(|(_, effect)| effect_affects(self.state, self.registry, effect, oid, result))
            .filter(|(_, effect)| self.characteristic_effect_condition_holds(effect, oid, result))
            .collect();
        effects.sort_by_key(|(index, effect)| (effect.timestamp, *index));
        for (_, effect) in effects {
            let ContinuousEffectKind::Layer3SetName(name) = &effect.kind else {
                unreachable!("filtered to layer-3 name effects");
            };
            result.names = vec![name.clone()];
        }
    }

    /// CR 205.1b / 613.1d: additive type-changing effects retain every printed and previously
    /// added type. Equal timestamps use insertion order so replay remains deterministic.
    fn apply_layer_4_type(&self, oid: ObjectId, result: &mut Characteristics) {
        let mut effects: Vec<(usize, &ContinuousEffect)> = self
            .state
            .continuous_effects
            .iter()
            .enumerate()
            .filter(|(_, effect)| {
                matches!(
                    effect.kind,
                    ContinuousEffectKind::Layer4AddTypes(_)
                        | ContinuousEffectKind::Layer4SetTypeLine(_)
                        | ContinuousEffectKind::Layer4SetCreatureTypes(_)
                )
            })
            .filter(|(_, effect)| effect_affects(self.state, self.registry, effect, oid, result))
            .filter(|(_, effect)| self.characteristic_effect_condition_holds(effect, oid, result))
            .collect();
        effects.sort_by_key(|(index, effect)| (effect.timestamp, *index));

        for (_, effect) in effects {
            match &effect.kind {
                ContinuousEffectKind::Layer4AddTypes(addition) => {
                    for card_type in &addition.card_types {
                        let card_type = card_type.as_str();
                        if !result.types.iter().any(|existing| existing == card_type) {
                            result.types.push(card_type.to_string());
                        }
                    }
                    if result.is_creature() || result.has_type("Kindred") {
                        for creature_type in &addition.creature_types {
                            if !result.types.contains(creature_type) {
                                result.types.push(creature_type.clone());
                            }
                        }
                    }
                }
                ContinuousEffectKind::Layer4SetTypeLine(replacement) => {
                    result.all_creature_types = false;
                    result.types.clear();
                    result.types.extend(
                        replacement
                            .card_types
                            .iter()
                            .map(|card_type| card_type.as_str().to_string()),
                    );
                    result
                        .types
                        .extend(replacement.creature_types.iter().cloned());
                }
                ContinuousEffectKind::Layer4SetCreatureTypes(creature_types) => {
                    result.all_creature_types = false;
                    result.types.retain(|value| !is_creature_type(value));
                    if result.is_creature() || result.has_type("Kindred") {
                        for creature_type in creature_types {
                            if !result.types.contains(creature_type) {
                                result.types.push(creature_type.clone());
                            }
                        }
                    }
                }
                _ => unreachable!("filtered to layer-4 type effects"),
            }
        }
    }

    fn apply_layer_5_color(&self, oid: ObjectId, result: &mut Characteristics) {
        let mut effects: Vec<(usize, &ContinuousEffect)> = self
            .state
            .continuous_effects
            .iter()
            .enumerate()
            .filter(|(_, effect)| matches!(effect.kind, ContinuousEffectKind::Layer5SetColors(_)))
            .filter(|(_, effect)| effect_affects(self.state, self.registry, effect, oid, result))
            .filter(|(_, effect)| self.characteristic_effect_condition_holds(effect, oid, result))
            .collect();
        effects.sort_by_key(|(index, effect)| (effect.timestamp, *index));
        for (_, effect) in effects {
            let ContinuousEffectKind::Layer5SetColors(colors) = &effect.kind else {
                unreachable!("filtered to layer-5 color effects");
            };
            result.colors.clone_from(colors);
        }
    }

    /// Active effects in CR 613.7 timestamp order. The original vector index makes equal
    /// timestamps deterministic. Layer-2 source-controller dependencies are handled by
    /// `layer_2_controller` before this later-layer pass.
    fn ordered_effects<'a>(
        &'a self,
        oid: ObjectId,
        pre_layer_6: &Characteristics,
    ) -> Vec<&'a ContinuousEffect> {
        let mut effects: Vec<(usize, &ContinuousEffect)> = self
            .state
            .continuous_effects
            .iter()
            .enumerate()
            .filter(|(_, effect)| {
                matches!(
                    effect.kind,
                    ContinuousEffectKind::Layer6RemoveAllAbilities
                        | ContinuousEffectKind::Layer6AddKeyword(_)
                        | ContinuousEffectKind::Layer6AddProtection(_)
                        | ContinuousEffectKind::Layer7bSetPt { .. }
                        | ContinuousEffectKind::PtModify { .. }
                        | ContinuousEffectKind::PtModifyByCount { .. }
                )
            })
            .filter(|(_, effect)| {
                effect_affects(self.state, self.registry, effect, oid, pre_layer_6)
            })
            .filter(|(_, effect)| {
                self.characteristic_effect_condition_holds(effect, oid, pre_layer_6)
            })
            .collect();
        effects.sort_by_key(|(index, effect)| (effect.timestamp, *index));
        effects.into_iter().map(|(_, effect)| effect).collect()
    }

    fn characteristic_effect_condition_holds(
        &self,
        effect: &ContinuousEffect,
        queried_oid: ObjectId,
        queried_pre_layer_6: &Characteristics,
    ) -> bool {
        let Some(condition) = effect.condition.as_ref() else {
            return true;
        };
        let Some(source_oid) = effect.source_id else {
            return false;
        };
        let controller = self.layer_2_controller(source_oid, &mut Vec::new());
        match condition {
            // Cast snapshots are internal to resolving spells, never continuous characteristics.
            GameCondition::CastSnapshot { .. } => false,
            GameCondition::LifeChangedThisTurn {
                players,
                change,
                quantifier,
            } => super::history::life_changed_this_turn(
                self.state,
                *players,
                *change,
                *quantifier,
                controller,
                None,
                None,
            ),
            GameCondition::ActivePlayer { players } => relative_player_set_contains(
                self.state,
                *players,
                controller,
                self.state.active_player_id(),
            ),
            GameCondition::PlayerLifeAggregate {
                players, aggregate, ..
            } => player_life_aggregate_value(
                self.state,
                *players,
                *aggregate,
                controller,
                |player_id| {
                    self.state
                        .players
                        .iter()
                        .find(|player| player.id == player_id)
                        .map(|player| player.life)
                },
            )
            .is_some_and(|value| condition.matches_life_value(value)),
            GameCondition::CreatureDeathsThisTurn { .. } => {
                condition.matches_value(self.state.turn_history.current.creatures_died)
            }
            GameCondition::PermanentCardsEnteredGraveyardThisTurn {
                players,
                permanent_type,
                ..
            } => condition.matches_value(super::history::permanent_history_count(
                self.state,
                &self
                    .state
                    .turn_history
                    .current
                    .permanent_cards_entered_graveyard,
                *players,
                *permanent_type,
                controller,
            )),
            GameCondition::PermanentsSacrificedThisTurn {
                players,
                permanent_type,
                ..
            } => condition.matches_value(super::history::permanent_history_count(
                self.state,
                &self.state.turn_history.current.permanents_sacrificed,
                *players,
                *permanent_type,
                controller,
            )),
            GameCondition::SpellsCastThisTurn {
                players, filter, ..
            } => condition.matches_value(super::history::spell_cast_count(
                self.state,
                ConditionPlayerSet::Relative(*players),
                filter,
                controller,
                None,
                false,
            )),
            GameCondition::CardsDrawnThisTurn { players, .. } => {
                let count = self
                    .state
                    .players
                    .iter()
                    .filter(|player| {
                        relative_player_set_contains(self.state, *players, controller, player.id)
                    })
                    .fold(0u32, |total, player| {
                        total.saturating_add(
                            self.state
                                .turn_history
                                .current
                                .player(player.id)
                                .cards_drawn,
                        )
                    });
                condition.matches_value(count)
            }
            GameCondition::CrimesCommittedThisTurn { players, .. } => {
                let count = self
                    .state
                    .players
                    .iter()
                    .filter(|player| {
                        relative_player_set_contains(self.state, *players, controller, player.id)
                    })
                    .fold(0u32, |total, player| {
                        total.saturating_add(
                            self.state
                                .turn_history
                                .current
                                .player(player.id)
                                .crimes_committed,
                        )
                    });
                condition.matches_value(count)
            }
            GameCondition::AttackedThisTurn { players } => self
                .state
                .players
                .iter()
                .filter(|player| {
                    relative_player_set_contains(self.state, *players, controller, player.id)
                })
                .any(|player| self.state.turn_history.current.player(player.id).attacked),
            GameCondition::AttackersDeclaredThisTurn {
                players, filter, ..
            } => {
                let count = self
                    .state
                    .turn_history
                    .current
                    .declared_attackers
                    .iter()
                    .filter(|fact| {
                        relative_player_set_contains(
                            self.state,
                            *players,
                            controller,
                            fact.controller,
                        ) && super::history::creature_event_fact_matches(filter, fact)
                    })
                    .count();
                condition.matches_value(u32::try_from(count).unwrap_or(u32::MAX))
            }
            GameCondition::PermanentsEnteredThisTurn {
                controllers,
                filter,
                ..
            } => {
                let source_generation = self
                    .state
                    .zone_change_generation
                    .get(&source_oid)
                    .copied()
                    .unwrap_or(0);
                let context = ConditionContext {
                    controller,
                    source_object_id: source_oid,
                    source_zone_change: source_generation,
                    resolving_spell_id: None,
                    stack_item: None,
                };
                let count = self
                    .state
                    .turn_history
                    .current
                    .permanents_entered
                    .iter()
                    .filter(|fact| {
                        relative_player_set_contains(
                            self.state,
                            *controllers,
                            controller,
                            fact.controller,
                        ) && super::history::permanent_event_fact_matches(
                            self.state, filter, fact, context,
                        )
                    })
                    .count();
                condition.matches_value(u32::try_from(count).unwrap_or(u32::MAX))
            }
            GameCondition::SourceCounterCount { counter, .. } => {
                let count = self
                    .state
                    .objects
                    .get(&source_oid)
                    .filter(|object| object.zone == Zone::Battlefield)
                    .map(|object| object.counter_count(*counter))
                    .unwrap_or(0);
                condition.matches_value(count)
            }
            GameCondition::ObjectWasDealtDamageThisTurn { .. } => false,
            // Registry validation rejects this dependency-sensitive condition for the only
            // current producer of conditional characteristic effects. Normal condition users
            // evaluate it through `GameEngine::condition_holds` instead.
            GameCondition::BattlefieldCreatureCount { .. } => false,
            GameCondition::BattlefieldAggregate {
                filter,
                aggregate: BattlefieldAggregate::Count,
                ..
            } => {
                let source_generation = self
                    .state
                    .zone_change_generation
                    .get(&source_oid)
                    .copied()
                    .unwrap_or(0);
                let context = ConditionContext {
                    controller,
                    source_object_id: source_oid,
                    source_zone_change: source_generation,
                    resolving_spell_id: None,
                    stack_item: None,
                };
                let count = self
                    .state
                    .players
                    .iter()
                    .flat_map(|player| player.battlefield.iter().copied())
                    .filter_map(|candidate_oid| {
                        let characteristics = if candidate_oid == queried_oid {
                            queried_pre_layer_6.clone()
                        } else {
                            self.characteristics_through_layer_5(candidate_oid)?
                        };
                        Some((candidate_oid, characteristics))
                    })
                    .filter(|(candidate_oid, characteristics)| {
                        history::battlefield_permanent_matches(
                            self.state,
                            filter,
                            *candidate_oid,
                            characteristics,
                            context,
                        )
                    })
                    .count();
                condition.matches_value(u32::try_from(count).unwrap_or(u32::MAX))
            }
            GameCondition::BattlefieldAggregate { .. } => false,
            GameCondition::UnlockedRoomDoorCount { controllers, .. } => {
                let count = self
                    .state
                    .room_states
                    .iter()
                    .filter_map(|(object_id, room)| {
                        let object = self.state.objects.get(object_id)?;
                        (object.zone == Zone::Battlefield
                            && relative_player_set_contains(
                                self.state,
                                *controllers,
                                controller,
                                object.controller,
                            ))
                        .then_some(
                            room.unlocked
                                .into_iter()
                                .filter(|unlocked| *unlocked)
                                .count(),
                        )
                    })
                    .sum::<usize>();
                condition.matches_value(u32::try_from(count).unwrap_or(u32::MAX))
            }
            GameCondition::GraveyardAggregate {
                owners,
                aggregate,
                filter,
                ..
            } => condition.matches_value(graveyard_aggregate_value(
                self.state,
                self.registry,
                *owners,
                *aggregate,
                filter.as_ref(),
                controller,
                None,
            )),
        }
    }
}

/// Apply the CR 708.2 battlefield characteristics to a snapshot. Battlefield-entry replacement
/// effects use this helper while the physical object is still in its source zone but has already
/// been designated to enter face down.
pub(super) fn apply_face_down_values(result: &mut Characteristics) {
    result.mana_value = 0;
    result.names.clear();
    result.types = vec!["Creature".to_string()];
    result.all_creature_types = false;
    result.supertypes.clear();
    result.colors.clear();
    result.keywords.clear();
    result.protections.clear();
    result.evasions.clear();
    result.power = Some(2);
    result.toughness = Some(2);
}

/// Whether an effect applies, evaluated from the relevant characteristic snapshot and direct
/// combat state. Characteristic predicates only depend on controller, types, and colors, avoiding
/// recursive full-characteristic queries. Dependency ordering becomes necessary once scopes can
/// depend on values changed in their layer.
pub(super) fn effect_affects(
    state: &GameState,
    registry: &'static CardRegistry,
    effect: &ContinuousEffect,
    oid: ObjectId,
    characteristics: &Characteristics,
) -> bool {
    if effect.duration == EffectDuration::WhileSourceOnBattlefield
        && !matches!(effect.kind, ContinuousEffectKind::Layer6RemoveAllAbilities)
        && effect.source_id.is_some_and(|source_id| {
            latest_remove_all_abilities_timestamp(state, source_id)
                .is_some_and(|removed_at| effect.timestamp <= removed_at)
        })
    {
        return false;
    }
    match &effect.affected {
        AffectedScope::Single(id) => *id == oid,
        AffectedScope::AllCreatures => characteristics.is_creature(),
        AffectedScope::AttachedTo(source_oid) => {
            state.objects.get(source_oid).is_some_and(|attachment| {
                attachment.zone == Zone::Battlefield
                    && attachment.attached_to == Some(AttachmentRecipient::Object(oid))
            })
        }
        AffectedScope::CreaturesMatching {
            reference_player,
            filter,
            exclude,
        } => {
            let current_reference = if filter.controller.is_some()
                && effect.duration == EffectDuration::WhileSourceOnBattlefield
            {
                effect
                    .source_id
                    .map(|source| {
                        CharacteristicsEvaluator { state, registry }
                            .layer_2_controller(source, &mut Vec::new())
                    })
                    .unwrap_or(*reference_player)
            } else {
                *reference_player
            };
            creature_matches_scope(
                state,
                registry,
                filter,
                current_reference,
                *exclude,
                oid,
                characteristics,
            )
        }
        AffectedScope::PermanentsMatching {
            reference_player,
            filter,
            exclude,
        } => permanent_matches_target_scope(
            state,
            filter,
            *reference_player,
            *exclude,
            oid,
            characteristics,
        ),
        AffectedScope::Player(_) => false,
    }
}

/// Latest remove-all-abilities timestamp for the scopes currently capable of creating that
/// effect. Kept side-effect-free so ability enumeration and source-effect suppression consume the
/// same layer-6 decision without recursively evaluating the full characteristics pipeline.
pub(super) fn latest_remove_all_abilities_timestamp(
    state: &GameState,
    oid: ObjectId,
) -> Option<u64> {
    state
        .continuous_effects
        .iter()
        .filter(|effect| matches!(effect.kind, ContinuousEffectKind::Layer6RemoveAllAbilities))
        .filter(|effect| match effect.affected {
            AffectedScope::Single(affected) => affected == oid,
            AffectedScope::AttachedTo(source_id) => {
                state.objects.get(&source_id).is_some_and(|source| {
                    source.zone == Zone::Battlefield
                        && source.attached_to == Some(AttachmentRecipient::Object(oid))
                })
            }
            _ => false,
        })
        .map(|effect| effect.timestamp)
        .max()
}

fn permanent_matches_target_scope(
    state: &GameState,
    filter: &TargetFilter,
    reference_player: PlayerId,
    source: Option<ObjectId>,
    oid: ObjectId,
    characteristics: &Characteristics,
) -> bool {
    if let Some(branches) = &filter.any_of {
        return branches.iter().any(|branch| {
            permanent_matches_target_scope(
                state,
                branch,
                reference_player,
                source,
                oid,
                characteristics,
            )
        });
    }
    let kind_matches = match filter.kind {
        TargetKind::Creature => characteristics.is_creature(),
        TargetKind::AnyPermanent => true,
        _ => false,
    };
    let controller_matches = match filter.controller {
        TargetController::Any => true,
        TargetController::You => characteristics.controller == reference_player,
        TargetController::Opponent => {
            state.are_opponents(characteristics.controller, reference_player)
        }
        TargetController::NotYou => characteristics.controller != reference_player,
        TargetController::DefendingPlayer => false,
    };
    (!filter
        .excluded_objects
        .contains(&tricerules_cards::TargetObjectExclusion::Source)
        || source != Some(oid))
        && kind_matches
        && controller_matches
        && permanent_matches_filter_characteristics(state, filter, oid, characteristics)
}

/// Characteristic predicates shared by targeted filters and dynamic rule-changing scopes. The
/// caller separately owns kind, controller, source exclusion, and targetability because those
/// differ between targeted and untargeted effects.
pub(super) fn permanent_matches_filter_characteristics(
    state: &GameState,
    filter: &TargetFilter,
    oid: ObjectId,
    characteristics: &Characteristics,
) -> bool {
    if let Some(branches) = &filter.any_of {
        return branches.iter().any(|branch| {
            permanent_matches_filter_characteristics(state, branch, oid, characteristics)
        });
    }
    let Some(object) = state.objects.get(&oid) else {
        return false;
    };
    if filter.token.is_some_and(|token| object.is_token() != token)
        || filter
            .min_mana_value
            .is_some_and(|min| characteristics.mana_value < min)
        || filter
            .max_mana_value
            .is_some_and(|max| characteristics.mana_value > max)
        || filter.excluded_permanent_types.iter().any(|kind| {
            characteristics.has_type(match kind {
                PermanentTypeFilter::Creature => "Creature",
                PermanentTypeFilter::Artifact => "Artifact",
                PermanentTypeFilter::Enchantment => "Enchantment",
                PermanentTypeFilter::Land => "Land",
                PermanentTypeFilter::Planeswalker => "Planeswalker",
                PermanentTypeFilter::Battle => "Battle",
            })
        })
        || filter.was_dealt_damage_this_turn.is_some_and(|required| {
            let generation = state.zone_change_generation.get(&oid).copied().unwrap_or(0);
            state
                .turn_history
                .current
                .damaged_objects
                .contains(&(oid, generation))
                != required
        })
    {
        return false;
    }
    if !filter.permanent_types.is_empty()
        && !filter.permanent_types.iter().any(|kind| match kind {
            PermanentTypeFilter::Creature => characteristics.is_creature(),
            PermanentTypeFilter::Artifact => characteristics.is_artifact(),
            PermanentTypeFilter::Enchantment => characteristics.has_type("Enchantment"),
            PermanentTypeFilter::Land => characteristics.has_type("Land"),
            PermanentTypeFilter::Planeswalker => characteristics.has_type("Planeswalker"),
            PermanentTypeFilter::Battle => characteristics.has_type("Battle"),
        })
    {
        return false;
    }
    if filter.not_land && characteristics.has_type("Land") {
        return false;
    }
    if !filter
        .required_subtypes
        .iter()
        .all(|subtype| characteristics.has_type(subtype))
    {
        return false;
    }
    if filter
        .excluded_subtypes
        .iter()
        .any(|subtype| characteristics.has_type(subtype))
    {
        return false;
    }
    if !filter
        .required_keywords
        .iter()
        .all(|keyword| characteristics.has_keyword(*keyword))
    {
        return false;
    }
    if filter
        .excluded_keywords
        .iter()
        .any(|keyword| characteristics.has_keyword(*keyword))
    {
        return false;
    }
    if let Some(comparison) = filter.power {
        let Some(power) = characteristics.power else {
            return false;
        };
        let matches = match comparison {
            PowerComparison::AtLeast(minimum) => power >= minimum,
            PowerComparison::AtMost(maximum) => power <= maximum,
        };
        if !matches {
            return false;
        }
    }
    if filter.not_artifact && characteristics.is_artifact() {
        return false;
    }
    if filter
        .tapped
        .is_some_and(|required| object.tapped != required)
    {
        return false;
    }
    if filter
        .not_color
        .is_some_and(|color| characteristics.colors.contains(&color))
    {
        return false;
    }
    if filter
        .is_color
        .is_some_and(|color| !characteristics.colors.contains(&color))
    {
        return false;
    }
    if let Some(role) = filter.combat_role {
        use tricerules_cards::CombatRole;
        let matches = match role {
            CombatRole::Attacking => super::combat::is_attacking(state, oid),
            CombatRole::Blocking => super::combat::is_blocking(state, oid),
            CombatRole::AttackingOrBlocking => super::combat::is_attacking_or_blocking(state, oid),
        };
        if !matches {
            return false;
        }
    }
    true
}

pub(super) fn creature_matches_scope(
    state: &GameState,
    _registry: &'static CardRegistry,
    filter: &CreatureScopeFilter,
    reference_player: PlayerId,
    exclude: Option<ObjectId>,
    oid: ObjectId,
    characteristics: &Characteristics,
) -> bool {
    let Some(object) = state.objects.get(&oid) else {
        return false;
    };
    let name_matches = filter
        .name
        .as_ref()
        .is_none_or(|required_name| characteristics.has_name(required_name));

    exclude != Some(oid)
        && (!filter.attacking || super::combat::is_attacking(state, oid))
        && match filter.controller {
            None => true,
            Some(CreatureScopeController::YouControl) => {
                characteristics.controller == reference_player
            }
            Some(CreatureScopeController::Opponents) => {
                state.are_opponents(characteristics.controller, reference_player)
            }
        }
        && characteristics.is_creature()
        && filter
            .subtype
            .as_ref()
            .is_none_or(|value| characteristics.has_type(value))
        && filter
            .color
            .is_none_or(|value| characteristics.colors.contains(&value))
        && name_matches
        && filter
            .required_counter
            .is_none_or(|counter| object.counter_count(counter) > 0)
}

impl CharacteristicsEvaluator<'_> {
    fn apply_layer_6_abilities(
        &self,
        object: &GameObject,
        result: &mut Characteristics,
        effects: &[&ContinuousEffect],
    ) {
        let mut last_removal_timestamp = None;
        for effect in effects {
            if matches!(effect.kind, ContinuousEffectKind::Layer6RemoveAllAbilities) {
                result.keywords.clear();
                result.protections.clear();
                result.evasions.clear();
                last_removal_timestamp = Some(effect.timestamp);
            }
            if let ContinuousEffectKind::Layer6AddKeyword(keyword) = effect.kind {
                if !result.keywords.contains(&keyword) {
                    result.keywords.push(keyword);
                }
            }
            if let ContinuousEffectKind::Layer6AddProtection(protection) = effect.kind {
                if !result.protections.contains(&protection) {
                    result.protections.push(protection);
                }
            }
        }
        // CR 613.1f / 122.1b: keyword counters grant abilities in timestamp order. A counter
        // created after the latest remove-all effect survives; an earlier one is removed.
        for (counter, count) in &object.counters {
            let CounterKind::Keyword(keyword) = counter else {
                continue;
            };
            let timestamp = object.counter_timestamps.get(counter).copied().unwrap_or(0);
            if *count > 0
                && last_removal_timestamp.is_none_or(|removal| timestamp > removal)
                && !result.keywords.contains(keyword)
            {
                result.keywords.push(*keyword);
            }
        }
    }

    fn apply_layer_7_power_toughness(
        &self,
        object: &GameObject,
        result: &mut Characteristics,
        effects: &[&ContinuousEffect],
    ) {
        // CR 613.4a/613.3: characteristic-defining abilities. None modeled yet.
        // CR 613.4b: apply setters in timestamp order; the last one wins.
        let mut power = result.power.map(i64::from);
        let mut toughness = result.toughness.map(i64::from);
        for effect in effects {
            if let ContinuousEffectKind::Layer7bSetPt {
                power: set_power,
                toughness: set_toughness,
            } = effect.kind
            {
                power = Some(set_power as i64);
                toughness = Some(set_toughness as i64);
            }
        }

        // CR 613.4c: modifying effects and P/T counters. Both are additive here, so applying the
        // counters after the other modifiers within the sublayer does not change the result.
        for effect in effects {
            if let ContinuousEffectKind::PtModify {
                delta_power,
                delta_toughness,
            } = effect.kind
            {
                if let Some(value) = &mut power {
                    *value = value.saturating_add(delta_power as i64);
                }
                if let Some(value) = &mut toughness {
                    *value = value.saturating_add(delta_toughness as i64);
                }
            }
            if let ContinuousEffectKind::PtModifyByCount {
                ref count,
                power_per_match,
                toughness_per_match,
            } = effect.kind
            {
                let Some(source_id) = effect.source_id else {
                    continue;
                };
                let source_controller = self.layer_2_controller(source_id, &mut Vec::new());
                let context = ConditionContext {
                    controller: source_controller,
                    source_object_id: source_id,
                    source_zone_change: self
                        .state
                        .zone_change_generation
                        .get(&source_id)
                        .copied()
                        .unwrap_or(0),
                    resolving_spell_id: None,
                    stack_item: None,
                };
                let count =
                    super::history::battlefield_quantity_value(self.state, count, context, |oid| {
                        self.characteristics_through_layer_5(oid)
                    })
                    .unwrap_or(0);
                if let Some(value) = &mut power {
                    *value = value.saturating_add((power_per_match as i64).saturating_mul(count));
                }
                if let Some(value) = &mut toughness {
                    *value =
                        value.saturating_add((toughness_per_match as i64).saturating_mul(count));
                }
            }
        }

        // +1/+1 and -1/-1 counters remain in layer 7c in the current rules.
        let counter_delta = object.counter_pt_delta();
        if let Some(value) = &mut power {
            *value = value.saturating_add(counter_delta as i64);
        }
        if let Some(value) = &mut toughness {
            *value = value.saturating_add(counter_delta as i64);
        }
        // CR 613.4d: P/T-switching effects. None modeled yet.

        result.signed_power = power;
        result.signed_toughness = toughness;
        result.power = power.map(|value| value.clamp(0, u32::MAX as i64) as u32);
        result.toughness = toughness.map(|value| value.clamp(0, u32::MAX as i64) as u32);
    }
}

impl GameEngine {
    /// Compute the rules-visible characteristics of `oid` through the ordered layer pipeline.
    pub fn characteristics(&self, oid: ObjectId) -> Option<Characteristics> {
        characteristics_from(&self.state, self.registry, oid)
    }

    /// Project an object through the copy, control, text, type, and color layers used by
    /// battlefield-entry replacement predicates (CR 614.12).
    pub(super) fn characteristics_through_layer_5(&self, oid: ObjectId) -> Option<Characteristics> {
        CharacteristicsEvaluator {
            state: &self.state,
            registry: self.registry,
        }
        .characteristics_through_layer_5(oid)
    }

    /// CR 110.2 controller of `oid`, through the layer pipeline.
    ///
    /// Prefer this over reading `GameObject::owner` anywhere the question is "whose permanent is
    /// this?" — owner and controller coincide only until a permanent changes hands. Hot loops that
    /// run per-permanent per-event may read the `controller` field directly instead (it is the
    /// layer-2 base value, identical while no continuous control effect exists) rather than paying
    /// for an unmemoized characteristics computation.
    pub(super) fn controller_of(&self, oid: ObjectId) -> Option<PlayerId> {
        self.characteristics(oid).map(|c| c.controller)
    }

    /// Compatibility query retained for scenario helpers and callers that only need power.
    pub fn effective_power(&self, oid: ObjectId) -> Option<u32> {
        self.characteristics(oid)?.power
    }

    /// Compatibility query retained for scenario helpers and callers that only need toughness.
    pub fn effective_toughness(&self, oid: ObjectId) -> Option<u32> {
        self.characteristics(oid)?.toughness
    }

    /// Compatibility query retained for callers that only need one keyword.
    pub fn effective_has_keyword(&self, oid: ObjectId, keyword: Keyword) -> bool {
        self.characteristics(oid)
            .is_some_and(|result| result.has_keyword(keyword))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tricerules_cards::{CharacteristicDefiningAbility, TypeLineAddition};

    fn install_changeling_face(engine: &mut GameEngine, face_down: bool) -> ObjectId {
        let oid = engine.state.players[0].library[0];
        let mut face = engine
            .registry
            .get("cavalry_drillmaster")
            .expect("Cavalry Drillmaster definition")
            .primary_face()
            .clone();
        face.characteristic_defining_abilities = vec![CharacteristicDefiningAbility::Changeling];
        let object = engine.state.objects.get_mut(&oid).expect("library object");
        object.zone = Zone::Battlefield;
        object.face_down = face_down;
        object.copiable_values = Some(CopiableValues {
            source_card_id: "cavalry_drillmaster".into(),
            source_face_index: 0,
            face,
            room_faces: None,
            display_name: "Cavalry Drillmaster".into(),
        });
        engine.state.players[0].battlefield.push(oid);
        oid
    }

    #[test]
    fn changeling_is_a_layer_4_cda_and_type_setting_overwrites_it() {
        let mut engine =
            GameEngine::new_with_default_decks(154_001, &[0, 1], 20).expect("new engine");
        let oid = install_changeling_face(&mut engine, false);

        let characteristics = engine.characteristics(oid).expect("changeling");
        assert!(characteristics.all_creature_types);
        assert!(characteristics.has_type("Goblin"));
        assert!(!characteristics.has_type("Forest"));

        engine.state.continuous_effects.push(ContinuousEffect {
            trigger_grant_origin: None,
            source_id: None,
            affected: AffectedScope::Single(oid),
            kind: ContinuousEffectKind::Layer4SetCreatureTypes(vec!["Frog".into()]),
            condition: None,
            duration: EffectDuration::UntilEndOfTurn,
            timestamp: 1,
        });

        let characteristics = engine.characteristics(oid).expect("type-set changeling");
        assert!(!characteristics.all_creature_types);
        assert!(characteristics.has_type("Frog"));
        assert!(!characteristics.has_type("Goblin"));
        assert!(characteristics.has_type("Creature"));

        engine.state.continuous_effects.push(ContinuousEffect {
            trigger_grant_origin: None,
            source_id: None,
            affected: AffectedScope::Single(oid),
            kind: ContinuousEffectKind::Layer4SetCreatureTypes(Vec::new()),
            condition: None,
            duration: EffectDuration::UntilEndOfTurn,
            timestamp: 2,
        });
        let characteristics = engine
            .characteristics(oid)
            .expect("Changeling with no creature types");
        assert!(!characteristics.all_creature_types);
        assert!(!characteristics.has_type("Frog"));
        assert!(!characteristics.has_type("Goblin"));
        assert!(characteristics.has_type("Creature"));
    }

    #[test]
    fn ability_removal_keeps_changeling_types_but_face_down_values_do_not() {
        let mut engine =
            GameEngine::new_with_default_decks(154_002, &[0, 1], 20).expect("new engine");
        let oid = install_changeling_face(&mut engine, false);
        engine.state.continuous_effects.push(ContinuousEffect {
            trigger_grant_origin: None,
            source_id: None,
            affected: AffectedScope::Single(oid),
            kind: ContinuousEffectKind::Layer6RemoveAllAbilities,
            condition: None,
            duration: EffectDuration::UntilEndOfTurn,
            timestamp: 1,
        });
        assert!(engine
            .characteristics(oid)
            .expect("ability-removed changeling")
            .has_type("Elf"));

        engine
            .state
            .objects
            .get_mut(&oid)
            .expect("changeling")
            .face_down = true;
        let face_down = engine.characteristics(oid).expect("face-down changeling");
        assert!(!face_down.all_creature_types);
        assert!(!face_down.has_type("Elf"));
    }

    #[test]
    fn layer_4_additions_are_ordered_deduplicated_and_feed_later_scopes() {
        let mut engine =
            GameEngine::new_with_default_decks(81_003, &[0, 1], 20).expect("new engine");
        let oid = engine.state.next_object_id;
        engine.state.next_object_id += 1;
        engine.state.objects.insert(
            oid,
            GameObject {
                id: oid,
                owner: 0,
                base_controller: 0,
                controller: 0,
                card_id: "cavalry_drillmaster".to_string(),
                token_origin: None,
                copiable_values: None,
                copy_revision: 0,
                zone: Zone::Battlefield,
                tapped: false,
                summoning_sick: false,
                power: Some(2),
                toughness: Some(2),
                damage: 0,
                deathtouch_damage: false,
                counters: BTreeMap::new(),
                counter_timestamps: BTreeMap::new(),
                attached_to: None,
                regeneration_shields: 0,
                must_attack_if_able: false,
                must_block_if_able: false,
                face_up_index: 0,
                face_down: false,
            },
        );
        engine.state.players[0].battlefield.push(oid);
        engine.state.continuous_effects.extend([
            ContinuousEffect {
                trigger_grant_origin: None,
                source_id: None,
                affected: AffectedScope::Single(oid),
                kind: ContinuousEffectKind::Layer4AddTypes(TypeLineAddition {
                    card_types: vec![PermanentTypeFilter::Artifact],
                    creature_types: vec!["Knight".to_string()],
                }),
                condition: None,
                duration: EffectDuration::UntilEndOfTurn,
                timestamp: 2,
            },
            ContinuousEffect {
                trigger_grant_origin: None,
                source_id: None,
                affected: AffectedScope::Single(oid),
                kind: ContinuousEffectKind::Layer4AddTypes(TypeLineAddition {
                    card_types: vec![PermanentTypeFilter::Enchantment],
                    creature_types: vec!["Knight".to_string()],
                }),
                condition: None,
                duration: EffectDuration::UntilEndOfTurn,
                timestamp: 1,
            },
            ContinuousEffect {
                trigger_grant_origin: None,
                source_id: None,
                affected: AffectedScope::Single(oid),
                kind: ContinuousEffectKind::Layer4AddTypes(TypeLineAddition {
                    card_types: vec![PermanentTypeFilter::Artifact],
                    creature_types: vec!["Knight".to_string()],
                }),
                condition: None,
                duration: EffectDuration::UntilEndOfTurn,
                timestamp: 2,
            },
            ContinuousEffect {
                trigger_grant_origin: None,
                source_id: None,
                affected: AffectedScope::CreaturesMatching {
                    reference_player: 0,
                    filter: CreatureScopeFilter {
                        subtype: Some("Knight".to_string()),
                        ..CreatureScopeFilter::default()
                    },
                    exclude: None,
                },
                kind: ContinuousEffectKind::PtModify {
                    delta_power: 1,
                    delta_toughness: 1,
                },
                condition: None,
                duration: EffectDuration::UntilEndOfTurn,
                timestamp: 3,
            },
        ]);

        let characteristics = engine.characteristics(oid).expect("bear characteristics");
        assert_eq!(
            characteristics.types,
            vec!["Creature", "Human", "Knight", "Enchantment", "Artifact"]
        );
        assert_eq!(
            characteristics
                .types
                .iter()
                .filter(|value| value.as_str() == "Knight")
                .count(),
            1
        );
        assert_eq!(engine.effective_power(oid), Some(3));
        assert_eq!(engine.effective_toughness(oid), Some(3));
    }

    #[test]
    fn one_snapshot_applies_types_colors_layer_6_and_layer_7() {
        let mut engine = GameEngine::new_with_default_decks(613, &[0, 1], 20).expect("new engine");
        let oid = engine.state.next_object_id;
        engine.state.next_object_id += 1;
        engine.state.objects.insert(
            oid,
            GameObject {
                id: oid,
                owner: 0,
                base_controller: 0,
                controller: 0,
                card_id: "grizzly_bears".to_string(),
                token_origin: None,
                copiable_values: None,
                copy_revision: 0,
                zone: Zone::Battlefield,
                tapped: false,
                summoning_sick: false,
                power: Some(2),
                toughness: Some(2),
                damage: 0,
                deathtouch_damage: false,
                counters: BTreeMap::from([(CounterKind::PlusOnePlusOne, 1)]),
                counter_timestamps: BTreeMap::from([(CounterKind::PlusOnePlusOne, 0)]),
                attached_to: None,
                regeneration_shields: 0,
                must_attack_if_able: false,
                must_block_if_able: false,
                face_up_index: 0,
                face_down: false,
            },
        );
        engine.state.continuous_effects.extend([
            ContinuousEffect {
                trigger_grant_origin: None,
                source_id: None,
                affected: AffectedScope::AllCreatures,
                kind: ContinuousEffectKind::PtModify {
                    delta_power: 2,
                    delta_toughness: 1,
                },
                condition: None,
                duration: EffectDuration::UntilEndOfTurn,
                timestamp: 2,
            },
            ContinuousEffect {
                trigger_grant_origin: None,
                source_id: None,
                affected: AffectedScope::AllCreatures,
                kind: ContinuousEffectKind::Layer6AddKeyword(Keyword::Haste),
                condition: None,
                duration: EffectDuration::UntilEndOfTurn,
                timestamp: 1,
            },
        ]);

        let characteristics = engine.characteristics(oid).expect("characteristics");
        assert_eq!(characteristics.controller, 0);
        assert!(characteristics.is_creature());
        assert!(characteristics.types.contains(&"Bear".to_string()));
        assert_eq!(characteristics.colors, vec![Color::Green]);
        assert!(characteristics.has_keyword(Keyword::Haste));
        assert_eq!(characteristics.power, Some(5));
        assert_eq!(characteristics.toughness, Some(4));
    }

    #[test]
    fn layer_2_control_changes_the_derived_controller() {
        let mut engine = GameEngine::new_with_default_decks(614, &[0, 1], 20).expect("new engine");
        let oid = engine.state.next_object_id;
        engine.state.next_object_id += 1;
        engine.state.objects.insert(
            oid,
            GameObject {
                id: oid,
                owner: 0,
                base_controller: 0,
                controller: 0,
                card_id: "grizzly_bears".to_string(),
                token_origin: None,
                copiable_values: None,
                copy_revision: 0,
                zone: Zone::Battlefield,
                tapped: false,
                summoning_sick: false,
                power: Some(2),
                toughness: Some(2),
                damage: 0,
                deathtouch_damage: false,
                counters: BTreeMap::new(),
                counter_timestamps: BTreeMap::new(),
                attached_to: None,
                regeneration_shields: 0,
                must_attack_if_able: false,
                must_block_if_able: false,
                face_up_index: 0,
                face_down: false,
            },
        );
        engine.state.continuous_effects.push(ContinuousEffect {
            trigger_grant_origin: None,
            source_id: None,
            affected: AffectedScope::Single(oid),
            kind: ContinuousEffectKind::Layer2Control {
                controller: ControllerReference::Fixed(1),
            },
            condition: None,
            duration: EffectDuration::UntilEndOfTurn,
            timestamp: 0,
        });

        assert_eq!(
            engine
                .characteristics(oid)
                .expect("characteristics")
                .controller,
            1
        );
    }

    #[test]
    fn source_controller_dependency_is_evaluated_before_attached_control() {
        let mut engine = GameEngine::new_with_default_decks(615, &[0, 1], 20).expect("new engine");
        let make_object = |id, owner, attached_to| GameObject {
            id,
            owner,
            base_controller: owner,
            controller: owner,
            card_id: "grizzly_bears".to_string(),
            token_origin: None,
            copiable_values: None,
            copy_revision: 0,
            zone: Zone::Battlefield,
            tapped: false,
            summoning_sick: false,
            power: Some(2),
            toughness: Some(2),
            damage: 0,
            deathtouch_damage: false,
            counters: BTreeMap::new(),
            counter_timestamps: BTreeMap::new(),
            attached_to,
            regeneration_shields: 0,
            must_attack_if_able: false,
            must_block_if_able: false,
            face_up_index: 0,
            face_down: false,
        };
        let target = engine.state.next_object_id;
        let control_aura = target + 1;
        let aura_thief = target + 2;
        engine.state.next_object_id += 3;
        engine
            .state
            .objects
            .insert(target, make_object(target, 0, None));
        engine.state.objects.insert(
            control_aura,
            make_object(control_aura, 0, Some(AttachmentRecipient::Object(target))),
        );
        engine.state.objects.insert(
            aura_thief,
            make_object(
                aura_thief,
                1,
                Some(AttachmentRecipient::Object(control_aura)),
            ),
        );
        engine.state.continuous_effects.extend([
            ContinuousEffect {
                trigger_grant_origin: None,
                source_id: Some(control_aura),
                affected: AffectedScope::AttachedTo(control_aura),
                kind: ContinuousEffectKind::Layer2Control {
                    controller: ControllerReference::SourceController,
                },
                condition: None,
                duration: EffectDuration::WhileSourceOnBattlefield,
                timestamp: 1,
            },
            ContinuousEffect {
                trigger_grant_origin: None,
                source_id: Some(aura_thief),
                affected: AffectedScope::AttachedTo(aura_thief),
                kind: ContinuousEffectKind::Layer2Control {
                    controller: ControllerReference::SourceController,
                },
                condition: None,
                duration: EffectDuration::WhileSourceOnBattlefield,
                timestamp: 2,
            },
        ]);

        assert_eq!(engine.controller_of(control_aura), Some(1));
        assert_eq!(engine.controller_of(target), Some(1));
    }

    #[test]
    fn later_layer_2_effect_wins_and_earlier_effect_resumes() {
        let mut engine = GameEngine::new_with_default_decks(616, &[0, 1], 20).expect("new engine");
        let oid = engine.state.next_object_id;
        engine.state.next_object_id += 1;
        engine.state.objects.insert(
            oid,
            GameObject {
                id: oid,
                owner: 0,
                base_controller: 0,
                controller: 0,
                card_id: "grizzly_bears".to_string(),
                token_origin: None,
                copiable_values: None,
                copy_revision: 0,
                zone: Zone::Battlefield,
                tapped: false,
                summoning_sick: false,
                power: Some(2),
                toughness: Some(2),
                damage: 0,
                deathtouch_damage: false,
                counters: BTreeMap::new(),
                counter_timestamps: BTreeMap::new(),
                attached_to: None,
                regeneration_shields: 0,
                must_attack_if_able: false,
                must_block_if_able: false,
                face_up_index: 0,
                face_down: false,
            },
        );
        for (timestamp, controller) in [(1, 1), (2, 0)] {
            engine.state.continuous_effects.push(ContinuousEffect {
                trigger_grant_origin: None,
                source_id: None,
                affected: AffectedScope::Single(oid),
                kind: ContinuousEffectKind::Layer2Control {
                    controller: ControllerReference::Fixed(controller),
                },
                condition: None,
                duration: EffectDuration::UntilEndOfTurn,
                timestamp,
            });
        }
        assert_eq!(engine.controller_of(oid), Some(0));
        engine.state.continuous_effects.pop();
        assert_eq!(engine.controller_of(oid), Some(1));
    }

    #[test]
    fn issue_75_static_opponent_scope_tracks_the_sources_current_controller() {
        let mut engine =
            GameEngine::new_with_default_decks(75_003, &[0, 1], 20).expect("new engine");
        let make_object = |id: ObjectId,
                           controller: PlayerId,
                           card_id: &str,
                           power: Option<u32>,
                           toughness: Option<u32>| GameObject {
            id,
            owner: controller,
            base_controller: controller,
            controller,
            card_id: card_id.to_string(),
            token_origin: None,
            copiable_values: None,
            copy_revision: 0,
            zone: Zone::Battlefield,
            tapped: false,
            summoning_sick: false,
            power,
            toughness,
            damage: 0,
            deathtouch_damage: false,
            counters: BTreeMap::new(),
            counter_timestamps: BTreeMap::new(),
            attached_to: None,
            regeneration_shields: 0,
            must_attack_if_able: false,
            must_block_if_able: false,
            face_up_index: 0,
            face_down: false,
        };
        let source = engine.state.next_object_id;
        let mine = source + 1;
        let theirs = source + 2;
        let late = source + 3;
        engine.state.next_object_id += 4;
        engine.state.objects.insert(
            source,
            make_object(source, 0, "glorious_anthem", None, None),
        );
        engine.state.objects.insert(
            mine,
            make_object(mine, 0, "grizzly_bears", Some(2), Some(2)),
        );
        engine.state.objects.insert(
            theirs,
            make_object(theirs, 1, "grizzly_bears", Some(2), Some(2)),
        );
        engine.state.players[0].battlefield.extend([source, mine]);
        engine.state.players[1].battlefield.push(theirs);
        engine.state.continuous_effects.push(ContinuousEffect {
            trigger_grant_origin: None,
            source_id: Some(source),
            affected: AffectedScope::CreaturesMatching {
                reference_player: 0,
                filter: CreatureScopeFilter {
                    controller: Some(CreatureScopeController::Opponents),
                    ..CreatureScopeFilter::default()
                },
                exclude: None,
            },
            kind: ContinuousEffectKind::PtModify {
                delta_power: -1,
                delta_toughness: 0,
            },
            condition: None,
            duration: EffectDuration::WhileSourceOnBattlefield,
            timestamp: 0,
        });

        assert_eq!(engine.effective_power(mine), Some(2));
        assert_eq!(engine.effective_power(theirs), Some(1));
        engine.state.objects.insert(
            late,
            make_object(late, 1, "grizzly_bears", Some(2), Some(2)),
        );
        engine.state.players[1].battlefield.push(late);
        assert_eq!(
            engine.effective_power(late),
            Some(1),
            "static scopes include later qualifying entrants"
        );

        engine.state.players[0]
            .battlefield
            .retain(|oid| *oid != source);
        engine.state.players[1].battlefield.push(source);
        let source_object = engine.state.objects.get_mut(&source).expect("source");
        source_object.base_controller = 1;
        source_object.controller = 1;
        assert_eq!(
            engine.effective_power(mine),
            Some(1),
            "the old controller is now the source controller's opponent"
        );
        assert_eq!(engine.effective_power(theirs), Some(2));
        assert_eq!(engine.effective_power(late), Some(2));
    }
}
