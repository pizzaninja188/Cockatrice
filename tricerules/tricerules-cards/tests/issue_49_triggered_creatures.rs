use tricerules_cards::primitives::{
    CardTypeFilter, CreatureScopeController, CreatureScopeFilter, EffectSubject, LifeAmount,
    PlayerRecipient, SpellEffectKind, TargetController, TargetFilter, TargetKind,
};
use tricerules_cards::{
    AbilityCost, Amount, CardRegistry, CastTriggerPlayer, CounterKind, Keyword, ManaAmount,
    PermanentTypeFilter, TriggerCondition, TriggeredAbilityDef,
};

struct ExpectedCard {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    types: &'static [&'static str],
    power: u32,
    toughness: u32,
    keywords: &'static [Keyword],
    activated_abilities: usize,
}

const COHORT: &[ExpectedCard] = &[
    ExpectedCard {
        id: "spellgorger_weird",
        name: "Spellgorger Weird",
        mana_cost: "{2}{R}",
        types: &["Creature", "Weird"],
        power: 2,
        toughness: 2,
        keywords: &[],
        activated_abilities: 0,
    },
    ExpectedCard {
        id: "gale_swooper",
        name: "Gale Swooper",
        mana_cost: "{3}{W}",
        types: &["Creature", "Griffin"],
        power: 3,
        toughness: 2,
        keywords: &[Keyword::Flying],
        activated_abilities: 0,
    },
    ExpectedCard {
        id: "steadfast_sentry",
        name: "Steadfast Sentry",
        mana_cost: "{2}{W}",
        types: &["Creature", "Human", "Soldier"],
        power: 3,
        toughness: 2,
        keywords: &[Keyword::Vigilance],
        activated_abilities: 0,
    },
    ExpectedCard {
        id: "daybreak_charger",
        name: "Daybreak Charger",
        mana_cost: "{1}{W}",
        types: &["Creature", "Unicorn"],
        power: 3,
        toughness: 1,
        keywords: &[],
        activated_abilities: 0,
    },
    ExpectedCard {
        id: "mistral_singer",
        name: "Mistral Singer",
        mana_cost: "{2}{U}",
        types: &["Creature", "Siren"],
        power: 2,
        toughness: 2,
        keywords: &[Keyword::Flying],
        activated_abilities: 0,
    },
    ExpectedCard {
        id: "skyscanner",
        name: "Skyscanner",
        mana_cost: "{3}",
        types: &["Artifact", "Creature", "Thopter"],
        power: 1,
        toughness: 1,
        keywords: &[Keyword::Flying],
        activated_abilities: 0,
    },
    ExpectedCard {
        id: "skymarch_bloodletter",
        name: "Skymarch Bloodletter",
        mana_cost: "{2}{B}",
        types: &["Creature", "Vampire", "Soldier"],
        power: 2,
        toughness: 2,
        keywords: &[Keyword::Flying],
        activated_abilities: 0,
    },
    ExpectedCard {
        id: "griffin_protector",
        name: "Griffin Protector",
        mana_cost: "{3}{W}",
        types: &["Creature", "Griffin"],
        power: 2,
        toughness: 3,
        keywords: &[Keyword::Flying],
        activated_abilities: 0,
    },
    ExpectedCard {
        id: "llanowar_visionary",
        name: "Llanowar Visionary",
        mana_cost: "{2}{G}",
        types: &["Creature", "Elf", "Druid"],
        power: 2,
        toughness: 2,
        keywords: &[],
        activated_abilities: 1,
    },
    ExpectedCard {
        id: "inspiring_captain",
        name: "Inspiring Captain",
        mana_cost: "{3}{W}",
        types: &["Creature", "Human", "Knight"],
        power: 3,
        toughness: 3,
        keywords: &[],
        activated_abilities: 0,
    },
    ExpectedCard {
        id: "aven_wind_mage",
        name: "Aven Wind Mage",
        mana_cost: "{2}{U}",
        types: &["Creature", "Bird", "Wizard"],
        power: 2,
        toughness: 2,
        keywords: &[Keyword::Flying],
        activated_abilities: 0,
    },
    ExpectedCard {
        id: "dawning_angel",
        name: "Dawning Angel",
        mana_cost: "{4}{W}",
        types: &["Creature", "Angel"],
        power: 3,
        toughness: 2,
        keywords: &[Keyword::Flying],
        activated_abilities: 0,
    },
    ExpectedCard {
        id: "library_larcenist",
        name: "Library Larcenist",
        mana_cost: "{2}{U}",
        types: &["Creature", "Merfolk", "Rogue"],
        power: 1,
        toughness: 2,
        keywords: &[],
        activated_abilities: 0,
    },
    ExpectedCard {
        id: "highland_game",
        name: "Highland Game",
        mana_cost: "{1}{G}",
        types: &["Creature", "Elk"],
        power: 2,
        toughness: 1,
        keywords: &[],
        activated_abilities: 0,
    },
    ExpectedCard {
        id: "audacious_thief",
        name: "Audacious Thief",
        mana_cost: "{2}{B}",
        types: &["Creature", "Human", "Rogue"],
        power: 2,
        toughness: 2,
        keywords: &[],
        activated_abilities: 0,
    },
    ExpectedCard {
        id: "spined_megalodon",
        name: "Spined Megalodon",
        mana_cost: "{5}{U}{U}",
        types: &["Creature", "Shark"],
        power: 5,
        toughness: 7,
        keywords: &[Keyword::Hexproof],
        activated_abilities: 0,
    },
    ExpectedCard {
        id: "cloudkin_seer",
        name: "Cloudkin Seer",
        mana_cost: "{2}{U}",
        types: &["Creature", "Elemental", "Wizard"],
        power: 2,
        toughness: 1,
        keywords: &[Keyword::Flying],
        activated_abilities: 0,
    },
    ExpectedCard {
        id: "wall_of_runes",
        name: "Wall of Runes",
        mana_cost: "{U}",
        types: &["Creature", "Wall"],
        power: 0,
        toughness: 4,
        keywords: &[Keyword::Defender],
        activated_abilities: 0,
    },
    ExpectedCard {
        id: "rhox_oracle",
        name: "Rhox Oracle",
        mana_cost: "{4}{G}",
        types: &["Creature", "Rhino", "Monk"],
        power: 4,
        toughness: 2,
        keywords: &[],
        activated_abilities: 0,
    },
    ExpectedCard {
        id: "cavalry_drillmaster",
        name: "Cavalry Drillmaster",
        mana_cost: "{1}{W}",
        types: &["Creature", "Human", "Knight"],
        power: 2,
        toughness: 1,
        keywords: &[],
        activated_abilities: 0,
    },
];

