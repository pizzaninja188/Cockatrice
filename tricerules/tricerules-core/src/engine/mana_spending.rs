//! Source-bound additional effects carried by individual mana contributions (CR 106.6).

use super::triggers::CollectedTrigger;
use super::*;
use crate::state::{ManaSpendingOrigin, SpellCastFact};
use std::collections::BTreeMap;
use tricerules_card_model::primitives::{ManaSpendingEffect, ManaSpendingRestriction};
use tricerules_card_model::{AbilityId, AbilityPresentation};

pub(super) fn restriction_requires_source_tracking(restriction: &ManaSpendingRestriction) -> bool {
    restriction.spending_effects.iter().any(|effect| {
        matches!(
            effect,
            ManaSpendingEffect::Scry1IfSpellSharesCreatureTypeWithCommander
        )
    })
}

pub(super) fn source_bound_mana_group_label(
    restriction: &ManaSpendingRestriction,
    origin: &ManaSpendingOrigin,
    restriction_group_id: u32,
) -> String {
    format!(
        "{} (player {}, mana group #{}) — {}",
        origin.source_label,
        origin.controller,
        restriction_group_id,
        restriction.fallback_label()
    )
}

fn mana_amount_total(amount: ManaAmount) -> u64 {
    [amount.w, amount.u, amount.b, amount.r, amount.g, amount.c]
        .into_iter()
        .map(u64::from)
        .sum()
}

fn shares_creature_type(
    left: &[String],
    left_all_creature_types: bool,
    right: &[String],
    right_all_creature_types: bool,
) -> bool {
    if left_all_creature_types && right_all_creature_types {
        return true;
    }
    if left_all_creature_types {
        return right.iter().any(|subtype| is_creature_type(subtype));
    }
    if right_all_creature_types {
        return left.iter().any(|subtype| is_creature_type(subtype));
    }
    left.iter().any(|left_type| {
        is_creature_type(left_type) && right.iter().any(|right_type| left_type == right_type)
    })
}

fn scry_trigger_ability() -> TriggeredAbilityDef {
    TriggeredAbilityDef {
        ability_id: AbilityId::new("commander_creature_scry")
            .expect("synthetic Path of Ancestry ability id is valid"),
        presentation: AbilityPresentation::Fallback,
        // The trigger is collected explicitly from its spent-mana event. This event descriptor is
        // retained for deterministic trigger presentation and stack resolution only.
        trigger: TriggerCondition::WheneverPlayerCastsSpell {
            caster: CastTriggerPlayer::Controller,
            filter: SpellCastFilter {
                card_type: Some(CardTypeFilter::Creature),
                ..Default::default()
            },
            ordinal: None,
            ordinal_scope: CastOrdinalScope::AllSpells,
        },
        effect: vec![SpellEffectKind::Scry {
            count: Amount::Fixed(1),
        }],
        program: None,
        modal: None,
        targeting: None,
        may: false,
        intervening_if: None,
        triggers_only_once: false,
        max_triggers_per_turn: None,
    }
}

impl GameEngine {
    /// Evaluate an object using the characteristics that apply in its current rules zone. An
    /// object that is a spell on the stack uses the face selected for that spell, even when the
    /// physical card's face-up index still names its front face (CR 715.3b).
    fn current_rules_characteristics(
        &self,
        object_id: ObjectId,
    ) -> Option<super::characteristics::Characteristics> {
        let Some(item) = self.state.stack.iter().find(|item| item.id == object_id) else {
            return self.characteristics(object_id);
        };
        let face = self.registry.get(&item.card_id)?.face(item.face_index)?;
        Some(super::characteristics::spell_cast_characteristics(
            &self.state,
            self.registry,
            item.id,
            item.controller,
            face,
        ))
    }

