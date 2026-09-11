use tricerules_cards::primitives::{
    EffectSubject, PermanentTypeFilter, ResolutionCost, StaticAbilityDef, TargetController,
    TargetKind, TargetingSourceFilter,
};
use tricerules_cards::{
    AbilityCost, AbilityPresentation, CardRegistry, CastTriggerPlayer,
    CharacteristicDefiningAbility, ManaCost, ObjectContributionKind, ObjectPaymentConstraint,
    SpellEffectKind, TriggerCondition,
};

#[test]
fn issue_257_registers_all_nine_generated_cards_with_exact_typed_abilities() {
    let registry = CardRegistry::global();

    for (id, threshold, oracle_line) in [
        ("skybox_ferry", 2, 2),
        ("air_response_unit", 1, 2),
        ("dependable_quinjet", 4, 3),
        ("clamorous_ironclad", 3, 2),
        ("cultivators_caravan", 3, 2),
    ] {
        let face = registry
            .get(id)
            .unwrap_or_else(|| panic!("missing generated Vehicle {id}"))
            .primary_face();
        assert_eq!(&face.types[..2], ["Artifact", "Vehicle"]);
        let crew = face
            .activated_abilities
            .iter()
            .find(|ability| {
                matches!(
                    ability.costs.as_slice(),
                    [AbilityCost::TapPermanents { .. }]
                )
            })
            .unwrap_or_else(|| panic!("{id} must have Crew"));
        assert_eq!(
            crew.presentation,
            AbilityPresentation::OracleLines(vec![oracle_line])
        );
        let [AbilityCost::TapPermanents {
            constraint,
            filter,
            exclude_source,
        }] = crew.costs.as_slice()
        else {
            unreachable!()
        };
        assert_eq!(
            *constraint,
            ObjectPaymentConstraint::AggregateMinimum {
                minimum: threshold,
                contribution: ObjectContributionKind::CurrentPower,
            }
        );
        assert_eq!(filter.kind, TargetKind::Creature);
        assert_eq!(filter.controller, TargetController::You);
        assert!(*exclude_source);
        assert!(matches!(
            crew.effect.as_slice(),
            [SpellEffectKind::AddTypes {
                subject: EffectSubject::Source,
                addition,
            }] if addition.card_types == [PermanentTypeFilter::Creature]
                && addition.creature_types.is_empty()
        ));
    }

    for (id, cost, oracle_line) in [
        ("spider-rex,_daring_dino", "{2}", 2),
        ("marauding_brinefang", "{3}", 1),
    ] {
        let face = registry
            .get(id)
            .unwrap_or_else(|| panic!("missing generated Ward card {id}"))
            .primary_face();
        let [ward] = face.triggered_abilities.as_slice() else {
            panic!("{id} must have one Ward trigger");
        };
        assert_eq!(
            ward.presentation,
            AbilityPresentation::OracleLines(vec![oracle_line])
        );
        assert_eq!(
            ward.trigger,
            TriggerCondition::WheneverSelfBecomesTarget {
                source: TargetingSourceFilter::SpellOrAbility,
                source_controller: CastTriggerPlayer::Opponent,
            }
        );
        assert_eq!(
            ward.effect,
            [SpellEffectKind::CounterTriggeringStackObjectUnlessPays {
                cost: ResolutionCost::Mana(ManaCost::parse(cost).unwrap()),
            }]
        );
    }

    let feastling = registry
        .get("prideful_feastling")
        .expect("missing generated Prideful Feastling")
        .primary_face();
    let [changeling] = feastling.characteristic_defining_abilities.as_slice() else {
        panic!("Prideful Feastling must have one CDA");
    };
    assert_eq!(changeling.ability_id.as_str(), "characteristic_01");
    assert_eq!(
        changeling.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(
        changeling.definition,
        CharacteristicDefiningAbility::Changeling
    );

    let bear = registry
        .get("gigantic_big_bear")
        .expect("missing generated Gigantic Big Bear")
        .primary_face();
    let [uncounterable] = bear.static_abilities.as_slice() else {
        panic!("Gigantic Big Bear must have one static ability");
    };
    assert_eq!(uncounterable.ability_id.as_str(), "static_01");
    assert_eq!(
        uncounterable.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(
        uncounterable.definition,
        StaticAbilityDef::SpellCannotBeCountered
    );
}
