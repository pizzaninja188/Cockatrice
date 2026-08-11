//! Continuous-effect lifecycle.
//!
//! Rules-visible characteristic evaluation lives in `characteristics`; the CR 704 fixed-point
//! loop lives in `state_based`. This module owns creation and expiry of active effects.

use super::resolution::resolve_creature_scope;
use super::*;

impl GameEngine {
    /// Sum of extra land plays granted to `pid` by active `ExtraLandPlays` continuous effects.
    pub(super) fn extra_land_plays_for(&self, pid: PlayerId) -> u32 {
        self.state
            .continuous_effects
            .iter()
            .filter_map(|effect| {
                if let AffectedScope::Player(affected_player) = effect.affected {
                    let affected_player =
                        if effect.duration == EffectDuration::WhileSourceOnBattlefield {
                            effect
                                .source_id
                                .and_then(|source| self.controller_of(source))
                                .unwrap_or(affected_player)
                        } else {
                            affected_player
                        };
                    if affected_player == pid {
                        if let ContinuousEffectKind::ExtraLandPlays(count) = effect.kind {
                            return Some(count);
                        }
                    }
                }
                None
            })
            .sum()
    }

    /// CR 604.3 / 611.3: when a permanent with static anthem abilities enters the battlefield,
    /// push the corresponding `WhileSourceOnBattlefield` continuous effects. The LTB drain in
    /// `move_object_to_zone` removes them when the source leaves.
    pub(super) fn emit_static_abilities_on_enter(&mut self, object_id: ObjectId) {
        let Some(object) = self.state.objects.get(&object_id) else {
            return;
        };
        // CR 604.2 / 611.2: a static ability's continuous effect is created by the permanent's
        // controller, and `CreatureScopeController::YouControl` scopes off this value. Reading the owner
        // here would make a reanimated Glorious Anthem pump its *former* controller's creatures.
        let controller = object.controller;
        let card_id = object.card_id.clone();
        let effective_name = self.effective_face(object_id).map(|face| face.name.clone());
        let statics: Vec<StaticAbilityDef> = self
            .effective_face(object_id)
            .map(|face| face.static_abilities.to_vec())
            .unwrap_or_default();
        let timestamp = self.state.command_index;

        for static_ability in statics {
            match static_ability {
                StaticAbilityDef::EntersAsCopy { .. } => {
                    // CR 614.12 / 707.5 entry replacement, handled before zone commitment in
                    // `engine::replacement`; there is no post-entry continuous effect to emit.
                }
                StaticAbilityDef::EntersTapped { .. } => {
                    // CR 614.12 entry replacements are evaluated against the proposed event in
                    // `engine::replacement`; there is no post-entry continuous effect to emit.
                }
                StaticAbilityDef::EntersWithCounters { .. } => {
                    // CR 614.1c / 122.6 entry replacements are evaluated against the proposed
                    // event in `engine::replacement`; there is no post-entry effect to emit.
                }
                StaticAbilityDef::PreventDamage {
                    subject,
                    amount,
                    additional_effect,
                } => {
                    let scope = match subject {
                        DamagePreventionSubject::Source => {
                            DamagePreventionScope::Recipient(object_id)
                        }
                        DamagePreventionSubject::OtherCreaturesYouControl => {
                            DamagePreventionScope::OtherCreaturesYouControl {
                                source_id: object_id,
                                controller,
                            }
                        }
                    };
                    let amount = match amount {
                        StaticDamagePreventionAmount::All => DamagePreventionAmount::All,
                        StaticDamagePreventionAmount::FixedPerEvent(amount) => {
                            DamagePreventionAmount::FixedPerEvent(amount)
                        }
                    };
                    let id = self.state.next_damage_prevention_effect_id;
                    self.state.next_damage_prevention_effect_id = id.saturating_add(1);
                    let source_label = effective_name.clone().unwrap_or_else(|| card_id.clone());
                    self.state
                        .damage_prevention_effects
                        .push(ActiveDamagePrevention {
                            id,
                            source_id: Some(object_id),
                            source_label,
                            scope,
                            amount,
                            duration: EffectDuration::WhileSourceOnBattlefield,
                            additional_effect,
                        });
                }
                StaticAbilityDef::AnthemPt {
                    filter,
                    delta_power,
                    delta_toughness,
                } => {
                    self.state.continuous_effects.push(ContinuousEffect {
                        source_id: Some(object_id),
                        affected: resolve_creature_scope(&filter, controller, object_id),
                        kind: ContinuousEffectKind::PtModify {
                            delta_power,
                            delta_toughness,
                        },
                        duration: EffectDuration::WhileSourceOnBattlefield,
                        timestamp,
                    });
                }
                StaticAbilityDef::AttachedModifier {
                    delta_power,
                    delta_toughness,
                    keywords,
                    cant_attack,
                    cant_block,
                } => {
                    let affected = AffectedScope::AttachedTo(object_id);
                    if delta_power != 0 || delta_toughness != 0 {
                        self.state.continuous_effects.push(ContinuousEffect {
                            source_id: Some(object_id),
                            affected: affected.clone(),
                            kind: ContinuousEffectKind::PtModify {
                                delta_power,
                                delta_toughness,
                            },
                            duration: EffectDuration::WhileSourceOnBattlefield,
                            timestamp,
                        });
                    }
                    for keyword in keywords {
                        self.state.continuous_effects.push(ContinuousEffect {
                            source_id: Some(object_id),
                            affected: affected.clone(),
                            kind: ContinuousEffectKind::Layer6AddKeyword(keyword),
                            duration: EffectDuration::WhileSourceOnBattlefield,
                            timestamp,
                        });
                    }
                    if cant_attack || cant_block {
                        self.state.continuous_effects.push(ContinuousEffect {
                            source_id: Some(object_id),
                            affected,
                            kind: ContinuousEffectKind::CombatRestriction {
                                cant_attack,
                                cant_block,
                            },
                            duration: EffectDuration::WhileSourceOnBattlefield,
                            timestamp,
                        });
                    }
                }
                StaticAbilityDef::ControlsAttached => {
                    self.state.continuous_effects.push(ContinuousEffect {
                        source_id: Some(object_id),
                        affected: AffectedScope::AttachedTo(object_id),
                        kind: ContinuousEffectKind::Layer2Control {
                            controller: ControllerReference::SourceController,
                        },
                        duration: EffectDuration::WhileSourceOnBattlefield,
                        timestamp,
                    });
                }
                StaticAbilityDef::AnthemKeyword { filter, keyword } => {
                    self.state.continuous_effects.push(ContinuousEffect {
                        source_id: Some(object_id),
                        affected: resolve_creature_scope(&filter, controller, object_id),
                        kind: ContinuousEffectKind::Layer6AddKeyword(keyword),
                        duration: EffectDuration::WhileSourceOnBattlefield,
                        timestamp,
                    });
                }
                StaticAbilityDef::ExtraLandPlays { count } => {
                    self.state.continuous_effects.push(ContinuousEffect {
                        source_id: Some(object_id),
                        affected: AffectedScope::Player(controller),
                        kind: ContinuousEffectKind::ExtraLandPlays(count),
                        duration: EffectDuration::WhileSourceOnBattlefield,
                        timestamp,
                    });
                }
            }
        }
    }

    /// CR 514.2: drain all until-end-of-turn continuous effects.
    pub(super) fn cleanup_until_end_of_turn_effects(&mut self) {
        self.state
            .continuous_effects
            .retain(|effect| effect.duration != EffectDuration::UntilEndOfTurn);
    }

    /// CR 514.2: remove marked damage and expire turn-scoped prevention/regeneration shields.
    pub(super) fn cleanup_marked_damage(&mut self) {
        for object in self.state.objects.values_mut() {
            if object.zone == Zone::Battlefield {
                object.damage = 0;
                object.deathtouch_damage = false;
                object.regeneration_shields = 0;
            }
        }
        self.state
            .damage_prevention_effects
            .retain(|effect| effect.duration != EffectDuration::UntilEndOfTurn);
        self.state.damage_prevention_prohibitions.clear();
    }
}
