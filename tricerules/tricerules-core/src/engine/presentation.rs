use tricerules_cards::{
    AbilityId, AbilityPresentation, CardFaceId, CardRegistry, ChoiceId, ModeId,
};
use tricerules_proto::ruled::v1 as rv1;

use crate::state::{AbilityDefinitionId, CastCostReceipt, ChosenMode, StackPresentation};

#[derive(Clone, Copy)]
pub(super) enum PresentationPath<'a> {
    Spell,
    Ability(&'a AbilityId),
    Mode(&'a ModeId),
    CastCostGroup(&'a ChoiceId),
    CastCostOption(&'a ChoiceId),
    ResolutionBranch(&'a ChoiceId),
    SearchSlot(&'a ChoiceId),
    ManaRestriction(&'a ChoiceId),
}

impl PresentationPath<'_> {
    fn into_proto(self) -> rv1::PresentationPathComponent {
        let (kind, id) = match self {
            Self::Spell => (rv1::PresentationPathKind::Spell, "spell"),
            Self::Ability(id) => (rv1::PresentationPathKind::Ability, id.as_str()),
            Self::Mode(id) => (rv1::PresentationPathKind::Mode, id.as_str()),
            Self::CastCostGroup(id) => (rv1::PresentationPathKind::CastCostGroup, id.as_str()),
            Self::CastCostOption(id) => (rv1::PresentationPathKind::CastCostOption, id.as_str()),
            Self::ResolutionBranch(id) => {
                (rv1::PresentationPathKind::ResolutionBranch, id.as_str())
            }
            Self::SearchSlot(id) => (rv1::PresentationPathKind::SearchSlot, id.as_str()),
            Self::ManaRestriction(id) => (rv1::PresentationPathKind::ManaRestriction, id.as_str()),
        };
        rv1::PresentationPathComponent {
            kind: kind as i32,
            id: id.to_owned(),
        }
    }
}

pub(super) fn child_presentation_ref(
    parent: &rv1::PresentationRef,
    child: PresentationPath<'_>,
    presentation: &AbilityPresentation,
    fallback_text: String,
) -> rv1::PresentationRef {
    let mut path = parent.path.clone();
    path.push(child.into_proto());
    rv1::PresentationRef {
        card_id: parent.card_id.clone(),
        face_id: parent.face_id.clone(),
        path,
        oracle_line_indices: match presentation {
            AbilityPresentation::OracleLines(indices) => {
                indices.iter().copied().map(u32::from).collect()
            }
            AbilityPresentation::Fallback => Vec::new(),
        },
        fallback_text,
        external_card_name: parent.external_card_name.clone(),
        external_face_name: parent.external_face_name.clone(),
        oracle_text_sha256: parent.oracle_text_sha256.clone(),
    }
}

/// Builds presentation for a node owned by a stack item. Activated and triggered abilities
/// already carry a primary presentation to extend. Physical spells intentionally do not, so their
/// children start directly at the stable spell path without manufacturing a displayable root.
#[derive(Clone, Copy)]
pub(super) enum StackPresentationSource<'a> {
    Parent(&'a rv1::PresentationRef),
    PhysicalSpell,
    Missing,
}

impl<'a> StackPresentationSource<'a> {
    pub(super) fn for_stack(
        parent: Option<&'a rv1::PresentationRef>,
        physical_spell: bool,
    ) -> Self {
        match (parent, physical_spell) {
            (Some(parent), _) => Self::Parent(parent),
            (None, true) => Self::PhysicalSpell,
            (None, false) => Self::Missing,
        }
    }
}

pub(super) fn stack_child_presentation_ref<'a>(
    registry: &CardRegistry,
    card_id: &str,
    face_index: usize,
    source: StackPresentationSource<'a>,
    child: PresentationPath<'a>,
    presentation: &AbilityPresentation,
    fallback_text: String,
) -> Option<rv1::PresentationRef> {
    if let StackPresentationSource::Parent(parent) = source {
        return Some(child_presentation_ref(
            parent,
            child,
            presentation,
            fallback_text,
        ));
    }
    if matches!(source, StackPresentationSource::Missing) {
        return None;
    }
    let face = registry.get(card_id)?.face(face_index)?;
    Some(presentation_ref(
        registry,
        card_id,
        &face.face_id,
        [PresentationPath::Spell, child],
        presentation,
        fallback_text,
    ))
}

pub(super) fn presentation_ref<'a>(
    registry: &CardRegistry,
    card_id: &str,
    face_id: &CardFaceId,
    path: impl IntoIterator<Item = PresentationPath<'a>>,
    presentation: &AbilityPresentation,
    fallback_text: String,
) -> rv1::PresentationRef {
    let metadata = registry.presentation_face(card_id, face_id.as_str());
    let (external_card_name, external_face_name, oracle_text_sha256) = metadata
        .map(|metadata| {
            (
                metadata.card_name.clone(),
                metadata.face_name.clone(),
                metadata.oracle_text_sha256.clone(),
            )
        })
        .unwrap_or_default();
    let oracle_line_indices = match presentation {
        AbilityPresentation::OracleLines(indices) => {
            indices.iter().copied().map(u32::from).collect()
        }
        AbilityPresentation::Fallback => Vec::new(),
    };
    rv1::PresentationRef {
        card_id: card_id.to_owned(),
        face_id: face_id.as_str().to_owned(),
        path: path.into_iter().map(PresentationPath::into_proto).collect(),
        oracle_line_indices,
        fallback_text,
        external_card_name,
        external_face_name,
        oracle_text_sha256,
    }
}

pub(super) fn spell_stack_presentation(
    registry: &CardRegistry,
    card_id: &str,
    face_index: usize,
    chosen_modes: &[ChosenMode],
    cast_cost_receipts: &[CastCostReceipt],
) -> StackPresentation {
    let Some(face) = registry.get(card_id).and_then(|card| card.face(face_index)) else {
        return StackPresentation::default();
    };
    let chosen_modes = chosen_modes
        .iter()
        .filter_map(|chosen| {
            let mode = face.modal_spell.as_ref()?.mode_by_id(&chosen.mode_id)?;
            Some(presentation_ref(
                registry,
                card_id,
                &face.face_id,
                [
                    PresentationPath::Spell,
                    PresentationPath::Mode(&mode.mode_id),
                ],
                &mode.presentation,
                tricerules_cards::mode_fallback(&face.name, &mode.mode_id),
            ))
        })
        .collect();
    let chosen_cast_costs = cast_cost_receipts
        .iter()
        .filter_map(|receipt| {
            let group = face.cast_cost_groups.get(receipt.group_index as usize)?;
            let option = group.options.get(receipt.option_index as usize)?;
            let (option_id, mapping) = match option {
                tricerules_cards::CastCostOptionDef::Blight {
                    option_id,
                    presentation,
                    ..
                }
                | tricerules_cards::CastCostOptionDef::Mana {
                    option_id,
                    presentation,
                    ..
                }
                | tricerules_cards::CastCostOptionDef::Behold {
                    option_id,
                    presentation,
                    ..
                } => (option_id, presentation),
            };
            Some(presentation_ref(
                registry,
                card_id,
                &face.face_id,
                [
                    PresentationPath::Spell,
                    PresentationPath::CastCostGroup(&group.group_id),
                    PresentationPath::CastCostOption(option_id),
                ],
                mapping,
                receipt.label.clone(),
            ))
        })
        .collect();
    StackPresentation {
        primary: None,
        chosen_modes,
        chosen_cast_costs,
    }
}

pub(super) fn ability_presentation(
    registry: &CardRegistry,
    definition: &AbilityDefinitionId,
    mapping: &AbilityPresentation,
    fallback: String,
) -> rv1::PresentationRef {
    presentation_ref(
        registry,
        &definition.card_id,
        &definition.face_id,
        definition
            .ability_path
            .iter()
            .map(PresentationPath::Ability),
        mapping,
        fallback,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::CastCostReceipt;

    #[test]
    fn real_card_cost_mapping_carries_stable_path_and_external_face_fingerprint() {
        let registry = CardRegistry::global();
        let presentation = spell_stack_presentation(
            registry,
            "grow_from_the_ashes",
            0,
            &[],
            &[CastCostReceipt {
                group_index: 0,
                option_index: 0,
                group_id: Some(tricerules_cards::ChoiceId::new("cast_cost_01").unwrap()),
                option_id: Some(tricerules_cards::ChoiceId::new("option_01").unwrap()),
                label: "Kicker {2}".into(),
                object: None,
            }],
        );
        let option = presentation
            .chosen_cast_costs
            .first()
            .expect("mapped kicker option");
        assert_eq!(option.card_id, "grow_from_the_ashes");
        assert_eq!(option.face_id, "grow_from_the_ashes");
        assert_eq!(option.external_card_name, "Grow from the Ashes");
        assert_eq!(option.external_face_name, "Grow from the Ashes");
        assert_eq!(option.oracle_line_indices, [1]);
        assert_eq!(option.oracle_text_sha256.len(), 64);
        assert_eq!(
            option
                .path
                .iter()
                .map(|component| component.id.as_str())
                .collect::<Vec<_>>(),
            ["spell", "cast_cost_01", "option_01"]
        );
    }

    #[test]
    fn real_ability_mapping_keeps_definition_identity_and_fallback() {
        let registry = CardRegistry::global();
        let card = registry
            .get("abandoned_campground")
            .expect("calibration card");
        let face = card.primary_face();
        let ability = face.activated_abilities.first().expect("mana ability");
        let definition = AbilityDefinitionId {
            card_id: card.id.clone(),
            face_id: face.face_id.clone(),
            ability_path: vec![ability.ability_id.clone()],
        };
        let fallback = ability.fallback_text(&face.name);
        let reference = ability_presentation(
            registry,
            &definition,
            &ability.presentation,
            fallback.clone(),
        );
        assert_eq!(reference.fallback_text, fallback);
        assert_eq!(reference.oracle_line_indices, [2]);
        assert_eq!(reference.path.len(), 1);
        assert_eq!(reference.path[0].id, "activated_01");
    }

    #[test]
    fn physical_spell_has_no_root_presentation() {
        let presentation =
            spell_stack_presentation(CardRegistry::global(), "aangs_journey", 0, &[], &[]);
        assert!(presentation.primary.is_none());
    }

    #[test]
    fn physical_spell_child_keeps_its_stable_path_without_a_root() {
        let ability_id = tricerules_cards::AbilityId::new("delayed_test").unwrap();
        let reference = stack_child_presentation_ref(
            CardRegistry::global(),
            "aangs_journey",
            0,
            StackPresentationSource::PhysicalSpell,
            PresentationPath::Ability(&ability_id),
            &AbilityPresentation::Fallback,
            "Delayed test".into(),
        )
        .expect("physical spell child presentation");
        assert_eq!(
            reference
                .path
                .iter()
                .map(|component| component.id.as_str())
                .collect::<Vec<_>>(),
            ["spell", "delayed_test"]
        );
        assert!(stack_child_presentation_ref(
            CardRegistry::global(),
            "aangs_journey",
            0,
            StackPresentationSource::Missing,
            PresentationPath::Ability(&ability_id),
            &AbilityPresentation::Fallback,
            "Delayed test".into(),
        )
        .is_none());
    }

    #[test]
    fn modal_spell_publication_uses_stable_mode_identity_and_mapping() {
        let presentation = spell_stack_presentation(
            CardRegistry::global(),
            "boros_charm",
            0,
            &[ChosenMode {
                mode_id: ModeId::new("mode_02").unwrap(),
                targets: Vec::new(),
            }],
            &[],
        );
        let mode = presentation.chosen_modes.first().expect("chosen mode");
        assert_eq!(mode.card_id, "boros_charm");
        assert_eq!(mode.face_id, "boros_charm");
        assert_eq!(mode.oracle_line_indices, [3]);
        assert_eq!(mode.path.last().unwrap().id, "mode_02");
        assert_eq!(
            mode.path.last().unwrap().kind,
            rv1::PresentationPathKind::Mode as i32
        );
    }
}
