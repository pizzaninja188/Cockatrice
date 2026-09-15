use std::collections::BTreeSet;

use tricerules_cards::primitives::{
    AbilityCost, AbilitySourceZone, ActivationTiming, Amount, CardResultAction, CardResultFilter,
    CardResultSource, CardTypeFilter, CountExpression, EffectSubject, GameCondition,
    ObjectPaymentConstraint, PlayerRecipient, PowerToughnessCharacteristic, RelativePlayerSet,
    SpellEffectKind, StaticAbilityDef, TargetController, TargetFilter, TargetKind,
    TypeLineAddition,
};
use tricerules_cards::{AbilityPresentation, CardRegistry, CounterKind, Keyword};

const COHORT: [&str; 2] = ["galvanizing_sawship", "wedgelight_rammer"];

fn station_cost() -> AbilityCost {
    AbilityCost::TapPermanents {
        constraint: ObjectPaymentConstraint::ExactCount(1),
        filter: TargetFilter {
            kind: TargetKind::Creature,
            controller: TargetController::You,
            ..TargetFilter::default()
        },
        exclude_source: true,
    }
}

fn station_effect() -> SpellEffectKind {
    SpellEffectKind::PutCounters {
        counter: CounterKind::Charge,
        count: Amount::Count(CountExpression::CardResultCharacteristicSum {
            filter: CardResultFilter {
                source: CardResultSource::Payment,
                action: CardResultAction::Tap,
                players: RelativePlayerSet::Controller,
                card_type: Some(CardTypeFilter::Creature),
            },
            characteristic: PowerToughnessCharacteristic::Power,
        }),
        subject: EffectSubject::Source,
    }
}

#[test]
fn issue_310_registry_contains_exactly_the_reviewed_station_cohort() {
    let registry = CardRegistry::global();
    let matching = registry
        .definitions()
        .filter(|definition| {
            definition.faces_iter().any(|face| {
                face.types == ["Artifact", "Spacecraft"]
                    && face.activated_abilities.first().is_some_and(|ability| {
                        ability.source_zone == AbilitySourceZone::Battlefield
                            && ability.timing == ActivationTiming::SorcerySpeed
                            && ability.costs == [station_cost()]
                            && ability.effect == [station_effect()]
                    })
                    && face.static_abilities.iter().any(|ability| {
                        matches!(
                            &ability.definition,
                            StaticAbilityDef::ConditionalSelfModifier {
                                condition: GameCondition::SourceCounterCount {
                                    counter: CounterKind::Charge,
                                    min: Some(3 | 9),
                                    max: None,
                                },
                                add_types: TypeLineAddition { card_types, .. },
                                keywords,
                                ..
                            } if card_types == &[tricerules_cards::primitives::PermanentTypeFilter::Creature]
                                && (keywords == &[Keyword::Flying, Keyword::Haste]
                                    || keywords == &[Keyword::Flying, Keyword::FirstStrike])
                        )
                    })
            })
        })
        .map(|definition| definition.id.as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        matching,
        COHORT.into_iter().collect::<BTreeSet<_>>(),
        "only the two reviewed Station-3/9 keyword cards may enter this recipe"
    );
}

#[test]
fn issue_310_cards_preserve_exact_faces_abilities_and_presentation_fingerprints() {
    let registry = CardRegistry::global();
    for (id, name, mana, power, toughness, threshold, keywords, fingerprint) in [
        (
            "galvanizing_sawship",
            "Galvanizing Sawship",
            "{5}{R}",
            6,
            5,
            3,
            vec![Keyword::Flying, Keyword::Haste],
            "84998c4d5ac3ee66bedc0eb9bd2f6859a728914ffd8d19dbb42347a323ecf70f",
        ),
        (
            "wedgelight_rammer",
            "Wedgelight Rammer",
            "{3}{W}",
            3,
            4,
            9,
            vec![Keyword::Flying, Keyword::FirstStrike],
            "4a07b7e27e96e8ede50eabaea18000556d4dd756680df813ac7ed84abfad4847",
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

        let station = &face.activated_abilities[0];
        assert_eq!(station.ability_id.as_str(), "activated_01");
        assert_eq!(
            station.presentation,
            AbilityPresentation::OracleLines(vec![if id == "galvanizing_sawship" { 1 } else { 2 }])
        );
        assert_eq!(station.source_zone, AbilitySourceZone::Battlefield);
        assert_eq!(station.timing, ActivationTiming::SorcerySpeed);
        assert_eq!(station.costs, [station_cost()]);
        assert_eq!(station.effect, [station_effect()]);

        let static_ability = &face.static_abilities[0];
        assert_eq!(static_ability.ability_id.as_str(), "static_01");
        assert_eq!(
            static_ability.presentation,
            AbilityPresentation::OracleLines(vec![if id == "galvanizing_sawship" { 2 } else { 3 }])
        );
        assert_eq!(
            static_ability.definition,
            StaticAbilityDef::ConditionalSelfModifier {
                condition: GameCondition::SourceCounterCount {
                    counter: CounterKind::Charge,
                    min: Some(threshold),
                    max: None,
                },
                set_types: None,
                add_types: TypeLineAddition {
                    card_types: vec![tricerules_cards::primitives::PermanentTypeFilter::Creature],
                    creature_types: Vec::new(),
                },
                base_power: Some(power.into()),
                base_toughness: Some(toughness.into()),
                delta_power: 0,
                delta_toughness: 0,
                keywords,
                activated_abilities: Vec::new(),
                triggered_abilities: Vec::new(),
                can_attack_as_though_without_defender: false,
            }
        );

        if id == "galvanizing_sawship" {
            assert!(face.triggered_abilities.is_empty());
        } else {
            let [trigger] = face.triggered_abilities.as_slice() else {
                panic!("Wedgelight must have one ETB trigger")
            };
            assert_eq!(trigger.ability_id.as_str(), "triggered_01");
            assert_eq!(
                trigger.presentation,
                AbilityPresentation::OracleLines(vec![1])
            );
            assert_eq!(
                trigger.effect,
                [SpellEffectKind::CreateTokens {
                    token: "robot_c_2_2".into(),
                    count: Amount::Fixed(1),
                    who: PlayerRecipient::Controller,
                    tapped: false,
                    sacrifice_timing: None,
                }]
            );
        }

        let presentation = registry
            .presentation_face(id, face.face_id.as_str())
            .unwrap_or_else(|| panic!("missing {id} presentation metadata"));
        assert_eq!(presentation.card_name, name);
        assert_eq!(presentation.face_name, name);
        assert_eq!(presentation.oracle_text_sha256, fingerprint);
    }
}
