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
) -> String {
    format!(
        "{} (player {}, source object {} generation {}) — {}",
        origin.source_label,
        origin.controller,
        origin.source_object_id,
        origin.source_zone_change_generation,
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
            let index = group_id.checked_sub(1).ok_or(EngineError::Illegal(
                "invalid preferred mana restriction group",
            ))? as usize;
            let matches_registry = self.state.mana_restrictions.get(index) == Some(restriction)
                && self
                    .state
                    .mana_restriction_origins
                    .get(index)
                    .and_then(Option::as_ref)
                    == origin.as_ref();
            if !matches_registry {
                return Err(EngineError::Illegal(
                    "replayed mana restriction group no longer matches its source",
                ));
            }
            return Ok(group_id);
        }

        if !source_bound {
            if let Some(position) = self
                .state
                .mana_restrictions
                .iter()
                .position(|candidate| candidate == restriction)
            {
                return Ok((position + 1) as u32);
            }
        }

        self.state.mana_restrictions.push(restriction.clone());
        self.state
            .mana_restriction_presentations
            .push(restriction_presentation);
        self.state.mana_restriction_origins.push(origin);
        debug_assert_eq!(
            self.state.mana_restrictions.len(),
            self.state.mana_restriction_presentations.len()
        );
        debug_assert_eq!(
            self.state.mana_restrictions.len(),
            self.state.mana_restriction_origins.len()
        );
        Ok(self.state.mana_restrictions.len() as u32)
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
            let Some(index) = group_id.checked_sub(1).map(|id| id as usize) else {
                continue;
            };
            let Some(restriction) = self.state.mana_restrictions.get(index) else {
                continue;
            };
            if !restriction.spending_effects.iter().any(|effect| {
                matches!(
                    effect,
                    ManaSpendingEffect::Scry1IfSpellSharesCreatureTypeWithCommander
                )
            }) {
                continue;
            }
            let Some(origin) = self
                .state
                .mana_restriction_origins
                .get(index)
                .and_then(Option::as_ref)
            else {
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
