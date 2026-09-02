//! Continuous-effect lifecycle.
//!
//! Rules-visible characteristic evaluation lives in `characteristics`; the CR 704 fixed-point
//! loop lives in `state_based`. This module owns creation and expiry of active effects.

use super::resolution::resolve_creature_scope;
use super::*;

impl GameEngine {
    /// Heirloom Auntie / Brambleback Brute: effects saturate; costs preflight the full debit.
    pub(super) fn remove_counters(
        &mut self,
        target: ObjectId,
        kind: CounterKind,
        count: u32,
    ) -> u32 {
        let Some(object) = self
            .state
            .objects
            .get_mut(&target)
            .filter(|o| o.zone == Zone::Battlefield)
        else {
            return 0;
        };
        let before = object.counter_count(kind);
        let removed = before.min(count);
        object.set_counter(kind, before - removed);
        if kind == CounterKind::Defense
            && before > 0
            && before == removed
            && self
                .characteristics(target)
                .is_some_and(|c| c.has_type("Battle") && c.has_type("Siege"))
        {
            self.stage_siege_defeat_trigger(target);
        }
        removed
    }
    /// Tatterkite / Blossombind: consult live copied abilities and attachment membership.
    /// This also runs after an entrant's face is established, before its entry counters.
    pub(super) fn can_receive_counters(&self, target: ObjectId) -> bool {
        if !self
            .state
            .objects
            .get(&target)
            .is_some_and(|o| o.zone == Zone::Battlefield)
        {
            return false;
        }
        !self.state.objects.iter().any(|(&source, object)| {
            if object.zone != Zone::Battlefield || object.face_down
                || super::characteristics::latest_remove_all_abilities_timestamp(&self.state, source).is_some()
            {
                return false;
            }
            self.effective_face(source).is_some_and(|face| face.static_abilities.iter().any(|ability| {
                match &ability.definition {
                    StaticAbilityDef::ProhibitCounters { affected: tricerules_cards::primitives::CounterPlacementAffected::Self_ } => source == target,
                    StaticAbilityDef::ProhibitCounters { affected: tricerules_cards::primitives::CounterPlacementAffected::AttachedPermanent } => object.attached_to == Some(AttachmentRecipient::Object(target)),
                    _ => false,
                }
            }))
        })
    }

    /// The sole gameplay counter-placement funnel. Removal and fixture construction are separate.
    pub(super) fn place_counters(
        &mut self,
        target: ObjectId,
        kind: CounterKind,
        count: u32,
    ) -> u32 {
        if count == 0 || !self.can_receive_counters(target) {
            return 0;
        }
        let timestamp = self.state.command_index;
        self.state
            .objects
            .get_mut(&target)
            .expect("validated counter recipient")
            .add_counters(kind, count, timestamp);
        count
    }

    /// CR 119.7 / 614.17: query the prohibition before committing a life-gain event.
    /// Printed and copied abilities are live only on the battlefield and while not blanked
    /// in layer 6. There is no historical source binding to retain after a zone change.
    ///
    /// Life-setting, exchanges/redistribution, and gain-life costs are not currently supported.
    /// Future upward setters must use the gain funnel; exchanges and gain-life costs must
    /// consult this query before committing any part of their transaction (CR 119.5 / 119.7).
    pub(super) fn can_player_gain_life(&self, player: PlayerId) -> bool {
        if self.state.player_idx(player).is_none() {
            return false;
        }
        !self.state.objects.iter().any(|(&oid, object)| {
            if object.zone != Zone::Battlefield
                || object.face_down
                || super::characteristics::latest_remove_all_abilities_timestamp(&self.state, oid)
                    .is_some()
            {
                return false;
            }
            let Some(face) = self.effective_face(oid) else {
                return false;
            };
            face.static_abilities.iter().any(|ability| {
                let StaticAbilityDef::ProhibitLifeGain { players } = &ability.definition else {
                    return false;
                };
                self.controller_of(oid).is_some_and(|controller| {
                    super::history::relative_player_set_contains(
                        &self.state,
                        *players,
                        controller,
                        player,
                    )
                })
            })
        })
    }

