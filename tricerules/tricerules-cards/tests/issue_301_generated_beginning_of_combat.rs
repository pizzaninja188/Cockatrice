use std::collections::BTreeSet;

use tricerules_cards::primitives::{
    CastTriggerPlayer, GraveyardDestination, GraveyardOwner, SpellEffectKind, TriggerCondition,
};
use tricerules_cards::{AbilityPresentation, CardRegistry, Keyword};

const ISSUE_301_CARD_IDS: [&str; 2] = ["ascendant_dustspeaker", "startled_relic_sloth"];

#[test]
fn issue_301_registers_exactly_the_reviewed_two_card_cohort() {
    let registry = CardRegistry::global();
    let matching_ids = registry
        .definitions()
        .filter(|card| {
            card.faces_iter().any(|face| {
                face.triggered_abilities.iter().any(|ability| {
                    ability.trigger
                        == TriggerCondition::AtBeginningOfCombat {
                            player: CastTriggerPlayer::Controller,
                        }
                        && matches!(
                            ability.effect.as_slice(),
                            [SpellEffectKind::MoveGraveyardCards {
                                filter,
                                destination: GraveyardDestination::Exile,
                                linked_exile_id: None,
                            }] if filter.owner == GraveyardOwner::AnyPlayer
                                && filter.card.is_none()
                                && filter.excluded_objects.is_empty()
                        )
                        && matches!(
                            ability.targeting.as_ref().map(|targeting| targeting.groups.as_slice()),
                            Some([group]) if group.min == 0
                                && group.max == 1
                                && group.effect_indices == [0]
                                && group.distinct_from.is_empty()
                                && !group.same_graveyard
                                && group.cast_cost_expansion.is_none()
                                && group.prompt == "Choose up to one target card from a graveyard"
                        )
                })
            })
        })
        .map(|card| card.id.as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        matching_ids,
        ISSUE_301_CARD_IDS.into_iter().collect::<BTreeSet<_>>(),
        "the exact beginning-of-combat graveyard exile recipe must have only its reviewed cohort"
    );
}

#[test]
fn issue_301_preserves_each_card_characteristics_and_other_abilities() {
    let registry = CardRegistry::global();

    let dustspeaker = registry
        .get("ascendant_dustspeaker")
        .expect("Ascendant Dustspeaker")
        .primary_face();
    assert_eq!(dustspeaker.name, "Ascendant Dustspeaker");
    assert_eq!(dustspeaker.face_id.as_str(), "ascendant_dustspeaker");
    assert_eq!(dustspeaker.mana_cost.to_string(), "{4}{W}");
    assert_eq!(dustspeaker.types, ["Creature", "Orc", "Cleric"]);
    assert_eq!(
        (dustspeaker.power, dustspeaker.toughness),
        (Some(3), Some(4))
    );
    assert_eq!(dustspeaker.keywords, [Keyword::Flying]);
    assert_eq!(dustspeaker.triggered_abilities.len(), 2);
    assert_eq!(
        dustspeaker.triggered_abilities[0].presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(
        dustspeaker.triggered_abilities[1].presentation,
        AbilityPresentation::OracleLines(vec![3])
    );

    let sloth = registry
        .get("startled_relic_sloth")
        .expect("Startled Relic Sloth")
        .primary_face();
    assert_eq!(sloth.name, "Startled Relic Sloth");
    assert_eq!(sloth.face_id.as_str(), "startled_relic_sloth");
    assert_eq!(sloth.mana_cost.to_string(), "{2}{R}{W}");
    assert_eq!(sloth.types, ["Creature", "Sloth", "Beast"]);
    assert_eq!((sloth.power, sloth.toughness), (Some(4), Some(4)));
    assert_eq!(sloth.keywords, [Keyword::Trample, Keyword::Lifelink]);
    assert_eq!(sloth.triggered_abilities.len(), 1);
    assert_eq!(
        sloth.triggered_abilities[0].presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
}
