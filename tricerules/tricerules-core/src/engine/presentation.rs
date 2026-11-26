use tricerules_card_model::{
    AbilityPresentation, ActivatedAbilityDef, CardFaceId, CardRegistry, ChoicePresentation,
    PresentationPathComponent, PresentationPathKind,
};
use tricerules_proto::ruled::v1 as rv1;

use crate::state::{AbilityDefinitionId, CastCostReceipt, ChosenMode, StackPresentation};

type ActivatedModePresentation = (Vec<u32>, Vec<String>, Vec<rv1::PresentationRef>);

fn component_to_proto(component: &PresentationPathComponent) -> rv1::PresentationPathComponent {
    let kind = match component.kind() {
        PresentationPathKind::Spell => rv1::PresentationPathKind::Spell,
        PresentationPathKind::Ability => rv1::PresentationPathKind::Ability,
        PresentationPathKind::Mode => rv1::PresentationPathKind::Mode,
        PresentationPathKind::CastCostGroup => rv1::PresentationPathKind::CastCostGroup,
        PresentationPathKind::CastCostOption => rv1::PresentationPathKind::CastCostOption,
        PresentationPathKind::ResolutionBranch => rv1::PresentationPathKind::ResolutionBranch,
        PresentationPathKind::SearchSlot => rv1::PresentationPathKind::SearchSlot,
        PresentationPathKind::ManaRestriction => rv1::PresentationPathKind::ManaRestriction,
    };
    rv1::PresentationPathComponent {
        kind: kind as i32,
        id: component.id().to_owned(),
    }
}

/// An authored presentation of either kind: printed abilities, modes and cost groups carry an
/// [`AbilityPresentation`]; interactive choice labels carry a [`ChoicePresentation`].
#[derive(Clone, Copy)]
pub(super) enum PresentationMapping<'a> {
    Ability(&'a AbilityPresentation),
    Choice(&'a ChoicePresentation),
}

impl<'a> From<&'a AbilityPresentation> for PresentationMapping<'a> {
    fn from(presentation: &'a AbilityPresentation) -> Self {
        Self::Ability(presentation)
    }
}

impl<'a> From<&'a ChoicePresentation> for PresentationMapping<'a> {
    fn from(presentation: &'a ChoicePresentation) -> Self {
        Self::Choice(presentation)
    }
}

/// External spans for one mapping on one exact face. A fragment whose generated offsets are
/// absent publishes no spans, so clients use the fallback text.
fn oracle_spans(
    registry: &CardRegistry,
    card_id: &str,
    face_id: &str,
    mapping: PresentationMapping<'_>,
) -> Vec<rv1::OracleSpan> {
    let spans = match mapping {
        PresentationMapping::Ability(presentation) => presentation.oracle_spans(|line, part| {
            registry.presentation_fragment(card_id, face_id, line, part)
        }),
        PresentationMapping::Choice(presentation) => presentation.oracle_spans(),
    };
    spans
        .into_iter()
        .map(|span| rv1::OracleSpan {
            line: u32::from(span.line),
            start: span.range.map(|(start, _)| start),
            end: span.range.map(|(_, end)| end),
        })
        .collect()
}

pub(super) fn child_presentation_ref<'p>(
    registry: &CardRegistry,
    parent: &rv1::PresentationRef,
    child: PresentationPathComponent,
    presentation: impl Into<PresentationMapping<'p>>,
    fallback_text: String,
) -> rv1::PresentationRef {
    let mut path = parent.path.clone();
    path.push(component_to_proto(&child));
    rv1::PresentationRef {
        card_id: parent.card_id.clone(),
        face_id: parent.face_id.clone(),
        path,
        // A child never inherits its parent's spans; it maps (or falls back) on its own.
        oracle_spans: oracle_spans(
            registry,
            &parent.card_id,
            &parent.face_id,
            presentation.into(),
        ),
        fallback_text,
        external_card_name: parent.external_card_name.clone(),
        external_face_name: parent.external_face_name.clone(),
        oracle_text_sha256: parent.oracle_text_sha256.clone(),
    }
}

