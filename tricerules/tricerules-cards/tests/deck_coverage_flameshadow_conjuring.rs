use tricerules_cards::primitives::{
    Amount, CastTriggerPlayer, EffectSubject, PermanentTypeFilter, PlayerRecipient, ResolutionCost,
    ResolvingEffectDuration, ResolvingPermanentModifier, SpellEffectKind, TokenCopySource,
    TriggerCondition,
};
use tricerules_cards::{AbilityPresentation, Layout, ManaCost};

#[test]
fn flameshadow_conjuring_is_registered_as_a_complete_card() {
    let card = tricerules_cards::registry::global()
        .get("flameshadow_conjuring")
        .expect("Flameshadow Conjuring needs a complete definition");
    assert_eq!(card.name, "Flameshadow Conjuring");
    assert_eq!(card.layout, Layout::Normal);
    assert_eq!(card.face_count(), 1);

    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), "flameshadow_conjuring");
    assert_eq!(face.mana_cost, ManaCost::parse("{3}{R}").unwrap());
    assert_eq!(face.types, ["Enchantment"]);
    let [trigger] = face.triggered_abilities.as_slice() else {
        panic!("Flameshadow Conjuring has exactly one triggered ability");
    };
    assert!(matches!(
        &trigger.trigger,
        TriggerCondition::WheneverPermanentEntersBattlefield { controller: CastTriggerPlayer::Controller, filter, creature_filter: None }
            if filter.permanent_type == Some(PermanentTypeFilter::Creature)
                && filter.token == Some(false)
                && !filter.exclude_source
    ));
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
                    && matches!(&branch.cost, ResolutionCost::Mana(cost) if cost == &ManaCost::parse("{R}").unwrap())
                    && branch.requirement == Default::default()
                    && matches!(branch.effects.as_slice(), [
                        SpellEffectKind::CreateTokenCopies { count: Amount::Fixed(1), source: TokenCopySource::TriggerObject },
                        SpellEffectKind::ApplyPermanentModifier {
                            subject: EffectSubject::PreviousEffectObject,
                            modifier: ResolvingPermanentModifier::GrantKeywords(keywords),
                            duration: ResolvingEffectDuration::Indefinite,
                        },
                        SpellEffectKind::CreateDelayedTrigger {
                            subject: Some(EffectSubject::PreviousEffectObject),
                            ability,
                            ..
                        },
                    ] if keywords == &[tricerules_cards::Keyword::Haste]
                        && matches!(ability.trigger, TriggerCondition::AtBeginningOfNextEndStep)
                        && matches!(ability.effect.as_slice(), [SpellEffectKind::Exile { subject: EffectSubject::TriggerObject, .. }])
                    )
            )
    ));
}
