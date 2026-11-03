use tricerules_cards::primitives::{
    AbilityCost, Amount, PlayerRecipient, SpellEffectKind, StaticEmblemEffect,
};
use tricerules_cards::AbilityPresentation;

#[test]
fn tamiyo_exact_identity_and_all_three_loyalty_abilities() {
    let registry = tricerules_cards::registry::global();
    assert_eq!(
        registry.id_for_name("Tamiyo, Field Researcher"),
        Some("tamiyo,_field_researcher")
    );
    let card = registry.get("tamiyo,_field_researcher").unwrap();
    assert!(!card.commander_setup_only);
    let face = card.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{1}{G}{W}{U}");
    assert_eq!(face.supertypes, ["Legendary"]);
    assert_eq!(face.types, ["Planeswalker", "Tamiyo"]);
    assert_eq!(face.loyalty, Some(4));
    assert_eq!(face.activated_abilities.len(), 3);
    for (index, cost) in [1, -2, -7].into_iter().enumerate() {
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
        [SpellEffectKind::CreateWatchedCombatDamageDrawUntilNextTurn { .. }]
    ));
    assert!(matches!(
        face.activated_abilities[1].effect.as_slice(),
        [
            SpellEffectKind::Tap { .. },
            SpellEffectKind::SkipNextUntap { .. }
        ]
    ));
    assert!(matches!(face.activated_abilities[2].effect.as_slice(), [
        SpellEffectKind::Draw {who:PlayerRecipient::Controller,count:Amount::Fixed(3)},
        SpellEffectKind::CreateStaticEmblem {effects,..}
    ] if effects == &[StaticEmblemEffect::CastSpellsFromHandWithoutPayingManaCost]));
}
