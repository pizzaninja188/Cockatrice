//! Direct-RON conformance for two corrected Standard graveyard ETBs. Their source identities and
//! Oracle text are pinned in Scryfall `oracle_cards` snapshot
//! `27bf3214-1271-490b-bdfe-c0be6c23d02e` (SHA-256
//! `9611b5d93b20478a0ee46bae8b20a9eb39ee980f0ef4f5f6f6aaa8f7ab010ab2`). Rooftop Percher's
//! 2025-11-17 ruling confirms that choosing no targets is legal; its Oracle plural “graveyards”
//! permits targets in different graveyards. Soul-Shackled Zombie's Oracle says “from a single
//! graveyard” and “loses 2 life”; its current rulings list adds no card-specific clarification.
//! Governed rules: triggered abilities (CR 603.1), target declaration/revalidation (CR 115.1,
//! 608.2b), and effects that cause life loss rather than damage (CR 119.3).

use tricerules_cards::primitives::{
    CardResultAction, CardResultFilter, CardResultSource, CardTypeFilter, LifeAmount,
    PlayerRecipient, RelativePlayerSet, ResolutionBranchRequirement, ResolutionBranchSelection,
    SpellEffectKind, TargetSchema,
};
use tricerules_cards::CardRegistry;

#[test]
fn rooftop_percher_allows_zero_targets_across_graveyards() {
    let face = CardRegistry::global()
        .get("rooftop_percher")
        .expect("Rooftop Percher is registered")
        .primary_face();
    let [ability] = face.triggered_abilities.as_slice() else {
        panic!("Rooftop Percher has one ETB ability");
    };
    let targeting = ability.targeting.as_ref().expect("graveyard targets");
    let [group] = targeting.groups.as_slice() else {
        panic!("Rooftop Percher uses one target group");
    };

    assert_eq!((group.min, group.max), (0, 2));
    assert_eq!(
        group.prompt,
        "Choose up to two target cards from graveyards"
    );
    assert!(!group.same_graveyard);
    assert!(TargetSchema::compile(&ability.effect, Some(targeting)).is_ok());
}

#[test]
fn soul_shackled_zombie_loses_life_only_when_a_creature_was_exiled() {
    let face = CardRegistry::global()
        .get("soul-shackled_zombie")
        .expect("Soul-Shackled Zombie is registered")
        .primary_face();
    let [ability] = face.triggered_abilities.as_slice() else {
        panic!("Soul-Shackled Zombie has one ETB ability");
    };
    let [SpellEffectKind::MoveGraveyardCards { .. }, SpellEffectKind::ChooseResolutionBranch {
        selection,
        branches,
        ..
    }] = ability.effect.as_slice()
    else {
        panic!("the ETB exiles cards before checking the exact exile result");
    };
    assert_eq!(*selection, ResolutionBranchSelection::FirstApplicable);

    let [drain, fallback] = branches.as_slice() else {
        panic!("the ETB has a creature-exiled branch and a no-op fallback");
    };
    assert!(matches!(
        drain.requirement,
        ResolutionBranchRequirement::CardResultCount {
            filter: CardResultFilter {
                source: CardResultSource::PreviousEffect,
                action: CardResultAction::Exile,
                players: RelativePlayerSet::All,
                card_type: Some(CardTypeFilter::Creature),
            },
            min: Some(1),
            max: None,
        }
    ));
    assert!(matches!(
        drain.effects.as_slice(),
        [
            SpellEffectKind::LoseLife {
                amount: LifeAmount::Fixed(2),
                who: PlayerRecipient::EachOpponent,
            },
            SpellEffectKind::GainLife { .. },
        ]
    ));
    assert!(matches!(
        fallback.requirement,
        ResolutionBranchRequirement::Always
    ));
    assert!(fallback.effects.is_empty());

    let targeting = ability.targeting.as_ref().expect("graveyard targets");
    let [group] = targeting.groups.as_slice() else {
        panic!("Soul-Shackled Zombie uses one target group");
    };
    assert_eq!((group.min, group.max), (0, 2));
    assert!(group.same_graveyard);
    assert!(TargetSchema::compile(&ability.effect, Some(targeting)).is_ok());
}
