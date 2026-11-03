use tricerules_cards::primitives::{
    AbilityCost, CounterKind, ResolvingEffectDuration, SpellEffectKind,
};
use tricerules_cards::AbilityPresentation;

#[test]
fn kaya_exact_identity_and_all_three_loyalty_abilities() {
    let registry = tricerules_cards::registry::global();
    assert_eq!(
        registry.id_for_name("Kaya the Inexorable"),
        Some("kaya_the_inexorable")
    );
    let card = registry.get("kaya_the_inexorable").unwrap();
    assert!(!card.commander_setup_only);
    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), "kaya_the_inexorable");
    assert_eq!(face.mana_cost.to_string(), "{3}{W}{B}");
    assert_eq!(face.loyalty, Some(5));
    assert_eq!(face.types, ["Planeswalker", "Kaya"]);
    assert_eq!(face.supertypes, ["Legendary"]);
    assert_eq!(face.activated_abilities.len(), 3);
    for (index, cost) in [1, -3, -7].into_iter().enumerate() {
        let ability = &face.activated_abilities[index];
        assert_eq!(ability.costs, [AbilityCost::Loyalty(cost)]);
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![index as u16 + 1])
        );
        assert!(ability.program.is_none() && ability.modal.is_none());
    }
    assert!(matches!(
        face.activated_abilities[0].effect.as_slice(),
        [
            SpellEffectKind::PutCounters {
                counter: CounterKind::Ghostform,
                count: tricerules_cards::primitives::Amount::Fixed(1),
                ..
            },
            SpellEffectKind::GrantTriggeredAbility {
                duration: ResolvingEffectDuration::Indefinite,
                ..
            }
        ]
    ));
    let targets = face.activated_abilities[0].targeting.as_ref().unwrap();
    assert_eq!(targets.groups[0].min, 0);
    assert_eq!(targets.groups[0].max, 1);
    assert!(matches!(
        face.activated_abilities[1].effect.as_slice(),
        [SpellEffectKind::Exile { .. }]
    ));
    assert!(
        matches!(face.activated_abilities[2].effect.as_slice(),[SpellEffectKind::CreateTriggeredEmblem{ability,..}]
        if ability.effect.as_slice()==[SpellEffectKind::CastLegendarySpellFromHandGraveyardOrOwnedExile])
    );
}
