use tricerules_cards::primitives::{
    CombatRestriction, CreatureScopeController, CreatureScopeFilter, StaticAbilityDef,
};
use tricerules_cards::{CardRegistry, CounterKind};

#[test]
fn issue_194_consumers_are_registered() {
    let registry = CardRegistry::global();

    let michelangelo = registry
        .get("michelangelo,_mutant_bff")
        .expect("Michelangelo, Mutant BFF must be registered");
    assert_eq!(
        michelangelo.primary_face().mana_cost.to_string(),
        "{2}{G}{G}"
    );
    assert_eq!(
        (
            michelangelo.primary_face().power,
            michelangelo.primary_face().toughness
        ),
        (Some(4), Some(4))
    );
    assert!(matches!(
        &michelangelo.primary_face().static_abilities[0].definition,
        StaticAbilityDef::CreatureScopeCombatRestriction {
            filter: CreatureScopeFilter {
                controller: Some(CreatureScopeController::YouControl),
                requires_any_counter: true,
                required_counter: None,
                ..
            },
            restriction: CombatRestriction {
                maximum_blockers: Some(1),
                ..
            },
        }
    ));

    let herald = registry
        .get("herald_of_secret_streams")
        .expect("Herald of Secret Streams must be registered");
    assert_eq!(herald.primary_face().mana_cost.to_string(), "{3}{U}");
    assert_eq!(
        (herald.primary_face().power, herald.primary_face().toughness),
        (Some(2), Some(3))
    );
    assert!(matches!(
        &herald.primary_face().static_abilities[0].definition,
        StaticAbilityDef::CreatureScopeCombatRestriction {
            filter: CreatureScopeFilter {
                controller: Some(CreatureScopeController::YouControl),
                requires_any_counter: false,
                required_counter: Some(CounterKind::PlusOnePlusOne),
                ..
            },
            restriction: CombatRestriction {
                cant_be_blocked: true,
                ..
            },
        }
    ));
}

#[test]
fn creature_scope_counter_predicates_are_mutually_exclusive() {
    assert!(CreatureScopeFilter {
        requires_any_counter: true,
        required_counter: Some(CounterKind::PlusOnePlusOne),
        ..CreatureScopeFilter::default()
    }
    .validate()
    .is_err());
    assert!(CreatureScopeFilter {
        requires_any_counter: true,
        ..CreatureScopeFilter::default()
    }
    .validate()
    .is_ok());
    assert!(CreatureScopeFilter {
        required_counter: Some(CounterKind::PlusOnePlusOne),
        ..CreatureScopeFilter::default()
    }
    .validate()
    .is_ok());
}
