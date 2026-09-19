//! Registry identities for the issue #423 Station threshold batch.
//!
//! Oracle text verified against the pinned Scryfall `oracle_cards` bulk
//! (`27bf3214-1271-490b-bdfe-c0be6c23d02e`, compressed SHA-256
//! `9bb0c7853625c76597fdee2f97d54d376feca71aa16645b7e38b214c74ef807a`) and the
//! full local corpus. CR 702.184 defines Station as a sorcery-speed activated
//! ability that puts charge counters equal to the tapped creature's power on the
//! source; the threshold striations are continuously reevaluated static
//! abilities (CR 611.3/613), with CR 721.2a base P/T for the animation.
//!
//! The excluded cohort identities (Adagia, Entropic Battlecruiser, Infinite
//! Guideline Station, Susurian Dirgecraft, The Seriema) intentionally have no
//! registry entry here; their unsupported clauses stay fail-closed.

use tricerules_cards::primitives::{
    AbilityCost, AbilitySourceZone, ActivationTiming, Amount, CardResultAction, CardResultFilter,
    CardResultSource, CardSearchZone, CardTypeFilter, CountExpression, CreatureScopeController,
    CreatureScopeFilter, EffectSubject, GameCondition, ObjectPaymentConstraint, PlayerRecipient,
    PowerToughnessCharacteristic, RelativePlayerSet, ResolvingEffectDuration,
    ResolvingPermanentModifier, SearchDestination, SearchZoneSelection, SpellEffectKind,
    StaticAbilityDef, TargetController, TargetFilter, TargetKind, TriggerCondition,
    TypeLineAddition, ZoneCardFilter,
};
use tricerules_cards::{
    AbilityPresentation, CardFace, CardRegistry, CounterKind, Keyword, ManaCost,
};

struct SpacecraftFixture {
    id: &'static str,
    name: &'static str,
    mana: &'static str,
    power: u32,
    toughness: u32,
    threshold: u32,
    station_line: u16,
    keywords: &'static [Keyword],
}

const SPACECRAFT: [SpacecraftFixture; 7] = [
    SpacecraftFixture {
        id: "atmospheric_greenhouse",
        name: "Atmospheric Greenhouse",
        mana: "{4}{G}",
        power: 5,
        toughness: 4,
        threshold: 8,
        station_line: 2,
        keywords: &[Keyword::Flying, Keyword::Trample],
    },
    SpacecraftFixture {
        id: "dawnsire,_sunstar_dreadnought",
        name: "Dawnsire, Sunstar Dreadnought",
        mana: "{5}",
        power: 20,
        toughness: 20,
        threshold: 20,
        station_line: 1,
        keywords: &[Keyword::Flying],
    },
    SpacecraftFixture {
        id: "larval_scoutlander",
        name: "Larval Scoutlander",
        mana: "{2}{G}",
        power: 3,
        toughness: 3,
        threshold: 7,
        station_line: 2,
        keywords: &[Keyword::Flying],
    },
    SpacecraftFixture {
        id: "lumen-class_frigate",
        name: "Lumen-Class Frigate",
        mana: "{1}{W}",
        power: 3,
        toughness: 5,
        threshold: 12,
        station_line: 1,
        keywords: &[Keyword::Flying, Keyword::Lifelink],
    },
    SpacecraftFixture {
        id: "sledge-class_seedship",
        name: "Sledge-Class Seedship",
        mana: "{2}{G}",
        power: 4,
        toughness: 5,
        threshold: 7,
        station_line: 1,
        keywords: &[Keyword::Flying],
    },
    SpacecraftFixture {
        id: "specimen_freighter",
        name: "Specimen Freighter",
        mana: "{5}{U}",
        power: 4,
        toughness: 7,
        threshold: 9,
        station_line: 2,
        keywords: &[Keyword::Flying],
    },
    SpacecraftFixture {
        id: "synthesizer_labship",
        name: "Synthesizer Labship",
        mana: "{U}",
        power: 4,
        toughness: 4,
        threshold: 9,
        station_line: 1,
        keywords: &[Keyword::Flying, Keyword::Vigilance],
    },
];

