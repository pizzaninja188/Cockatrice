use tricerules_cards::primitives::{SpellEffectKind, TargetSchema, TargetingDef};
use tricerules_cards::{CardRegistry, ModalDef};

fn assert_schema(
    card_id: &str,
    location: &str,
    effects: &[SpellEffectKind],
    targeting: Option<&TargetingDef>,
) {
    let schema = TargetSchema::compile(effects, targeting)
        .unwrap_or_else(|error| panic!("{card_id} {location}: {error}"));
    let bound_roles = schema
        .groups
        .iter()
        .flat_map(|group| &group.bindings)
        .count();
    let declared_roles = effects
        .iter()
        .map(|effect| effect.target_roles().len())
        .sum::<usize>();
    assert_eq!(
        bound_roles, declared_roles,
        "{card_id} {location}: every declared target role must be bound exactly once"
    );
}

fn assert_modal_schema(card_id: &str, location: &str, modal: &ModalDef) {
    for (mode_index, mode) in modal.modes.iter().enumerate() {
        assert_schema(
            card_id,
            &format!("{location} mode {mode_index}"),
            &mode.effects,
            mode.targeting.as_ref(),
        );
    }
}

#[test]
fn every_authored_effect_list_has_one_complete_target_schema() {
    for card in CardRegistry::global().definitions() {
        for (face_index, face) in card.faces.iter().enumerate() {
            assert_schema(
                &card.id,
                &format!("face {face_index} spell"),
                &face.spell_effect,
                face.targeting.as_ref(),
            );
            if let Some(modal) = &face.modal_spell {
                assert_modal_schema(&card.id, &format!("face {face_index} spell"), modal);
            }
            for (ability_index, ability) in face.activated_abilities.iter().enumerate() {
                assert_schema(
                    &card.id,
                    &format!("face {face_index} activated ability {ability_index}"),
                    &ability.effect,
                    ability.targeting.as_ref(),
                );
            }
            for (ability_index, ability) in face.triggered_abilities.iter().enumerate() {
                assert_schema(
                    &card.id,
                    &format!("face {face_index} triggered ability {ability_index}"),
                    &ability.effect,
                    ability.targeting.as_ref(),
                );
                if let Some(modal) = &ability.modal {
                    assert_modal_schema(
                        &card.id,
                        &format!("face {face_index} triggered ability {ability_index}"),
                        modal,
                    );
                }
            }
        }
    }
}
