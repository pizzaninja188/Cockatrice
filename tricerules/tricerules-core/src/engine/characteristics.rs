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
//! CR 613.8 dependency ordering is modeled for layer-4 type-changing effects using a pure
//! evolving battlefield snapshot. Replacement/prevention choice ordering
//! (CR 616) is the separate shared pipeline in `engine/replacement.rs`.
//!
//! The calculation is side-effect-free and depends only on `GameState`, the registry, and the
//! queried object id. Its owned result and single ordered-effect pass make it straightforward
//! to memoize later without changing callers.

use super::history::{
    battlefield_quantity_value, graveyard_aggregate_value, graveyard_named_card_count,
    player_life_aggregate_value, relative_player_set_contains,
};
use super::*;

/// The complete rules-visible characteristic snapshot currently modeled for a permanent.
#[derive(serde::Serialize, Debug, Clone, PartialEq, Eq)]
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

pub(super) fn apply_type_line_replacement(
    characteristics: &mut Characteristics,
    replacement: &tricerules_cards::primitives::TypeLineReplacement,
) {
    characteristics.all_creature_types = false;
    characteristics.types.clear();
    characteristics.types.extend(
        replacement
            .card_types
            .iter()
            .map(|card_type| card_type.as_str().to_string()),
    );
    characteristics
        .types
        .extend(replacement.creature_types.iter().cloned());
    characteristics.types.extend(
        replacement
            .land_types
            .iter()
            .map(|land_type| land_type.as_str().to_string()),
    );
}

pub(super) fn apply_creature_type_removal(characteristics: &mut Characteristics) {
    characteristics.types.retain(|value| value != "Creature");
    if !characteristics.has_type("Kindred") {
        characteristics.all_creature_types = false;
        characteristics
            .types
            .retain(|value| !is_creature_type(value));
    }
}

pub(super) fn apply_type_line_addition(
    characteristics: &mut Characteristics,
    addition: &tricerules_cards::TypeLineAddition,
) {
    for card_type in &addition.card_types {
        let card_type = card_type.as_str();
        if !characteristics
            .types
            .iter()
            .any(|existing| existing == card_type)
        {
            characteristics.types.push(card_type.to_string());
        }
    }
    if characteristics.is_creature() || characteristics.has_type("Kindred") {
        for creature_type in &addition.creature_types {
            if !characteristics.types.contains(creature_type) {
                characteristics.types.push(creature_type.clone());
            }
        }
    }
    if characteristics.has_type("Land") {
        for land_type in &addition.land_types {
            if !characteristics.has_type(land_type.as_str()) {
                characteristics.types.push(land_type.as_str().to_string());
            }
        }
    }
}

mod early_layers;
use early_layers::EarlyLayerView;

struct CharacteristicsEvaluator<'a> {
    state: &'a GameState,
    registry: &'static CardRegistry,
}

/// CR 700.5a: devotion reads only copy/face-down mana costs and layer-2 control. Calling the
/// full characteristics evaluator here would recurse through the God's own type condition.
pub(super) fn devotion_value(
    state: &GameState,
    registry: &'static CardRegistry,
    controller: PlayerId,
    color: Color,
    excluded_entrant: Option<ObjectId>,
) -> u32 {
    use tricerules_cards::ManaSymbol;
    let evaluator = CharacteristicsEvaluator { state, registry };
    state
        .objects
        .iter()
        .filter(|(oid, object)| {
            object.zone == Zone::Battlefield
                && excluded_entrant != Some(**oid)
                && !object.face_down
                && evaluator.layer_2_controller(**oid, &mut Vec::new()) == controller
        })
        .filter_map(|(&oid, object)| {
            let face = effective_face_from(state, registry, oid)?;
            let retained_flip_cost = (object.copiable_values.is_none()
                && object.token_origin.is_none())
            .then(|| registry.get(&object.card_id))
            .flatten()
            .filter(|definition| definition.layout == Layout::Flip)
            .map(|definition| &definition.primary_face().mana_cost);
            let cost = retained_flip_cost.unwrap_or(&face.mana_cost);
            Some(
                cost.pips
                    .iter()
                    .filter(|pip| match pip {
                        ManaSymbol::W => color == Color::White,
                        ManaSymbol::U => color == Color::Blue,
                        ManaSymbol::B => color == Color::Black,
                        ManaSymbol::R => color == Color::Red,
                        ManaSymbol::G => color == Color::Green,
                        ManaSymbol::Hybrid(first, second) => {
                            first.color() == color || second.color() == color
                        }
                        ManaSymbol::MonoHybrid(_, pip) | ManaSymbol::Phyrexian(pip) => {
                            pip.color() == color
                        }
                        ManaSymbol::C | ManaSymbol::Generic(_) | ManaSymbol::X => false,
                    })
                    .count() as u32,
            )
        })
        .fold(0, u32::saturating_add)
}

pub(super) fn characteristics_from(
    state: &GameState,
    registry: &'static CardRegistry,
    oid: ObjectId,
) -> Option<Characteristics> {
    CharacteristicsEvaluator { state, registry }.characteristics(oid)
}

pub(super) fn entry_characteristics_through_layer_5(
    state: &GameState,
    registry: &'static CardRegistry,
    event: &BattlefieldEntryEvent,
    face: &CardFace,
    statics: &[(AbilityDefinitionId, &StaticAbilityDef)],
    effects: &[ContinuousEffect],
) -> Option<(Characteristics, bool)> {
    CharacteristicsEvaluator { state, registry }
        .evaluate_entry_layers(event, face, statics, effects)
}

/// CR 601.2f: characteristics of a proposed spell while its cost is determined. Legal-action
/// generation evaluates the card before its physical object is moved to the stack, so start from
/// the selected face and apply the type/color layers that the current effect model allows to
/// affect that exact object. Battlefield permanent scopes do not affect a spell candidate.
pub(super) fn spell_cast_characteristics(
    state: &GameState,
    registry: &'static CardRegistry,
    oid: ObjectId,
    controller: PlayerId,
    face: &CardFace,
) -> Characteristics {
    let mut characteristics = Characteristics {
        mana_value: face.mana_cost.mana_value_on_stack(0),
        controller,
        names: vec![face.name.clone()],
        types: face.types.clone(),
        all_creature_types: face
            .characteristic_defining_abilities
            .iter()
            .any(|ability| {
                matches!(
                    &ability.definition,
                    CharacteristicDefiningAbility::Changeling
                )
            }),
        supertypes: face.supertypes.clone(),
        colors: face.colors(),
        keywords: face.keywords.clone(),
        protections: face.protections.clone(),
        evasions: face.evasions.clone(),
        power: face.power,
        toughness: face.toughness,
        signed_power: face.power.map(i64::from),
        signed_toughness: face.toughness.map(i64::from),
    };
    let evaluator = CharacteristicsEvaluator { state, registry };
    evaluator.apply_layer_4_type(oid, &mut characteristics, &mut Vec::new());
    evaluator.apply_layer_5_color(oid, &mut characteristics, &mut Vec::new());
    characteristics
}

/// CR 105.2/707.10: a spell uses its selected face, even when it is a copy with no
/// backing GameObject. Only effects that actually affect this stack object apply;
/// battlefield creature/permanent scopes must not color a creature/permanent spell.
pub(super) fn stack_spell_colors(
    state: &GameState,
    registry: &'static CardRegistry,
    item: &StackItem,
) -> Option<Vec<Color>> {
    let face = registry.get(&item.card_id)?.face(item.face_index)?;
    let mut characteristics = Characteristics {
        mana_value: face.mana_cost.mana_value_on_stack(item.chosen_x),
        controller: item.controller,
        names: vec![face.name.clone()],
        types: face.types.clone(),
        all_creature_types: false,
        supertypes: face.supertypes.clone(),
        colors: face.colors(),
        keywords: face.keywords.clone(),
        protections: Vec::new(),
        evasions: Vec::new(),
        power: None,
        toughness: None,
        signed_power: None,
        signed_toughness: None,
    };
    CharacteristicsEvaluator { state, registry }.apply_layer_5_color(
        item.id,
        &mut characteristics,
        &mut Vec::new(),
    );
    Some(characteristics.colors)
}