    pub(super) fn continuous_effect_condition_holds(&self, effect: &ContinuousEffect) -> bool {
        let Some(condition) = effect.condition.as_ref() else {
            return true;
        };
        let Some(source_oid) = effect.source_id else {
            return false;
        };
        let Some(controller) = self.controller_of(source_oid) else {
            return false;
        };
        self.condition_holds(
            condition,
            ConditionContext {
                controller,
                source_object_id: source_oid,
                source_zone_change: self
                    .state
                    .zone_change_generation
                    .get(&source_oid)
                    .copied()
                    .unwrap_or(0),
                resolving_spell_id: None,
                stack_item: None,
            },
        )
    }

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
        if object.face_down {
            return;
        }
        if super::characteristics::latest_remove_all_abilities_timestamp(&self.state, object_id)
            .is_some()
        {
            return;
        }
        // CR 604.2 / 611.2: a static ability's continuous effect is created by the permanent's
        // controller, and `CreatureScopeController::YouControl` scopes off this value. Reading the owner
        // here would make a reanimated Glorious Anthem pump its *former* controller's creatures.
        let controller = object.controller;
        let card_id = object.card_id.clone();
        let effective_name = self.effective_face(object_id).map(|face| face.name.clone());
        let mut statics = Vec::new();
        if let Some(faces) = self.room_faces(object_id) {
            for door in self
                .state
                .room_states
                .get(&object_id)
                .copied()
                .unwrap_or_default()
                .unlocked_indices()
            {
                for ability in &faces[door].static_abilities {
                    statics.push((
                        self.ability_definition(object_id, door, vec![ability.ability_id.clone()]),
                        ability.clone(),
                    ));
                }
            }
        } else if let Some(face) = self.effective_face(object_id) {
            for ability in &face.static_abilities {
                statics.push((
                    self.ability_definition(
                        object_id,
                        object.face_up_index,
                        vec![ability.ability_id.clone()],
                    ),
                    ability.clone(),
                ));
            }
        }
        let source_zone_change = self
            .state
            .zone_change_generation
            .get(&object_id)
            .copied()
            .unwrap_or(0);
        let timestamp = self.state.command_index;

