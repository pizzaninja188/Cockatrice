//! Registry and presentation conformance for issue #313.
//!
//! The two Spacecraft share the reviewed Station assembly, but their entry
//! abilities are intentionally different: Extinguisher's targeted destroy is
//! the only target-bearing effect (so an illegal target fizzles the whole
//! ability), while Fell's mandatory graveyard choice is published only after
//! its resolution-time mill.

use std::collections::BTreeSet;

use tricerules_cards::primitives::{
    AbilityCost, AbilitySourceZone, ActivationTiming, Amount, CardTypeFilter, CountExpression,
    EffectSubject, GameCondition, GraveyardDestination, ObjectPaymentConstraint, PlayerRecipient,
    PowerToughnessCharacteristic, RelativePlayerSet, SpellEffectKind, StaticAbilityDef,
    TargetController, TargetFilter, TargetKind, TriggerCondition, TypeLineAddition, ZoneCardFilter,
};
use tricerules_cards::{AbilityPresentation, CardRegistry, CounterKind, Keyword};

const COHORT: [&str; 2] = ["extinguisher_battleship", "fell_gravship"];

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
            filter: tricerules_cards::primitives::CardResultFilter {
                source: tricerules_cards::primitives::CardResultSource::Payment,
                action: tricerules_cards::primitives::CardResultAction::Tap,
                players: RelativePlayerSet::Controller,
                card_type: Some(CardTypeFilter::Creature),
            },
            characteristic: PowerToughnessCharacteristic::Power,
        }),
        subject: EffectSubject::Source,
    }
}

fn noncreature_target() -> TargetFilter {
    TargetFilter {
        kind: TargetKind::AnyPermanent,
        excluded_permanent_types: vec![tricerules_cards::primitives::PermanentTypeFilter::Creature],
        ..TargetFilter::default()
    }
}

fn fell_filter() -> ZoneCardFilter {
    ZoneCardFilter {
        any_of: Some(vec![
            ZoneCardFilter {
                card_type: Some(CardTypeFilter::Creature),
                ..ZoneCardFilter::default()
            },
            ZoneCardFilter {
                required_subtypes: vec!["Spacecraft".into()],
                ..ZoneCardFilter::default()
            },
        ]),
        ..ZoneCardFilter::default()
    }
}

#[test]
fn issue_313_registry_contains_exactly_the_reviewed_station_cohort() {
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
                                    min: Some(5 | 8),
                                    max: None,
                                },
                                add_types: TypeLineAddition { card_types, .. },
                                keywords,
                                ..
                            } if card_types == &[tricerules_cards::primitives::PermanentTypeFilter::Creature]
                                && (keywords == &[Keyword::Flying, Keyword::Trample]
                                    || keywords == &[Keyword::Flying, Keyword::Lifelink])
                        )
                    })
            })
        })
        .map(|definition| definition.id.as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        matching,
        COHORT.into_iter().collect::<BTreeSet<_>>(),
        "only the two reviewed Station-5/8 keyword cards may enter this recipe"
    );
}

#[test]
fn issue_313_cards_preserve_exact_faces_abilities_and_presentation_fingerprints() {
    let registry = CardRegistry::global();
    for (id, name, mana, power, toughness, threshold, keywords, fingerprint) in [
        (
            "extinguisher_battleship",
            "Extinguisher Battleship",
            "{8}",
            10,
            10,
            5,
            vec![Keyword::Flying, Keyword::Trample],
            "01cafbc8db9a4187655f115911ab9366845fbb0fd7edbe4f12457cb1b9877574",
        ),
        (
            "fell_gravship",
            "Fell Gravship",
            "{2}{B}",
            3,
            2,
            8,
            vec![Keyword::Flying, Keyword::Lifelink],
            "817ca241a62f83109078d8b686da7e5056c9136afc2bf440d2825e6188959536",
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
            AbilityPresentation::OracleLines(vec![2])
        );
        assert_eq!(station.source_zone, AbilitySourceZone::Battlefield);
        assert_eq!(station.timing, ActivationTiming::SorcerySpeed);
        assert_eq!(station.costs, [station_cost()]);
        assert_eq!(station.effect, [station_effect()]);

        let static_ability = &face.static_abilities[0];
        assert_eq!(static_ability.ability_id.as_str(), "static_01");
        assert_eq!(
            static_ability.presentation,
            AbilityPresentation::OracleLines(vec![3])
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

        let [trigger] = face.triggered_abilities.as_slice() else {
            panic!("{name} must have one ETB trigger")
        };
        assert_eq!(trigger.ability_id.as_str(), "triggered_01");
        assert_eq!(
            trigger.presentation,
            AbilityPresentation::OracleLines(vec![1])
        );
        assert_eq!(trigger.trigger, TriggerCondition::WhenSelfEntersBattlefield);
        match id {
            "extinguisher_battleship" => {
                assert_eq!(trigger.effect.len(), 2);
                assert_eq!(
                    trigger.effect[0],
                    SpellEffectKind::Destroy {
                        subject: EffectSubject::Chosen(Box::new(noncreature_target()))
                    }
                );
                assert_eq!(
                    trigger.effect[1],
                    SpellEffectKind::DamageAll {
                        amount: Amount::Fixed(4),
                        players: RelativePlayerSet::All,
                        kind: TargetFilter::default_creature(),
                    }
                );
                let targeting = trigger.targeting.as_ref().expect("targeting");
                assert_eq!(targeting.groups.len(), 1);
                assert_eq!((targeting.groups[0].min, targeting.groups[0].max), (1, 1));
                assert_eq!(targeting.groups[0].effect_indices, [0]);
                assert!(!trigger.may);
            }
            "fell_gravship" => {
                assert!(trigger.targeting.is_none());
                assert_eq!(
                    trigger.effect,
                    [
                        SpellEffectKind::Mill {
                            count: Amount::Fixed(3),
                            who: PlayerRecipient::Controller,
                        },
                        SpellEffectKind::ChooseGraveyardCard {
                            filter: fell_filter(),
                            destination: GraveyardDestination::Hand,
                            optional: false,
                            from_result: None,
                        },
                    ]
                );
            }
            _ => unreachable!(),
        }

        let presentation = registry
            .presentation_face(id, face.face_id.as_str())
            .unwrap_or_else(|| panic!("missing {id} presentation metadata"));
        assert_eq!(presentation.card_name, name);
        assert_eq!(presentation.face_name, name);
        assert_eq!(presentation.oracle_text_sha256, fingerprint);
    }

    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, fingerprint) in [
        (
            "extinguisher_battleship",
            "01cafbc8db9a4187655f115911ab9366845fbb0fd7edbe4f12457cb1b9877574",
        ),
        (
            "fell_gravship",
            "817ca241a62f83109078d8b686da7e5056c9136afc2bf440d2825e6188959536",
        ),
    ] {
        assert!(
            fingerprints.lines().any(|line| {
                let fields = line.split('\t').collect::<Vec<_>>();
                fields.len() == 5 && fields[0] == id && fields[4] == fingerprint
            }),
            "missing exact presentation fingerprint for {id}"
        );
    }
}
