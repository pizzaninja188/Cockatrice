use std::collections::BTreeSet;

use tricerules_cards::mana::ManaCost;
use tricerules_cards::primitives::{
    AbilityCost, AbilitySourceZone, ActivationTiming, Amount, CardResultAction, CardResultFilter,
    CardResultSource, CardTypeFilter, CountExpression, DrawDiscardOrder, EffectSubject,
    GameCondition, ObjectPaymentConstraint, PlayerRecipient, PowerToughnessCharacteristic,
    SpellEffectKind, StaticAbilityDef, TargetController, TargetFilter, TargetKind,
};
use tricerules_cards::{AbilityPresentation, CardRegistry, CounterKind, Keyword};

const COHORT: [&str; 2] = ["debris_field_crusher", "uthros_scanship"];

#[test]
fn issue_309_registry_contains_exactly_the_reviewed_station_cohort() {
    let registry = CardRegistry::global();
    let matching = registry
        .definitions()
        .filter(|definition| {
            definition.faces_iter().any(|face| {
                face.types == ["Artifact", "Spacecraft"]
                    && face.activated_abilities.first().is_some_and(|ability| {
                        ability.presentation == AbilityPresentation::OracleLines(vec![2])
                            && ability.source_zone == AbilitySourceZone::Battlefield
                            && ability.timing == ActivationTiming::SorcerySpeed
                    })
                    && face.static_abilities.iter().any(|ability| {
                        ability.presentation == AbilityPresentation::OracleLines(vec![3])
                            && matches!(
                                &ability.definition,
                                StaticAbilityDef::ConditionalSelfModifier {
                                    condition: GameCondition::SourceCounterCount {
                                        counter: CounterKind::Charge,
                                        min: Some(8),
                                        max: None,
                                    },
                                    keywords,
                                    ..
                                } if *keywords == vec![Keyword::Flying]
                            )
                    })
            })
        })
        .map(|definition| definition.id.as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        matching,
        COHORT.into_iter().collect::<BTreeSet<_>>(),
        "only the two reviewed Station-8 Flying cards may enter this recipe"
    );
}

#[test]
fn issue_309_cards_preserve_exact_faces_abilities_and_presentation_fingerprints() {
    let registry = CardRegistry::global();
    for (id, name, mana, power, toughness, fingerprint) in [
        (
            "uthros_scanship",
            "Uthros Scanship",
            "{3}{U}",
            4,
            4,
            "90f5c08a3d44a4aeb08118a522a3ccbacb24b6d574382a36d7c4d2e3a16fbdf9",
        ),
        (
            "debris_field_crusher",
            "Debris Field Crusher",
            "{4}{R}",
            1,
            5,
            "dd97e1af88b765d5f11e48ad40b093d3ac458dfa6a3ddb6bdd5b3353c83c3afb",
        ),
    ] {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.name, name);
        assert_eq!(registry.id_for_name(name), Some(id));
        assert_eq!(definition.face_count(), 1);
        let face = definition.primary_face();
        assert_eq!(face.face_id.as_str(), id);
        assert_eq!(face.mana_cost.to_string(), mana);
        assert_eq!(face.types, ["Artifact", "Spacecraft"]);
        assert_eq!((face.power, face.toughness), (Some(power), Some(toughness)));
        assert!(face.keywords.is_empty());
        assert!(face.custom_effect.is_none());
        assert!(face.modal_spell.is_none());
        let presentation = registry
            .presentation_face(id, face.face_id.as_str())
            .unwrap_or_else(|| panic!("missing {id} presentation metadata"));
        assert_eq!(presentation.card_name, name);
        assert_eq!(presentation.face_name, name);
        assert_eq!(presentation.oracle_text_sha256, fingerprint);

        let station = &face.activated_abilities[0];
        assert_eq!(station.ability_id.as_str(), "activated_01");
        assert_eq!(
            station.presentation,
            AbilityPresentation::OracleLines(vec![2])
        );
        assert_eq!(station.source_zone, AbilitySourceZone::Battlefield);
        assert_eq!(station.timing, ActivationTiming::SorcerySpeed);
        assert_eq!(
            station.costs,
            [AbilityCost::TapPermanents {
                constraint: ObjectPaymentConstraint::ExactCount(1),
                filter: TargetFilter {
                    kind: TargetKind::Creature,
                    controller: TargetController::You,
                    ..TargetFilter::default()
                },
                exclude_source: true,
            }]
        );
        assert_eq!(
            station.effect,
            [SpellEffectKind::PutCounters {
                counter: CounterKind::Charge,
                count: Amount::Count(CountExpression::CardResultCharacteristicSum {
                    filter: CardResultFilter {
                        source: CardResultSource::Payment,
                        action: CardResultAction::Tap,
                        players: tricerules_cards::primitives::RelativePlayerSet::Controller,
                        card_type: Some(CardTypeFilter::Creature),
                    },
                    characteristic: PowerToughnessCharacteristic::Power,
                }),
                subject: EffectSubject::Source,
            }]
        );
        assert_eq!(face.static_abilities.len(), 1);
        if id == "uthros_scanship" {
            assert_eq!(face.activated_abilities.len(), 1);
            let [trigger] = face.triggered_abilities.as_slice() else {
                panic!("Uthros must have one ETB trigger")
            };
            assert_eq!(
                trigger.presentation,
                AbilityPresentation::OracleLines(vec![1])
            );
            assert_eq!(
                trigger.effect,
                [SpellEffectKind::DrawDiscard {
                    who: PlayerRecipient::Controller,
                    draw_count: 2,
                    discard_count: 1,
                    order: DrawDiscardOrder::DrawThenDiscard,
                    optional: false,
                }]
            );
        } else {
            assert_eq!(face.activated_abilities.len(), 2);
            let pump = &face.activated_abilities[1];
            assert_eq!(pump.ability_id.as_str(), "activated_02");
            assert_eq!(pump.presentation, AbilityPresentation::OracleLines(vec![4]));
            assert_eq!(
                pump.costs,
                [AbilityCost::Mana(ManaCost::parse("{1}{R}").unwrap())]
            );
            assert_eq!(
                pump.effect,
                [SpellEffectKind::PumpTarget {
                    power: 2,
                    toughness: 0,
                    scale: None,
                    subject: EffectSubject::Source,
                }]
            );
            let [trigger] = face.triggered_abilities.as_slice() else {
                panic!("Debris must have one ETB trigger")
            };
            assert_eq!(
                trigger.presentation,
                AbilityPresentation::OracleLines(vec![1])
            );
            assert_eq!(
                trigger.effect,
                [SpellEffectKind::DamageTarget {
                    amount: Amount::Fixed(3),
                    target: TargetFilter {
                        kind: TargetKind::AnyTarget,
                        ..TargetFilter::default()
                    },
                }]
            );
            let [group] = trigger.targeting.as_ref().unwrap().groups.as_slice() else {
                panic!("Debris target group")
            };
            assert_eq!(
                (group.min, group.max, group.prompt.as_str()),
                (1, 1, "Choose any target")
            );
        }
    }
}
