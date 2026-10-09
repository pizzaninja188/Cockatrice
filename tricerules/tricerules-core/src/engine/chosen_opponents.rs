//! CR 607.2d/607.5a: linked designations belong to one ability occurrence and incarnation.
use super::*;
use crate::state::{ChosenOpponentKey, ChosenOpponentOccurrence};

impl GameEngine {
    pub(super) fn chosen_opponent_key(
        &self,
        source: ObjectId,
        face_index: usize,
        face: &CardFace,
        link: &tricerules_card_model::AbilityLinkId,
    ) -> Option<ChosenOpponentKey> {
        let object = self.state.objects.get(&source)?;
        let mut producers = face.static_abilities.iter().filter(|ability| {
            matches!(&ability.definition, StaticAbilityDef::AsEntersChooseOpponent { link_id } if link_id == link)
        });
        let producer = producers.next()?;
        if producers.next().is_some() {
            return None;
        }
        Some(ChosenOpponentKey {
            source_object_id: source,
            source_zone_change: self
                .state
                .zone_change_generation
                .get(&source)
                .copied()
                .unwrap_or(0),
            producer: self.ability_definition(
                source,
                face_index,
                vec![producer.ability_id.clone()],
            ),
            link_id: link.clone(),
            occurrence: if object.copiable_values.is_some() {
                ChosenOpponentOccurrence::AcquiredCopy(object.copy_revision)
            } else {
                ChosenOpponentOccurrence::NativeOrTokenBase
            },
        })
    }

    pub(super) fn chosen_opponent_for(
        &self,
        source: ObjectId,
        link: &tricerules_card_model::AbilityLinkId,
    ) -> Option<PlayerId> {
        let object = self.state.objects.get(&source)?;
        if object.zone != Zone::Battlefield
            || !characteristics::printed_static_source_is_available(
                &self.state,
                self.registry,
                source,
            )
        {
            return None;
        }
        let face = self.effective_face(source)?;
        let key = self.chosen_opponent_key(source, object.face_up_index, &face, link)?;
        self.state
            .chosen_opponents
            .iter()
            .find(|record| record.key == key)
            .map(|record| record.player)
    }

    pub(super) fn chosen_opponent_labels(&self, source: ObjectId) -> Vec<String> {
        self.effective_face(source)
            .into_iter()
            .flat_map(|face| {
                face.static_abilities
                    .iter()
                    .filter_map(|ability| match &ability.definition {
                        StaticAbilityDef::AsEntersChooseOpponent { link_id } => self
                            .chosen_opponent_for(source, link_id)
                            .map(|player| format!("Chosen opponent: P{player}")),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
            })
            .collect()
    }
}
