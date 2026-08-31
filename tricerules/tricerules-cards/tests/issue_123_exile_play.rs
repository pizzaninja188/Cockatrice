use tricerules_cards::primitives::{
    Amount, GameCondition, GraveyardAggregate, PlayerRecipient, RelativePlayerSet,
    ResolutionBranchRequirement, ResolutionBranchSelection, SpellEffectKind,
};
use tricerules_cards::{CardRegistry, TriggerCondition};

#[test]
fn issue_123_cards_use_the_generic_exile_play_effect() {
    let registry = CardRegistry::global();

    let percussionist = registry
        .get("clockwork_percussionist")
        .expect("Clockwork Percussionist");
    let trigger = &percussionist.primary_face().triggered_abilities[0];
    assert_eq!(trigger.trigger, TriggerCondition::WhenSelfDies);
    assert!(matches!(
        trigger.effect.as_slice(),
        [SpellEffectKind::ExileTopWithPlayPermission {
            player: PlayerRecipient::Controller,
            ..
        }]
    ));

    let inferno = registry
        .get("impossible_inferno")
        .expect("Impossible Inferno");
    let effects = &inferno.primary_face().spell_effect;
    assert!(matches!(
        effects[0],
        SpellEffectKind::DamageTarget {
            amount: Amount::Fixed(6),
            ..
        }
    ));
    let SpellEffectKind::ChooseResolutionBranch {
        selection,
        branches,
        ..
    } = &effects[1]
    else {
        panic!("Impossible Inferno must use a delirium branch")
    };
    assert_eq!(*selection, ResolutionBranchSelection::FirstApplicable);
    assert!(matches!(
        branches[0].requirement,
        ResolutionBranchRequirement::GameCondition(GameCondition::GraveyardAggregate {
            owners: RelativePlayerSet::Controller,
            aggregate: GraveyardAggregate::DistinctCardTypes,
            min: Some(4),
            max: None,
            ..
        })
    ));
    assert!(matches!(
        branches[0].effects.as_slice(),
        [SpellEffectKind::ExileTopWithPlayPermission {
            player: PlayerRecipient::Controller,
            ..
        }]
    ));
}