fn registered_face(card_id: &str) -> CardFace {
    CardRegistry::global()
        .get(card_id)
        .unwrap_or_else(|| panic!("{card_id} must be registered"))
        .primary_face()
        .clone()
}

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

fn threshold_static(face: &CardFace, threshold: u32) -> StaticAbilityDef {
    face.static_abilities
        .iter()
        .find(|ability| {
            matches!(
                ability.definition,
                StaticAbilityDef::ConditionalSelfModifier {
                    condition: GameCondition::SourceCounterCount {
                        counter: CounterKind::Charge,
                        min: Some(min),
                        max: None,
                    },
                    ..
                } if min == threshold
            )
        })
        .unwrap_or_else(|| panic!("{} must have a {threshold}+ charge threshold", face.name))
        .definition
        .clone()
}

#[test]
fn issue_423_spacecraft_faces_have_exact_station_activation_and_thresholds() {
    for SpacecraftFixture {
        id,
        name,
        mana,
        power,
        toughness,
        threshold,
        station_line,
        keywords,
    } in SPACECRAFT
    {
        let face = registered_face(id);
        assert_eq!(face.name, name, "{id}");
        assert_eq!(
            face.mana_cost,
            ManaCost::parse(mana).expect("printed cost"),
            "{id}"
        );
        assert_eq!(face.types, ["Artifact", "Spacecraft"], "{id}");
        assert_eq!(
            (face.power, face.toughness),
            (Some(power), Some(toughness)),
            "{id}"
        );
        assert_eq!(
            face.keywords,
            Vec::<Keyword>::new(),
            "{id} prints no standalone keyword"
        );

        let [station] = face.activated_abilities.as_slice() else {
            panic!("{id} must have exactly one printed activated ability");
        };
        assert_eq!(station.ability_id.as_str(), "activated_01", "{id}");
        assert_eq!(station.source_zone, AbilitySourceZone::Battlefield, "{id}");
        assert_eq!(station.timing, ActivationTiming::SorcerySpeed, "{id}");
        assert_eq!(station.costs, [station_cost()], "{id}");
        assert_eq!(station.effect, [station_effect()], "{id}");
        assert_eq!(
            station.presentation,
            AbilityPresentation::OracleLines(vec![station_line]),
            "{id} maps the activation to its printed Station line"
        );

        let StaticAbilityDef::ConditionalSelfModifier {
            condition,
            add_types,
            base_power,
            base_toughness,
            keywords: static_keywords,
            ..
        } = threshold_static(&face, threshold)
        else {
            panic!("{id} threshold must be a conditional self modifier");
        };
        assert_eq!(
            condition,
            GameCondition::SourceCounterCount {
                counter: CounterKind::Charge,
                min: Some(threshold),
                max: None,
            },
            "{id}"
        );
        assert_eq!(
            add_types,
            TypeLineAddition {
                card_types: vec![tricerules_cards::primitives::PermanentTypeFilter::Creature],
                creature_types: Vec::new(),
            },
            "{id} animates into an artifact creature"
        );
        assert_eq!(
            (base_power, base_toughness),
            (Some(power as i64), Some(toughness as i64)),
            "{id}"
        );
        assert_eq!(static_keywords, keywords, "{id}");
    }
}

