//! Issue #373 registry and presentation conformance for the retained graveyard-count batch.
//!
//! Thirteen Standard identities generate from the exact typed recipes; the rest of the 32-card
//! cohort stays excluded behind open blockers (see the issue report). These assertions pin the
//! complete typed payloads consumers rely on: CR 404.2 public graveyard card data, CR 608.2h
//! resolution-time quantity evaluation (including Join the Dead's exhaustive "instead" branch),
//! CR 603.4 intervening-if re-checks, CR 602.5b activation conditions, and CR 701.17/701.25 mill
//! and surveil ordering.

use tricerules_cards::primitives::{
    ActivationLimit, BattlefieldPermanentFilter, CardTypeFilter, CountExpression, EffectSubject,
    GameCondition, GraveyardAggregate, LibraryPartitionKind, PlayerRecipient, PtScale,
    PtScaleBasis, RelativePlayerSet, TargetController, TargetFilter, TargetKind, ZoneCardFilter,
};
use tricerules_cards::{
    AbilityCost, AbilitySourceZone, ActivationTiming, Amount, CardRegistry, CounterKind, Keyword,
    ManaCost, SpellEffectKind, TriggerCondition,
};

fn permanent_card_filter() -> ZoneCardFilter {
    ZoneCardFilter {
        excluded_card_types: vec![CardTypeFilter::Instant, CardTypeFilter::Sorcery],
        ..ZoneCardFilter::default()
    }
}

#[test]
fn issue_373_registers_the_thirteen_retained_identities() {
    let registry = CardRegistry::global();
    for (id, name, face_id) in [
        ("beastie_beatdown", "Beastie Beatdown", "beastie_beatdown"),
        (
            "cloud_of_darkness",
            "Cloud of Darkness",
            "cloud_of_darkness",
        ),
        (
            "combustion_technique",
            "Combustion Technique",
            "combustion_technique",
        ),
        ("frantic_firebolt", "Frantic Firebolt", "frantic_firebolt"),
        ("gloom_ripper", "Gloom Ripper", "gloom_ripper"),
        ("gran_pulse_ochu", "Gran Pulse Ochu", "gran_pulse_ochu"),
        ("join_the_dead", "Join the Dead", "join_the_dead"),
        ("lasyd_prowler", "Lasyd Prowler", "lasyd_prowler"),
        ("malamet_veteran", "Malamet Veteran", "malamet_veteran"),
        ("ooze_patrol", "Ooze Patrol", "ooze_patrol"),
        (
            "swallowed_by_leviathan",
            "Swallowed by Leviathan",
            "swallowed_by_leviathan",
        ),
        ("thought_shucker", "Thought Shucker", "thought_shucker"),
        ("violent_urge", "Violent Urge", "violent_urge"),
    ] {
        let definition = registry
            .get(id)
            .unwrap_or_else(|| panic!("missing retained card {id}"));
        assert_eq!(definition.name, name);
        assert_eq!(registry.id_for_name(name), Some(id));
        assert_eq!(definition.layout, tricerules_cards::Layout::Normal);
        assert_eq!(definition.face_count(), 1);
        assert_eq!(definition.primary_face().face_id.as_str(), face_id);
    }

    for blocked in [
        "peer_past_the_veil",
        "wicks_patrol",
        "quag_feast",
        "accumulate_wisdom",
        "terror_tide",
    ] {
        assert!(
            registry.get(blocked).is_none(),
            "{blocked} must stay excluded until its blocker lands"
        );
    }
}

