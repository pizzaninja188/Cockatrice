//! Exact source-backed Professor Onyx definition; runtime evidence is separate.
use tricerules_cards::primitives::{
    AbilityCost, Amount, LifeAmount, PlayerRecipient, SacrificeRestriction, SpellEffectKind,
    TriggerCondition,
};
use tricerules_cards::AbilityPresentation;

#[test]
fn professor_onyx_exact_identity_and_all_four_abilities() {
    let registry = tricerules_cards::registry::global();
    assert_eq!(
        registry.id_for_name("Professor Onyx"),
        Some("professor_onyx")
    );
    let card = registry.get("professor_onyx").unwrap();
    let face = card.primary_face();
    assert!(!card.commander_setup_only);
    assert_eq!(face.face_id.as_str(), "professor_onyx");
    assert_eq!(face.mana_cost.to_string(), "{4}{B}{B}");
    assert_eq!(face.mana_cost.mana_value(), 6);
    assert_eq!(face.supertypes, ["Legendary"]);
    assert_eq!(face.types, ["Planeswalker", "Liliana"]);
    assert_eq!(face.loyalty, Some(5));
    assert_eq!(face.triggered_abilities.len(), 1);
    let magecraft = &face.triggered_abilities[0];
    assert_eq!(
        magecraft.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert!(matches!(
        magecraft.trigger,
        TriggerCondition::WheneverPlayerCastsOrCopiesSpell { .. }
    ));
    assert!(matches!(
        magecraft.effect.as_slice(),
        [
            SpellEffectKind::LoseLife {
                amount: LifeAmount::Fixed(2),
                who: PlayerRecipient::EachOpponent
            },
            SpellEffectKind::GainLife {
                amount: Amount::Fixed(2),
                who: PlayerRecipient::Controller
            },
        ]
    ));
    assert_eq!(face.activated_abilities.len(), 3);
    for (index, cost) in [1, -3, -8].into_iter().enumerate() {
        let ability = &face.activated_abilities[index];
        assert_eq!(ability.costs, [AbilityCost::Loyalty(cost)]);
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![index as u16 + 2])
        );
        assert!(
            ability.targeting.is_none() && ability.program.is_none() && ability.modal.is_none()
        );
    }
    assert!(matches!(
        face.activated_abilities[0].effect.as_slice(),
        [
            SpellEffectKind::LoseLife {
                amount: LifeAmount::Fixed(1),
                who: PlayerRecipient::Controller
            },
            SpellEffectKind::LookChooseOneToHandRestToGraveyard { count: 3 },
        ]
    ));
    assert!(matches!(
        face.activated_abilities[1].effect.as_slice(),
        [SpellEffectKind::ChooseAndSacrifice {
            restriction: SacrificeRestriction::GreatestPower,
            ..
        },]
    ));
    assert_eq!(face.activated_abilities[2].effect.len(), 7);
    assert!(face.activated_abilities[2]
        .effect
        .iter()
        .all(|effect| matches!(
            effect,
            SpellEffectKind::EachOpponentMayDiscardOrLoseLife { amount: 3 }
        )));
}
