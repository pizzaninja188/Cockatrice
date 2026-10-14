use crate::common::draw_discard_program::{
    assert_ability_draw_discard_steps, AuthoredDrawDiscardExpectation, AuthoredDrawDiscardOrder,
};
use tricerules_cards::primitives::{CastTriggerPlayer, PermanentTypeFilter, TriggerCondition};
use tricerules_cards::{AbilityPresentation, Layout};

#[test]
fn quicksmith_genius_registers_its_artifact_loot_trigger() {
    let registry = tricerules_cards::registry::global();
    let card = registry
        .get("quicksmith_genius")
        .expect("Quicksmith Genius registry definition");

    assert_eq!(card.name, "Quicksmith Genius");
    assert_eq!(
        registry.id_for_name("Quicksmith Genius"),
        Some("quicksmith_genius")
    );
    assert_eq!(card.layout, Layout::Normal);
    assert_eq!(card.face_count(), 1);

    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), "quicksmith_genius");
    assert_eq!(face.mana_cost.to_string(), "{2}{R}");
    assert_eq!(face.types, ["Creature", "Human", "Artificer"]);
    assert!(face.spell_effect.is_empty());
    assert!(face.activated_abilities.is_empty());

    let [ability] = face.triggered_abilities.as_slice() else {
        panic!("Quicksmith Genius has exactly one triggered ability");
    };
    assert!(matches!(
        &ability.presentation,
        AbilityPresentation::OracleLines(lines) if lines == &[1]
    ));
    assert!(matches!(
        &ability.trigger,
        TriggerCondition::WheneverPermanentEntersBattlefield {
            controller: CastTriggerPlayer::Controller,
            filter,
            creature_filter: None,
        } if filter.permanent_type == Some(PermanentTypeFilter::Artifact)
    ));
    assert!(ability.effect.is_empty() && ability.program.is_some());
    assert_ability_draw_discard_steps(
        registry,
        "quicksmith_genius",
        &face.face_id,
        &ability.ability_id,
        AuthoredDrawDiscardExpectation {
            first_instruction: 0,
            trailing_instructions: 0,
            who: tricerules_cards::primitives::PlayerRecipient::Controller,
            draw_count: 1,
            discard_count: 1,
            order: AuthoredDrawDiscardOrder::OptionalDiscardThenDraw,
        },
    );
}
