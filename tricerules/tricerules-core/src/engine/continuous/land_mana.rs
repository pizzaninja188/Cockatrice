use super::*;

pub(super) fn derived_intrinsic_land_mana(
    face: Option<&CardFace>,
    characteristics: &Characteristics,
) -> Option<(usize, ActivatedAbilityDef)> {
    if !characteristics.has_type("Land") {
        return None;
    }
    let current = BasicLandType::ALL
        .into_iter()
        .filter(|land_type| characteristics.has_type(land_type.as_str()))
        .map(BasicLandType::mana)
        .collect::<Vec<_>>();
    if current.is_empty() {
        return None;
    }
    let authored_span = face.map_or(0, |face| face.activated_abilities.len());
    let anchor = face.and_then(|face| {
        face.activated_abilities
            .iter()
            .enumerate()
            .find(|(_, ability)| ability.intrinsic_land_mana)
    });
    // Preserve existing option numbers for basic types that remain, then append new types.
    let mut options = anchor
        .and_then(|(_, ability)| match ability.effect.as_slice() {
            [SpellEffectKind::ProduceMana { options, .. }] => Some(
                options
                    .iter()
                    .copied()
                    .filter(|option| current.contains(option))
                    .collect::<Vec<_>>(),
            ),
            _ => None,
        })
        .unwrap_or_default();
    for option in current {
        if !options.contains(&option) {
            options.push(option);
        }
    }
    let index = anchor.map_or(authored_span, |(index, _)| index);
    let ability_id = anchor
        .map(|(_, ability)| ability.ability_id.clone())
        .unwrap_or_else(|| {
            tricerules_card_model::AbilityId::new("basic_land_mana").expect("constant ability id")
        });
    Some((
        index,
        ActivatedAbilityDef {
            intrinsic_land_mana: true,
            ability_id,
            presentation: tricerules_card_model::AbilityPresentation::Fallback,
            source_zone: AbilitySourceZone::Battlefield,
            costs: vec![AbilityCost::Tap],
            cost_modifiers: Vec::new(),
            effect: vec![SpellEffectKind::ProduceMana {
                options,
                commander_color_identity: false,
                restriction: None,
                conditional: None,
            }],
            targeting: None,
            timing: Default::default(),
            conditions: Vec::new(),
            activation_limit: None,
        },
    ))
}