pub(super) fn activated_stack_mode_presentation(
    registry: &CardRegistry,
    ability: &ActivatedAbilityDef,
    ability_text: &str,
    chosen_modes: &[ChosenMode],
    parent: Option<&rv1::PresentationRef>,
) -> Result<ActivatedModePresentation, &'static str> {
    if chosen_modes.is_empty() {
        return Ok((Vec::new(), Vec::new(), Vec::new()));
    }
    let modal = ability
        .modal
        .as_ref()
        .ok_or("captured activated mode has no modal definition")?;
    let mut indices = Vec::with_capacity(chosen_modes.len());
    let mut labels = Vec::with_capacity(chosen_modes.len());
    let mut presentations = Vec::with_capacity(chosen_modes.len());
    for chosen in chosen_modes {
        let (mode_index, mode) = modal
            .modes
            .iter()
            .enumerate()
            .find(|(_, mode)| mode.mode_id == chosen.mode_id)
            .ok_or("captured activated mode is absent from its modal definition")?;
        let label = tricerules_card_model::mode_fallback(ability_text, &mode.mode_id);
        indices.push(mode_index as u32);
        labels.push(label.clone());
        if let Some(parent) = parent {
            presentations.push(child_presentation_ref(
                registry,
                parent,
                PresentationPathComponent::Mode(mode.mode_id.clone()),
                &mode.presentation,
                label,
            ));
        }
    }
    Ok((indices, labels, presentations))
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

