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
//! Layers 1-5 and the unimplemented layer-7 sublayers are explicit identity stages today.
//! CR 613.8 dependency ordering is intentionally deferred until the first effect that needs it;
//! the `ordered_effects` boundary is the insertion point. Replacement/prevention choice ordering
//! (CR 616) is the separate shared pipeline in `engine/replacement.rs`.
//!
//! The calculation is side-effect-free and depends only on `GameState`, the registry, and the
//! queried object id. Its owned result and single ordered-effect pass make it straightforward
//! to memoize later without changing callers.

use super::*;

/// The complete rules-visible characteristic snapshot currently modeled for a permanent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Characteristics {
    pub controller: PlayerId,
    pub types: Vec<String>,
    pub supertypes: Vec<String>,
    pub colors: Vec<Color>,
    pub keywords: Vec<Keyword>,
    pub evasions: Vec<Evasion>,
    pub power: Option<u32>,
    pub toughness: Option<u32>,
}

impl Characteristics {
    pub fn has_type(&self, card_type: &str) -> bool {
        self.types.iter().any(|t| t == card_type)
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
        let definition = self.registry.get(&object.card_id)?;
        let copied = object.copiable_values.as_ref();
        let face = copied
            .map(|values| &values.face)
            .or_else(|| definition.face(object.face_up_index))?;

        let mut result = Characteristics {
            // CR 110.2 base value set by the instruction that put the object onto the battlefield.
            // Layer 2 below applies control-changing continuous effects on top.
            controller: object.base_controller,
            types: face.types.to_vec(),
            supertypes: face.supertypes.to_vec(),
            colors: if copied.is_none()
                && definition.layout == Layout::Flip
                && object.face_up_index > 0
            {
                definition.primary_face().colors()
            } else {
                face.colors()
            },
            keywords: face.keywords.to_vec(),
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
        self.apply_layer_2_control(oid, &mut result);
        self.apply_layer_3_text(&mut result);
        self.apply_layer_4_type(&mut result);
        self.apply_layer_5_color(&mut result);

        let ordered_effects = self.ordered_effects(oid, &result);
        self.apply_layer_6_abilities(&mut result, &ordered_effects);
        self.apply_layer_7_power_toughness(object, &mut result, &ordered_effects);
        Some(result)
    }

    // These identity stages are intentionally separate: adding the first effect in a layer must
    // fill its existing slot rather than creating another characteristics path.
    fn apply_layer_1_copy(&self, _result: &mut Characteristics) {
        // The owned snapshot was selected above as the base printed face. Keeping this named
        // stage makes the CR 613 order explicit while avoiding a second characteristics path.
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
                                source.zone == Zone::Battlefield && source.attached_to == Some(oid)
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

    fn apply_layer_3_text(&self, _result: &mut Characteristics) {}

    fn apply_layer_4_type(&self, _result: &mut Characteristics) {}

    fn apply_layer_5_color(&self, _result: &mut Characteristics) {}

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
                effect_affects(self.state, self.registry, effect, oid, pre_layer_6)
            })
            .collect();
        effects.sort_by_key(|(index, effect)| (effect.timestamp, *index));
        effects.into_iter().map(|(_, effect)| effect).collect()
    }
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
    match &effect.affected {
        AffectedScope::Single(id) => *id == oid,
        AffectedScope::AllCreatures => characteristics.is_creature(),
        AffectedScope::AttachedTo(source_oid) => {
            state.objects.get(source_oid).is_some_and(|attachment| {
                attachment.zone == Zone::Battlefield && attachment.attached_to == Some(oid)
            })
        }
        AffectedScope::CreaturesMatching {
            players,
            reference_player,
            subtype,
            color,
            exclude,
            attacking,
        } => {
            let current_reference = if *players != RelativePlayerSet::All
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
            *exclude != Some(oid)
                && (!attacking || super::combat::is_attacking(state, oid))
                && match players {
                    RelativePlayerSet::Controller => {
                        characteristics.controller == current_reference
                    }
                    RelativePlayerSet::Opponents => {
                        state.are_opponents(characteristics.controller, current_reference)
                    }
                    RelativePlayerSet::All => true,
                }
                && characteristics.is_creature()
                && subtype
                    .as_ref()
                    .is_none_or(|value| characteristics.types.contains(value))
                && color.is_none_or(|value| characteristics.colors.contains(&value))
        }
        AffectedScope::Player(_) => false,
    }
}