    /// Keep the public mana group id stable while giving each source-bound trigger rider a
    /// separate group, even when two mana sources carry structurally identical restrictions.
    pub(super) fn register_mana_restriction_group(
        &mut self,
        restriction: &ManaSpendingRestriction,
        restriction_presentation: Option<rv1::PresentationRef>,
        origin: Option<ManaSpendingOrigin>,
        preferred_group_id: Option<u32>,
    ) -> Result<u32, EngineError> {
        let source_bound = restriction_requires_source_tracking(restriction);
        if source_bound != origin.is_some() {
            return Err(EngineError::Illegal(
                "mana spending origin does not match its spending effects",
            ));
        }

        if let Some(group_id) = preferred_group_id {
            if group_id == 0 {
                return Err(EngineError::Illegal(
                    "invalid preferred mana restriction group",
                ));
            }
            let matches_registry = self
                .state
                .mana_restriction_groups
                .get(group_id)
                .is_some_and(|group| {
                    &group.restriction == restriction && group.origin.as_ref() == origin.as_ref()
                });
            if !matches_registry {
                return Err(EngineError::Illegal(
                    "replayed mana restriction group no longer matches its source",
                ));
            }
            return Ok(group_id);
        }

        if !source_bound {
            if let Some((group_id, _)) = self
                .state
                .mana_restriction_groups
                .iter()
                .find(|(_, group)| &group.restriction == restriction)
            {
                return Ok(group_id);
            }
        }

        Ok(self
            .state
            .mana_restriction_groups
            .push(crate::state::ManaRestrictionGroup {
                restriction: restriction.clone(),
                presentation: restriction_presentation,
                origin,
            }))
    }

    /// Produce the delayed triggers for the *committed* restricted mana in one completed spell
    /// cast. Cost previews and client-supplied selections are never trigger evidence.
    pub(super) fn collect_mana_spending_triggers(
        &self,
        fact: &SpellCastFact,
        restricted_mana_spent: &[(u32, ManaAmount)],
    ) -> Vec<CollectedTrigger> {
        let Some(spell_characteristics) =
            self.current_rules_characteristics(fact.occurrence.object_id)
        else {
            return Vec::new();
        };
        if !spell_characteristics.has_type("Creature") {
            return Vec::new();
        }

        let mut units_by_group = BTreeMap::<u32, u64>::new();
        for (group_id, amount) in restricted_mana_spent {
            let units = mana_amount_total(*amount);
            if units > 0 {
                let total = units_by_group.entry(*group_id).or_default();
                *total = total.saturating_add(units);
            }
        }

        let mut triggers = Vec::new();
        for (group_id, units) in units_by_group {
            let Some(group) = self.state.mana_restriction_groups.get(group_id) else {
                continue;
            };
            let restriction = &group.restriction;
            if !restriction.spending_effects.iter().any(|effect| {
                matches!(
                    effect,
                    ManaSpendingEffect::Scry1IfSpellSharesCreatureTypeWithCommander
                )
            }) {
                continue;
            }
            let Some(origin) = group.origin.as_ref() else {
                continue;
            };
            let shares_commander_type = self
                .state
                .players
                .iter()
                // “Your commander” remains anchored to the player who controlled the mana
                // ability, even if CR 106.13 transfers the mana to a different spell caster.
                .find(|player| player.id == origin.controller)
                .into_iter()
                .flat_map(|player| player.declared_commander_object_ids.iter().copied())
                .filter_map(|commander| {
                    let object = self.state.objects.get(&commander)?;
                    // CR 406.3a: a face-down exiled card has no characteristics.
                    if object.zone == Zone::Exile && object.face_down {
                        return None;
                    }
                    self.current_rules_characteristics(commander)
                })
                .any(|commander| {
                    shares_creature_type(
                        &spell_characteristics.types,
                        spell_characteristics.all_creature_types,
                        &commander.types,
                        commander.all_creature_types,
                    )
                });
            if !shares_commander_type {
                continue;
            }

            let total_instances = units;
            let ability = scry_trigger_ability();
            let ability_text = format!("{} — scry 1", origin.source_label);
            let presentation = origin.ability_presentation.as_ref().map(|reference| {
                let mut reference = reference.clone();
                reference.fallback_text = ability_text.clone();
                reference
            });
            let mut remaining = total_instances;
            while remaining > 0 {
                // `additional_instances` encodes one base trigger plus up to u32::MAX more.
                let instances = remaining.min(u64::from(u32::MAX) + 1);
                triggers.push(CollectedTrigger {
                    captured_spell: None,
                    source_id: origin.source_object_id,
                    card_id: origin.card_id.clone(),
                    source_label: origin.source_label.clone(),
                    face_index: origin.source_face_index,
                    source_zone_change: origin.source_zone_change_generation,
                    source_face_change: origin.source_face_change_generation,
                    source_fact: None,
                    additional_instances: (instances - 1) as u32,
                    controller: origin.controller,
                    ability_index: usize::MAX,
                    ability_origin: None,
                    presentation: presentation.clone(),
                    ability: ability.clone(),
                    ability_text: ability_text.clone(),
                    trigger_context: TriggerContext::default(),
                });
                remaining -= instances;
            }
        }
        triggers
    }
}

#[cfg(test)]
mod group_contract_tests {
    use super::*;

