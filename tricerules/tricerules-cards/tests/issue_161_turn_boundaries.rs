use tricerules_cards::primitives::{
    AbilityCost, DelayedTokenSacrificeTiming, SpellEffectKind, StaticAbilityDef,
};
use tricerules_cards::{CardRegistry, Keyword};

#[test]
fn issue_161_cards_use_shared_turn_boundary_primitives() {
    let registry = CardRegistry::global();

    let kav = registry
        .get("kav_landseeker")
        .expect("Kav Landseeker")
        .primary_face();
    assert_eq!((kav.power, kav.toughness), (Some(4), Some(3)));
    assert_eq!(kav.keywords, vec![Keyword::Menace]);
    assert!(kav.triggered_abilities.iter().any(|ability| matches!(
        ability.effect.as_slice(),
        [SpellEffectKind::CreateTokens {
            token,
            sacrifice_timing:
                Some(DelayedTokenSacrificeTiming::ControllerNextTurnEndStep),
            ..
        }] if token == "lander"
    )));

    let waterskin = registry
        .get("benders_waterskin")
        .expect("Bender's Waterskin")
        .primary_face();
    assert_eq!(
        waterskin.static_abilities,
        vec![StaticAbilityDef::UntapsDuringOtherPlayersUntapSteps]
    );
    let mana_ability = waterskin.activated_abilities.first().expect("mana ability");
    assert_eq!(mana_ability.costs, vec![AbilityCost::Tap]);
    assert!(matches!(
        mana_ability.effect.as_slice(),
        [SpellEffectKind::ProduceMana { options, .. }] if options.len() == 5
    ));
}