impl CharacteristicsEvaluator<'_> {
    fn apply_layer_6_abilities(&self, result: &mut Characteristics, effects: &[&ContinuousEffect]) {
        for effect in effects {
            if let ContinuousEffectKind::Layer6AddKeyword(keyword) = effect.kind {
                if !result.keywords.contains(&keyword) {
                    result.keywords.push(keyword);
                }
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
        // CR 613.4b: P/T-setting effects. None modeled yet.
        let mut power = result.power.map(|value| value as i32);
        let mut toughness = result.toughness.map(|value| value as i32);

        // CR 613.4c: modifying effects.
        for effect in effects {
            if let ContinuousEffectKind::PtModify {
                delta_power,
                delta_toughness,
            } = effect.kind
            {
                if let Some(value) = &mut power {
                    *value += delta_power;
                }
                if let Some(value) = &mut toughness {
                    *value += delta_toughness;
                }
            }
        }

        // CR 613.4d: +1/+1 and -1/-1 counters.
        let counter_delta = object.counter_pt_delta();
        if let Some(value) = &mut power {
            *value += counter_delta;
        }
        if let Some(value) = &mut toughness {
            *value += counter_delta;
        }
        // CR 613.4e: P/T-switching effects. None modeled yet.

        result.power = power.map(|value| value.max(0) as u32);
        result.toughness = toughness.map(|value| value.max(0) as u32);
    }
}

impl GameEngine {
    /// Compute the rules-visible characteristics of `oid` through the ordered layer pipeline.
    pub fn characteristics(&self, oid: ObjectId) -> Option<Characteristics> {
        characteristics_from(&self.state, self.registry, oid)
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
                attached_to: None,
                regeneration_shields: 0,
                must_attack_if_able: false,
                must_block_if_able: false,
                face_up_index: 0,
                adventure_cast_permission: None,
            },
        );
        engine.state.continuous_effects.extend([
            ContinuousEffect {
                source_id: None,
                affected: AffectedScope::AllCreatures,
                kind: ContinuousEffectKind::PtModify {
                    delta_power: 2,
                    delta_toughness: 1,
                },
                duration: EffectDuration::UntilEndOfTurn,
                timestamp: 2,
            },
            ContinuousEffect {
                source_id: None,
                affected: AffectedScope::AllCreatures,
                kind: ContinuousEffectKind::Layer6AddKeyword(Keyword::Haste),
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
                attached_to: None,
                regeneration_shields: 0,
                must_attack_if_able: false,
                must_block_if_able: false,
                face_up_index: 0,
                adventure_cast_permission: None,
            },
        );
        engine.state.continuous_effects.push(ContinuousEffect {
            source_id: None,
            affected: AffectedScope::Single(oid),
            kind: ContinuousEffectKind::Layer2Control {
                controller: ControllerReference::Fixed(1),
            },
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
            attached_to,
            regeneration_shields: 0,
            must_attack_if_able: false,
            must_block_if_able: false,
            face_up_index: 0,
            adventure_cast_permission: None,
        };
        let target = engine.state.next_object_id;
        let control_aura = target + 1;
        let aura_thief = target + 2;
        engine.state.next_object_id += 3;
        engine
            .state
            .objects
            .insert(target, make_object(target, 0, None));
        engine
            .state
            .objects
            .insert(control_aura, make_object(control_aura, 0, Some(target)));
        engine
            .state
            .objects
            .insert(aura_thief, make_object(aura_thief, 1, Some(control_aura)));
        engine.state.continuous_effects.extend([
            ContinuousEffect {
                source_id: Some(control_aura),
                affected: AffectedScope::AttachedTo(control_aura),
                kind: ContinuousEffectKind::Layer2Control {
                    controller: ControllerReference::SourceController,
                },
                duration: EffectDuration::WhileSourceOnBattlefield,
                timestamp: 1,
            },
            ContinuousEffect {
                source_id: Some(aura_thief),
                affected: AffectedScope::AttachedTo(aura_thief),
                kind: ContinuousEffectKind::Layer2Control {
                    controller: ControllerReference::SourceController,
                },
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
                attached_to: None,
                regeneration_shields: 0,
                must_attack_if_able: false,
                must_block_if_able: false,
                face_up_index: 0,
                adventure_cast_permission: None,
            },
        );
        for (timestamp, controller) in [(1, 1), (2, 0)] {
            engine.state.continuous_effects.push(ContinuousEffect {
                source_id: None,
                affected: AffectedScope::Single(oid),
                kind: ContinuousEffectKind::Layer2Control {
                    controller: ControllerReference::Fixed(controller),
                },
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
            attached_to: None,
            regeneration_shields: 0,
            must_attack_if_able: false,
            must_block_if_able: false,
            face_up_index: 0,
            adventure_cast_permission: None,
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
            source_id: Some(source),
            affected: AffectedScope::CreaturesMatching {
                players: RelativePlayerSet::Opponents,
                reference_player: 0,
                subtype: None,
                color: None,
                exclude: None,
                attacking: false,
            },
            kind: ContinuousEffectKind::PtModify {
                delta_power: -1,
                delta_toughness: 0,
            },
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