fn trigger(card_id: &str) -> &'static TriggeredAbilityDef {
    let face = CardRegistry::global()
        .get(card_id)
        .unwrap_or_else(|| panic!("missing issue #49 card {card_id}"))
        .primary_face();
    assert_eq!(face.triggered_abilities.len(), 1, "{card_id}");
    &face.triggered_abilities[0]
}

fn creature_target(controller: TargetController) -> TargetFilter {
    TargetFilter {
        kind: TargetKind::Creature,
        controller,
        ..TargetFilter::default()
    }
}

#[test]
fn issue_49_cohort_has_exact_oracle_characteristics() {
    let registry = CardRegistry::global();
    for expected in COHORT {
        let definition = registry
            .get(expected.id)
            .unwrap_or_else(|| panic!("missing issue #49 card {}", expected.id));
        assert_eq!(definition.name, expected.name, "{}", expected.id);
        assert_eq!(registry.id_for_name(expected.name), Some(expected.id));
        assert!(definition.partial.is_none(), "{} must be full", expected.id);

        let face = definition.primary_face();
        assert_eq!(
            face.mana_cost.to_string(),
            expected.mana_cost,
            "{}",
            expected.id
        );
        assert_eq!(
            face.types.iter().map(String::as_str).collect::<Vec<_>>(),
            expected.types,
            "{}",
            expected.id
        );
        assert_eq!(face.power, Some(expected.power), "{}", expected.id);
        assert_eq!(face.toughness, Some(expected.toughness), "{}", expected.id);
        assert_eq!(
            face.keywords.as_slice(),
            expected.keywords,
            "{}",
            expected.id
        );
        assert_eq!(
            face.activated_abilities.len(),
            expected.activated_abilities,
            "{}",
            expected.id
        );
    }
}

#[test]
fn issue_49_cast_trigger_cards_use_existing_filters_and_source_effects() {
    let spellgorger = trigger("spellgorger_weird");
    assert_eq!(
        spellgorger.trigger,
        TriggerCondition::WheneverPlayerCastsSpell {
            caster: CastTriggerPlayer::Controller,
            spell_type: Some(CardTypeFilter::Noncreature),
        }
    );
    assert_eq!(
        spellgorger.effect,
        [SpellEffectKind::PutCounters {
            counter: CounterKind::PlusOnePlusOne,
            count: 1,
            subject: EffectSubject::Source,
        }]
    );

    for id in ["mistral_singer", "aven_wind_mage"] {
        let ability = trigger(id);
        let spell_type = if id == "mistral_singer" {
            CardTypeFilter::Noncreature
        } else {
            CardTypeFilter::InstantOrSorcery
        };
        assert_eq!(
            ability.trigger,
            TriggerCondition::WheneverPlayerCastsSpell {
                caster: CastTriggerPlayer::Controller,
                spell_type: Some(spell_type),
            },
            "{id}"
        );
        assert_eq!(
            ability.effect,
            [SpellEffectKind::PumpTarget {
                power: 1,
                toughness: 1,
                scale: None,
                subject: EffectSubject::Source,
            }],
            "{id}"
        );
    }
}