#[test]
fn issue_423_dawnsire_striation_grants_the_threshold_attack_trigger() {
    let face = registered_face("dawnsire,_sunstar_dreadnought");
    assert_eq!(face.supertypes, ["Legendary"]);
    let StaticAbilityDef::ConditionalSelfModifier {
        condition,
        triggered_abilities,
        ..
    } = threshold_static(&face, 10)
    else {
        panic!("Dawnsire's 10+ striation must be a conditional self modifier");
    };
    assert_eq!(
        condition,
        GameCondition::SourceCounterCount {
            counter: CounterKind::Charge,
            min: Some(10),
            max: None,
        }
    );
    let [attack] = triggered_abilities.as_slice() else {
        panic!("Dawnsire's 10+ striation grants exactly one triggered ability");
    };
    assert_eq!(
        attack.trigger,
        TriggerCondition::WheneverControllerAttacks {
            min_attackers: None,
            max_attackers: None,
        }
    );
    assert_eq!(
        attack.effect,
        [SpellEffectKind::DamageTarget {
            amount: Amount::Fixed(100),
            target: TargetFilter {
                kind: TargetKind::AnyPermanent,
                permanent_types: vec![
                    tricerules_cards::primitives::PermanentTypeFilter::Creature,
                    tricerules_cards::primitives::PermanentTypeFilter::Planeswalker,
                ],
                ..TargetFilter::default()
            },
        }]
    );
    let targeting = attack
        .targeting
        .as_ref()
        .expect("Dawnsire's trigger targets");
    assert_eq!((targeting.groups[0].min, targeting.groups[0].max), (0, 1));
}

#[test]
fn issue_423_lumen_and_synthesizer_striations_are_threshold_gated() {
    let lumen = registered_face("lumen-class_frigate");
    let anthem = lumen
        .static_abilities
        .iter()
        .find_map(|ability| match &ability.definition {
            StaticAbilityDef::AnthemPt {
                condition,
                filter,
                delta_power,
                delta_toughness,
            } => Some((
                condition.clone(),
                filter.clone(),
                *delta_power,
                *delta_toughness,
            )),
            _ => None,
        })
        .expect("Lumen-Class Frigate prints the 2+ anthem");
    assert_eq!(
        anthem.0,
        Some(GameCondition::SourceCounterCount {
            counter: CounterKind::Charge,
            min: Some(2),
            max: None,
        })
    );
    assert_eq!(
        anthem.1.controller,
        Some(CreatureScopeController::YouControl)
    );
    assert!(anthem.1.exclude_self, "the anthem excludes its own source");
    assert_eq!((anthem.2, anthem.3), (1, 1));

    let synthesizer = registered_face("synthesizer_labship");
    let StaticAbilityDef::ConditionalSelfModifier {
        condition,
        triggered_abilities,
        ..
    } = threshold_static(&synthesizer, 2)
    else {
        panic!("Synthesizer's 2+ striation must be a conditional self modifier");
    };
    assert_eq!(
        condition,
        GameCondition::SourceCounterCount {
            counter: CounterKind::Charge,
            min: Some(2),
            max: None,
        }
    );
    let [combat] = triggered_abilities.as_slice() else {
        panic!("Synthesizer's 2+ striation grants exactly one triggered ability");
    };
    assert_eq!(
        combat.trigger,
        TriggerCondition::AtBeginningOfCombat {
            player: tricerules_cards::CastTriggerPlayer::Controller,
        }
    );
    assert_eq!(combat.effect.len(), 3);
    let [SpellEffectKind::ApplyPermanentModifier {
        modifier: ResolvingPermanentModifier::AddTypes(addition),
        duration: ResolvingEffectDuration::UntilEndOfTurn,
        ..
    }, SpellEffectKind::ApplyPermanentModifier {
        modifier: ResolvingPermanentModifier::SetBasePowerToughness { power, toughness },
        duration: ResolvingEffectDuration::UntilEndOfTurn,
        ..
    }, SpellEffectKind::ApplyPermanentModifier {
        modifier: ResolvingPermanentModifier::GrantKeywords(keywords),
        duration: ResolvingEffectDuration::UntilEndOfTurn,
        ..
    }] = combat.effect.as_slice()
    else {
        panic!("Synthesizer composes type, base P/T, and keyword modifiers");
    };
    assert_eq!(
        addition.card_types,
        [tricerules_cards::primitives::PermanentTypeFilter::Creature]
    );
    assert_eq!((*power, *toughness), (2, 2));
    assert_eq!(keywords, &[Keyword::Flying]);
    let targeting = combat.targeting.as_ref().expect("the animation targets");
    assert_eq!((targeting.groups[0].min, targeting.groups[0].max), (0, 1));
}

