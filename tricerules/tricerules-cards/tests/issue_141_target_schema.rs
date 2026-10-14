use tricerules_cards::primitives::{SpellEffectKind, TargetSchema, TargetingDef, TokenCopySource};
use tricerules_cards::ModalDef;

#[test]
fn token_copy_requires_one_permanent_source() {
    use tricerules_cards::primitives::{EffectContext, TargetFilter, TargetGroupDef, TargetKind};
    let effect = SpellEffectKind::CreateTokenCopies {
        count: 1.into(),
        source: TokenCopySource::Chosen(Box::new(TargetFilter {
            kind: TargetKind::AnyPlayer,
            ..Default::default()
        })),
    };
    assert!(effect.validate(EffectContext::Spell).is_err());
    let effect = SpellEffectKind::CreateTokenCopies {
        count: 2.into(),
        source: TokenCopySource::Chosen(Box::new(TargetFilter::default_creature())),
    };
    let targeting = TargetingDef {
        groups: vec![TargetGroupDef {
            chooser: Default::default(),
            min: 1,
            max: 2,
            prompt: "Sources".into(),
            effect_indices: vec![0],
            distinct_from: vec![],
            same_graveyard: false,
            cast_cost_expansion: None,
        }],
    };
    assert!(TargetSchema::compile(&[effect], Some(&targeting)).is_err());
}

fn assert_schema(
    card_id: &str,
    location: &str,
    effects: &[SpellEffectKind],
    program: Option<&tricerules_cards::CompiledProgram>,
    targeting: Option<&TargetingDef>,
) {
    let schema = match program {
        Some(program) => TargetSchema::compile_program(program, targeting),
        None => TargetSchema::compile(effects, targeting),
    }
    .unwrap_or_else(|error| panic!("{card_id} {location}: {error}"));
    let bound_roles = schema
        .groups
        .iter()
        .flat_map(|group| &group.bindings)
        .count();
    let declared_roles = program.map_or_else(
        || {
            effects
                .iter()
                .map(|effect| effect.target_roles().len())
                .sum::<usize>()
        },
        |program| program.target_bindings.len(),
    );
    assert_eq!(
        bound_roles, declared_roles,
        "{card_id} {location}: every declared target role must be bound exactly once"
    );
}

fn assert_modal_schema(
    registry: &tricerules_cards::CardRegistry,
    card_id: &str,
    face_id: &tricerules_cards::CardFaceId,
    ability_owner: Option<tricerules_cards::AbilityDefinitionId>,
    location: &str,
    modal: &ModalDef,
) {
    for (mode_index, mode) in modal.modes.iter().enumerate() {
        let owner = match &ability_owner {
            Some(ability) => tricerules_cards::ProgramOwnerId::ability_mode(
                ability.clone(),
                mode.mode_id.clone(),
            ),
            None => tricerules_cards::ProgramOwnerId::face_mode(
                card_id.to_owned(),
                face_id.clone(),
                mode.mode_id.clone(),
            ),
        };
        let program = mode
            .program
            .as_ref()
            .and_then(|_| registry.compiled_program_for_owner(&owner));
        assert_schema(
            card_id,
            &format!("{location} mode {mode_index}"),
            &mode.effects,
            program,
            mode.targeting.as_ref(),
        );
    }
}

#[test]
fn every_authored_effect_list_has_one_complete_target_schema() {
    let registry = tricerules_cards::registry::global();
    for card in registry.definitions() {
        for (face_index, face) in card.faces.iter().enumerate() {
            let face_owner =
                tricerules_cards::ProgramOwnerId::face(card.id.clone(), face.face_id.clone());
            let spell_program = face
                .spell_program
                .as_ref()
                .and_then(|_| registry.compiled_program_for_owner(&face_owner));
            assert_schema(
                &card.id,
                &format!("face {face_index} spell"),
                &face.spell_effect,
                spell_program,
                face.targeting.as_ref(),
            );
            if let Some(modal) = &face.modal_spell {
                assert_modal_schema(
                    registry,
                    &card.id,
                    &face.face_id,
                    None,
                    &format!("face {face_index} spell"),
                    modal,
                );
            }
            for (ability_index, ability) in face.activated_abilities.iter().enumerate() {
                let ability_definition = tricerules_cards::AbilityDefinitionId {
                    card_id: card.id.clone(),
                    face_id: face.face_id.clone(),
                    ability_path: vec![ability.ability_id.clone()],
                };
                let owner = tricerules_cards::ProgramOwnerId::ability(ability_definition.clone());
                let program = ability
                    .program
                    .as_ref()
                    .and_then(|_| registry.compiled_program_for_owner(&owner));
                assert_schema(
                    &card.id,
                    &format!("face {face_index} activated ability {ability_index}"),
                    &ability.effect,
                    program,
                    ability.targeting.as_ref(),
                );
            }
            for (ability_index, ability) in face.triggered_abilities.iter().enumerate() {
                let ability_definition = tricerules_cards::AbilityDefinitionId {
                    card_id: card.id.clone(),
                    face_id: face.face_id.clone(),
                    ability_path: vec![ability.ability_id.clone()],
                };
                let owner = tricerules_cards::ProgramOwnerId::ability(ability_definition.clone());
                let program = ability
                    .program
                    .as_ref()
                    .and_then(|_| registry.compiled_program_for_owner(&owner));
                assert_schema(
                    &card.id,
                    &format!("face {face_index} triggered ability {ability_index}"),
                    &ability.effect,
                    program,
                    ability.targeting.as_ref(),
                );
                if let Some(modal) = &ability.modal {
                    assert_modal_schema(
                        registry,
                        &card.id,
                        &face.face_id,
                        Some(ability_definition),
                        &format!("face {face_index} triggered ability {ability_index}"),
                        modal,
                    );
                }
            }
        }
    }
}
