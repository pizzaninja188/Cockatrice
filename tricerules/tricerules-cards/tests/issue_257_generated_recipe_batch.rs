use tricerules_cards::primitives::{
    EffectSubject, InstructionKind, PermanentTypeFilter, ResolutionCost, StaticAbilityDef,
    TargetController, TargetKind, TargetingSourceFilter,
};
use tricerules_cards::{
    AbilityCost, AbilityPresentation, CastTriggerPlayer, CharacteristicDefiningAbility, ManaCost,
    ObjectContributionKind, ObjectPaymentConstraint, SpellEffectKind, TriggerCondition,
    TriggeredAbilityDef,
};

fn assert_ward_program(ward: &TriggeredAbilityDef, cost: ResolutionCost) {
    assert!(
        ward.effect.is_empty(),
        "Ward must not retain a legacy effect leaf"
    );
    assert!(
        ward.targeting.is_none(),
        "Ward consumes a triggering stack reference without targeting"
    );
    let program = ward
        .program
        .as_ref()
        .expect("Ward is owned by an ability Program");
    assert!(program.body.yields.is_empty(), "Ward is resultless");
    let [instruction] = program.body.steps.as_slice() else {
        panic!("Ward must be one resultless Program leaf");
    };
    assert!(instruction.bind.is_empty(), "Ward leaf binds no result");
    assert!(
        instruction.input.is_none(),
        "Ward leaf consumes only its captured trigger context"
    );
    let InstructionKind::Perform(effect) = &instruction.kind else {
        panic!("Ward must be a direct Perform leaf");
    };
    assert_eq!(
        **effect,
        SpellEffectKind::CounterTriggeringStackObjectUnlessPays { cost }
    );
}

#[test]
fn issue_257_registers_all_nine_generated_cards_with_exact_typed_abilities() {
    let registry = tricerules_cards::registry::global();

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
        ("archive_dragon", "{2}", 2),
        ("hoverstone_pilgrim", "{2}", 2),
        ("spider-rex,_daring_dino", "{2}", 2),
        ("marauding_brinefang", "{3}", 1),
        ("toadstool_admirer", "{2}", 1),
        ("tolarian_terror", "{2}", 2),
    ] {
        let face = registry
            .get(id)
            .unwrap_or_else(|| panic!("missing generated Ward card {id}"))
            .primary_face();
        let ward = face
            .triggered_abilities
            .iter()
            .find(|ability| {
                matches!(
                    &ability.trigger,
                    TriggerCondition::WheneverSelfBecomesTarget { .. }
                )
            })
            .unwrap_or_else(|| panic!("{id} must have one Ward trigger"));
        assert_eq!(ward.ability_id.as_str(), "triggered_01");
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
        assert_ward_program(ward, ResolutionCost::Mana(ManaCost::parse(cost).unwrap()));
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

#[test]
fn issue_257_all_seventeen_existing_ward_owners_use_exact_resultless_program_leaves() {
    let registry = tricerules_cards::registry::global();
    for (id, ability_id, cost) in [
        ("cackling_prowler", "triggered_01", "{2}"),
        ("colorstorm_stallion", "ward", "{1}"),
        ("dusk_rose_reliquary", "triggered_01", "{2}"),
        ("forum_necroscribe", "triggered_01", "discard"),
        ("archive_dragon", "triggered_01", "{2}"),
        ("hoverstone_pilgrim", "triggered_01", "{2}"),
        ("marauding_sphinx", "triggered_01", "{2}"),
        ("marauding_brinefang", "triggered_01", "{3}"),
        (
            "dirgur_island_dragon_skimming_strike",
            "triggered_01",
            "{2}",
        ),
        ("toadstool_admirer", "triggered_01", "{2}"),
        ("tolarian_terror", "triggered_01", "{2}"),
        ("punk_frogs", "triggered_01", "{3}"),
        ("spider-rex,_daring_dino", "triggered_01", "{2}"),
        ("spectral_snatcher", "triggered_01", "discard"),
        ("sunset_saboteur", "triggered_01", "discard"),
        ("trapped_in_the_screen", "triggered_01", "{2}"),
    ] {
        let face = registry
            .get(id)
            .unwrap_or_else(|| panic!("missing existing Ward card {id}"))
            .primary_face();
        let ward = face
            .triggered_abilities
            .iter()
            .find(|ability| {
                matches!(
                    &ability.trigger,
                    TriggerCondition::WheneverSelfBecomesTarget { .. }
                )
            })
            .unwrap_or_else(|| panic!("{id} must keep its face-level Ward owner"));
        assert_eq!(
            ward.ability_id.as_str(),
            ability_id,
            "{id} preserves the exact ability owner path"
        );
        assert_eq!(
            ward.trigger,
            TriggerCondition::WheneverSelfBecomesTarget {
                source: TargetingSourceFilter::SpellOrAbility,
                source_controller: CastTriggerPlayer::Opponent,
            },
            "{id} keeps its exact Ward trigger"
        );
        let cost = if cost == "discard" {
            ResolutionCost::DiscardCard { filter: None }
        } else {
            ResolutionCost::Mana(ManaCost::parse(cost).unwrap())
        };
        assert_ward_program(ward, cost);
    }

    let thorin = registry
        .get("thorin_oakenshield")
        .expect("missing Thorin Oakenshield")
        .primary_face();
    let ward_grants: Vec<_> = thorin
        .static_abilities
        .iter()
        .filter(|ability| {
            matches!(
                &ability.definition,
                StaticAbilityDef::GrantTriggeredAbilityToPermanents { .. }
            )
        })
        .collect();
    assert_eq!(
        ward_grants.len(),
        1,
        "Thorin preserves static_02 as the grant owner"
    );
    assert_eq!(ward_grants[0].ability_id.as_str(), "static_02");
    let StaticAbilityDef::GrantTriggeredAbilityToPermanents {
        triggered_abilities,
        ..
    } = &ward_grants[0].definition
    else {
        unreachable!()
    };
    let [ward] = triggered_abilities.as_slice() else {
        panic!("Thorin's static grant keeps one nested Ward ability");
    };
    assert_eq!(ward.ability_id.as_str(), "triggered_01");
    assert_ward_program(ward, ResolutionCost::Mana(ManaCost::parse("{1}").unwrap()));
}
