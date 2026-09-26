use tricerules_cards::primitives::{
    CastTriggerPlayer, DrawDiscardOrder, PermanentTypeFilter, PlayerRecipient, SpellEffectKind,
    TriggerCondition,
};
use tricerules_cards::{AbilityPresentation, CardRegistry, Layout};

#[test]
fn quicksmith_genius_registers_its_artifact_loot_trigger() {
    let registry = CardRegistry::global();
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
    assert!(matches!(
        ability.effect.as_slice(),
        [SpellEffectKind::DrawDiscard {
            who: PlayerRecipient::Controller,
            draw_count: 1,
            discard_count: 1,
            order: DrawDiscardOrder::DiscardThenDraw,
            optional: true,
        }]
    ));
}