#[test]
fn issue_423_companion_abilities_are_registered_exactly() {
    let greenhouse = registered_face("atmospheric_greenhouse");
    let [entry] = greenhouse.triggered_abilities.as_slice() else {
        panic!("Atmospheric Greenhouse has one entry trigger");
    };
    assert_eq!(entry.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert_eq!(
        entry.effect,
        [SpellEffectKind::PutCountersAll {
            counter: CounterKind::PlusOnePlusOne,
            count: Amount::Fixed(1),
            filter: CreatureScopeFilter {
                controller: Some(CreatureScopeController::YouControl),
                ..CreatureScopeFilter::default()
            },
        }]
    );

    let larval = registered_face("larval_scoutlander");
    let [entry] = larval.triggered_abilities.as_slice() else {
        panic!("Larval Scoutlander has one entry trigger");
    };
    let [SpellEffectKind::ChooseResolutionBranch {
        optional, branches, ..
    }] = entry.effect.as_slice()
    else {
        panic!("Larval's entry offers its printed optional sacrifice branch");
    };
    assert!(optional);
    assert_eq!(branches.len(), 1);
    assert_eq!(branches[0].branch_id.as_str(), "sacrifice_a_land_or_lander");
    let [SpellEffectKind::SearchLibrary {
        count,
        zones,
        destination,
        shuffle,
        ..
    }] = branches[0].effects.as_slice()
    else {
        panic!("Larval's branch searches for basic lands");
    };
    assert_eq!((*count, *shuffle), (2, true));
    assert_eq!(zones, &SearchZoneSelection::default());
    assert_eq!(
        destination,
        &SearchDestination::Battlefield { tapped: true }
    );

    let sledge = registered_face("sledge-class_seedship");
    let [attack] = sledge.triggered_abilities.as_slice() else {
        panic!("Sledge-Class Seedship has one attack trigger");
    };
    assert_eq!(
        attack.trigger,
        TriggerCondition::WheneverSelfAttacks {
            minimum_other_attackers: 0,
        }
    );
    let [SpellEffectKind::SearchLibrary {
        optional,
        zones,
        destination,
        shuffle,
        filter,
        ..
    }] = attack.effect.as_slice()
    else {
        panic!("Sledge-Class Seedship puts a hand card onto the battlefield");
    };
    assert!(*optional);
    assert_eq!(
        zones,
        &SearchZoneSelection::Fixed(vec![CardSearchZone::Hand])
    );
    assert_eq!(
        destination,
        &SearchDestination::Battlefield { tapped: false }
    );
    assert!(!*shuffle);
    assert_eq!(
        filter,
        &Some(ZoneCardFilter {
            card_type: Some(CardTypeFilter::Creature),
            ..ZoneCardFilter::default()
        })
    );

    let specimen = registered_face("specimen_freighter");
    assert_eq!(specimen.triggered_abilities.len(), 2);
    let bounce = specimen
        .triggered_abilities
        .iter()
        .find(|ability| {
            ability
                .effect
                .iter()
                .any(|effect| matches!(effect, SpellEffectKind::ReturnToOwnersHand { .. }))
        })
        .expect("Specimen Freighter's entry bounces");
    assert_eq!(bounce.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    let targeting = bounce.targeting.as_ref().expect("the bounce targets");
    assert_eq!((targeting.groups[0].min, targeting.groups[0].max), (0, 2));
    let mill = specimen
        .triggered_abilities
        .iter()
        .find(|ability| {
            ability
                .effect
                .iter()
                .any(|effect| matches!(effect, SpellEffectKind::Mill { .. }))
        })
        .expect("Specimen Freighter's attack mills");
    assert_eq!(
        mill.trigger,
        TriggerCondition::WheneverSelfAttacks {
            minimum_other_attackers: 0,
        }
    );
    assert_eq!(
        mill.effect,
        [SpellEffectKind::Mill {
            count: Amount::Fixed(4),
            who: PlayerRecipient::DefendingPlayer,
        }]
    );
}

#[test]
fn issue_423_planets_grant_their_twelve_plus_activated_abilities() {
    for (id, name, basic) in [
        ("kavaron,_memorial_world", "Kavaron, Memorial World", "r"),
        (
            "susur_secundi,_void_altar",
            "Susur Secundi, Void Altar",
            "b",
        ),
    ] {
        let face = registered_face(id);
        assert_eq!(face.name, name);
        assert_eq!(
            face.mana_cost,
            ManaCost::parse("").expect("land cost"),
            "{id}"
        );
        assert_eq!(face.types, ["Land", "Planet"], "{id}");
        assert_eq!((face.power, face.toughness), (None, None), "{id}");
        assert_eq!(
            face.activated_abilities.len(),
            2,
            "{id} prints Station plus a mana ability"
        );
        assert_eq!(
            face.activated_abilities[1].ability_id.as_str(),
            "activated_02",
            "{id}"
        );
        assert_eq!(
            face.activated_abilities[1].costs,
            [AbilityCost::Tap],
            "{id}"
        );
        assert!(
            matches!(
                &face.activated_abilities[1].effect[0],
                SpellEffectKind::ProduceMana { .. }
            ),
            "{id} produces {basic} mana"
        );

        let StaticAbilityDef::ConditionalSelfModifier {
            condition,
            activated_abilities,
            ..
        } = threshold_static(&face, 12)
        else {
            panic!("{id} must gate its activation at 12+");
        };
        assert_eq!(
            condition,
            GameCondition::SourceCounterCount {
                counter: CounterKind::Charge,
                min: Some(12),
                max: None,
            }
        );
        let [inner] = activated_abilities.as_slice() else {
            panic!("{id} grants exactly one threshold activation");
        };
        assert_eq!(inner.timing, ActivationTiming::SorcerySpeed, "{id}");
    }

    let kavaron = registered_face("kavaron,_memorial_world");
    let StaticAbilityDef::ConditionalSelfModifier {
        activated_abilities,
        ..
    } = threshold_static(&kavaron, 12)
    else {
        unreachable!()
    };
    assert_eq!(
        activated_abilities[0].costs,
        [
            AbilityCost::Mana(ManaCost::parse("{1}{R}").expect("printed cost")),
            AbilityCost::Tap,
            AbilityCost::SacrificePermanent {
                filter: TargetFilter {
                    kind: TargetKind::AnyPermanent,
                    controller: TargetController::You,
                    permanent_types: vec![tricerules_cards::primitives::PermanentTypeFilter::Land],
                    ..TargetFilter::default()
                },
            },
        ]
    );
    assert_eq!(activated_abilities[0].effect.len(), 3);

    let susur = registered_face("susur_secundi,_void_altar");
    let StaticAbilityDef::ConditionalSelfModifier {
        activated_abilities,
        ..
    } = threshold_static(&susur, 12)
    else {
        unreachable!()
    };
    assert_eq!(
        activated_abilities[0].costs,
        [
            AbilityCost::Mana(ManaCost::parse("{1}{B}").expect("printed cost")),
            AbilityCost::Tap,
            AbilityCost::PayLife { amount: 2 },
            AbilityCost::SacrificePermanent {
                filter: TargetFilter {
                    kind: TargetKind::Creature,
                    controller: TargetController::You,
                    ..TargetFilter::default()
                },
            },
        ]
    );
    assert_eq!(
        activated_abilities[0].effect,
        [SpellEffectKind::Draw {
            who: PlayerRecipient::Controller,
            count: Amount::Count(CountExpression::CardResultCharacteristicSum {
                filter: CardResultFilter {
                    source: CardResultSource::Payment,
                    action: CardResultAction::Sacrifice,
                    players: RelativePlayerSet::Controller,
                    card_type: Some(CardTypeFilter::Creature),
                },
                characteristic: PowerToughnessCharacteristic::Power,
            }),
        }]
    );
}

#[test]
fn issue_423_excluded_cohort_identities_have_no_registry_entry() {
    for id in [
        "adagia,_windswept_bastion",
        "entropic_battlecruiser",
        "infinite_guideline_station",
        "susurian_dirgecraft",
        "the_seriema",
    ] {
        assert!(
            CardRegistry::global().get(id).is_none(),
            "{id} must stay unsupported until its missing capability ships"
        );
    }
}