    fn restriction() -> ManaSpendingRestriction {
        ManaSpendingRestriction {
            restriction_id: tricerules_card_model::ChoiceId::new("contract").unwrap(),
            presentation: AbilityPresentation::Fallback,
            unrestricted: true,
            cast_spell: vec![],
            activate_ability: vec![],
            activate_any_ability: false,
            all_nonspell_costs: false,
            special_actions: vec![],
            spending_effects: vec![],
        }
    }

    #[test]
    fn group_registration_preserves_interning_source_identity_and_diagnostic_shape() {
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            u64::MAX,
            &[17, 29],
            20,
            None,
            false,
        )
        .unwrap();
        let initial = engine.diagnostic_snapshot().unwrap();
        let ordinary = restriction();
        assert_eq!(
            engine
                .register_mana_restriction_group(&ordinary, None, None, None)
                .unwrap(),
            1
        );
        let presentation = rv1::PresentationRef {
            fallback_text: "source-bound mana".into(),
            ..Default::default()
        };
        assert_eq!(
            engine
                .register_mana_restriction_group(&ordinary, Some(presentation.clone()), None, None)
                .unwrap(),
            1
        );

        let mut source_bound = ordinary.clone();
        source_bound.spending_effects =
            vec![ManaSpendingEffect::Scry1IfSpellSharesCreatureTypeWithCommander];
        let origin = ManaSpendingOrigin {
            source_object_id: 42,
            source_zone_change_generation: u64::MAX,
            source_face_change_generation: 3,
            source_face_index: 0,
            controller: 17,
            card_id: "path_of_ancestry".into(),
            source_label: "Path of Ancestry".into(),
            ability_presentation: Some(presentation.clone()),
        };
        for expected_id in [2, 3] {
            assert_eq!(
                engine
                    .register_mana_restriction_group(
                        &source_bound,
                        Some(presentation.clone()),
                        Some(origin.clone()),
                        None
                    )
                    .unwrap(),
                expected_id
            );
        }
        assert_eq!(
            engine
                .register_mana_restriction_group(&source_bound, None, Some(origin.clone()), Some(2))
                .unwrap(),
            2
        );
        let before_invalid = engine.diagnostic_snapshot().unwrap();
        let mut different_origin = origin.clone();
        different_origin.source_zone_change_generation = 0;
        assert!(engine
            .register_mana_restriction_group(&source_bound, None, Some(different_origin), Some(2))
            .is_err());
        assert!(engine
            .register_mana_restriction_group(&ordinary, None, None, Some(0))
            .is_err());
        assert!(engine
            .register_mana_restriction_group(&source_bound, None, None, None)
            .is_err());
        assert_eq!(engine.diagnostic_snapshot().unwrap(), before_invalid);

        let state = &before_invalid["state"];
        assert_eq!(state["_type"], "GameState");
        assert_eq!(state["seed"], u64::MAX.to_string());
        assert_eq!(state["players"][0]["id"], 17);
        assert!(state["objects"].is_object());
        assert!(state["pending_resolution"].is_null());
        assert!(state.get("mana_restriction_groups").is_none());
        assert_eq!(
            state["mana_restrictions"],
            crate::diagnostic_json::to_value(&vec![&ordinary, &source_bound, &source_bound])
                .unwrap()
        );
        assert_eq!(
            state["mana_restriction_presentations"],
            crate::diagnostic_json::to_value(&vec![None, Some(&presentation), Some(&presentation)])
                .unwrap()
        );
        assert_eq!(
            state["mana_restriction_origins"],
            crate::diagnostic_json::to_value(&vec![None, Some(&origin), Some(&origin)]).unwrap()
        );
        assert_eq!(
            state["mana_restriction_origins"][1]["source_zone_change_generation"],
            u64::MAX.to_string()
        );
        let raw_state = crate::diagnostic_json::to_value(&engine.state).unwrap();
        assert_eq!(
            raw_state["mana_restriction_groups"]["groups"]
                .as_array()
                .unwrap()
                .len(),
            3,
            "raw state serialization retains the complete store for rejection snapshots"
        );
        let mut initial_state = initial["state"].as_object().unwrap().clone();
        let mut final_state = state.as_object().unwrap().clone();
        for field in [
            "mana_restrictions",
            "mana_restriction_presentations",
            "mana_restriction_origins",
        ] {
            initial_state.remove(field);
            final_state.remove(field);
        }
        assert_eq!(
            initial_state, final_state,
            "all unrelated snapshot fields are unchanged"
        );
    }
}