impl CharacteristicsEvaluator<'_> {
    fn characteristics(&self, oid: ObjectId) -> Option<Characteristics> {
        let object = self.state.objects.get(&oid)?;
        let mut result = self.characteristics_through_layer_5(oid)?;

        let pre_layer_6 = result.clone();
        let layer_6_effects = self.ordered_layer_6_effects(oid, &pre_layer_6);
        self.apply_layer_6_abilities(object, &mut result, &layer_6_effects);
        // Layer-7 scopes may inspect current keywords. Resolve their membership only after all
        // layer-6 additions/removals have been applied; a keyword grant in layer 6 and a P/T
        // modifier in layer 7 are in different layers, so this adds no 613.8 dependency.
        let mut ordered_effects = layer_6_effects;
        ordered_effects.extend(self.ordered_layer_7_effects(oid, &result, &pre_layer_6));
        self.apply_layer_7_power_toughness(oid, object, &mut result, &ordered_effects);
        Some(result)
    }

    /// Evaluate a public, dependency-free count for a static P/T scaling modifier inside the
    /// layer pipeline. Battlefield counts use pre-layer-7 derived characteristics, so a layer-7
    /// effect cannot recurse into itself; graveyard counts use printed public card data
    /// (CR 404.2, 613.8). Mirrors `CountExpression::validate_static_count`.
    fn static_scaling_count(
        &self,
        expression: &CountExpression,
        context: ConditionContext<'_>,
    ) -> Option<i64> {
        match expression {
            CountExpression::CardsInHand {
                players: ConditionPlayerSet::Relative(RelativePlayerSet::Controller),
            } => {
                // CR 109.5: "you" means the current controller on the battlefield/stack,
                // and the owner in a zone where the card has no controller. Stack objects
                // retain their owner as base_controller, so read the actual spell controller.
                let object = self.state.objects.get(&context.source_object_id)?;
                let player = match object.zone {
                    Zone::Battlefield => context.controller,
                    Zone::Stack => self
                        .state
                        .stack
                        .iter()
                        .find(|item| item.id == object.id)
                        .map(|item| item.controller)
                        .or_else(|| {
                            self.state
                                .pending_spell_cast
                                .as_ref()
                                .filter(|pending| pending.reserved_object_id == object.id)
                                .map(|pending| pending.caster)
                        })
                        .unwrap_or(context.controller),
                    _ => object.owner,
                };
                self.state
                    .players
                    .iter()
                    .find(|candidate| candidate.id == player && !candidate.has_lost)
                    .map(|candidate| {
                        i64::from(super::history::clamp_public_count(candidate.hand.len()))
                    })
            }
            CountExpression::BattlefieldPermanents { .. }
            | CountExpression::BattlefieldCreatures { .. }
            | CountExpression::BattlefieldMaximum { .. } => {
                battlefield_quantity_value(self.state, expression, context, |oid| {
                    self.characteristics_through_layer_5(oid)
                })
            }
            CountExpression::GraveyardCards { owners, filter } => {
                Some(i64::from(graveyard_aggregate_value(
                    self.state,
                    self.registry,
                    *owners,
                    GraveyardAggregate::CardCount,
                    filter.as_ref(),
                    context.controller,
                    context.resolving_spell_id,
                )))
            }
            CountExpression::GraveyardCardsNamed { owners, name } => {
                Some(i64::from(graveyard_named_card_count(
                    self.state,
                    self.registry,
                    *owners,
                    name,
                    context.controller,
                    context.resolving_spell_id,
                )))
            }
            CountExpression::Affine { constant, terms } => {
                let mut total = i64::from(*constant);
                for term in terms {
                    let value = self.static_scaling_count(&term.quantity, context)?;
                    total = total.saturating_add(i64::from(term.coefficient).saturating_mul(value));
                }
                Some(total)
            }
            _ => None,
        }
    }

    /// Snapshot through CR 613 layer 5. Conditional layer-6/7 effects may inspect controller,
    /// type, and color through this boundary without recursively asking for their own result.
    fn characteristics_through_layer_5(&self, oid: ObjectId) -> Option<Characteristics> {
        self.characteristics_through_layer_5_with_started(oid)
            .map(|(snapshot, _)| snapshot)
    }

    fn characteristics_through_layer_5_with_started(
        &self,
        oid: ObjectId,
    ) -> Option<(Characteristics, Vec<TriggerAbilityOrigin>)> {
        let mut view = self.evaluate_early_layers(oid, false)?;
        let result = view.objects.remove(&oid)?.characteristics;
        Some((result, view.started.remove(&oid).unwrap_or_default()))
    }

    fn base_characteristics_through_layer_2(&self, oid: ObjectId) -> Option<Characteristics> {
        let object = self.state.objects.get(&oid)?;
        let face = effective_face_from(self.state, self.registry, oid)?;
        let mut result = self.base_characteristics_from_face(
            oid,
            &face,
            object.face_up_index,
            object.base_controller,
            object.face_down && object.zone == Zone::Battlefield,
        )?;
        self.apply_layer_2_control(oid, &mut result);
        Some(result)
    }

    fn base_characteristics_from_face(
        &self,
        oid: ObjectId,
        face: &CardFace,
        face_index: usize,
        controller: PlayerId,
        face_down: bool,
    ) -> Option<Characteristics> {
        let object = self.state.objects.get(&oid)?;
        let definition = self.registry.get(&object.card_id);
        let copied = object.copiable_values.as_ref();
        let mut result = Characteristics {
            // CR 202.3b/710.2: original transformed/flip cards retain front mana value.
            // A copy of a transforming back face has that face's (normally absent) mana cost.
            mana_value: if object.copiable_values.is_none()
                && object
                    .token_faces
                    .as_ref()
                    .is_some_and(|faces| faces.layout == Layout::Transform)
            {
                object
                    .token_faces
                    .as_ref()
                    .expect("checked token faces")
                    .faces[0]
                    .face
                    .mana_cost
                    .mana_value()
            } else if object.copiable_values.is_none()
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
            controller,
            names: (!face.name.is_empty())
                .then(|| face.name.clone())
                .into_iter()
                .collect(),
            types: face.types.to_vec(),
            all_creature_types: face
                .characteristic_defining_abilities
                .iter()
                .any(|ability| {
                    matches!(
                        &ability.definition,
                        CharacteristicDefiningAbility::Changeling
                    )
                }),
            supertypes: face.supertypes.to_vec(),
            colors: if copied.is_none()
                && definition.is_some_and(|definition| definition.layout == Layout::Flip)
                && face_index > 0
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
        self.apply_layer_1b_face_down(face_down, &mut result);
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
    fn apply_layer_1b_face_down(&self, face_down: bool, result: &mut Characteristics) {
        if !face_down {
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

    /// CR 205.1b / 613.1d: additive type-changing effects retain every printed and previously
    /// added type. Equal timestamps use insertion order so replay remains deterministic.
    fn apply_layer_4_type(
        &self,
        oid: ObjectId,
        result: &mut Characteristics,
        started: &mut Vec<TriggerAbilityOrigin>,
    ) {
        let mut effects: Vec<(usize, &ContinuousEffect)> = self
            .state
            .continuous_effects
            .iter()
            .enumerate()
            .filter(|(_, effect)| {
                matches!(
                    effect.kind,
                    ContinuousEffectKind::Layer4AddTypes(_)
                        | ContinuousEffectKind::Layer4RemoveCreature
                        | ContinuousEffectKind::Layer4SetTypeLine(_)
                        | ContinuousEffectKind::Layer4SetBasicLandType(_)
                        | ContinuousEffectKind::Layer4SetCreatureTypes(_)
                        | ContinuousEffectKind::Layer4SetAllCreatureTypes
                )
            })
            .filter(|(_, effect)| effect_affects(self.state, self.registry, effect, oid, result))
            .filter(|(_, effect)| {
                component_group(self.state, self.registry, effect)
                    .is_some_and(|origin| started.contains(&origin))
                    || self.characteristic_effect_condition_holds(effect, oid, result)
            })
            .collect();
        effects.sort_by_key(|(index, effect)| (effect.timestamp, *index));

        for (_, effect) in effects {
            if let Some(origin) = component_group(self.state, self.registry, effect) {
                if !started.contains(&origin) {
                    started.push(origin);
                }
            }
            match &effect.kind {
                ContinuousEffectKind::Layer4AddTypes(addition) => {
                    apply_type_line_addition(result, addition);
                }
                ContinuousEffectKind::Layer4RemoveCreature => {
                    apply_creature_type_removal(result);
                }
                ContinuousEffectKind::Layer4SetTypeLine(replacement) => {
                    apply_type_line_replacement(result, replacement);
                }
                ContinuousEffectKind::Layer4SetBasicLandType(land_type) => {
                    apply_basic_land_type(result, *land_type);
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
                ContinuousEffectKind::Layer4SetAllCreatureTypes => {
                    result.all_creature_types = true;
                    result.types.retain(|value| !is_creature_type(value));
                }
                _ => unreachable!("filtered to layer-4 type effects"),
            }
        }
    }

    fn apply_layer_5_color(
        &self,
        oid: ObjectId,
        result: &mut Characteristics,
        started: &mut Vec<TriggerAbilityOrigin>,
    ) {
        let mut effects: Vec<(usize, &ContinuousEffect)> = self
            .state
            .continuous_effects
            .iter()
            .enumerate()
            .filter(|(_, effect)| matches!(effect.kind, ContinuousEffectKind::Layer5SetColors(_)))
            .filter(|(_, effect)| effect_affects(self.state, self.registry, effect, oid, result))
            .filter(|(_, effect)| {
                component_group(self.state, self.registry, effect)
                    .is_some_and(|origin| started.contains(&origin))
                    || self.characteristic_effect_condition_holds(effect, oid, result)
            })
            .collect();
        effects.sort_by_key(|(index, effect)| (effect.timestamp, *index));
        for (_, effect) in effects {
            if let Some(origin) = component_group(self.state, self.registry, effect) {
                if !started.contains(&origin) {
                    started.push(origin);
                }
            }
            let ContinuousEffectKind::Layer5SetColors(colors) = &effect.kind else {
                unreachable!("filtered to layer-5 color effects");
            };
            result.colors.clone_from(colors);
        }
    }

    /// Whether a one-layer static keyword grant still has its generating ability. Only removal
    /// candidates are examined, from a through-layer-5 snapshot, avoiding a recursive layer-6
    /// query. CR 613.8 makes this grant depend on removal of the source's generating ability.
    fn static_keyword_grant_source_is_active(&self, effect: &ContinuousEffect) -> bool {
        let ContinuousEffectKind::Layer6AddKeywordFromStatic {
            keyword,
            source_zone_change,
        } = effect.kind
        else {
            return true;
        };
        let Some(source) = effect.source_id else {
            return false;
        };
        let Some(object) = self.state.objects.get(&source) else {
            return false;
        };
        if effect.duration == EffectDuration::WhileSourceInGraveyard {
            return self.graveyard_keyword_grant_source_is_active(effect, source, keyword);
        }
        if object.zone != Zone::Battlefield
            || object.face_down
            || self
                .state
                .zone_change_generation
                .get(&source)
                .copied()
                .unwrap_or(0)
                != source_zone_change
            || !printed_rules_text_is_present(self.state, self.registry, source)
        {
            return false;
        }
        let AffectedScope::PermanentsMatching { filter, .. } = &effect.affected else {
            return false;
        };
        let class_level = self.state.class_level(source);
        let still_grants =
            effective_face_from(self.state, self.registry, source).is_some_and(|face| {
                face.static_abilities
                    .iter()
                    .chain(
                        face.class_level_bars
                            .iter()
                            .filter(|bar| class_level >= bar.level)
                            .flat_map(|bar| &bar.static_abilities),
                    )
                    .any(|ability| {
                        matches!(&ability.definition, StaticAbilityDef::GrantKeywordToPermanents {
                            filter: current_filter, keyword: current_keyword,
                        } if current_keyword == &keyword && current_filter == filter.as_ref())
                    })
            });
        if !still_grants {
            return false;
        }
        !self.source_has_active_ability_removal(source)
    }

    fn graveyard_keyword_grant_source_is_active(
        &self,
        effect: &ContinuousEffect,
        source: ObjectId,
        keyword: Keyword,
    ) -> bool {
        let Some(TriggerAbilityOrigin::StaticGrant {
            source_id,
            source_zone_change,
            definition,
        }) = &effect.trigger_grant_origin
        else {
            return false;
        };
        if *source_id != source
            || !matches!(effect.kind, ContinuousEffectKind::Layer6AddKeywordFromStatic {
                source_zone_change: generation, ..
            } if generation == *source_zone_change)
            || !static_source_identity_is_current(self.state, self.registry, effect)
        {
            return false;
        }
        let Some(face) = effective_face_from(self.state, self.registry, source) else {
            return false;
        };
        let Some(ability) = face
            .static_abilities
            .iter()
            .find(|ability| definition.ability_path == [ability.ability_id.clone()])
        else {
            return false;
        };
        let expected_definition = super::triggers::ability_definition_from(
            self.state,
            self.registry,
            source,
            self.state.objects[&source].face_up_index,
            vec![ability.ability_id.clone()],
        );
        if *definition != expected_definition {
            return false;
        }
        let StaticAbilityDef::GraveyardAnthemKeyword {
            required_land_type,
            keyword: current_keyword,
        } = ability.definition
        else {
            return false;
        };
        let owner = self.state.objects[&source].owner;
        let expected_filter = TargetFilter {
            kind: TargetKind::Creature,
            controller: TargetController::You,
            ..TargetFilter::default()
        };
        if current_keyword != keyword
            || !matches!(&effect.affected, AffectedScope::PermanentsMatching {
                reference_player, filter, exclude: None,
            } if *reference_player == owner && filter.as_ref() == &expected_filter)
        {
            return false;
        }
        self.state.objects.iter().any(|(&id, object)| {
            object.zone == Zone::Battlefield
                && self
                    .characteristics_through_layer_5(id)
                    .is_some_and(|land| {
                        land.controller == owner
                            && land.has_type("Land")
                            && land.has_type(required_land_type.as_str())
                    })
        })
    }

    fn source_has_active_ability_removal(&self, source: ObjectId) -> bool {
        let Some(snapshot) = self.characteristics_through_layer_5(source) else {
            return false;
        };
        self.state.continuous_effects.iter().any(|removal| {
            matches!(removal.kind, ContinuousEffectKind::Layer6RemoveAllAbilities)
                && (removal.duration != EffectDuration::WhileSourceOnBattlefield
                    || removal.source_id.is_some_and(|id| {
                        self.state
                            .objects
                            .get(&id)
                            .is_some_and(|candidate| candidate.zone == Zone::Battlefield)
                        }))
                && static_source_identity_is_current(self.state, self.registry, removal)
                // Raw scope avoids asking whether another source-removal query suppresses
                // this candidate. Authored static removal producers start before layer six;
                // removal-only dependency ordering is a separate, unsupported boundary.
                && effect_scope_affects(self.state, self.registry, removal, source, &snapshot)
                && self.characteristic_effect_condition_holds(removal, source, &snapshot)
        })
    }

    /// Active layer-6 effects in CR 613.7 timestamp order. The original vector index makes equal
    /// timestamps deterministic. Their affected scope is evaluated from the layer-5 snapshot;
    /// current-keyword predicates are rejected for layer-6 users until CR 613.8 ordering exists.
    fn ordered_layer_6_effects<'a>(
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
                        | ContinuousEffectKind::Layer6AddKeywordFromStatic { .. }
                        | ContinuousEffectKind::Layer6AddProtection(_)
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

    /// Active layer-7 P/T effects in CR 613.7 timestamp order. Their affected scopes read the
    /// post-layer-6 snapshot, while existing conditional-effect predicates retain their
    /// established pre-layer-6 evaluation.
    fn ordered_layer_7_effects<'a>(
        &'a self,
        oid: ObjectId,
        post_layer_6: &Characteristics,
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
                    ContinuousEffectKind::Layer7bSetPt { .. }
                        | ContinuousEffectKind::Layer7bSetPower { .. }
                        | ContinuousEffectKind::PtModify { .. }
                        | ContinuousEffectKind::PtModifyByCount { .. }
                )
            })
            .filter(|(_, effect)| {
                effect_affects(self.state, self.registry, effect, oid, post_layer_6)
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
        if static_component_started_for(self.state, self.registry, effect, queried_oid) {
            return true;
        }
        let Some(condition) = effect.condition.as_ref() else {
            return true;
        };
        let Some(source_oid) = effect.source_id else {
            return false;
        };
        let controller = self.layer_2_controller(source_oid, &mut Vec::new());
        self.characteristic_condition_holds(
            condition,
            source_oid,
            controller,
            queried_oid,
            queried_pre_layer_6,
        )
    }

    fn characteristic_condition_holds(
        &self,
        condition: &GameCondition,
        source_oid: ObjectId,
        controller: PlayerId,
        queried_oid: ObjectId,
        queried_pre_layer_6: &Characteristics,
    ) -> bool {
        let view = self.evaluate_early_layers(queried_oid, true);
        self.characteristic_condition_holds_in_view(
            condition,
            source_oid,
            controller,
            queried_oid,
            queried_pre_layer_6,
            view.as_ref(),
        )
    }

    fn characteristic_condition_holds_in_view(
        &self,
        condition: &GameCondition,
        source_oid: ObjectId,
        controller: PlayerId,
        queried_oid: ObjectId,
        queried_pre_layer_6: &Characteristics,
        view: Option<&EarlyLayerView>,
    ) -> bool {
        match condition {
            GameCondition::ControllerLibraryEmpty => {
                super::draw::controller_library_empty(self.state, controller)
            }
            GameCondition::AllOf(branches) => branches.iter().all(|branch| {
                self.characteristic_condition_holds_in_view(
                    branch,
                    source_oid,
                    controller,
                    queried_oid,
                    queried_pre_layer_6,
                    view,
                )
            }),
            GameCondition::AnyOf(branches) => branches.iter().any(|branch| {
                self.characteristic_condition_holds_in_view(
                    branch,
                    source_oid,
                    controller,
                    queried_oid,
                    queried_pre_layer_6,
                    view,
                )
            }),
            GameCondition::HasEnduringStory { players } => {
                self.state.players.iter().any(|player| {
                    relative_player_set_contains(self.state, *players, controller, player.id)
                        && player.has_enduring_story
                })
            }
            // Cast snapshots are internal to resolving spells, never continuous characteristics.
            GameCondition::CastSnapshot { .. }
            | GameCondition::SelfWasCast
            | GameCondition::SelfWasBargained
            | GameCondition::CastOrigin { .. }
            | GameCondition::TriggeringSpellManaSpent { .. }
            | GameCondition::ObjectMatches { .. }
            | GameCondition::ObjectManaValue { .. } => false,
            GameCondition::Void => self.state.turn_history.current.void_holds(),
            GameCondition::PermanentLeftBattlefieldThisTurn { controllers } => self
                .state
                .players
                .iter()
                .filter(|player| {
                    relative_player_set_contains(self.state, *controllers, controller, player.id)
                })
                .any(|player| {
                    self.state
                        .turn_history
                        .current
                        .player(player.id)
                        .permanent_left_battlefield
                }),
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
            GameCondition::CardsInHand { players, .. } => match players {
                ConditionPlayerSet::Relative(RelativePlayerSet::Controller) => self
                    .state
                    .players
                    .iter()
                    .find(|player| player.id == controller && !player.has_lost)
                    .is_some_and(|player| {
                        condition
                            .matches_value(super::history::clamp_public_count(player.hand.len()))
                    }),
                _ => false,
            },
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
            GameCondition::SpellsCastLastTurn {
                players, filter, ..
            } => {
                let count = if *players == RelativePlayerSet::All
                    && *filter == SpellCastFilter::default()
                {
                    self.state.turn_history.previous.spells_cast
                } else {
                    super::history::spell_cast_count_in_record(
                        self.state,
                        &self.state.turn_history.previous,
                        ConditionPlayerSet::Relative(*players),
                        filter,
                        controller,
                        None,
                        false,
                    )
                };
                condition.matches_value(count)
            }
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
                    previous_effect_result: None,
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
            GameCondition::SourceTotalCounterCount { .. } => {
                let count = self
                    .state
                    .objects
                    .get(&source_oid)
                    .filter(|object| object.zone == Zone::Battlefield)
                    .map(|object| {
                        object
                            .counters
                            .values()
                            .copied()
                            .fold(0, u32::saturating_add)
                    })
                    .unwrap_or(0);
                condition.matches_value(count)
            }
            GameCondition::ObjectWasDealtDamageThisTurn { .. } => false,
            GameCondition::ObjectTapped { object, tapped } => {
                if !matches!(object, ConditionObjectRef::Source) {
                    return false;
                }
                self.state
                    .objects
                    .get(&source_oid)
                    .filter(|candidate| candidate.zone == Zone::Battlefield)
                    .map(|candidate| candidate.tapped)
                    == Some(*tapped)
            }
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
                    previous_effect_result: None,
                };
                let count = self
                    .state
                    .players
                    .iter()
                    .flat_map(|player| player.battlefield.iter().copied())
                    .filter(|candidate| {
                        view.is_none_or(|view| view.projected_entrant != Some(*candidate))
                    })
                    .filter_map(|candidate_oid| {
                        let characteristics = if candidate_oid == queried_oid {
                            queried_pre_layer_6.clone()
                        } else {
                            view?.objects.get(&candidate_oid)?.characteristics.clone()
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
            GameCondition::OpponentHasMoreThanYou { metric } => {
                let Some(reference) = self
                    .state
                    .players
                    .iter()
                    .find(|player| player.id == controller && !player.has_lost)
                else {
                    return false;
                };
                self.state
                    .players
                    .iter()
                    .filter(|player| {
                        !player.has_lost
                            && relative_player_set_contains(
                                self.state,
                                RelativePlayerSet::Opponents,
                                controller,
                                player.id,
                            )
                    })
                    .any(|opponent| match metric {
                        PlayerComparisonMetric::LifeTotal => opponent.life > reference.life,
                        PlayerComparisonMetric::HandSize => {
                            opponent.hand.len() > reference.hand.len()
                        }
                        // These need current derived types. Their conditional layer use remains
                        // rejected by CardRegistry until CR 613.8 dependency ordering is modeled.
                        PlayerComparisonMetric::LandCount
                        | PlayerComparisonMetric::CreatureCount => false,
                    })
            }
            GameCondition::BattlefieldAggregate { .. }
            | GameCondition::ObservedObjectNameIsUnique => false,
            GameCondition::Devotion { color, .. } => condition.matches_value(devotion_value(
                self.state,
                self.registry,
                controller,
                *color,
                view.and_then(|view| view.projected_entrant),
            )),
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

/// Availability of printed/copied static hooks using the same pure removal predicate as static
/// keyword grants. This does not resolve removal-only dependency cycles.
pub(super) fn printed_static_source_is_available(
    state: &GameState,
    registry: &'static CardRegistry,
    source: ObjectId,
) -> bool {
    state.objects.get(&source).is_some_and(|object| {
        object.zone == Zone::Battlefield
            && !object.face_down
            && printed_rules_text_is_present(state, registry, source)
            && effective_face_from(state, registry, source).is_some()
            && !(CharacteristicsEvaluator { state, registry })
                .source_has_active_ability_removal(source)
    })
}

/// Layer-4 basic-land setting removes printed/copied rules text, independently of layer-6
/// removal. Consumers must preserve independently granted abilities and started effect groups.
pub(super) fn printed_rules_text_is_present(
    state: &GameState,
    registry: &'static CardRegistry,
    source: ObjectId,
) -> bool {
    (CharacteristicsEvaluator { state, registry })
        .evaluate_early_layers(source, false)
        .and_then(|view| {
            view.objects
                .get(&source)
                .map(|object| object.printed_rules_text_present)
        })
        .unwrap_or(false)
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
    if !static_source_identity_is_current(state, registry, effect) {
        return false;
    }
    if !matches!(effect.affected, AffectedScope::Single(_))
        && !state
            .objects
            .get(&oid)
            .is_some_and(|object| object.zone == Zone::Battlefield)
    {
        return false;
    }
    // Resolved removal is independent of subsequent source ability loss for every duration.
    let resolved_removal = matches!(effect.kind, ContinuousEffectKind::Layer6RemoveAllAbilities)
        && component_group(state, registry, effect).is_none();
    let independent_at_this_layer = is_earlier_characteristic_component(&effect.kind)
        || resolved_removal
        || matches!(
            effect.kind,
            ContinuousEffectKind::Layer6AddKeywordFromStatic { .. }
        )
        || static_component_started_for(state, registry, effect, oid);
    if !independent_at_this_layer
        && component_group(state, registry, effect).is_some()
        && effect
            .source_id
            .is_some_and(|source| !printed_rules_text_is_present(state, registry, source))
    {
        return false;
    }
    if effect.duration == EffectDuration::WhileSourceOnBattlefield
        && !independent_at_this_layer
        && effect.source_id.is_some_and(|source_id| {
            if component_group(state, registry, effect).is_some()
                && !matches!(effect.kind, ContinuousEffectKind::Layer6RemoveAllAbilities)
            {
                // A refreshed unstarted component still depends on its generating ability.
                // Removal components keep their existing same-layer boundary; this does not
                // add an algorithm for removal-only static dependency cycles.
                (CharacteristicsEvaluator { state, registry })
                    .source_has_active_ability_removal(source_id)
            } else {
                latest_remove_all_abilities_timestamp(state, source_id)
                    .is_some_and(|removed_at| effect.timestamp <= removed_at)
            }
        })
    {
        return false;
    }
    if !(CharacteristicsEvaluator { state, registry }).static_keyword_grant_source_is_active(effect)
    {
        return false;
    }
    effect_scope_affects(state, registry, effect, oid, characteristics)
}

fn effect_scope_affects(
    state: &GameState,
    registry: &'static CardRegistry,
    effect: &ContinuousEffect,
    oid: ObjectId,
    characteristics: &Characteristics,
) -> bool {
    effect_scope_affects_with_reference(state, registry, effect, oid, characteristics, None)
}

fn effect_scope_affects_with_reference(
    state: &GameState,
    registry: &'static CardRegistry,
    effect: &ContinuousEffect,
    oid: ObjectId,
    characteristics: &Characteristics,
    reference_override: Option<PlayerId>,
) -> bool {
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
                reference_override
                    .or_else(|| {
                        effect.source_id.map(|source| {
                            CharacteristicsEvaluator { state, registry }
                                .layer_2_controller(source, &mut Vec::new())
                        })
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
        } => {
            let current_reference = if effect.duration == EffectDuration::WhileSourceOnBattlefield {
                reference_override
                    .or_else(|| {
                        effect.source_id.map(|source| {
                            CharacteristicsEvaluator { state, registry }
                                .layer_2_controller(source, &mut Vec::new())
                        })
                    })
                    .unwrap_or(*reference_player)
            } else {
                *reference_player
            };
            permanent_matches_target_scope(
                state,
                filter,
                current_reference,
                *exclude,
                oid,
                characteristics,
            )
        }
        AffectedScope::Player(_) => false,
    }
}

fn static_source_identity_is_current(
    state: &GameState,
    registry: &'static CardRegistry,
    effect: &ContinuousEffect,
) -> bool {
    let Some(TriggerAbilityOrigin::StaticGrant {
        source_id,
        source_zone_change,
        ..
    }) = &effect.trigger_grant_origin
    else {
        return true;
    };
    state.objects.get(source_id).is_some_and(|source| {
        source.zone
            == if effect.duration == EffectDuration::WhileSourceInGraveyard
                && matches!(
                    effect.kind,
                    ContinuousEffectKind::Layer6AddKeywordFromStatic { .. }
                )
            {
                Zone::Graveyard
            } else {
                Zone::Battlefield
            }
            && !source.face_down
            && state
                .zone_change_generation
                .get(source_id)
                .copied()
                .unwrap_or(0)
                == *source_zone_change
    }) && component_group(state, registry, effect).is_some()
}

fn is_earlier_characteristic_component(kind: &ContinuousEffectKind) -> bool {
    matches!(
        kind,
        ContinuousEffectKind::Layer3SetName(_)
            | ContinuousEffectKind::Layer4AddTypes(_)
            | ContinuousEffectKind::Layer4RemoveCreature
            | ContinuousEffectKind::Layer4SetTypeLine(_)
            | ContinuousEffectKind::Layer4SetBasicLandType(_)
            | ContinuousEffectKind::Layer4SetCreatureTypes(_)
            | ContinuousEffectKind::Layer4SetAllCreatureTypes
            | ContinuousEffectKind::Layer5SetColors(_)
    )
}

fn is_characteristic_component(kind: &ContinuousEffectKind) -> bool {
    is_earlier_characteristic_component(kind)
        || matches!(
            kind,
            ContinuousEffectKind::Layer6RemoveAllAbilities
                | ContinuousEffectKind::Layer6AddKeyword(_)
                | ContinuousEffectKind::Layer6AddProtection(_)
                | ContinuousEffectKind::GrantActivatedAbility(_)
                | ContinuousEffectKind::GrantTriggeredAbility(_)
                | ContinuousEffectKind::Layer7bSetPt { .. }
                | ContinuousEffectKind::Layer7bSetPower { .. }
                | ContinuousEffectKind::PtModify { .. }
                | ContinuousEffectKind::PtModifyByCount { .. }
        )
}

/// Validate raw source identity and normalize a nested grant to its containing static ability.
/// An unrelated ability on the same permanent remains a separate logical effect.
fn component_group(
    state: &GameState,
    registry: &'static CardRegistry,
    effect: &ContinuousEffect,
) -> Option<TriggerAbilityOrigin> {
    let origin = effect.trigger_grant_origin.as_ref()?;
    let TriggerAbilityOrigin::StaticGrant { source_id, .. } = origin else {
        return None;
    };
    if effect.source_id != Some(*source_id)
        || !matches!(
            effect.duration,
            EffectDuration::WhileSourceOnBattlefield | EffectDuration::WhileSourceInGraveyard
        )
    {
        return None;
    }
    normalized_static_origin(state, registry, origin)
}

pub(super) fn normalized_static_origin(
    state: &GameState,
    registry: &'static CardRegistry,
    origin: &TriggerAbilityOrigin,
) -> Option<TriggerAbilityOrigin> {
    let TriggerAbilityOrigin::StaticGrant {
        source_id,
        source_zone_change,
        definition,
    } = origin
    else {
        return None;
    };
    let object = state.objects.get(source_id)?;
    if state
        .zone_change_generation
        .get(source_id)
        .copied()
        .unwrap_or(0)
        != *source_zone_change
    {
        return None;
    }
    let card_id = object
        .copiable_values
        .as_ref()
        .or(object.token_origin.as_ref())
        .filter(|values| !values.source_card_id.is_empty())
        .map(|values| values.source_card_id.as_str())
        .unwrap_or(&object.card_id);
    let room_faces = object
        .copiable_values
        .as_ref()
        .or(object.token_origin.as_ref())
        .map(|values| values.room_faces.as_deref())
        .unwrap_or_else(|| {
            registry
                .get(&object.card_id)
                .and_then(|card| (card.layout == Layout::Room).then_some(card.faces.as_slice()))
        });
    let face = if object.zone == Zone::Battlefield {
        if let Some(faces) = room_faces {
            let door = faces
                .iter()
                .position(|face| face.face_id == definition.face_id)?;
            if !state
                .room_states
                .get(source_id)
                .copied()
                .unwrap_or_default()
                .unlocked_indices()
                .any(|index| index == door)
            {
                return None;
            }
            Cow::Borrowed(&faces[door])
        } else {
            effective_face_from(state, registry, *source_id)?
        }
    } else {
        effective_face_from(state, registry, *source_id)?
    };
    if card_id != definition.card_id || face.face_id != definition.face_id {
        return None;
    }
    let outer = definition.ability_path.first()?;
    let class_bar_ability = face.class_level_bars.iter().find_map(|bar| {
        bar.static_abilities
            .iter()
            .find(|ability| &ability.ability_id == outer)
            .map(|ability| (bar.level, ability))
    });
    let ability = if let Some((level, ability)) = class_bar_ability {
        if object.zone != Zone::Battlefield || state.class_level(*source_id) < level {
            return None;
        }
        ability
    } else {
        face.static_abilities
            .iter()
            .find(|ability| &ability.ability_id == outer)?
    };
    let mut outer_definition = definition.clone();
    outer_definition.ability_path = vec![ability.ability_id.clone()];
    Some(TriggerAbilityOrigin::StaticGrant {
        source_id: *source_id,
        source_zone_change: *source_zone_change,
        definition: outer_definition,
    })
}

/// Pure, recipient-specific CR 613.6 start lookup. Earlier passes never ask about layer-six
/// suppression, so recomputing this boundary cannot recurse through a later-layer component.
pub(super) fn static_component_started_for(
    state: &GameState,
    registry: &'static CardRegistry,
    effect: &ContinuousEffect,
    oid: ObjectId,
) -> bool {
    if !is_characteristic_component(&effect.kind)
        || is_earlier_characteristic_component(&effect.kind)
    {
        return false;
    }
    let Some(origin) = component_group(state, registry, effect) else {
        return false;
    };
    (CharacteristicsEvaluator { state, registry })
        .characteristics_through_layer_5_with_started(oid)
        .is_some_and(|(_, started)| started.contains(&origin))
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

pub(super) fn latest_active_ability_removal(
    state: &GameState,
    registry: &'static CardRegistry,
    oid: ObjectId,
) -> Option<(u64, usize)> {
    if !state
        .continuous_effects
        .iter()
        .any(|effect| matches!(effect.kind, ContinuousEffectKind::Layer6RemoveAllAbilities))
    {
        return None;
    }
    let evaluator = CharacteristicsEvaluator { state, registry };
    let snapshot = evaluator.characteristics_through_layer_5(oid)?;
    state
        .continuous_effects
        .iter()
        .enumerate()
        .filter(|(_, effect)| matches!(effect.kind, ContinuousEffectKind::Layer6RemoveAllAbilities))
        .filter(|(_, effect)| match (&effect.duration, effect.source_id) {
            (EffectDuration::WhileSourceOnBattlefield, Some(source)) => state
                .objects
                .get(&source)
                .is_some_and(|object| object.zone == Zone::Battlefield),
            (EffectDuration::WhileSourceInGraveyard, Some(source)) => state
                .objects
                .get(&source)
                .is_some_and(|object| object.zone == Zone::Graveyard),
            _ => true,
        })
        .filter(|(_, effect)| {
            effect_affects(state, registry, effect, oid, &snapshot)
                && evaluator.characteristic_effect_condition_holds(effect, oid, &snapshot)
        })
        .map(|(index, effect)| (effect.timestamp, index))
        .max()
}

const LAND_SUBTYPES: &[&str] = &[
    "Cave",
    "Desert",
    "Forest",
    "Gate",
    "Island",
    "Lair",
    "Locus",
    "Mine",
    "Mountain",
    "Plains",
    "Planet",
    "Power-Plant",
    "Sphere",
    "Swamp",
    "Tower",
    "Town",
    "Urza's",
];

pub(super) fn is_land_subtype(value: &str) -> bool {
    LAND_SUBTYPES.contains(&value)
}

pub(super) fn apply_basic_land_type(result: &mut Characteristics, land_type: BasicLandType) {
    result.types.retain(|value| !is_land_subtype(value));
    result.types.push(land_type.as_str().to_string());
}

/// The last active CR 305.7 setting operation for this object. Equal timestamps use insertion
/// order, matching the layer-4 characteristics pass.
pub(super) fn basic_land_type_setting(
    state: &GameState,
    oid: ObjectId,
) -> Option<(u64, usize, BasicLandType)> {
    state
        .continuous_effects
        .iter()
        .enumerate()
        .filter_map(|(index, effect)| {
            let ContinuousEffectKind::Layer4SetBasicLandType(land_type) = effect.kind else {
                return None;
            };
            matches!(effect.affected, AffectedScope::Single(affected) if affected == oid)
                .then_some((effect.timestamp, index, land_type))
        })
        .max_by_key(|(timestamp, index, _)| (*timestamp, *index))
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
        && super::targeting::target_owner_matches(state, filter.owner, reference_player, oid)
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
        || filter.dealt_damage_this_turn.is_some_and(|required| {
            let generation = state.zone_change_generation.get(&oid).copied().unwrap_or(0);
            state
                .turn_history
                .current
                .dealt_damage_objects
                .contains(&(oid, generation))
                != required
        })
        || filter
            .required_counter
            .is_some_and(|counter| object.counter_count(counter) == 0)
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
        .required_supertypes
        .iter()
        .all(|supertype| characteristics.supertypes.contains(supertype))
    {
        return false;
    }
    if filter
        .excluded_supertypes
        .iter()
        .any(|supertype| characteristics.supertypes.contains(supertype))
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
    if let Some(comparison) = filter.toughness {
        let Some(toughness) = characteristics.toughness else {
            return false;
        };
        let matches = match comparison {
            PowerComparison::AtLeast(minimum) => toughness >= minimum,
            PowerComparison::AtMost(maximum) => toughness <= maximum,
        };
        if !matches {
            return false;
        }
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
            Some(CreatureScopeController::TargetedPlayer { .. }) => false,
        }
        && characteristics.is_creature()
        && filter
            .subtype
            .as_ref()
            .is_none_or(|value| characteristics.has_type(value))
        && filter
            .color
            .is_none_or(|value| characteristics.colors.contains(&value))
        && filter
            .required_keyword
            .is_none_or(|value| characteristics.has_keyword(value))
        && name_matches
        && (!filter.requires_any_counter || object.has_any_counter())
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
        if !printed_rules_text_is_present(self.state, self.registry, object.id) {
            result.keywords.clear();
            result.protections.clear();
            result.evasions.clear();
        }
        let mut last_removal_timestamp = None;
        for effect in effects {
            if matches!(effect.kind, ContinuousEffectKind::Layer6RemoveAllAbilities) {
                result.keywords.clear();
                result.protections.clear();
                result.evasions.clear();
                last_removal_timestamp = Some(effect.timestamp);
            }
            if let ContinuousEffectKind::Layer6AddKeyword(keyword)
            | ContinuousEffectKind::Layer6AddKeywordFromStatic { keyword, .. } = effect.kind
            {
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
        oid: ObjectId,
        object: &GameObject,
        result: &mut Characteristics,
        effects: &[&ContinuousEffect],
    ) {
        // CR 613.4a/613.3: characteristic-defining abilities apply before setters. A layer-6
        // remove-all effect suppresses a P/T CDA, while a face-down object uses its layer-1b 2/2.
        let abilities_removed = effects
            .iter()
            .any(|effect| matches!(effect.kind, ContinuousEffectKind::Layer6RemoveAllAbilities));
        // CR 208.5: a creature with an undefined power or toughness uses zero.
        // This also supplies the base values when layer 6 removes a */* CDA.
        let creature_default = result.is_creature().then_some(0);
        let mut power = result.power.or(creature_default).map(i64::from);
        let mut toughness = result.toughness.or(creature_default).map(i64::from);
        if !object.face_down
            && !abilities_removed
            && printed_rules_text_is_present(self.state, self.registry, oid)
        {
            if let Some(face) = effective_face_from(self.state, self.registry, oid) {
                for ability in &face.characteristic_defining_abilities {
                    let CharacteristicDefiningAbility::CountScaledPowerToughness {
                        count,
                        power_per_match,
                        toughness_per_match,
                    } = &ability.definition
                    else {
                        continue;
                    };
                    let context = ConditionContext {
                        controller: result.controller,
                        source_object_id: oid,
                        source_zone_change: self
                            .state
                            .zone_change_generation
                            .get(&oid)
                            .copied()
                            .unwrap_or(0),
                        resolving_spell_id: None,
                        stack_item: None,
                        previous_effect_result: None,
                    };
                    let matches = self.static_scaling_count(count, context).unwrap_or(0);
                    if *power_per_match != 0 {
                        power = Some(i64::from(*power_per_match).saturating_mul(matches));
                    }
                    if *toughness_per_match != 0 {
                        toughness = Some(i64::from(*toughness_per_match).saturating_mul(matches));
                    }
                }
            }
        }
        for effect in effects {
            match effect.kind {
                ContinuousEffectKind::Layer7bSetPt {
                    power: set_power,
                    toughness: set_toughness,
                } => {
                    power = Some(set_power);
                    toughness = Some(set_toughness);
                }
                ContinuousEffectKind::Layer7bSetPower { power: set_power } => {
                    power = Some(set_power);
                }
                _ => {}
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
                    previous_effect_result: None,
                };
                let count = self.static_scaling_count(count, context).unwrap_or(0);
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

        // CR 208.3: a noncreature permanent has no power or toughness. Printed/CDA values remain
        // available in other zones and reappear if a layer-4 effect makes the permanent a creature.
        if object.zone == Zone::Battlefield && !result.is_creature() {
            power = None;
            toughness = None;
        }
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
    #[cfg(test)]
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

    mod devotion_tests;

    fn early_layer_type_effect(
        affected: AffectedScope,
        kind: ContinuousEffectKind,
        timestamp: u64,
    ) -> ContinuousEffect {
        ContinuousEffect {
            trigger_grant_origin: None,
            source_id: None,
            affected,
            kind,
            condition: None,
            duration: EffectDuration::UntilEndOfTurn,
            timestamp,
        }
    }

    #[test]
    fn class_section_static_abilities_stay_dormant_until_level_and_keep_entry_timestamp() {
        let class = r#"(
            id: "class_static_fixture", name: "Class Static Fixture", face_id: "class_static_fixture",
            mana_cost: "{U}", types: ["Enchantment", "Class"],
            class_level_bars: [(
                level: 2,
                level_ability: (ability_id: "level_two", presentation: Fallback,
                    costs: [Mana("{2}{U}")], effect: [SetClassLevel(level: 2)],
                    timing: SorcerySpeed),
                static_abilities: [
                    (ability_id: "max_hand", presentation: Fallback,
                        definition: NoMaximumHandSize(players: Controller)),
                    (ability_id: "anthem", presentation: Fallback,
                        definition: AnthemPt(filter: (controller: Some(YouControl)),
                            delta_power: 1, delta_toughness: 0)),
                ],
            ), (
                level: 3,
                level_ability: (ability_id: "level_three", presentation: Fallback,
                    costs: [Mana("{4}{U}")], effect: [SetClassLevel(level: 3)],
                    timing: SorcerySpeed),
            )],
        )"#;
        let registry = CardRegistry::from_chunks_and_tokens(
            &[
                class,
                include_str!("../../../tricerules-cards/data/grizzly_bears.ron"),
            ],
            &[],
        )
        .unwrap();
        let mut engine = GameEngine::new(305_099, &[0, 1], 20, None, true).unwrap();
        engine.registry = Box::leak(Box::new(registry));
        let source = insert_fixture(&mut engine, 0, "class_static_fixture", Zone::Battlefield);
        let creature = insert_fixture(&mut engine, 0, "grizzly_bears", Zone::Battlefield);
        engine.state.command_index = 11;
        engine.emit_static_abilities_on_enter(source);

        assert_eq!(engine.state.class_level(source), 1);
        assert_eq!(engine.maximum_hand_size(0), 7);
        assert_eq!(engine.effective_power(creature), Some(1));
        let anthem = engine
            .state
            .continuous_effects
            .iter()
            .find(|effect| effect.source_id == Some(source))
            .expect("the dormant section static is registered at entry");
        assert_eq!(anthem.timestamp, 11);

        let generation = engine
            .state
            .zone_change_generation
            .get(&source)
            .copied()
            .unwrap_or(0);
        assert!(engine.change_class_level(source, generation, 2).is_some());
        assert_eq!(engine.maximum_hand_size(0), usize::MAX);
        assert_eq!(engine.effective_power(creature), Some(2));
        assert_eq!(
            engine
                .state
                .continuous_effects
                .iter()
                .find(|effect| effect.source_id == Some(source))
                .expect("the section static remains registered")
                .timestamp,
            11,
            "level changes do not refresh the static effect timestamp"
        );
    }

    #[test]
    fn copied_class_bars_do_not_copy_level_and_same_object_keeps_its_level() {
        let class = r#"(
            id: "class_copy_fixture", name: "Class Copy Fixture", face_id: "class_copy_fixture",
            mana_cost: "{U}", types: ["Enchantment", "Class"],
            class_level_bars: [(
                level: 2,
                level_ability: (ability_id: "level_two", presentation: Fallback,
                    costs: [Mana("{2}{U}")], effect: [SetClassLevel(level: 2)],
                    timing: SorcerySpeed),
                static_abilities: [(ability_id: "anthem", presentation: Fallback,
                    definition: AnthemPt(filter: (controller: Some(YouControl)),
                        delta_power: 1, delta_toughness: 0))],
            ), (
                level: 3,
                level_ability: (ability_id: "level_three", presentation: Fallback,
                    costs: [Mana("{4}{U}")], effect: [SetClassLevel(level: 3)],
                    timing: SorcerySpeed),
            )],
        )"#;
        let registry = CardRegistry::from_chunks_and_tokens(
            &[
                class,
                include_str!("../../../tricerules-cards/data/grizzly_bears.ron"),
            ],
            &[],
        )
        .unwrap();
        let mut engine = GameEngine::new(305_100, &[0, 1], 20, None, true).unwrap();
        engine.registry = Box::leak(Box::new(registry));
        let source = insert_fixture(&mut engine, 0, "class_copy_fixture", Zone::Battlefield);
        let creature = insert_fixture(&mut engine, 0, "grizzly_bears", Zone::Battlefield);
        engine.emit_static_abilities_on_enter(source);
        let source_generation = engine
            .state
            .zone_change_generation
            .get(&source)
            .copied()
            .unwrap_or(0);
        assert!(engine
            .change_class_level(source, source_generation, 2)
            .is_some());

        let copy = insert_fixture(&mut engine, 0, "grizzly_bears", Zone::Battlefield);
        let class_values = engine.copiable_values_for(source).unwrap();
        engine.state.objects.get_mut(&copy).unwrap().copiable_values = Some(class_values);
        engine.refresh_source_static_abilities(copy);
        assert_eq!(engine.state.class_level(copy), 1);
        assert_eq!(engine.effective_power(creature), Some(2));

        let copy_generation = engine
            .state
            .zone_change_generation
            .get(&copy)
            .copied()
            .unwrap_or(0);
        assert!(engine
            .change_class_level(copy, copy_generation, 2)
            .is_some());
        assert_eq!(engine.effective_power(creature), Some(3));

        engine.state.continuous_effects.push(ContinuousEffect {
            trigger_grant_origin: None,
            source_id: None,
            affected: AffectedScope::Single(copy),
            kind: ContinuousEffectKind::Layer4SetTypeLine(tricerules_cards::TypeLineReplacement {
                card_types: vec![PermanentTypeFilter::Artifact],
                creature_types: Vec::new(),
                land_types: Vec::new(),
            }),
            condition: None,
            duration: EffectDuration::UntilEndOfTurn,
            timestamp: 100,
        });
        let copy_characteristics = engine.characteristics(copy).unwrap();
        assert!(copy_characteristics.is_artifact());
        assert!(!copy_characteristics.has_type("Class"));
        assert_eq!(engine.effective_power(creature), Some(3));
        assert!(engine
            .change_class_level(copy, copy_generation, 3)
            .is_some());

        let bear_values = engine.copiable_values_for(creature).unwrap();
        engine.state.objects.get_mut(&copy).unwrap().copiable_values = Some(bear_values);
        engine.refresh_source_static_abilities(copy);
        assert_eq!(
            engine.state.class_level(copy),
            3,
            "a same-object copy change does not copy or reset the level designation"
        );
        assert_eq!(engine.effective_power(creature), Some(2));
    }

    fn early_layer_type_scope(kind: PermanentTypeFilter) -> AffectedScope {
        AffectedScope::PermanentsMatching {
            reference_player: 0,
            filter: Box::new(TargetFilter {
                kind: TargetKind::AnyPermanent,
                permanent_types: vec![kind],
                ..TargetFilter::default()
            }),
            exclude: None,
        }
    }

    #[test]
    fn early_layer_scope_observes_later_land_creation() {
        for (addition_time, setting_time) in [(1, 2), (2, 1)] {
            let mut engine = GameEngine::new_with_default_decks(305_001, &[0, 1], 20).unwrap();
            let oid = insert_fixture(&mut engine, 0, "grizzly_bears", Zone::Battlefield);
            engine.state.continuous_effects.extend([
                early_layer_type_effect(
                    early_layer_type_scope(PermanentTypeFilter::Land),
                    ContinuousEffectKind::Layer4AddTypes(TypeLineAddition {
                        card_types: vec![PermanentTypeFilter::Artifact],
                        ..Default::default()
                    }),
                    addition_time,
                ),
                early_layer_type_effect(
                    AffectedScope::Single(oid),
                    ContinuousEffectKind::Layer4SetTypeLine(
                        tricerules_cards::TypeLineReplacement {
                            card_types: vec![PermanentTypeFilter::Land],
                            creature_types: Vec::new(),
                            land_types: Vec::new(),
                        },
                    ),
                    setting_time,
                ),
            ]);
            let result = engine.characteristics(oid).unwrap();
            assert!(
                result.has_type("Land") && result.has_type("Artifact"),
                "land creation precedes its dependent land scope in either timestamp order: {:?}",
                result.types
            );
            assert!(!result.has_type("Creature"));
        }
    }

    #[test]
    fn early_layer_type_scope_loop_uses_timestamp_order() {
        for (artifact_to_land, land_to_artifact, expected) in
            [(1, 2, "Artifact"), (2, 1, "Land"), (1, 1, "Artifact")]
        {
            let mut engine = GameEngine::new_with_default_decks(305_002, &[0, 1], 20).unwrap();
            let artifact = insert_fixture(&mut engine, 0, "sol_ring", Zone::Battlefield);
            let land = insert_fixture(&mut engine, 1, "forest", Zone::Battlefield);
            engine.state.continuous_effects.extend([
                early_layer_type_effect(
                    early_layer_type_scope(PermanentTypeFilter::Artifact),
                    ContinuousEffectKind::Layer4SetTypeLine(
                        tricerules_cards::TypeLineReplacement {
                            card_types: vec![PermanentTypeFilter::Land],
                            creature_types: Vec::new(),
                            land_types: Vec::new(),
                        },
                    ),
                    artifact_to_land,
                ),
                early_layer_type_effect(
                    early_layer_type_scope(PermanentTypeFilter::Land),
                    ContinuousEffectKind::Layer4SetTypeLine(
                        tricerules_cards::TypeLineReplacement {
                            card_types: vec![PermanentTypeFilter::Artifact],
                            creature_types: Vec::new(),
                            land_types: Vec::new(),
                        },
                    ),
                    land_to_artifact,
                ),
            ]);
            for oid in [artifact, land] {
                assert_eq!(
                    engine.characteristics(oid).unwrap().types,
                    vec![expected.to_string()],
                    "dependency-loop members use timestamp then insertion order"
                );
            }
        }
    }

    #[test]
    fn early_layer_basic_subtype_addition_requires_evolving_land_type() {
        let mut engine = GameEngine::new_with_default_decks(305_003, &[0, 1], 20).unwrap();
        let land = insert_fixture(&mut engine, 0, "island", Zone::Battlefield);
        let creature = insert_fixture(&mut engine, 1, "grizzly_bears", Zone::Battlefield);
        for oid in [land, creature] {
            engine
                .state
                .continuous_effects
                .push(early_layer_type_effect(
                    AffectedScope::Single(oid),
                    ContinuousEffectKind::Layer4AddTypes(TypeLineAddition {
                        land_types: vec![BasicLandType::Forest],
                        ..Default::default()
                    }),
                    1,
                ));
        }
        let result = engine.characteristics(land).unwrap();
        assert!(result.has_type("Island") && result.has_type("Forest"));
        assert!(!engine.characteristics(creature).unwrap().has_type("Forest"));
    }

    fn early_layer_static_land_fixture(timestamp: u64) -> (GameEngine, ObjectId) {
        let card = r#"(
            id: "early_static_land", name: "Early Static Land", face_id: "early_static_land",
            types: ["Land"],
            static_abilities: [(ability_id: "static_01", presentation: Fallback,
                definition: ConditionalSelfModifier(condition: ActivePlayer(players: Controller),
                    add_types: (card_types: [Artifact])))],
        )"#;
        let registry = CardRegistry::from_chunks_and_tokens(&[card], &[]).unwrap();
        let mut engine = GameEngine::new(305_004, &[0, 1], 20, None, true).unwrap();
        engine.registry = Box::leak(Box::new(registry));
        let source = insert_fixture(&mut engine, 0, "early_static_land", Zone::Battlefield);
        engine.state.command_index = timestamp;
        engine.emit_static_abilities_on_enter(source);
        assert!(engine.characteristics(source).unwrap().has_type("Artifact"));
        (engine, source)
    }

    #[test]
    fn early_layer_source_text_suppression_precedes_static_type_effect() {
        for (static_time, setting_time) in [(1, 2), (2, 1)] {
            let (mut engine, source) = early_layer_static_land_fixture(static_time);
            engine
                .state
                .continuous_effects
                .push(early_layer_type_effect(
                    AffectedScope::Single(source),
                    ContinuousEffectKind::Layer4SetBasicLandType(BasicLandType::Forest),
                    setting_time,
                ));
            let result = engine.characteristics(source).unwrap();
            assert!(result.has_type("Land") && result.has_type("Forest"));
            assert!(!result.has_type("Artifact"), "setting a basic land type removes the generating printed ability before its type effect");
            engine.state.continuous_effects.retain(|effect| {
                !matches!(effect.kind, ContinuousEffectKind::Layer4SetBasicLandType(_))
            });
            assert!(
                engine.characteristics(source).unwrap().has_type("Artifact"),
                "retained static records restore after suppression expires"
            );
        }
    }

    #[test]
    fn early_layer_resolved_basic_land_setting_survives_static_refresh() {
        let (mut engine, source) = early_layer_static_land_fixture(1);
        let mut setter = early_layer_type_effect(
            AffectedScope::Single(source),
            ContinuousEffectKind::Layer4SetBasicLandType(BasicLandType::Forest),
            2,
        );
        setter.source_id = Some(source);
        setter.duration = EffectDuration::WhileSourceOnBattlefield;
        engine.state.continuous_effects.push(setter);
        engine.refresh_source_static_abilities(source);
        assert!(
            engine
                .state
                .continuous_effects
                .iter()
                .any(|effect| effect.source_id == Some(source)
                    && effect.trigger_grant_origin.is_none()
                    && matches!(
                        effect.kind,
                        ContinuousEffectKind::Layer4SetBasicLandType(BasicLandType::Forest)
                    )),
            "an entry-created resolved setter is independent of printed static abilities"
        );
        let result = engine.characteristics(source).unwrap();
        assert!(result.has_type("Forest") && !result.has_type("Artifact"));
        engine.state.continuous_effects.retain(|effect| {
            !matches!(effect.kind, ContinuousEffectKind::Layer4SetBasicLandType(_))
        });
        assert!(
            engine.characteristics(source).unwrap().has_type("Artifact"),
            "refresh during temporary suppression retains the static record for restoration"
        );
    }

    #[test]
    fn early_layer_basic_setting_preserves_nonland_creature_subtypes() {
        for changeling in [true, false] {
            let mut engine = GameEngine::new_with_default_decks(305_005, &[0, 1], 20).unwrap();
            let source = insert_fixture(&mut engine, 0, "dryad_arbor", Zone::Battlefield);
            if changeling {
                let mut values = engine.copiable_values_for(source).unwrap();
                values.face.characteristic_defining_abilities.push(
                    tricerules_cards::IdentifiedAbility::fallback(
                        "changeling",
                        CharacteristicDefiningAbility::Changeling,
                    )
                    .unwrap(),
                );
                engine
                    .state
                    .objects
                    .get_mut(&source)
                    .unwrap()
                    .copiable_values = Some(values);
            } else {
                engine
                    .state
                    .continuous_effects
                    .push(early_layer_type_effect(
                        AffectedScope::Single(source),
                        ContinuousEffectKind::Layer4SetAllCreatureTypes,
                        1,
                    ));
            }
            engine
                .state
                .continuous_effects
                .push(early_layer_type_effect(
                    AffectedScope::Single(source),
                    ContinuousEffectKind::Layer4SetBasicLandType(BasicLandType::Island),
                    2,
                ));
            let result = engine.characteristics(source).unwrap();
            assert!(
                result.is_creature() && result.has_type("Island") && !result.has_type("Forest")
            );
            assert!(result.all_creature_types,
                "a subtype-only basic-land setting preserves creature types already established in layer 4");
            engine
                .state
                .continuous_effects
                .push(early_layer_type_effect(
                    AffectedScope::Single(source),
                    ContinuousEffectKind::Layer4SetTypeLine(
                        tricerules_cards::TypeLineReplacement {
                            card_types: vec![PermanentTypeFilter::Land],
                            creature_types: Vec::new(),
                            land_types: vec![BasicLandType::Forest],
                        },
                    ),
                    3,
                ));
            let result = engine.characteristics(source).unwrap();
            assert!(
                result.has_type("Forest") && !result.is_creature() && !result.all_creature_types,
                "a full type-line replacement removes creature types with their card type"
            );
        }
    }

    #[test]
    fn early_layer_loop_before_independent_addition_retains_later_type() {
        let mut engine = GameEngine::new_with_default_decks(305_006, &[0, 1], 20).unwrap();
        insert_fixture(&mut engine, 0, "forest", Zone::Battlefield);
        insert_fixture(&mut engine, 1, "sol_ring", Zone::Battlefield);
        let hybrid = insert_fixture(&mut engine, 0, "darksteel_citadel", Zone::Battlefield);
        engine.state.continuous_effects.extend([
            early_layer_type_effect(
                early_layer_type_scope(PermanentTypeFilter::Land),
                ContinuousEffectKind::Layer4SetTypeLine(tricerules_cards::TypeLineReplacement {
                    card_types: vec![PermanentTypeFilter::Artifact],
                    creature_types: Vec::new(),
                    land_types: Vec::new(),
                }),
                1,
            ),
            early_layer_type_effect(
                early_layer_type_scope(PermanentTypeFilter::Artifact),
                ContinuousEffectKind::Layer4SetTypeLine(tricerules_cards::TypeLineReplacement {
                    card_types: vec![PermanentTypeFilter::Land],
                    creature_types: Vec::new(),
                    land_types: Vec::new(),
                }),
                2,
            ),
            early_layer_type_effect(
                AffectedScope::Single(hybrid),
                ContinuousEffectKind::Layer4AddTypes(TypeLineAddition {
                    card_types: vec![PermanentTypeFilter::Enchantment],
                    ..Default::default()
                }),
                3,
            ),
        ]);
        assert_eq!(
            engine.characteristics(hybrid).unwrap().types,
            ["Land", "Enchantment"],
            "an available earlier loop precedes an independent later type addition"
        );
    }

    #[test]
    fn early_layer_unstarted_static_keyword_is_suppressed_and_restored() {
        let card = r#"(
            id: "early_static_land", name: "Early Static Land", face_id: "early_static_land",
            types: ["Land"],
            static_abilities: [(ability_id: "static_01", presentation: Fallback,
                definition: ConditionalSelfModifier(condition: ActivePlayer(players: Controller),
                    keywords: [Flying]))],
        )"#;
        let registry = CardRegistry::from_chunks_and_tokens(&[card], &[]).unwrap();
        let mut engine = GameEngine::new(305_007, &[0, 1], 20, None, true).unwrap();
        engine.registry = Box::leak(Box::new(registry));
        let source = insert_fixture(&mut engine, 0, "early_static_land", Zone::Battlefield);
        engine.emit_static_abilities_on_enter(source);
        assert!(engine
            .characteristics(source)
            .unwrap()
            .has_keyword(Keyword::Flying));
        engine
            .state
            .continuous_effects
            .push(early_layer_type_effect(
                AffectedScope::Single(source),
                ContinuousEffectKind::Layer4SetBasicLandType(BasicLandType::Forest),
                2,
            ));
        engine.refresh_source_static_abilities(source);
        assert!(
            !engine
                .characteristics(source)
                .unwrap()
                .has_keyword(Keyword::Flying),
            "an unstarted layer-6 static component loses its generating printed ability in layer 4"
        );
        engine.state.continuous_effects.retain(|effect| {
            !matches!(effect.kind, ContinuousEffectKind::Layer4SetBasicLandType(_))
        });
        assert!(engine
            .characteristics(source)
            .unwrap()
            .has_keyword(Keyword::Flying));
    }

    #[test]
    fn early_layer_world_count_reads_evolving_nonrecursive_snapshot() {
        let first = r#"(
            id: "early_artifact", name: "Early Artifact", face_id: "early_artifact",
            types: ["Creature"], power: 1, toughness: 1,
            static_abilities: [(ability_id: "static_01", presentation: Fallback,
                definition: ConditionalSelfModifier(
                    condition: BattlefieldAggregate(filter: (controllers: All, card_type: Some(Land)),
                        aggregate: Count, min: Some(1)), add_types: (card_types: [Artifact])))],
        )"#;
        let second = first
            .replace("early_artifact", "early_enchantment")
            .replace("Early Artifact", "Early Enchantment")
            .replace("[Artifact]", "[Enchantment]");
        let third = r#"(id: "early_creature", name: "Early Creature", face_id: "early_creature",
            types: ["Creature"], power: 1, toughness: 1)"#;
        let registry = CardRegistry::from_chunks_and_tokens(&[first, &second, third], &[]).unwrap();
        let mut engine = GameEngine::new(305_008, &[0, 1], 20, None, true).unwrap();
        engine.registry = Box::leak(Box::new(registry));
        let a = insert_fixture(&mut engine, 0, "early_artifact", Zone::Battlefield);
        let b = insert_fixture(&mut engine, 1, "early_enchantment", Zone::Battlefield);
        let c = insert_fixture(&mut engine, 0, "early_creature", Zone::Battlefield);
        engine.emit_static_abilities_on_enter(a);
        engine.emit_static_abilities_on_enter(b);
        assert!(!engine.characteristics(a).unwrap().has_type("Artifact"));
        assert!(!engine.characteristics(b).unwrap().has_type("Enchantment"));
        engine
            .state
            .continuous_effects
            .push(early_layer_type_effect(
                AffectedScope::Single(c),
                ContinuousEffectKind::Layer4SetTypeLine(tricerules_cards::TypeLineReplacement {
                    card_types: vec![PermanentTypeFilter::Land],
                    creature_types: Vec::new(),
                    land_types: Vec::new(),
                }),
                10,
            ));
        for order in [[a, b], [b, a]] {
            for oid in order {
                assert!(engine.characteristics(oid).unwrap().has_type(if oid == a {
                    "Artifact"
                } else {
                    "Enchantment"
                }));
            }
        }
        engine.state.players[0].battlefield.retain(|&oid| oid != c);
        engine.state.objects.get_mut(&c).unwrap().zone = Zone::Hand;
        assert!(
            engine.characteristics(c).unwrap().has_type("Land"),
            "a queried hand object still has its single-object effect"
        );
        assert!(
            !engine.characteristics(a).unwrap().has_type("Artifact"),
            "query-only hand objects never enter battlefield counts"
        );
        assert!(!engine.characteristics(b).unwrap().has_type("Enchantment"));
        engine
            .state
            .continuous_effects
            .retain(|effect| !matches!(effect.kind, ContinuousEffectKind::Layer4SetTypeLine(_)));
        engine.state.objects.get_mut(&c).unwrap().zone = Zone::Battlefield;
        engine.state.players[0].battlefield.push(c);
        assert!(!engine.characteristics(a).unwrap().has_type("Artifact"));
        assert!(!engine.characteristics(b).unwrap().has_type("Enchantment"));
    }

    #[test]
    fn early_layer_printed_ability_readers_share_type_setting_suppression() {
        let mut engine = GameEngine::new_with_default_decks(305_009, &[0, 1], 20).unwrap();
        let mana = insert_fixture(&mut engine, 0, "sol_ring", Zone::Battlefield);
        let trigger = insert_fixture(&mut engine, 0, "psychosis_crawler", Zone::Battlefield);
        let counter_lock = insert_fixture(&mut engine, 0, "tatterkite", Zone::Battlefield);
        let forge = insert_fixture(&mut engine, 0, "darksteel_forge", Zone::Battlefield);
        let recipient = insert_fixture(&mut engine, 1, "sol_ring", Zone::Battlefield);
        // The forge grants only to its controller; keep a separate unsuppressed artifact there.
        engine
            .state
            .objects
            .get_mut(&recipient)
            .unwrap()
            .base_controller = 0;
        engine.state.objects.get_mut(&recipient).unwrap().controller = 0;
        engine.emit_static_abilities_on_enter(forge);
        engine.reconcile_activated_ability_slots();
        assert!(!engine.effective_activated_abilities(mana).is_empty());
        assert!(!engine
            .effective_triggered_abilities(trigger, "psychosis_crawler", 0)
            .is_empty());
        assert!(!engine.can_receive_counters(counter_lock));
        assert!(engine
            .characteristics(recipient)
            .unwrap()
            .has_keyword(Keyword::Indestructible));
        assert!(engine.effective_power(trigger).unwrap() > 0);
        for oid in [mana, trigger, counter_lock, forge] {
            engine
                .state
                .continuous_effects
                .push(early_layer_type_effect(
                    AffectedScope::Single(oid),
                    ContinuousEffectKind::Layer4SetTypeLine(
                        tricerules_cards::TypeLineReplacement {
                            card_types: vec![PermanentTypeFilter::Land],
                            creature_types: Vec::new(),
                            land_types: vec![BasicLandType::Forest],
                        },
                    ),
                    5,
                ));
        }
        let mana_abilities = engine.effective_activated_abilities(mana);
        assert_eq!(mana_abilities.len(), 1);
        assert!(
            mana_abilities[0].definition.intrinsic_land_mana,
            "the printed ability disappears; the resulting Forest derives its own mana ability"
        );
        assert_eq!(
            mana_abilities[0].definition.mana_options(),
            Some(&vec![BasicLandType::Forest.mana()])
        );
        assert!(
            engine
                .effective_triggered_abilities(trigger, "psychosis_crawler", 0)
                .is_empty(),
            "printed damage trigger disappears"
        );
        assert!(
            engine.can_receive_counters(counter_lock),
            "printed static prohibition disappears"
        );
        assert!(engine.active_static_ability_definitions(forge).is_empty());
        assert!(
            !engine
                .characteristics(recipient)
                .unwrap()
                .has_keyword(Keyword::Indestructible),
            "one-layer global grant loses its printed source"
        );
        assert_eq!(
            engine.effective_power(trigger),
            None,
            "a noncreature no longer applies its printed power CDA"
        );
        engine
            .state
            .continuous_effects
            .retain(|effect| !matches!(effect.kind, ContinuousEffectKind::Layer4SetTypeLine(_)));
        assert!(!engine.effective_activated_abilities(mana).is_empty());
        assert!(!engine
            .effective_triggered_abilities(trigger, "psychosis_crawler", 0)
            .is_empty());
        assert!(!engine.can_receive_counters(counter_lock));
        assert!(engine
            .characteristics(recipient)
            .unwrap()
            .has_keyword(Keyword::Indestructible));
        assert!(engine.effective_power(trigger).unwrap() > 0);
    }

    #[test]
    fn early_layer_life_prohibition_tracks_printed_text_restoration() {
        let mut engine = GameEngine::new_with_default_decks(305_011, &[0, 1], 20).unwrap();
        let source = insert_fixture(&mut engine, 0, "giant_cindermaw", Zone::Battlefield);
        assert!(!engine.can_player_gain_life(1));
        engine
            .state
            .continuous_effects
            .push(early_layer_type_effect(
                AffectedScope::Single(source),
                ContinuousEffectKind::Layer4SetTypeLine(tricerules_cards::TypeLineReplacement {
                    card_types: vec![PermanentTypeFilter::Land],
                    creature_types: Vec::new(),
                    land_types: vec![BasicLandType::Forest],
                }),
                5,
            ));
        assert!(
            engine.can_player_gain_life(1),
            "printed life-gain prohibition disappears"
        );
        engine.state.continuous_effects.clear();
        assert!(!engine.can_player_gain_life(1));
    }

    #[test]
    fn early_layer_hand_rules_track_printed_text_restoration() {
        let mut engine = GameEngine::new_with_default_decks(305_012, &[0, 1], 20).unwrap();
        let vessel = insert_fixture(&mut engine, 0, "thought_vessel", Zone::Battlefield);
        let library = insert_fixture(&mut engine, 0, "library_of_leng", Zone::Battlefield);
        assert_eq!(engine.maximum_hand_size(0), usize::MAX);
        assert!(engine.has_discard_library_replacement(0));
        for oid in [vessel, library] {
            engine
                .state
                .continuous_effects
                .push(early_layer_type_effect(
                    AffectedScope::Single(oid),
                    ContinuousEffectKind::Layer4SetTypeLine(
                        tricerules_cards::TypeLineReplacement {
                            card_types: vec![PermanentTypeFilter::Land],
                            creature_types: Vec::new(),
                            land_types: vec![BasicLandType::Forest],
                        },
                    ),
                    5,
                ));
        }
        assert_eq!(engine.maximum_hand_size(0), 7);
        assert!(!engine.has_discard_library_replacement(0));
        engine.state.continuous_effects.clear();
        assert_eq!(engine.maximum_hand_size(0), usize::MAX);
        assert!(engine.has_discard_library_replacement(0));
    }

    #[test]
    fn early_layer_live_land_subtype_static_tracks_scope_source_and_incarnation() {
        let source_card = r#"(id: "type_addition_source", name: "Type Addition Source", face_id: "type_addition_source",
            supertypes: ["Legendary"], types: ["Land"],
            static_abilities: [(ability_id: "static_01", presentation: Fallback,
                definition: AddTypesToPermanents(filter: (kind: AnyPermanent, permanent_types: [Land]),
                    addition: (land_types: [Forest])))])"#;
        let island_card = r#"(id: "type_addition_island", name: "Type Addition Island", face_id: "type_addition_island", types: ["Land", "Island"])"#;
        let creature_card = r#"(id: "type_addition_creature", name: "Type Addition Creature", face_id: "type_addition_creature", types: ["Creature"], power: 2, toughness: 2)"#;
        for setting_time in [0, 5] {
            let registry = CardRegistry::from_chunks_and_tokens(
                &[source_card, island_card, creature_card],
                &[],
            )
            .unwrap();
            let mut engine = GameEngine::new(305_013, &[0, 1], 20, None, true).unwrap();
            engine.registry = Box::leak(Box::new(registry));
            let source = insert_fixture(&mut engine, 0, "type_addition_source", Zone::Battlefield);
            let opponent_land =
                insert_fixture(&mut engine, 1, "type_addition_island", Zone::Battlefield);
            let creature =
                insert_fixture(&mut engine, 1, "type_addition_creature", Zone::Battlefield);
            let hand = insert_fixture(&mut engine, 0, "type_addition_island", Zone::Hand);
            let graveyard = insert_fixture(&mut engine, 1, "type_addition_island", Zone::Graveyard);
            engine.state.command_index = 1;
            engine.emit_static_abilities_on_enter(source);
            assert!(engine.characteristics(source).unwrap().has_type("Forest"));
            let result = engine.characteristics(opponent_land).unwrap();
            assert!(result.has_type("Island") && result.has_type("Forest"));
            for oid in [creature, hand, graveyard] {
                assert!(!engine.characteristics(oid).unwrap().has_type("Forest"));
            }
            engine
                .state
                .continuous_effects
                .push(early_layer_type_effect(
                    AffectedScope::Single(creature),
                    ContinuousEffectKind::Layer4SetTypeLine(
                        tricerules_cards::TypeLineReplacement {
                            card_types: vec![PermanentTypeFilter::Land],
                            creature_types: Vec::new(),
                            land_types: Vec::new(),
                        },
                    ),
                    3,
                ));
            assert!(
                engine.characteristics(creature).unwrap().has_type("Forest"),
                "an older global scope observes new lands"
            );
            engine
                .state
                .continuous_effects
                .push(early_layer_type_effect(
                    AffectedScope::Single(source),
                    ContinuousEffectKind::Layer4SetTypeLine(
                        tricerules_cards::TypeLineReplacement {
                            card_types: vec![PermanentTypeFilter::Land],
                            creature_types: Vec::new(),
                            land_types: vec![BasicLandType::Swamp],
                        },
                    ),
                    setting_time,
                ));
            assert!(
                !engine
                    .characteristics(opponent_land)
                    .unwrap()
                    .has_type("Forest"),
                "printed-source suppression precedes its global effect in either timestamp order"
            );
            assert!(!engine.characteristics(creature).unwrap().has_type("Forest"));
            engine.state.continuous_effects.retain(|effect| {
                !(effect.affected == AffectedScope::Single(source)
                    && matches!(effect.kind, ContinuousEffectKind::Layer4SetTypeLine(_)))
            });
            assert!(engine
                .characteristics(opponent_land)
                .unwrap()
                .has_type("Forest"));
            *engine
                .state
                .zone_change_generation
                .entry(source)
                .or_default() += 1;
            assert!(
                !engine
                    .characteristics(opponent_land)
                    .unwrap()
                    .has_type("Forest"),
                "a static effect cannot bind to a later source incarnation"
            );
            engine.refresh_source_static_abilities(source);
            assert!(engine
                .characteristics(opponent_land)
                .unwrap()
                .has_type("Forest"));
        }
    }

    #[test]
    fn static_opponent_comparisons_support_live_life_and_hand_values() {
        fn holds(engine: &GameEngine, metric: PlayerComparisonMetric) -> bool {
            let queried_oid = engine.state.players[0].library[0];
            let queried = engine
                .characteristics(queried_oid)
                .expect("library object characteristics");
            CharacteristicsEvaluator {
                state: &engine.state,
                registry: engine.registry,
            }
            .characteristic_condition_holds(
                &GameCondition::OpponentHasMoreThanYou { metric },
                queried_oid,
                engine.state.players[0].id,
                queried_oid,
                &queried,
            )
        }

        let mut engine =
            GameEngine::new_with_default_decks(490_105, &[0, 1], 20).expect("new engine");
        assert!(!holds(&engine, PlayerComparisonMetric::LifeTotal));
        engine.state.players[1].life += 1;
        assert!(holds(&engine, PlayerComparisonMetric::LifeTotal));
        engine.state.players[1].life -= 1;

        assert!(!holds(&engine, PlayerComparisonMetric::HandSize));
        let drawn = engine.state.players[1]
            .library
            .pop_front()
            .expect("card to move into the opponent hand");
        engine.state.players[1].hand.push(drawn);
        engine
            .state
            .objects
            .get_mut(&drawn)
            .expect("drawn card")
            .zone = Zone::Hand;
        assert!(holds(&engine, PlayerComparisonMetric::HandSize));
        engine.state.players[1].has_lost = true;
        assert!(!holds(&engine, PlayerComparisonMetric::HandSize));
    }

    fn install_changeling_face(engine: &mut GameEngine, face_down: bool) -> ObjectId {
        let oid = engine.state.players[0].library[0];
        let mut face = engine
            .registry
            .get("cavalry_drillmaster")
            .expect("Cavalry Drillmaster definition")
            .primary_face()
            .clone();
        face.characteristic_defining_abilities =
            vec![tricerules_cards::IdentifiedAbility::fallback(
                "characteristic_01",
                CharacteristicDefiningAbility::Changeling,
            )
            .unwrap()];
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
    fn color_defining_cda_sets_base_color_before_layer_five_effects() {
        let mut engine =
            GameEngine::new_with_default_decks(903_401, &[0, 1], 20).expect("new engine");
        let oid = engine.state.players[0].library[0];
        let mut face = engine
            .registry
            .get("grizzly_bears")
            .expect("Grizzly Bears definition")
            .primary_face()
            .clone();
        face.characteristic_defining_abilities =
            vec![tricerules_cards::IdentifiedAbility::fallback(
                "defines_colors",
                CharacteristicDefiningAbility::DefinesColors {
                    colors: vec![Color::Blue, Color::White],
                },
            )
            .unwrap()];
        let object = engine.state.objects.get_mut(&oid).expect("library object");
        object.zone = Zone::Battlefield;
        object.copiable_values = Some(CopiableValues {
            source_card_id: "grizzly_bears".into(),
            source_face_index: 0,
            face,
            room_faces: None,
            display_name: "Grizzly Bears with a color CDA".into(),
        });
        engine.state.players[0].battlefield.push(oid);

        assert_eq!(
            engine.characteristics(oid).expect("characteristics").colors,
            vec![Color::Blue, Color::White]
        );
        engine.state.continuous_effects.push(ContinuousEffect {
            trigger_grant_origin: None,
            source_id: None,
            affected: AffectedScope::Single(oid),
            kind: ContinuousEffectKind::Layer5SetColors(vec![Color::Red]),
            condition: None,
            duration: EffectDuration::UntilEndOfTurn,
            timestamp: 0,
        });
        assert_eq!(
            engine
                .characteristics(oid)
                .expect("characteristics after layer 5")
                .colors,
            vec![Color::Red]
        );
    }

    #[test]
    fn devoid_remains_applied_after_layer_six_ability_removal() {
        let mut engine =
            GameEngine::new_with_default_decks(903_402, &[0, 1], 20).expect("new engine");
        let oid = engine.state.players[0].library[0];
        let mut face = engine
            .registry
            .get("spyglass_siren")
            .expect("Spyglass Siren definition")
            .primary_face()
            .clone();
        face.characteristic_defining_abilities =
            vec![tricerules_cards::IdentifiedAbility::fallback(
                "devoid",
                CharacteristicDefiningAbility::Devoid,
            )
            .unwrap()];
        let object = engine.state.objects.get_mut(&oid).expect("library object");
        object.zone = Zone::Battlefield;
        object.copiable_values = Some(CopiableValues {
            source_card_id: "spyglass_siren".into(),
            source_face_index: 0,
            face,
            room_faces: None,
            display_name: "Spyglass Siren with Devoid".into(),
        });
        engine.state.players[0].battlefield.push(oid);
        engine.state.continuous_effects.push(ContinuousEffect {
            trigger_grant_origin: None,
            source_id: None,
            affected: AffectedScope::Single(oid),
            kind: ContinuousEffectKind::Layer6RemoveAllAbilities,
            condition: None,
            duration: EffectDuration::UntilEndOfTurn,
            timestamp: 0,
        });

        let characteristics = engine.characteristics(oid).expect("characteristics");
        assert!(characteristics.colors.is_empty());
        assert!(!characteristics.has_keyword(Keyword::Flying));

        engine.state.continuous_effects.push(ContinuousEffect {
            trigger_grant_origin: None,
            source_id: None,
            affected: AffectedScope::Single(oid),
            kind: ContinuousEffectKind::Layer5SetColors(vec![Color::Red]),
            condition: None,
            duration: EffectDuration::UntilEndOfTurn,
            timestamp: 1,
        });
        assert_eq!(
            engine
                .characteristics(oid)
                .expect("characteristics after layer 5")
                .colors,
            vec![Color::Red]
        );
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
                token_faces: None,
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
                    land_types: Vec::new(),
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
                    land_types: Vec::new(),
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
                    land_types: Vec::new(),
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
                token_faces: None,
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
                token_faces: None,
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
            token_faces: None,
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
                token_faces: None,
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
            token_faces: None,
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

    /// Issue #342: a registry whose only cards are fixtures for public graveyard counts.
    fn graveyard_scaling_engine(seed: u64) -> GameEngine {
        let mut engine = GameEngine::new_with_default_decks(seed, &[0, 1], 20).expect("new engine");
        engine.registry = Box::leak(Box::new(
            CardRegistry::from_chunks_and_tokens(
                &[
                    r#"(id: "gy_source", name: "Gy Source", face_id: "gy_source", types: ["Creature"], power: 1, toughness: 1,
                        static_abilities: [(ability_id: "static_01", presentation: Fallback,
                            definition: CountScaledSelfPt(count: GraveyardCards(owners: Controller, filter: Some((card_type: Some(Creature)))), power_per_match: 1, toughness_per_match: 1))])"#,
                    r#"(id: "gy_affine", name: "Gy Affine", face_id: "gy_affine", types: ["Creature"], power: 1, toughness: 1,
                        static_abilities: [(ability_id: "static_01", presentation: Fallback,
                            definition: CountScaledSelfPt(count: Affine(constant: 0, terms: [(coefficient: 1, quantity: BattlefieldCreatures(filter: (controllers: Controller))), (coefficient: 1, quantity: GraveyardCards(owners: Controller, filter: Some((card_type: Some(Creature)))))]), power_per_match: 1, toughness_per_match: 1))])"#,
                    r#"(id: "gy_aura", name: "Gy Aura", face_id: "gy_aura", types: ["Enchantment", "Aura"],
                        spell_effect: [AuraAttach(target: (kind: Creature))],
                        static_abilities: [(ability_id: "static_01", presentation: Fallback,
                            definition: AttachedModifier(count: GraveyardCards(owners: Controller, filter: Some((card_type: Some(Creature)))), power_per_match: 1, toughness_per_match: 1))])"#,
                    r#"(id: "gy_creature", name: "Gy Creature", face_id: "gy_creature", types: ["Creature"], power: 1, toughness: 1)"#,
                    r#"(id: "gy_land", name: "Gy Land", face_id: "gy_land", types: ["Land"])"#,
                ],
                &[],
            )
            .expect("graveyard-scaling fixtures"),
        ));
        engine
    }

    fn insert_fixture(
        engine: &mut GameEngine,
        controller: PlayerId,
        card_id: &str,
        zone: Zone,
    ) -> ObjectId {
        let oid = engine.state.next_object_id;
        engine.state.next_object_id += 1;
        engine.state.objects.insert(
            oid,
            GameObject {
                id: oid,
                owner: controller,
                base_controller: controller,
                controller,
                card_id: card_id.to_string(),
                token_origin: None,
                token_faces: None,
                copiable_values: None,
                copy_revision: 0,
                zone,
                tapped: false,
                summoning_sick: false,
                power: Some(1),
                toughness: Some(1),
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
        match zone {
            Zone::Battlefield => engine.state.players[controller as usize]
                .battlefield
                .push(oid),
            Zone::Graveyard => engine.state.players[controller as usize]
                .graveyard
                .push(oid),
            _ => {}
        }
        oid
    }

    #[test]
    fn issue_342_graveyard_counts_scale_static_and_attached_pt() {
        let mut engine = graveyard_scaling_engine(342_001);
        let source = insert_fixture(&mut engine, 0, "gy_source", Zone::Battlefield);
        engine.emit_static_abilities_on_enter(source);
        assert_eq!(
            (
                engine.effective_power(source),
                engine.effective_toughness(source)
            ),
            (Some(1), Some(1)),
            "an empty graveyard contributes nothing"
        );

        insert_fixture(&mut engine, 0, "gy_creature", Zone::Graveyard);
        insert_fixture(&mut engine, 0, "gy_land", Zone::Graveyard);
        insert_fixture(&mut engine, 1, "gy_creature", Zone::Graveyard);
        assert_eq!(
            (
                engine.effective_power(source),
                engine.effective_toughness(source)
            ),
            (Some(2), Some(2)),
            "only the controller's graveyard creature cards count"
        );

        let aura = insert_fixture(&mut engine, 0, "gy_aura", Zone::Battlefield);
        engine.emit_static_abilities_on_enter(aura);
        assert_eq!(
            (
                engine.effective_power(source),
                engine.effective_toughness(source)
            ),
            (Some(2), Some(2)),
            "an unattached Aura contributes nothing"
        );
        engine
            .state
            .objects
            .get_mut(&aura)
            .expect("aura")
            .attached_to = Some(AttachmentRecipient::Object(source));
        assert_eq!(
            (
                engine.effective_power(source),
                engine.effective_toughness(source)
            ),
            (Some(3), Some(3)),
            "the attached graveyard count scaling adds a second point"
        );
    }

    #[test]
    fn issue_342_affine_static_scaling_combines_battlefield_and_graveyard() {
        let mut engine = graveyard_scaling_engine(342_002);
        let source = insert_fixture(&mut engine, 0, "gy_affine", Zone::Battlefield);
        engine.emit_static_abilities_on_enter(source);
        assert_eq!(
            engine.effective_power(source),
            Some(2),
            "the source counts itself as one battlefield creature"
        );

        insert_fixture(&mut engine, 0, "gy_creature", Zone::Battlefield);
        insert_fixture(&mut engine, 0, "gy_creature", Zone::Graveyard);
        assert_eq!(
            engine.effective_power(source),
            Some(4),
            "battlefield and graveyard terms both add"
        );

        insert_fixture(&mut engine, 1, "gy_creature", Zone::Battlefield);
        assert_eq!(
            engine.effective_power(source),
            Some(4),
            "opponents' permanents do not count"
        );
    }
}
