//! One-shot cast observers share normal trigger ordering and the ordinary copy executor.
use super::triggers::CollectedTrigger;
use super::*;
use tricerules_cards::{AbilityPresentation, SpellCastFilter};

impl GameEngine {
    pub(super) fn register_next_spell_copy(
        &mut self,
        top: &StackItem,
        controller: PlayerId,
        label: &str,
    ) {
        let source = TriggerObjectRef {
            object_id: top.source_permanent_id.unwrap_or(top.id),
            zone_change_generation: top
                .source_permanent_id
                .map(|_| top.source_zone_change)
                .unwrap_or_else(|| {
                    self.state
                        .zone_change_generation
                        .get(&top.id)
                        .copied()
                        .unwrap_or(0)
                }),
            controller_at_event: controller,
        };
        let ability = TriggeredAbilityDef {
            ability_id: tricerules_cards::AbilityId::new("copy_next_spell")
                .expect("static ability id"),
            presentation: AbilityPresentation::Fallback,
            trigger: TriggerCondition::WheneverPlayerCastsSpell {
                caster: Default::default(),
                filter: SpellCastFilter {
                    card_type: Some(CardTypeFilter::InstantOrSorcery),
                    ..Default::default()
                },
                ordinal: None,
                ordinal_scope: Default::default(),
            },
            effect: vec![SpellEffectKind::CopyCapturedSpell { count: 1 }],
            modal: None,
            targeting: None,
            may: false,
            intervening_if: None,
            triggers_only_once: false,
            max_triggers_per_turn: None,
        };
        let presentation = super::presentation::stack_child_presentation_ref(
            self.registry,
            &top.card_id,
            top.face_index,
            super::presentation::StackPresentationSource::for_stack(
                self.state
                    .stack_presentations
                    .get(&top.id)
                    .and_then(|p| p.primary.as_ref()),
                top.ability_text.is_none(),
            ),
            super::presentation::PresentationPath::Ability(&ability.ability_id),
            &ability.presentation,
            format!("{label} — copy the next instant or sorcery"),
        );
        self.state.active_event_observers.push(ActiveEventObserver {
            watched: source,
            matcher: EventObserverMatcher::NextInstantOrSorceryThisTurn { controller },
            payload: EventObserverPayload::StageDelayedTrigger(Box::new(DelayedTriggerPayload {
                source,
                controller,
                card_id: top.card_id.clone(),
                card_name: label.into(),
                source_face_index: top.face_index,
                presentation,
                ability,
            })),
        });
    }

    pub(super) fn collect_delayed_copy_triggers(
        &mut self,
        fact: &crate::state::SpellCastFact,
    ) -> Vec<CollectedTrigger> {
        let matched = self
            .state
            .dispatch_event_observers(ObservedGameEvent::SpellCast {
                controller: fact.caster,
                instant_or_sorcery: fact
                    .matched_card_types
                    .contains(&CardTypeFilter::InstantOrSorcery),
            });
        let snapshot = self
            .state
            .stack
            .iter()
            .find(|spell| spell.id == fact.occurrence.object_id)
            .cloned();
        matched
            .into_iter()
            .map(|(_, delayed)| CollectedTrigger {
                captured_spell: snapshot.clone().map(Box::new),
                source_id: delayed.source.object_id,
                card_id: delayed.card_id,
                face_index: delayed.source_face_index,
                source_zone_change: delayed.source.zone_change_generation,
                source_face_change: 0,
                source_fact: None,
                additional_instances: 0,
                controller: delayed.controller,
                ability_index: 0,
                ability_origin: None,
                presentation: delayed.presentation,
                ability_text: format!("{} — copy the next instant or sorcery", delayed.card_name),
                trigger_context: TriggerContext::default(),
                ability: delayed.ability,
            })
            .collect()
    }
}
