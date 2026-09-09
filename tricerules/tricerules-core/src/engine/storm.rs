//! CR 702.40 storm ability collection for printed and emblem-granted spell keywords.

use super::presentation::{
    stack_child_presentation_ref, PresentationPath, StackPresentationSource,
};
use super::triggers::CollectedTrigger;
use super::*;

impl GameEngine {
    /// Collect the spell's storm abilities at the successful-cast boundary. The copy count is
    /// fixed from committed global turn history, and the stack snapshot survives the original
    /// being countered before its storm trigger resolves.
    pub(super) fn collect_storm_triggers(
        &self,
        fact: &crate::state::SpellCastFact,
    ) -> Vec<CollectedTrigger> {
        let Some(spell) = self
            .state
            .stack
            .iter()
            .find(|item| item.id == fact.occurrence.object_id)
            .cloned()
        else {
            return Vec::new();
        };
        let Some(face) = self
            .registry
            .get(&spell.card_id)
            .and_then(|definition| definition.face(spell.face_index))
        else {
            return Vec::new();
        };

        let printed = face
            .spell_keywords
            .iter()
            .filter(|keyword| **keyword == SpellKeyword::Storm)
            .count() as u32;
        let granted = self
            .state
            .static_emblems
            .iter()
            .filter(|emblem| emblem.controller == fact.caster)
            .flat_map(|emblem| emblem.effects.iter())
            .filter(|effect| {
                matches!(
                    effect,
                    StaticEmblemEffect::GrantSpellKeyword {
                        filter,
                        keyword: SpellKeyword::Storm,
                    } if super::history::spell_cast_matches(filter, fact)
                )
            })
            .count() as u32;
        let instances = printed.saturating_add(granted);
        if instances == 0 {
            return Vec::new();
        }

        let earlier_casts = self
            .state
            .turn_history
            .current
            .spells_cast
            .saturating_sub(1);
        let ability_id = tricerules_cards::AbilityId::new("storm").expect("intrinsic ability id");
        let ability_text = format!(
            "Storm — copy this spell {earlier_casts} time{}",
            if earlier_casts == 1 { "" } else { "s" }
        );
        let ability = TriggeredAbilityDef {
            ability_id: ability_id.clone(),
            presentation: tricerules_cards::AbilityPresentation::Fallback,
            trigger: TriggerCondition::WheneverPlayerCastsSpell {
                caster: CastTriggerPlayer::Controller,
                filter: SpellCastFilter::default(),
                ordinal: None,
                ordinal_scope: CastOrdinalScope::AllSpells,
            },
            effect: vec![SpellEffectKind::CopyCapturedSpell {
                count: earlier_casts,
            }],
            modal: None,
            targeting: None,
            may: false,
            intervening_if: None,
            triggers_only_once: false,
            max_triggers_per_turn: None,
        };
        let presentation = stack_child_presentation_ref(
            self.registry,
            &spell.card_id,
            spell.face_index,
            StackPresentationSource::for_stack(
                self.state
                    .stack_presentations
                    .get(&spell.id)
                    .and_then(|value| value.primary.as_ref()),
                true,
            ),
            PresentationPath::Ability(&ability_id),
            &ability.presentation,
            ability_text.clone(),
        );

        vec![CollectedTrigger {
            captured_spell: Some(Box::new(spell.clone())),
            source_id: spell.id,
            card_id: spell.card_id.clone(),
            face_index: spell.face_index,
            source_zone_change: fact.occurrence.zone_change_generation.unwrap_or(0),
            source_face_change: 0,
            source_fact: None,
            additional_instances: instances.saturating_sub(1),
            controller: spell.controller,
            ability_index: usize::MAX,
            ability_origin: None,
            presentation,
            ability,
            ability_text,
            trigger_context: TriggerContext::default(),
        }]
    }
}