        for (definition, static_ability) in statics {
            match static_ability.definition {
                StaticAbilityDef::ProhibitLifeGain { .. }
                | StaticAbilityDef::ProhibitCounters { .. } => {
                    // Queried at each life-gain event; no independent effect record is needed.
                }
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
                StaticAbilityDef::UntapsDuringOtherPlayersUntapSteps => {
                    // CR 502.3 turn-based hook, queried at the untap boundary. It emits no
                    // independent continuous-effect record.
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
                    condition,
                    delta_power,
                    delta_toughness,
                } => {
                    self.state.continuous_effects.push(ContinuousEffect {
                        trigger_grant_origin: None,
                        source_id: Some(object_id),
                        affected: resolve_creature_scope(&filter, controller, object_id),
                        kind: ContinuousEffectKind::PtModify {
                            delta_power,
                            delta_toughness,
                        },
                        condition,
                        duration: EffectDuration::WhileSourceOnBattlefield,
                        timestamp,
                    });
                }
                StaticAbilityDef::TargetingCostIncrease { .. }
                | StaticAbilityDef::SpellGenericReduction { .. } => {
                    // Evaluated live while an action's targets and total cost are finalized.
                }
                StaticAbilityDef::ProhibitSpecialAction {
                    action,
                    affected,
                    condition,
                } => {
                    let affected = match affected {
                        SpecialActionAffected::AttachedPermanent => {
                            AffectedScope::AttachedTo(object_id)
                        }
                        SpecialActionAffected::Permanents(filter) => {
                            AffectedScope::PermanentsMatching {
                                reference_player: controller,
                                filter: *filter,
                                exclude: None,
                            }
                        }
                    };
                    self.state.continuous_effects.push(ContinuousEffect {
                        trigger_grant_origin: None,
                        source_id: Some(object_id),
                        affected,
                        kind: ContinuousEffectKind::ProhibitSpecialAction(action),
                        condition,
                        duration: EffectDuration::WhileSourceOnBattlefield,
                        timestamp,
                    });
                }
                StaticAbilityDef::AttachedModifier {
                    condition,
                    add_types,
                    set_types,
                    set_name,
                    set_colors,
                    delta_power,
                    delta_toughness,
                    set_power,
                    set_toughness,
                    remove_all_abilities,
                    keywords,
                    activated_abilities,
                    triggered_abilities,
                    cant_attack,
                    cant_block,
                    doesnt_untap_during_untap_step,
                    cant_untap,
                } => {
                    let affected = AffectedScope::AttachedTo(object_id);
                    if let Some(name) = set_name {
                        self.state.continuous_effects.push(ContinuousEffect {
                            trigger_grant_origin: None,
                            source_id: Some(object_id),
                            affected: affected.clone(),
                            kind: ContinuousEffectKind::Layer3SetName(name),
                            condition: condition.clone(),
                            duration: EffectDuration::WhileSourceOnBattlefield,
                            timestamp,
                        });
                    }
                    if !add_types.is_empty() {
                        self.state.continuous_effects.push(ContinuousEffect {
                            trigger_grant_origin: None,
                            source_id: Some(object_id),
                            affected: affected.clone(),
                            kind: ContinuousEffectKind::Layer4AddTypes(add_types),
                            condition: condition.clone(),
                            duration: EffectDuration::WhileSourceOnBattlefield,
                            timestamp,
                        });
                    }
                    if let Some(replacement) = set_types {
                        self.state.continuous_effects.push(ContinuousEffect {
                            trigger_grant_origin: None,
                            source_id: Some(object_id),
                            affected: affected.clone(),
                            kind: ContinuousEffectKind::Layer4SetTypeLine(replacement),
                            condition: condition.clone(),
                            duration: EffectDuration::WhileSourceOnBattlefield,
                            timestamp,
                        });
                    }
                    if let Some(colors) = set_colors {
                        self.state.continuous_effects.push(ContinuousEffect {
                            trigger_grant_origin: None,
                            source_id: Some(object_id),
                            affected: affected.clone(),
                            kind: ContinuousEffectKind::Layer5SetColors(colors),
                            condition: condition.clone(),
                            duration: EffectDuration::WhileSourceOnBattlefield,
                            timestamp,
                        });
                    }
                    if delta_power != 0 || delta_toughness != 0 {
                        self.state.continuous_effects.push(ContinuousEffect {
                            trigger_grant_origin: None,
                            source_id: Some(object_id),
                            affected: affected.clone(),
                            kind: ContinuousEffectKind::PtModify {
                                delta_power,
                                delta_toughness,
                            },
                            condition: condition.clone(),
                            duration: EffectDuration::WhileSourceOnBattlefield,
                            timestamp,
                        });
                    }
                    if let (Some(power), Some(toughness)) = (set_power, set_toughness) {
                        self.state.continuous_effects.push(ContinuousEffect {
                            trigger_grant_origin: None,
                            source_id: Some(object_id),
                            affected: affected.clone(),
                            kind: ContinuousEffectKind::Layer7bSetPt {
                                power: i64::from(power),
                                toughness: i64::from(toughness),
                            },
                            condition: condition.clone(),
                            duration: EffectDuration::WhileSourceOnBattlefield,
                            timestamp,
                        });
                    }
                    if remove_all_abilities {
                        self.state.continuous_effects.push(ContinuousEffect {
                            trigger_grant_origin: None,
                            source_id: Some(object_id),
                            affected: affected.clone(),
                            kind: ContinuousEffectKind::Layer6RemoveAllAbilities,
                            condition: condition.clone(),
                            duration: EffectDuration::WhileSourceOnBattlefield,
                            timestamp,
                        });
                    }
                    for keyword in keywords {
                        self.state.continuous_effects.push(ContinuousEffect {
                            trigger_grant_origin: None,
                            source_id: Some(object_id),
                            affected: affected.clone(),
                            kind: ContinuousEffectKind::Layer6AddKeyword(keyword),
                            condition: condition.clone(),
                            duration: EffectDuration::WhileSourceOnBattlefield,
                            timestamp,
                        });
                    }
                    for ability in activated_abilities {
                        let mut granted_definition = definition.clone();
                        granted_definition
                            .ability_path
                            .push(ability.ability_id.clone());
                        self.state.continuous_effects.push(ContinuousEffect {
                            trigger_grant_origin: Some(TriggerAbilityOrigin::StaticGrant {
                                source_id: object_id,
                                source_zone_change,
                                definition: granted_definition,
                            }),
                            source_id: Some(object_id),
                            affected: affected.clone(),
                            kind: ContinuousEffectKind::GrantActivatedAbility(Box::new(ability)),
                            condition: None,
                            duration: EffectDuration::WhileSourceOnBattlefield,
                            timestamp,
                        });
                    }
                    for ability in triggered_abilities {
                        let mut granted_definition = definition.clone();
                        granted_definition
                            .ability_path
                            .push(ability.ability_id.clone());
                        self.state.add_triggered_ability_grant(ContinuousEffect {
                            trigger_grant_origin: Some(TriggerAbilityOrigin::StaticGrant {
                                source_id: object_id,
                                source_zone_change,
                                definition: granted_definition,
                            }),
                            source_id: Some(object_id),
                            affected: affected.clone(),
                            kind: ContinuousEffectKind::GrantTriggeredAbility(Box::new(ability)),
                            condition: None,
                            duration: EffectDuration::WhileSourceOnBattlefield,
                            timestamp,
                        });
                    }
                    if cant_attack || cant_block {
                        self.state.continuous_effects.push(ContinuousEffect {
                            trigger_grant_origin: None,
                            source_id: Some(object_id),
                            affected: affected.clone(),
                            kind: ContinuousEffectKind::CombatRestriction(CombatRestriction {
                                cant_attack,
                                cant_block,
                                ..Default::default()
                            }),
                            condition: None,
                            duration: EffectDuration::WhileSourceOnBattlefield,
                            timestamp,
                        });
                    }
                    if cant_untap {
                        self.state.continuous_effects.push(ContinuousEffect {
                            trigger_grant_origin: None,
                            source_id: Some(object_id),
                            affected: affected.clone(),
                            kind: ContinuousEffectKind::ProhibitUntap,
                            condition: condition.clone(),
                            duration: EffectDuration::WhileSourceOnBattlefield,
                            timestamp,
                        });
                    }
                    if doesnt_untap_during_untap_step {
                        self.state.continuous_effects.push(ContinuousEffect {
                            trigger_grant_origin: None,
                            source_id: Some(object_id),
                            affected,
                            kind: ContinuousEffectKind::DoesntUntapDuringUntapStep,
                            condition: None,
                            duration: EffectDuration::WhileSourceOnBattlefield,
                            timestamp,
                        });
                    }
                }
                StaticAbilityDef::SelfCombatRestriction {
                    restriction,
                    condition,
                } => {
                    self.state.continuous_effects.push(ContinuousEffect {
                        trigger_grant_origin: None,
                        source_id: Some(object_id),
                        affected: AffectedScope::Single(object_id),
                        kind: ContinuousEffectKind::CombatRestriction(restriction),
                        condition,
                        duration: EffectDuration::WhileSourceOnBattlefield,
                        timestamp,
                    });
                }
                StaticAbilityDef::CreatureScopeCombatRestriction {
                    filter,
                    restriction,
                } => {
                    self.state.continuous_effects.push(ContinuousEffect {
                        trigger_grant_origin: None,
                        source_id: Some(object_id),
                        affected: resolve_creature_scope(&filter, controller, object_id),
                        kind: ContinuousEffectKind::CombatRestriction(restriction),
                        condition: None,
                        duration: EffectDuration::WhileSourceOnBattlefield,
                        timestamp,
                    });
                }
                StaticAbilityDef::ControlsAttached => {
                    self.state.continuous_effects.push(ContinuousEffect {
                        trigger_grant_origin: None,
                        source_id: Some(object_id),
                        affected: AffectedScope::AttachedTo(object_id),
                        kind: ContinuousEffectKind::Layer2Control {
                            controller: ControllerReference::SourceController,
                        },
                        condition: None,
                        duration: EffectDuration::WhileSourceOnBattlefield,
                        timestamp,
                    });
                }
                StaticAbilityDef::AnthemKeyword {
                    filter,
                    condition,
                    keyword,
                } => {
                    self.state.continuous_effects.push(ContinuousEffect {
                        trigger_grant_origin: None,
                        source_id: Some(object_id),
                        affected: resolve_creature_scope(&filter, controller, object_id),
                        kind: ContinuousEffectKind::Layer6AddKeyword(keyword),
                        condition,
                        duration: EffectDuration::WhileSourceOnBattlefield,
                        timestamp,
                    });
                }
                StaticAbilityDef::ConditionalSelfModifier {
                    condition,
                    delta_power,
                    delta_toughness,
                    keywords,
                    triggered_abilities,
                    can_attack_as_though_without_defender,
                } => {
                    let affected = AffectedScope::Single(object_id);
                    if delta_power != 0 || delta_toughness != 0 {
                        self.state.continuous_effects.push(ContinuousEffect {
                            trigger_grant_origin: None,
                            source_id: Some(object_id),
                            affected: affected.clone(),
                            kind: ContinuousEffectKind::PtModify {
                                delta_power,
                                delta_toughness,
                            },
                            condition: Some(condition.clone()),
                            duration: EffectDuration::WhileSourceOnBattlefield,
                            timestamp,
                        });
                    }
                    for keyword in keywords {
                        self.state.continuous_effects.push(ContinuousEffect {
                            trigger_grant_origin: None,
                            source_id: Some(object_id),
                            affected: affected.clone(),
                            kind: ContinuousEffectKind::Layer6AddKeyword(keyword),
                            condition: Some(condition.clone()),
                            duration: EffectDuration::WhileSourceOnBattlefield,
                            timestamp,
                        });
                    }
                    for ability in triggered_abilities {
                        let mut granted_definition = definition.clone();
                        granted_definition
                            .ability_path
                            .push(ability.ability_id.clone());
                        self.state.add_triggered_ability_grant(ContinuousEffect {
                            trigger_grant_origin: Some(TriggerAbilityOrigin::StaticGrant {
                                source_id: object_id,
                                source_zone_change,
                                definition: granted_definition,
                            }),
                            source_id: Some(object_id),
                            affected: affected.clone(),
                            kind: ContinuousEffectKind::GrantTriggeredAbility(Box::new(ability)),
                            condition: Some(condition.clone()),
                            duration: EffectDuration::WhileSourceOnBattlefield,
                            timestamp,
                        });
                    }
                    if can_attack_as_though_without_defender {
                        self.state.continuous_effects.push(ContinuousEffect {
                            trigger_grant_origin: None,
                            source_id: Some(object_id),
                            affected,
                            kind: ContinuousEffectKind::AttackAsThoughWithoutDefender,
                            condition: Some(condition),
                            duration: EffectDuration::WhileSourceOnBattlefield,
                            timestamp,
                        });
                    }
                }
                StaticAbilityDef::CountScaledSelfPt {
                    count,
                    power_per_match,
                    toughness_per_match,
                } => {
                    self.state.continuous_effects.push(ContinuousEffect {
                        trigger_grant_origin: None,
                        source_id: Some(object_id),
                        affected: AffectedScope::Single(object_id),
                        kind: ContinuousEffectKind::PtModifyByCount {
                            count,
                            power_per_match,
                            toughness_per_match,
                        },
                        condition: None,
                        duration: EffectDuration::WhileSourceOnBattlefield,
                        timestamp,
                    });
                }
                StaticAbilityDef::ExtraLandPlays { count } => {
                    self.state.continuous_effects.push(ContinuousEffect {
                        trigger_grant_origin: None,
                        source_id: Some(object_id),
                        affected: AffectedScope::Player(controller),
                        kind: ContinuousEffectKind::ExtraLandPlays(count),
                        condition: None,
                        duration: EffectDuration::WhileSourceOnBattlefield,
                        timestamp,
                    });
                }
            }
        }
    }

    /// CR 113.10 / 613.1f: printed abilities retain their card-definition order, followed by
    /// applicable granted abilities in continuous-effect timestamp and insertion order.
    pub(super) fn effective_activated_abilities(
        &self,
        source_id: ObjectId,
    ) -> Vec<(
        usize,
        ActivatedAbilityDef,
        bool,
        Vec<tricerules_cards::AbilityId>,
    )> {
        let face_down = self
            .state
            .objects
            .get(&source_id)
            .is_some_and(|object| object.face_down);
        let removed_at =
            super::characteristics::latest_remove_all_abilities_timestamp(&self.state, source_id);
        let mut abilities: Vec<(
            usize,
            ActivatedAbilityDef,
            bool,
            Vec<tricerules_cards::AbilityId>,
        )> = (!face_down && removed_at.is_none())
            .then(|| self.effective_face(source_id))
            .flatten()
            .map(|face| {
                face.activated_abilities
                    .iter()
                    .enumerate()
                    .filter(|(_, ability)| ability.source_zone == AbilitySourceZone::Battlefield)
                    .map(|(index, ability)| {
                        (
                            index,
                            ability.clone(),
                            false,
                            vec![ability.ability_id.clone()],
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        let Some(characteristics) = self.characteristics(source_id) else {
            return abilities;
        };
        let mut granted: Vec<(
            u64,
            usize,
            ActivatedAbilityDef,
            Vec<tricerules_cards::AbilityId>,
        )> = self
            .state
            .continuous_effects
            .iter()
            .enumerate()
            .filter_map(|(insertion_index, effect)| {
                let ContinuousEffectKind::GrantActivatedAbility(ability) = &effect.kind else {
                    return None;
                };
                if removed_at.is_some_and(|timestamp| effect.timestamp <= timestamp) {
                    return None;
                }
                super::characteristics::effect_affects(
                    &self.state,
                    self.registry,
                    effect,
                    source_id,
                    &characteristics,
                )
                .then(|| {
                    let path = match effect.trigger_grant_origin.as_ref() {
                        Some(TriggerAbilityOrigin::StaticGrant { definition, .. }) => {
                            definition.ability_path.clone()
                        }
                        _ => vec![ability.ability_id.clone()],
                    };
                    (effect.timestamp, insertion_index, (**ability).clone(), path)
                })
            })
            .collect();
        granted.sort_by_key(|(timestamp, insertion_index, _, _)| (*timestamp, *insertion_index));
        let mut next_index = abilities.len();
        abilities.extend(granted.into_iter().map(|(_, _, ability, path)| {
            let indexed = (next_index, ability, true, path);
            next_index += 1;
            indexed
        }));
        abilities
    }

    /// CR 514.2: drain all until-end-of-turn continuous effects.
    pub(super) fn cleanup_until_end_of_turn_effects(&mut self) {
        self.state
            .continuous_effects
            .retain(|effect| effect.duration != EffectDuration::UntilEndOfTurn);
        self.state.active_event_observers.retain(|observer| {
            observer.matcher != EventObserverMatcher::WhenWatchedObjectDiesThisTurn
        });
    }

    pub(super) fn special_action_prohibited(
        &self,
        object_id: ObjectId,
        action: SpecialActionKind,
    ) -> bool {
        let Some(characteristics) = self.characteristics(object_id) else {
            return false;
        };
        self.state.continuous_effects.iter().any(|effect| {
            if !matches!(effect.kind, ContinuousEffectKind::ProhibitSpecialAction(kind) if kind == action)
                || !super::characteristics::effect_affects(
                    &self.state,
                    self.registry,
                    effect,
                    object_id,
                    &characteristics,
                )
            {
                return false;
            }
            let Some(condition) = effect.condition.as_ref() else {
                return true;
            };
            let Some(source_id) = effect.source_id else {
                return false;
            };
            let Some(controller) = self.controller_of(source_id) else {
                return false;
            };
            self.condition_holds(
                condition,
                ConditionContext {
                    controller,
                    source_object_id: source_id,
                    source_zone_change: self
                        .state
                        .zone_change_generation
                        .get(&source_id)
                        .copied()
                        .unwrap_or(0),
                    resolving_spell_id: None,
                    stack_item: None,
                },
            )
        })
    }

    /// CR 502.3: whether the normal untap-step turn-based action excludes this permanent.
    /// Explicit untap effects do not consult this restriction.
    pub(super) fn doesnt_untap_during_untap_step(&self, oid: ObjectId) -> bool {
        let Some(characteristics) = self.characteristics(oid) else {
            return false;
        };
        self.state.continuous_effects.iter().any(|effect| {
            matches!(
                effect.kind,
                ContinuousEffectKind::DoesntUntapDuringUntapStep
            ) && super::characteristics::effect_affects(
                &self.state,
                self.registry,
                effect,
                oid,
                &characteristics,
            )
        })
    }

    /// CR 502.3: this printed/copied static ability adds an untap attempt during each other
    /// player's untap step. A layer-6 remove-all-abilities effect suppresses the hook.
    pub(super) fn untaps_during_other_players_untap_steps(&self, oid: ObjectId) -> bool {
        let Some(object) = self.state.objects.get(&oid) else {
            return false;
        };
        if object.zone != Zone::Battlefield
            || object.face_down
            || super::characteristics::latest_remove_all_abilities_timestamp(&self.state, oid)
                .is_some()
        {
            return false;
        }
        self.effective_face(oid).is_some_and(|face| {
            face.static_abilities.iter().any(|ability| {
                ability.definition == StaticAbilityDef::UntapsDuringOtherPlayersUntapSteps
            })
        })
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
        self.state.death_replacement_effects.clear();
    }
}