#[test]
fn issue_373_cloud_of_darkness_scales_the_pump_from_the_graveyard() {
    let definition = CardRegistry::global()
        .get("cloud_of_darkness")
        .expect("Cloud of Darkness");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{2}{B}{G}{G}");
    assert_eq!(face.types, ["Creature", "Avatar"]);
    assert_eq!((face.power, face.toughness), (Some(3), Some(3)));
    assert_eq!(face.keywords, [Keyword::Flying]);
    let [ability] = face.triggered_abilities.as_slice() else {
        panic!("Cloud of Darkness must have exactly one triggered ability");
    };
    assert_eq!(ability.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert_eq!(
        ability.presentation,
        tricerules_cards::AbilityPresentation::OracleLines(vec![2])
    );
    let [SpellEffectKind::PumpTarget {
        power,
        toughness,
        scale: Some(scale),
        subject,
    }] = ability.effect.as_slice()
    else {
        panic!("Particle Beam must emit one scaled pump");
    };
    assert_eq!((*power, *toughness), (0, 0));
    assert_eq!((scale.power_per_unit, scale.toughness_per_unit), (-1, -1));
    assert_eq!(
        scale.basis,
        PtScaleBasis::Amount(Amount::Count(CountExpression::GraveyardCards {
            owners: RelativePlayerSet::Controller,
            filter: Some(permanent_card_filter()),
        }))
    );
    let EffectSubject::Chosen(target) = subject else {
        panic!("Particle Beam targets a creature");
    };
    assert_eq!(target.controller, TargetController::Opponent);
    let targeting = ability.targeting.as_ref().expect("targeting");
    assert_eq!(targeting.groups[0].effect_indices, [0]);
    assert_eq!(
        targeting.groups[0].prompt,
        "Choose target creature an opponent controls"
    );
}

#[test]
fn issue_373_combustion_technique_affine_damage_and_exile_rider() {
    let definition = CardRegistry::global()
        .get("combustion_technique")
        .expect("Combustion Technique");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{1}{R}");
    assert_eq!(face.types, ["Instant", "Lesson"]);
    assert_eq!(
        face.spell_effect,
        [
            SpellEffectKind::DamageTarget {
                amount: Amount::Count(CountExpression::Affine {
                    constant: 2,
                    terms: vec![tricerules_cards::primitives::QuantityTerm {
                        coefficient: 1,
                        quantity: CountExpression::GraveyardCards {
                            owners: RelativePlayerSet::Controller,
                            filter: Some(ZoneCardFilter {
                                required_subtypes: vec!["Lesson".into()],
                                ..ZoneCardFilter::default()
                            }),
                        },
                    }],
                }),
                target: tricerules_cards::primitives::TargetFilter::default_creature(),
            },
            SpellEffectKind::ExileIfWouldDieThisTurn {
                target: tricerules_cards::primitives::TargetFilter::default_creature(),
            },
        ]
    );
    let targeting = face.targeting.as_ref().expect("targeting");
    assert_eq!(targeting.groups[0].effect_indices, [0, 1]);
}

#[test]
fn issue_373_frantic_firebolt_union_filter_includes_adventure() {
    let definition = CardRegistry::global()
        .get("frantic_firebolt")
        .expect("Frantic Firebolt");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{2}{R}");
    let [SpellEffectKind::DamageTarget { amount, .. }] = face.spell_effect.as_slice() else {
        panic!("Frantic Firebolt must deal damage");
    };
    let Amount::Count(CountExpression::Affine { constant, terms }) = amount else {
        panic!("Frantic Firebolt must use an affine count");
    };
    assert_eq!(*constant, 2);
    let CountExpression::GraveyardCards {
        filter: Some(filter),
        ..
    } = &terms[0].quantity
    else {
        panic!("Frantic Firebolt must count graveyard cards");
    };
    let branches = filter.any_of.as_ref().expect("union filter");
    assert_eq!(branches.len(), 3);
    assert_eq!(branches[0].card_type, Some(CardTypeFilter::Instant));
    assert_eq!(branches[1].card_type, Some(CardTypeFilter::Sorcery));
    assert_eq!(branches[2].has_adventure, Some(true));
}

#[test]
fn issue_373_gloom_ripper_shares_one_affine_count_between_two_pumps() {
    let definition = CardRegistry::global()
        .get("gloom_ripper")
        .expect("Gloom Ripper");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{3}{B}{B}");
    assert_eq!(face.types, ["Creature", "Elf", "Assassin"]);
    assert_eq!((face.power, face.toughness), (Some(4), Some(4)));
    let [ability] = face.triggered_abilities.as_slice() else {
        panic!("Gloom Ripper must have exactly one triggered ability");
    };
    assert_eq!(
        ability.presentation,
        tricerules_cards::AbilityPresentation::OracleLines(vec![1])
    );
    let [SpellEffectKind::PumpTarget {
        scale: Some(first), ..
    }, SpellEffectKind::PumpTarget {
        scale: Some(second),
        ..
    }] = ability.effect.as_slice()
    else {
        panic!("Gloom Ripper must scale two pumps");
    };
    assert_eq!((first.power_per_unit, first.toughness_per_unit), (1, 0));
    assert_eq!((second.power_per_unit, second.toughness_per_unit), (0, -1));
    let PtScaleBasis::Amount(amount) = &first.basis else {
        panic!("the first pump must read the shared count");
    };
    let Amount::Count(CountExpression::Affine { constant, terms }) = amount else {
        panic!("Gloom Ripper must use an affine count");
    };
    assert_eq!(*constant, 0);
    assert_eq!(terms.len(), 2);
    let CountExpression::BattlefieldCreatures { filter } = &terms[0].quantity else {
        panic!("the battlefield term must count creatures");
    };
    assert_eq!(filter.subtype.as_deref(), Some("Elf"));
    assert_eq!(filter.controllers, RelativePlayerSet::Controller);
    assert_eq!(terms[1].quantity, second_elf_graveyard_term());
    let groups = &ability.targeting.as_ref().expect("targeting").groups;
    assert_eq!((groups[0].min, groups[0].max), (1, 1));
    assert_eq!((groups[1].min, groups[1].max), (0, 1));
    assert_eq!(
        groups[1].prompt,
        "Choose up to one target creature an opponent controls"
    );
}

fn second_elf_graveyard_term() -> CountExpression {
    CountExpression::GraveyardCards {
        owners: RelativePlayerSet::Controller,
        filter: Some(ZoneCardFilter {
            required_subtypes: vec!["Elf".into()],
            ..ZoneCardFilter::default()
        }),
    }
}

#[test]
fn issue_373_gran_pulse_ochu_activation_scales_self_pump() {
    let definition = CardRegistry::global()
        .get("gran_pulse_ochu")
        .expect("Gran Pulse Ochu");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{G}");
    assert_eq!(face.keywords, [Keyword::Deathtouch]);
    let [ability] = face.activated_abilities.as_slice() else {
        panic!("Gran Pulse Ochu must have exactly one activated ability");
    };
    assert_eq!(ability.source_zone, AbilitySourceZone::Battlefield);
    assert_eq!(ability.timing, ActivationTiming::Normal);
    assert_eq!(
        ability.costs,
        [AbilityCost::Mana(ManaCost::parse("{8}").expect("cost"))]
    );
    let [SpellEffectKind::PumpTarget {
        scale: Some(scale),
        subject: EffectSubject::Source,
        ..
    }] = ability.effect.as_slice()
    else {
        panic!("Gran Pulse Ochu must pump its own source");
    };
    assert_eq!((scale.power_per_unit, scale.toughness_per_unit), (1, 1));
    assert_eq!(
        scale.basis,
        PtScaleBasis::Amount(Amount::Count(CountExpression::GraveyardCards {
            owners: RelativePlayerSet::Controller,
            filter: Some(permanent_card_filter()),
        }))
    );
    assert_eq!(
        ability.presentation,
        tricerules_cards::AbilityPresentation::OracleLines(vec![2])
    );
}

#[test]
fn issue_373_malamet_veteran_intervening_if_gates_the_counter() {
    let definition = CardRegistry::global()
        .get("malamet_veteran")
        .expect("Malamet Veteran");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{4}{G}");
    assert_eq!(face.keywords, [Keyword::Trample]);
    let [ability] = face.triggered_abilities.as_slice() else {
        panic!("Malamet Veteran must have exactly one triggered ability");
    };
    assert_eq!(
        ability.trigger,
        TriggerCondition::WheneverSelfAttacks {
            minimum_other_attackers: 0
        }
    );
    assert_eq!(
        ability.intervening_if,
        Some(GameCondition::GraveyardAggregate {
            owners: RelativePlayerSet::Controller,
            aggregate: GraveyardAggregate::CardCount,
            filter: Some(permanent_card_filter()),
            min: Some(4),
            max: None,
        })
    );
    assert_eq!(
        ability.effect,
        [SpellEffectKind::PutCounters {
            counter: CounterKind::PlusOnePlusOne,
            count: Amount::Fixed(1),
            subject: EffectSubject::Chosen(Box::new(
                tricerules_cards::primitives::TargetFilter::default_creature()
            )),
        }]
    );
    assert_eq!(
        ability.targeting.as_ref().expect("targeting").groups[0].effect_indices,
        [0]
    );
}

#[test]
fn issue_373_ooze_patrol_mills_before_counting_the_graveyard() {
    let definition = CardRegistry::global()
        .get("ooze_patrol")
        .expect("Ooze Patrol");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{3}{G}");
    let [ability] = face.triggered_abilities.as_slice() else {
        panic!("Ooze Patrol must have exactly one triggered ability");
    };
    assert_eq!(ability.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    let [SpellEffectKind::Mill {
        count: Amount::Fixed(2),
        who: PlayerRecipient::Controller,
    }, SpellEffectKind::PutCounters {
        counter: CounterKind::PlusOnePlusOne,
        count,
        subject: EffectSubject::Source,
    }] = ability.effect.as_slice()
    else {
        panic!("Ooze Patrol must mill two then place counters on itself");
    };
    assert_eq!(
        *count,
        Amount::Count(CountExpression::GraveyardCards {
            owners: RelativePlayerSet::Controller,
            filter: Some(ZoneCardFilter {
                any_of: Some(vec![
                    ZoneCardFilter {
                        card_type: Some(CardTypeFilter::Artifact),
                        ..ZoneCardFilter::default()
                    },
                    ZoneCardFilter {
                        card_type: Some(CardTypeFilter::Creature),
                        ..ZoneCardFilter::default()
                    },
                ]),
                ..ZoneCardFilter::default()
            }),
        })
    );
    assert_eq!(
        ability.presentation,
        tricerules_cards::AbilityPresentation::OracleLines(vec![1])
    );
}

#[test]
fn issue_373_swallowed_by_leviathan_surveils_before_the_soft_counter() {
    let definition = CardRegistry::global()
        .get("swallowed_by_leviathan")
        .expect("Swallowed by Leviathan");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{2}{U}");
    assert_eq!(
        face.spell_effect[0],
        SpellEffectKind::LibraryPartition {
            count: 2,
            top_min: 0,
            top_max: None,
            kind: LibraryPartitionKind::Surveil,
        }
    );
    let SpellEffectKind::CounterTargetSpell {
        spell_filter,
        unless_controller_pays: Some(amount),
        unless_controller_pays_by_cast_cost: None,
    } = &face.spell_effect[1]
    else {
        panic!("Swallowed by Leviathan must soft-counter the chosen spell");
    };
    assert!(spell_filter.is_unrestricted());
    assert_eq!(
        *amount,
        Amount::Count(CountExpression::GraveyardCards {
            owners: RelativePlayerSet::Controller,
            filter: None,
        })
    );
    let targeting = face.targeting.as_ref().expect("targeting");
    assert_eq!(targeting.groups[0].effect_indices, [1]);
}

#[test]
fn issue_373_thought_shucker_condition_and_once_limit_are_typed() {
    let definition = CardRegistry::global()
        .get("thought_shucker")
        .expect("Thought Shucker");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{1}{U}");
    let [ability] = face.activated_abilities.as_slice() else {
        panic!("Thought Shucker must have exactly one activated ability");
    };
    assert_eq!(
        ability.costs,
        [AbilityCost::Mana(ManaCost::parse("{1}{U}").expect("cost"))]
    );
    assert_eq!(
        ability.conditions,
        [GameCondition::GraveyardAggregate {
            owners: RelativePlayerSet::Controller,
            aggregate: GraveyardAggregate::CardCount,
            filter: None,
            min: Some(7),
            max: None,
        }]
    );
    assert_eq!(
        ability.activation_limit,
        Some(ActivationLimit::PerObject { max_activations: 1 })
    );
    assert_eq!(
        ability.effect,
        [
            SpellEffectKind::PutCounters {
                counter: CounterKind::PlusOnePlusOne,
                count: Amount::Fixed(1),
                subject: EffectSubject::Source,
            },
            SpellEffectKind::Draw {
                who: PlayerRecipient::Controller,
                count: Amount::Fixed(1),
            },
        ]
    );
}

#[test]
fn issue_373_join_the_dead_uses_one_exhaustive_conditional_scale() {
    let definition = CardRegistry::global()
        .get("join_the_dead")
        .expect("Join the Dead");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{1}{B}{B}");
    assert_eq!(face.types, ["Instant"]);
    assert_eq!(
        face.spell_effect,
        [SpellEffectKind::PumpTarget {
            power: 0,
            toughness: 0,
            scale: Some(PtScale {
                basis: PtScaleBasis::Amount(Amount::Conditional {
                    condition: GameCondition::GraveyardAggregate {
                        owners: RelativePlayerSet::Controller,
                        aggregate: GraveyardAggregate::CardCount,
                        filter: Some(permanent_card_filter()),
                        min: Some(4),
                        max: None,
                    },
                    when_true: 10,
                    otherwise: 5,
                }),
                power_per_unit: -1,
                toughness_per_unit: -1,
            }),
            subject: EffectSubject::Chosen(Box::new(TargetFilter::default_creature())),
        }]
    );
    assert!(
        face.targeting.is_none(),
        "the single replacement pump uses the implicit fallback target group"
    );
}

#[test]
fn issue_373_lasyd_prowler_mills_per_land_and_renews_from_the_graveyard() {
    let definition = CardRegistry::global()
        .get("lasyd_prowler")
        .expect("Lasyd Prowler");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{2}{G}{G}");
    assert_eq!(face.types, ["Creature", "Snake", "Ranger"]);
    let [trigger] = face.triggered_abilities.as_slice() else {
        panic!("Lasyd Prowler must have exactly one triggered ability");
    };
    assert_eq!(trigger.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert!(trigger.may, "the printed mill is optional");
    assert_eq!(
        trigger.effect,
        [SpellEffectKind::Mill {
            count: Amount::Count(CountExpression::BattlefieldPermanents {
                filter: BattlefieldPermanentFilter {
                    token: None,
                    any_of: None,
                    controllers: RelativePlayerSet::Controller,
                    card_type: Some(CardTypeFilter::Land),
                    color: None,
                    name: None,
                    required_subtypes: Vec::new(),
                    exclude_source: false,
                },
            }),
            who: PlayerRecipient::Controller,
        }]
    );
    let [renew] = face.activated_abilities.as_slice() else {
        panic!("Lasyd Prowler must keep exactly one activated ability");
    };
    assert_eq!(renew.source_zone, AbilitySourceZone::Graveyard);
    assert_eq!(renew.timing, ActivationTiming::SorcerySpeed);
    assert_eq!(
        renew.costs,
        [
            AbilityCost::Mana(ManaCost::parse("{1}{G}").expect("cost")),
            AbilityCost::ExileSelf,
        ]
    );
}

#[test]
fn issue_373_violent_urge_shares_one_group_across_the_pump_and_grants() {
    let definition = CardRegistry::global()
        .get("violent_urge")
        .expect("Violent Urge");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{R}");
    assert_eq!(face.types, ["Instant"]);
    assert_eq!(face.spell_effect.len(), 3);
    assert!(matches!(
        face.spell_effect[0],
        SpellEffectKind::PumpTarget {
            power: 1,
            toughness: 0,
            ..
        }
    ));
    assert!(matches!(
        &face.spell_effect[1],
        SpellEffectKind::GrantKeywords { keywords, .. } if keywords == &[Keyword::FirstStrike]
    ));
    assert!(matches!(
        &face.spell_effect[2],
        SpellEffectKind::Conditional { condition, .. }
            if condition
                == &GameCondition::GraveyardAggregate {
                    owners: RelativePlayerSet::Controller,
                    aggregate: GraveyardAggregate::DistinctCardTypes,
                    filter: None,
                    min: Some(4),
                    max: None,
                }
    ));
    let targeting = face.targeting.as_ref().expect("one authored group");
    assert_eq!(targeting.groups[0].effect_indices, [0, 1, 2]);
}

#[test]
fn issue_373_beastie_beatdown_authors_conditional_counters_and_power_damage() {
    let definition = CardRegistry::global()
        .get("beastie_beatdown")
        .expect("Beastie Beatdown");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{R}{G}");
    assert_eq!(face.types, ["Sorcery"]);
    let [SpellEffectKind::PutCounters {
        counter,
        count,
        subject,
    }, SpellEffectKind::CreatureDealsDamageEqualToPower { source, target }] =
        face.spell_effect.as_slice()
    else {
        panic!(
            "unexpected Beastie Beatdown payload: {:?}",
            face.spell_effect
        );
    };
    assert_eq!(*counter, CounterKind::PlusOnePlusOne);
    assert_eq!(
        *count,
        Amount::Conditional {
            condition: GameCondition::GraveyardAggregate {
                owners: RelativePlayerSet::Controller,
                aggregate: GraveyardAggregate::DistinctCardTypes,
                filter: None,
                min: Some(4),
                max: None,
            },
            when_true: 2,
            otherwise: 0,
        }
    );
    assert_eq!(
        *subject,
        EffectSubject::Chosen(Box::new(TargetFilter {
            kind: TargetKind::Creature,
            controller: TargetController::You,
            ..TargetFilter::default()
        }))
    );
    assert_eq!(source.controller, TargetController::You);
    assert_eq!(target.controller, TargetController::Opponent);
    let groups = &face.targeting.as_ref().expect("two authored groups").groups;
    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0].effect_indices, [0, 1]);
    assert_eq!(groups[1].effect_indices, [1]);
    assert_eq!(groups[1].distinct_from, [0]);
}

#[test]
fn issue_373_fingerprint_rows_match_the_presentation_registry() {
    let registry = CardRegistry::global();
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, face_id) in [
        ("beastie_beatdown", "beastie_beatdown"),
        ("cloud_of_darkness", "cloud_of_darkness"),
        ("combustion_technique", "combustion_technique"),
        ("frantic_firebolt", "frantic_firebolt"),
        ("gloom_ripper", "gloom_ripper"),
        ("gran_pulse_ochu", "gran_pulse_ochu"),
        ("join_the_dead", "join_the_dead"),
        ("lasyd_prowler", "lasyd_prowler"),
        ("malamet_veteran", "malamet_veteran"),
        ("ooze_patrol", "ooze_patrol"),
        ("swallowed_by_leviathan", "swallowed_by_leviathan"),
        ("thought_shucker", "thought_shucker"),
        ("violent_urge", "violent_urge"),
    ] {
        let presentation = registry
            .presentation_face(id, face_id)
            .unwrap_or_else(|| panic!("missing presentation metadata for {id}/{face_id}"));
        assert_eq!(presentation.oracle_text_sha256.len(), 64);
        let row = fingerprints
            .lines()
            .find(|line| {
                line.starts_with(&format!("{id}\t")) && line.contains(&format!("\t{face_id}\t"))
            })
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}/{face_id}"));
        assert!(
            row.ends_with(&presentation.oracle_text_sha256),
            "fingerprint drift for {id}/{face_id}: {row}"
        );
    }
}