#[test]
fn issue_49_targeted_etbs_use_one_existing_target_group() {
    assert_eq!(
        trigger("gale_swooper").effect,
        [SpellEffectKind::GrantKeywords {
            subject: EffectSubject::Chosen(creature_target(TargetController::Any)),
            keywords: vec![Keyword::Flying],
        }]
    );
    assert_eq!(
        trigger("daybreak_charger").effect,
        [SpellEffectKind::PumpTarget {
            power: 2,
            toughness: 0,
            scale: None,
            subject: EffectSubject::Chosen(creature_target(TargetController::Any)),
        }]
    );
    assert_eq!(
        trigger("skymarch_bloodletter").effect,
        [SpellEffectKind::DrainTarget {
            amount: 1,
            target: TargetFilter {
                kind: TargetKind::OpponentPlayer,
                ..TargetFilter::default()
            },
        }]
    );
    assert_eq!(
        trigger("cavalry_drillmaster").effect,
        [
            SpellEffectKind::PumpTarget {
                power: 2,
                toughness: 0,
                scale: None,
                subject: EffectSubject::Chosen(creature_target(TargetController::Any)),
            },
            SpellEffectKind::GrantKeywords {
                subject: EffectSubject::Chosen(creature_target(TargetController::Any)),
                keywords: vec![Keyword::FirstStrike],
            },
        ]
    );
}

#[test]
fn issue_49_untargeted_etbs_compose_existing_effects() {
    for id in [
        "skyscanner",
        "llanowar_visionary",
        "cloudkin_seer",
        "rhox_oracle",
    ] {
        assert_eq!(
            trigger(id).effect,
            [SpellEffectKind::Draw {
                count: Amount::Fixed(1),
            }],
            "{id}"
        );
    }
    assert_eq!(
        trigger("dawning_angel").effect,
        [SpellEffectKind::GainLife {
            amount: Amount::Fixed(4),
        }]
    );
    assert_eq!(
        trigger("wall_of_runes").effect,
        [SpellEffectKind::Scry { count: 1 }]
    );
    assert_eq!(
        trigger("inspiring_captain").effect,
        [SpellEffectKind::PumpAll {
            filter: CreatureScopeFilter {
                controller: Some(CreatureScopeController::YouControl),
                ..CreatureScopeFilter::default()
            },
            power: 1,
            toughness: 1,
        }]
    );

    let visionary = CardRegistry::global()
        .get("llanowar_visionary")
        .unwrap()
        .primary_face();
    assert_eq!(visionary.activated_abilities[0].costs, [AbilityCost::Tap]);
    assert_eq!(
        visionary.activated_abilities[0].effect,
        [SpellEffectKind::ProduceMana {
            options: vec![ManaAmount {
                g: 1,
                ..ManaAmount::default()
            }],
        }]
    );
}

#[test]
fn issue_49_observer_and_dies_triggers_use_existing_subjects() {
    let griffin = trigger("griffin_protector");
    assert_eq!(
        griffin.trigger,
        TriggerCondition::WheneverPermanentEntersBattlefield {
            controller: CastTriggerPlayer::Controller,
            permanent_type: Some(PermanentTypeFilter::Creature),
            exclude_self: true,
        }
    );
    assert_eq!(
        griffin.effect,
        [SpellEffectKind::PumpTarget {
            power: 1,
            toughness: 1,
            scale: None,
            subject: EffectSubject::Source,
        }]
    );

    assert_eq!(
        trigger("steadfast_sentry").trigger,
        TriggerCondition::WhenSelfDies
    );
    assert_eq!(
        trigger("steadfast_sentry").effect,
        [SpellEffectKind::PutCounters {
            counter: CounterKind::PlusOnePlusOne,
            count: 1,
            subject: EffectSubject::Chosen(creature_target(TargetController::You)),
        }]
    );
    assert_eq!(
        trigger("highland_game").trigger,
        TriggerCondition::WhenSelfDies
    );
    assert_eq!(
        trigger("highland_game").effect,
        [SpellEffectKind::GainLife {
            amount: Amount::Fixed(2),
        }]
    );
}

#[test]
fn issue_49_attack_triggers_use_existing_ordered_effects() {
    assert_eq!(
        trigger("library_larcenist").trigger,
        TriggerCondition::WheneverSelfAttacks
    );
    assert_eq!(
        trigger("library_larcenist").effect,
        [SpellEffectKind::Draw {
            count: Amount::Fixed(1),
        }]
    );
    assert_eq!(
        trigger("audacious_thief").effect,
        [
            SpellEffectKind::Draw {
                count: Amount::Fixed(1),
            },
            SpellEffectKind::LoseLife {
                amount: LifeAmount::Fixed(1),
                who: PlayerRecipient::Controller,
            },
        ]
    );
    assert_eq!(
        trigger("spined_megalodon").effect,
        [SpellEffectKind::Scry { count: 1 }]
    );
}