pub(super) fn stack_child_presentation_ref<'a, 'p>(
    registry: &CardRegistry,
    card_id: &str,
    face_index: usize,
    source: StackPresentationSource<'a>,
    child: PresentationPathComponent,
    presentation: impl Into<PresentationMapping<'p>>,
    fallback_text: String,
) -> Option<rv1::PresentationRef> {
    if let StackPresentationSource::Parent(parent) = source {
        return Some(child_presentation_ref(
            registry,
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
        [PresentationPathComponent::Spell, child],
        presentation,
        fallback_text,
    ))
}

pub(super) fn presentation_ref<'p>(
    registry: &CardRegistry,
    card_id: &str,
    face_id: &CardFaceId,
    path: impl IntoIterator<Item = PresentationPathComponent>,
    presentation: impl Into<PresentationMapping<'p>>,
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
    rv1::PresentationRef {
        card_id: card_id.to_owned(),
        face_id: face_id.as_str().to_owned(),
        path: path
            .into_iter()
            .map(|component| component_to_proto(&component))
            .collect(),
        oracle_spans: oracle_spans(registry, card_id, face_id.as_str(), presentation.into()),
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
                    PresentationPathComponent::Spell,
                    PresentationPathComponent::Mode(mode.mode_id.clone()),
                ],
                &mode.presentation,
                tricerules_card_model::mode_fallback(&face.name, &mode.mode_id),
            ))
        })
        .collect();
    let chosen_cast_costs = cast_cost_receipts
        .iter()
        .filter_map(|receipt| {
            let group = face.cast_cost_groups.get(receipt.group_index as usize)?;
            let option = group.options.get(receipt.option_index as usize)?;
            let (option_id, mapping) = match option {
                tricerules_card_model::CastCostOptionDef::Blight {
                    option_id,
                    presentation,
                    ..
                }
                | tricerules_card_model::CastCostOptionDef::Mana {
                    option_id,
                    presentation,
                    ..
                }
                | tricerules_card_model::CastCostOptionDef::Behold {
                    option_id,
                    presentation,
                    ..
                }
                | tricerules_card_model::CastCostOptionDef::DiscardCard {
                    option_id,
                    presentation,
                }
                | tricerules_card_model::CastCostOptionDef::PayLife {
                    option_id,
                    presentation,
                    ..
                }
                | tricerules_card_model::CastCostOptionDef::TapPermanents {
                    option_id,
                    presentation,
                    ..
                }
                | tricerules_card_model::CastCostOptionDef::SacrificePermanent {
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
                    PresentationPathComponent::Spell,
                    PresentationPathComponent::CastCostGroup(group.group_id.clone()),
                    PresentationPathComponent::CastCostOption(option_id.clone()),
                ],
                // The number paid is a runtime rules fact, absent from the static Oracle line.
                // Preserve that complete annotation while offers retain their Oracle mapping.
                if receipt.multikicker.is_some() {
                    &ChoicePresentation::Generated
                } else {
                    mapping
                },
                receipt.label.clone(),
            ))
        })
        .collect();
    StackPresentation {
        source_label: None,
        source_token_identity: None,
        activation_reveal_id: None,
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
            .cloned()
            .map(PresentationPathComponent::Ability),
        mapping,
        fallback,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::CastCostReceipt;
    use tricerules_card_model::ModeId;

    /// The published path round-trips through the shared type and is one the authoring visitor
    /// (and so the presentation audit) yields for that exact face.
    fn assert_authored_path(reference: &rv1::PresentationRef) {
        let path =
            tricerules_card_model::PresentationPath::new(reference.path.iter().map(|wire| {
                let kind = PresentationPathKind::ALL
                    .into_iter()
                    .find(|kind| {
                        component_to_proto(
                            &PresentationPathComponent::from_parts(
                                *kind,
                                if *kind == PresentationPathKind::Spell {
                                    "spell"
                                } else {
                                    "probe"
                                },
                            )
                            .unwrap(),
                        )
                        .kind
                            == wire.kind
                    })
                    .expect("published kind maps to the shared kind");
                PresentationPathComponent::from_parts(kind, &wire.id).unwrap()
            }));
        let reproto = path
            .components()
            .iter()
            .map(component_to_proto)
            .collect::<Vec<_>>();
        assert_eq!(reproto, reference.path);
        let face = tricerules_cards::registry::global()
            .get(&reference.card_id)
            .and_then(|card| {
                card.faces
                    .iter()
                    .find(|face| face.face_id.as_str() == reference.face_id)
            })
            .expect("authored face");
        assert!(
            tricerules_card_model::face_presentation_paths(face).contains(&path),
            "{path} is not an authored presentation path"
        );
    }

    #[test]
    fn real_card_cost_mapping_carries_stable_path_and_external_face_fingerprint() {
        let registry = tricerules_cards::registry::global();
        let presentation = spell_stack_presentation(
            registry,
            "grow_from_the_ashes",
            0,
            &[],
            &[CastCostReceipt {
                group_index: 0,
                option_index: 0,
                group_id: Some(tricerules_card_model::ChoiceId::new("cast_cost_01").unwrap()),
                option_id: Some(tricerules_card_model::ChoiceId::new("option_01").unwrap()),
                object_cost_kind: None,
                multikicker: None,
                label: "Kicker {2}".into(),
                objects: vec![],
            }],
        );
        let option = presentation
            .chosen_cast_costs
            .first()
            .expect("mapped kicker option");
        assert_authored_path(option);
        assert_eq!(option.card_id, "grow_from_the_ashes");
        assert_eq!(option.face_id, "grow_from_the_ashes");
        assert_eq!(option.external_card_name, "Grow from the Ashes");
        assert_eq!(option.external_face_name, "Grow from the Ashes");
        assert_eq!(option.whole_oracle_lines(), [1]);
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
        let registry = tricerules_cards::registry::global();
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
        assert_eq!(reference.whole_oracle_lines(), [2]);
        assert_eq!(reference.path.len(), 1);
        assert_eq!(reference.path[0].id, "activated_01");
    }

    #[test]
    fn token_ability_publishes_readable_fallback_without_unresolvable_oracle_mapping() {
        let registry = tricerules_cards::registry::global();
        let card = registry.get("clue").unwrap();
        let face = card.primary_face();
        let ability = &face.activated_abilities[0];
        let definition = AbilityDefinitionId {
            card_id: card.id.clone(),
            face_id: face.face_id.clone(),
            ability_path: vec![ability.ability_id.clone()],
        };
        let reference = ability_presentation(
            registry,
            &definition,
            &ability.presentation,
            ability.fallback_text(&face.name),
        );
        assert_eq!(reference.fallback_text, "{2}, Sacrifice Clue: Draw a card.");
        assert!(reference.oracle_spans.is_empty());
        assert!(reference.oracle_text_sha256.is_empty());
    }

    #[test]
    fn physical_spell_has_no_root_presentation() {
        let presentation = spell_stack_presentation(
            tricerules_cards::registry::global(),
            "aangs_journey",
            0,
            &[],
            &[],
        );
        assert!(presentation.primary.is_none());
    }

    #[test]
    fn physical_spell_child_keeps_its_stable_path_without_a_root() {
        let ability_id = tricerules_card_model::AbilityId::new("delayed_test").unwrap();
        let reference = stack_child_presentation_ref(
            tricerules_cards::registry::global(),
            "aangs_journey",
            0,
            StackPresentationSource::PhysicalSpell,
            PresentationPathComponent::Ability(ability_id.clone()),
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
            tricerules_cards::registry::global(),
            "aangs_journey",
            0,
            StackPresentationSource::Missing,
            PresentationPathComponent::Ability(ability_id.clone()),
            &AbilityPresentation::Fallback,
            "Delayed test".into(),
        )
        .is_none());
    }

    #[test]
    fn modal_spell_publication_uses_stable_mode_identity_and_mapping() {
        let presentation = spell_stack_presentation(
            tricerules_cards::registry::global(),
            "boros_charm",
            0,
            &[ChosenMode {
                mode_id: ModeId::new("mode_02").unwrap(),
                targets: Vec::new(),
            }],
            &[],
        );
        let mode = presentation.chosen_modes.first().expect("chosen mode");
        assert_authored_path(mode);
        assert_eq!(mode.card_id, "boros_charm");
        assert_eq!(mode.face_id, "boros_charm");
        assert_eq!(mode.whole_oracle_lines(), [3]);
        assert_eq!(mode.path.last().unwrap().id, "mode_02");
        assert_eq!(
            mode.path.last().unwrap().kind,
            rv1::PresentationPathKind::Mode as i32
        );
    }
}
