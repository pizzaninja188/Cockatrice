use tricerules_cards::primitives::{CardTypeFilter, ResolutionCost};
use tricerules_cards::{
    AbilityPresentation, CardRegistry, CastTriggerPlayer, PermanentTypeFilter, SpellCastFilter,
    SpellEffectKind, TriggerCondition,
};

fn repartee_filter(card_id: &str) -> &SpellCastFilter {
    let face = CardRegistry::global()
        .get(card_id)
        .unwrap_or_else(|| panic!("missing Issue #191 card {card_id}"))
        .primary_face();
    let ability = face
        .triggered_abilities
        .iter()
        .find(|ability| {
            matches!(
                ability.trigger,
                TriggerCondition::WheneverPlayerCastsSpell { .. }
            )
        })
        .expect("Repartee cast trigger");
    let TriggerCondition::WheneverPlayerCastsSpell {
        caster,
        filter,
        ordinal,
        ..
    } = &ability.trigger
    else {
        unreachable!()
    };
    assert_eq!(*caster, CastTriggerPlayer::Controller);
    assert_eq!(*ordinal, None);
    filter
}

#[test]
fn issue_191_cards_author_the_shared_repartee_filter() {
    for card_id in ["forum_necroscribe", "graduation_day"] {
        let filter = repartee_filter(card_id);
        assert_eq!(filter.card_type, Some(CardTypeFilter::InstantOrSorcery));
        assert_eq!(
            filter.targeted_permanent_type,
            Some(PermanentTypeFilter::Creature)
        );
    }
}

#[test]
fn forum_necroscribe_keeps_ward_and_oracle_line_identity() {
    let forum = CardRegistry::global()
        .get("forum_necroscribe")
        .expect("Forum Necroscribe")
        .primary_face();
    assert_eq!(forum.triggered_abilities.len(), 2);
    assert_eq!(
        forum.triggered_abilities[0].presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert!(matches!(
        forum.triggered_abilities[0].effect.as_slice(),
        [SpellEffectKind::CounterTriggeringStackObjectUnlessPays {
            cost: ResolutionCost::DiscardCard { filter: None }
        }]
    ));
    assert_eq!(
        forum.triggered_abilities[1].presentation,
        AbilityPresentation::OracleLines(vec![2])
    );

    let graduation = CardRegistry::global()
        .get("graduation_day")
        .expect("Graduation Day")
        .primary_face();
    assert_eq!(
        graduation.triggered_abilities[0].presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
}
