use crate::common::draw_discard_program::{
    assert_ability_draw_discard_steps, AuthoredDrawDiscardExpectation, AuthoredDrawDiscardOrder,
};
use tricerules_cards::{AbilityPresentation, TriggerCondition};

#[test]
fn issue_282_registers_the_four_generated_self_tap_loot_cards() {
    let registry = tricerules_cards::registry::global();
    for (id, name, mana_cost, types, stats, order) in [
        (
            "silvergill_peddler",
            "Silvergill Peddler",
            "{2}{U}",
            vec!["Creature", "Merfolk", "Citizen"],
            (2, 3),
            AuthoredDrawDiscardOrder::DrawThenDiscard,
        ),
        (
            "mechan_navigator",
            "Mechan Navigator",
            "{1}{U}",
            vec!["Artifact", "Creature", "Robot", "Pilot"],
            (2, 1),
            AuthoredDrawDiscardOrder::DrawThenDiscard,
        ),
        (
            "rescue_leopard",
            "Rescue Leopard",
            "{2}{R}",
            vec!["Creature", "Cat"],
            (4, 2),
            AuthoredDrawDiscardOrder::OptionalDiscardThenDraw,
        ),
        (
            "volatile_wanderglyph",
            "Volatile Wanderglyph",
            "{1}{R}",
            vec!["Artifact", "Creature", "Golem"],
            (2, 2),
            AuthoredDrawDiscardOrder::OptionalDiscardThenDraw,
        ),
    ] {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.name, name);
        let face = definition.primary_face();
        assert_eq!(face.face_id.as_str(), id);
        assert_eq!(face.mana_cost.to_string(), mana_cost);
        assert_eq!(face.types, types);
        assert_eq!(face.power.zip(face.toughness), Some(stats));
        let [ability] = face.triggered_abilities.as_slice() else {
            panic!("{id} must have exactly one generated trigger");
        };
        assert_eq!(ability.ability_id.as_str(), "triggered_01");
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![1])
        );
        assert_eq!(ability.trigger, TriggerCondition::WheneverSelfBecomesTapped);
        assert!(!ability.may, "optional discard belongs to the effect");
        assert!(ability.targeting.is_none(), "self-tap loot has no targets");
        assert!(ability.effect.is_empty() && ability.program.is_some());
        assert_ability_draw_discard_steps(
            registry,
            id,
            &face.face_id,
            &ability.ability_id,
            AuthoredDrawDiscardExpectation {
                first_instruction: 0,
                trailing_instructions: 0,
                who: tricerules_cards::primitives::PlayerRecipient::Controller,
                draw_count: 1,
                discard_count: 1,
                order,
            },
        );
    }
}
