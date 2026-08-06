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
//! the `ordered_effects` boundary is the insertion point. Replacement-effect choice ordering
//! (CR 616) is a separate pipeline and is likewise deferred.
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
        let face = definition.face(object.face_up_index)?;

        let mut result = Characteristics {
            // CR 110.2 base value: the controller recorded on the object, set when it entered the
            // battlefield. Layer 2 below applies control-*changing* continuous effects on top.
            controller: object.controller,
            types: face.types.to_vec(),
            supertypes: face.supertypes.to_vec(),
            colors: face.colors(),
            keywords: face.keywords.to_vec(),
            evasions: face.evasions.to_vec(),
            // Object snapshots take precedence for tokens and scenario overrides. Multi-face
            // objects leave these unset and read the active face.
            power: object.power.or(face.power),
            toughness: object.toughness.or(face.toughness),
        };

        self.apply_layer_1_copy(&mut result);
        self.apply_layer_2_control(&mut result);
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
    fn apply_layer_1_copy(&self, _result: &mut Characteristics) {}

    /// CR 613 layer 2 — control-changing *continuous* effects (Mind Control, Threaten,
    /// Confiscate). Still empty: the only control changes modelled so far are decided when a
    /// permanent enters the battlefield, and those live in `GameObject::controller`, which is
    /// this layer's base value.
    ///
    /// When the first such card lands, note two things this slot cannot do naively:
    /// 1. it must **not** use [`GameEngine::ordered_effects`] — that is computed after layer 5 and
    ///    its `effect_affects` reads `pre_layer_6.controller`, which is circular for a
    ///    `CreaturesMatching { controller }` scope (this is exactly CR 613.8 dependency
    ///    ordering). It needs its own earlier pass over `AffectedScope::Single` effects, sorted by
    ///    the same `(timestamp, index)` key;
    /// 2. once the derived controller can differ from `GameObject::controller`, the battlefield
    ///    lists stop being a valid control index, so they must be rebuilt whenever
    ///    `continuous_effects` changes. Until then the `debug_assert` in `apply_sbas` holds the
    ///    two in sync.
    fn apply_layer_2_control(&self, _result: &mut Characteristics) {}

    fn apply_layer_3_text(&self, _result: &mut Characteristics) {}

    fn apply_layer_4_type(&self, _result: &mut Characteristics) {}

    fn apply_layer_5_color(&self, _result: &mut Characteristics) {}

    /// Active effects in CR 613.7 timestamp order. The original vector index makes equal
    /// timestamps deterministic. CR 613.8 dependency ordering will be inserted here.
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
            .filter(|(_, effect)| effect_affects(self.state, effect, oid, pre_layer_6))
            .collect();
        effects.sort_by_key(|(index, effect)| (effect.timestamp, *index));
        effects.into_iter().map(|(_, effect)| effect).collect()
    }
}

/// Whether an effect applies, evaluated from the relevant characteristic snapshot. Current scopes
/// only depend on controller, types, and colors, avoiding recursive full-characteristic queries.
/// Dependency ordering becomes necessary once scopes can depend on values changed in their layer.
pub(super) fn effect_affects(
    state: &GameState,
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
            controller,
            subtype,
            color,
            exclude,
        } => {
            *exclude != Some(oid)
                && controller.is_none_or(|pid| characteristics.controller == pid)
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
                controller: 0,
                card_id: "grizzly_bears".to_string(),
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
}
