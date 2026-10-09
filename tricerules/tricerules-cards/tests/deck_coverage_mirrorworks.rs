use tricerules_cards::primitives::{
    Amount, CastTriggerPlayer, PermanentTypeFilter, PlayerRecipient, ResolutionCost,
    SpellEffectKind, TokenCopySource, TriggerCondition,
};
use tricerules_cards::{AbilityPresentation, Layout, ManaCost};

#[test]
fn mirrorworks_is_registered_as_a_complete_card() {
    let card = tricerules_cards::registry::global()
        .get("mirrorworks")
        .expect("Mirrorworks needs a complete definition");
    assert_eq!(card.name, "Mirrorworks");
    assert_eq!(card.layout, Layout::Normal);
    assert_eq!(card.face_count(), 1);

    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), "mirrorworks");
    assert_eq!(face.mana_cost, ManaCost::parse("{5}").unwrap());
    assert_eq!(face.types, ["Artifact"]);
    assert!(face.keywords.is_empty());

    let [trigger] = face.triggered_abilities.as_slice() else {
        panic!("Mirrorworks has exactly one triggered ability");
    };
    assert_eq!(trigger.ability_id.as_str(), "triggered_01");
    assert!(matches!(
        &trigger.presentation,
        AbilityPresentation::OracleLines(lines) if lines == &[1]
    ));
    match &trigger.trigger {
        TriggerCondition::WheneverPermanentEntersBattlefield {
            controller,
            filter,
            creature_filter,
        } => {
            assert_eq!(*controller, CastTriggerPlayer::Controller);
            assert_eq!(filter.permanent_type, Some(PermanentTypeFilter::Artifact));
            assert_eq!(filter.token, Some(false));
            assert!(filter.exclude_source);
            assert!(creature_filter.is_none());
        }
        other => panic!("unexpected Mirrorworks trigger: {other:?}"),
    }
    assert!(matches!(
        trigger.effect.as_slice(),
        [SpellEffectKind::ChooseResolutionBranch {
            chooser: PlayerRecipient::Controller,
            optional: true,
            branches,
            otherwise,
            ..
        }] if otherwise.is_empty()
            && matches!(
                branches.as_slice(),
                [branch] if branch.branch_id.as_str() == "pay_to_copy"
                    && matches!(&branch.presentation, AbilityPresentation::OracleLines(lines) if lines == &[1])
                    && matches!(&branch.cost, ResolutionCost::Mana(cost) if cost == &ManaCost::parse("{2}").unwrap())
                    && branch.requirement == Default::default()
                    && matches!(
                        branch.effects.as_slice(),
                        [SpellEffectKind::CreateTokenCopies {
                            count: Amount::Fixed(1),
                            source: TokenCopySource::TriggerObject,
                        }]
                    )
            )
    ));
}
