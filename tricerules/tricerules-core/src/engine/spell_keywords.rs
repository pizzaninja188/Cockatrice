//! Cast-time trigger capture for spell keyword abilities.

use super::presentation::{
    stack_child_presentation_ref, PresentationPath, StackPresentationSource,
};
use super::triggers::CollectedTrigger;
use super::*;
use crate::state::SpellKeywordOrigin;

impl GameEngine {
    /// Collect printed and granted spell keywords at the successful-cast boundary. Effects are
    /// captured from the completed cast and survive later changes to their granting sources.
    pub(super) fn collect_spell_keyword_triggers(
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

        let mut triggers = Vec::new();
        for keyword in [SpellKeyword::Storm, SpellKeyword::Cascade] {
            for origin in self.spell_keyword_origins(face, fact, keyword) {
                triggers.push(self.collected_spell_keyword_trigger(
                    &spell,
                    face.name.clone(),
                    fact,
                    keyword,
                    origin,
                ));
            }
        }
        triggers
    }

    fn spell_keyword_origins(
        &self,
        face: &CardFace,
        fact: &crate::state::SpellCastFact,
        keyword: SpellKeyword,
    ) -> Vec<SpellKeywordOrigin> {
        let mut origins = face
            .spell_keywords
            .iter()
            .enumerate()
            .filter(|(_, candidate)| **candidate == keyword)
            .map(|(instance_index, _)| SpellKeywordOrigin::Printed {
                keyword,
                instance_index: instance_index as u32,
            })
            .collect::<Vec<_>>();

        for emblem in self
            .state
            .static_emblems
            .iter()
            .filter(|emblem| emblem.controller == fact.caster)
        {
            origins.extend(emblem.effects.iter().enumerate().filter_map(
                |(effect_index, effect)| {
                    matches!(
                        effect,
                        StaticEmblemEffect::GrantSpellKeyword {
                            filter,
                            keyword: granted,
                        } if *granted == keyword
                            && super::history::spell_cast_matches(filter, fact)
                    )
                    .then_some(SpellKeywordOrigin::StaticEmblemGrant {
                        keyword,
                        emblem_id: emblem.object_id,
                        effect_index,
                    })
                },
            ));
        }

        let mut sources = self
            .state
            .objects
            .values()
            .filter(|object| object.zone == Zone::Battlefield && object.controller == fact.caster)
            .map(|object| object.id)
            .collect::<Vec<_>>();
        sources.sort_unstable();
        for source in sources {
            let source_zone_change = self
                .state
                .zone_change_generation
                .get(&source)
                .copied()
                .unwrap_or(0);
            let face_index = self
                .state
                .objects
                .get(&source)
                .map(|object| object.face_up_index)
                .unwrap_or(0);
            for ability in self.active_static_abilities(source) {
                if let StaticAbilityDef::GrantSpellKeyword {
                    filter,
                    keyword: granted,
                } = &ability.definition
                {
                    if *granted == keyword && super::history::spell_cast_matches(filter, fact) {
                        origins.push(SpellKeywordOrigin::StaticGrant {
                            keyword,
                            source_id: source,
                            source_zone_change,
                            definition: self.ability_definition(
                                source,
                                face_index,
                                vec![ability.ability_id.clone()],
                            ),
                        });
                    }
                }
            }
        }

        origins
    }

    fn collected_spell_keyword_trigger(
        &self,
        spell: &StackItem,
        source_label: String,
        fact: &crate::state::SpellCastFact,
        keyword: SpellKeyword,
        origin: SpellKeywordOrigin,
    ) -> CollectedTrigger {
        let ability_id_text = match keyword {
            SpellKeyword::Storm => "storm",
            SpellKeyword::Cascade => "cascade",
        };
        let ability_id = tricerules_card_model::AbilityId::new(ability_id_text)
            .expect("intrinsic spell keyword ability id");
        let ability_text = match keyword {
            SpellKeyword::Storm => {
                let earlier_casts = self
                    .state
                    .turn_history
                    .current
                    .spells_cast
                    .saturating_sub(1);
                format!(
                    "Storm — copy this spell {earlier_casts} time{}",
                    if earlier_casts == 1 { "" } else { "s" }
                )
            }
            SpellKeyword::Cascade => "Cascade".to_owned(),
        };
        let effect = match keyword {
            SpellKeyword::Storm => SpellEffectKind::CopyCapturedSpell {
                count: self
                    .state
                    .turn_history
                    .current
                    .spells_cast
                    .saturating_sub(1),
            },
            SpellKeyword::Cascade => SpellEffectKind::Cascade {
                source_mana_value: fact.mana_value,
            },
        };
        let ability = TriggeredAbilityDef {
            ability_id: ability_id.clone(),
            presentation: tricerules_card_model::AbilityPresentation::Fallback,
            trigger: TriggerCondition::WheneverPlayerCastsSpell {
                caster: CastTriggerPlayer::Controller,
                filter: SpellCastFilter::default(),
                ordinal: None,
                ordinal_scope: CastOrdinalScope::AllSpells,
            },
            effect: vec![effect],
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

        CollectedTrigger {
            captured_spell: Some(Box::new(spell.clone())),
            source_label,
            source_id: spell.id,
            card_id: spell.card_id.clone(),
            face_index: spell.face_index,
            source_zone_change: fact.occurrence.zone_change_generation.unwrap_or(0),
            source_face_change: 0,
            source_fact: None,
            additional_instances: 0,
            controller: fact.caster,
            ability_index: usize::MAX,
            ability_origin: Some(TriggerAbilityOrigin::SpellKeyword(origin)),
            presentation,
            ability,
            ability_text,
            trigger_context: TriggerContext::default(),
        }
    }
}
