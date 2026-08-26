use tricerules_cards::primitives::{
    CombatRestrictionScope, DrawDiscardOrder, EffectSubject, LibraryPartitionKind, SpellEffectKind,
    TargetController, TargetKind,
};
use tricerules_cards::{
    AbilityCost, AbilitySourceZone, ActivationTiming, CardRegistry, Color, CounterKind,
    GameCondition, Keyword, SearchDestination, SpellCostModifier, TriggerCondition,
};

struct ExpectedCard {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    types: &'static [&'static str],
    stats: (Option<u32>, Option<u32>),
    keywords: &'static [Keyword],
}

const COHORT: &[ExpectedCard] = &[
    ExpectedCard {
        id: "galactic_wayfarer",
        name: "Galactic Wayfarer",
        mana_cost: "{2}{G}",
        types: &["Creature", "Human", "Scout"],
        stats: (Some(3), Some(3)),
        keywords: &[],
    },
    ExpectedCard {
        id: "sun-blessed_peak",
        name: "Sun-Blessed Peak",
        mana_cost: "",
        types: &["Land"],
        stats: (None, None),
        keywords: &[],
    },
    ExpectedCard {
        id: "biosynthic_burst",
        name: "Biosynthic Burst",
        mana_cost: "{1}{G}",
        types: &["Instant"],
        stats: (None, None),
        keywords: &[],
    },
    ExpectedCard {
        id: "glider_kids",
        name: "Glider Kids",
        mana_cost: "{2}{W}",
        types: &["Creature", "Human", "Pilot", "Ally"],
        stats: (Some(2), Some(3)),
        keywords: &[Keyword::Flying],
    },
    ExpectedCard {
        id: "pretending_poxbearers",
        name: "Pretending Poxbearers",
        mana_cost: "{1}{W/B}",
        types: &["Creature", "Human", "Citizen", "Ally"],
        stats: (Some(2), Some(1)),
        keywords: &[],
    },
    ExpectedCard {
        id: "radiant_strike",
        name: "Radiant Strike",
        mana_cost: "{3}{W}",
        types: &["Instant"],
        stats: (None, None),
        keywords: &[],
    },
    ExpectedCard {
        id: "cloudsculpt_technician",
        name: "Cloudsculpt Technician",
        mana_cost: "{2}{U}",
        types: &["Creature", "Jellyfish", "Artificer"],
        stats: (Some(1), Some(4)),
        keywords: &[Keyword::Flying],
    },
    ExpectedCard {
        id: "mistmeadow_council",
        name: "Mistmeadow Council",
        mana_cost: "{4}{G}",
        types: &["Creature", "Kithkin", "Advisor"],
        stats: (Some(4), Some(3)),
        keywords: &[],
    },
    ExpectedCard {
        id: "octopus_form",
        name: "Octopus Form",
        mana_cost: "{U}",
        types: &["Instant", "Lesson"],
        stats: (None, None),
        keywords: &[],
    },
    ExpectedCard {
        id: "rig_for_war",
        name: "Rig for War",
        mana_cost: "{1}{R}",
        types: &["Instant"],
        stats: (None, None),
        keywords: &[],
    },
    ExpectedCard {
        id: "boggart_prankster",
        name: "Boggart Prankster",
        mana_cost: "{1}{B}",
        types: &["Creature", "Goblin", "Warrior"],
        stats: (Some(1), Some(3)),
        keywords: &[],
    },
    ExpectedCard {
        id: "azula_always_lies",
        name: "Azula Always Lies",
        mana_cost: "{1}{B}",
        types: &["Instant", "Lesson"],
        stats: (None, None),
        keywords: &[],
    },
    ExpectedCard {
        id: "surly_farrier",
        name: "Surly Farrier",
        mana_cost: "{1}{G}",
        types: &["Creature", "Kithkin", "Citizen"],
        stats: (Some(2), Some(2)),
        keywords: &[],
    },
    ExpectedCard {
        id: "otter-penguin",
        name: "Otter-Penguin",
        mana_cost: "{1}{U}",
        types: &["Creature", "Otter", "Bird"],
        stats: (Some(2), Some(1)),
        keywords: &[],
    },
    ExpectedCard {
        id: "crossroads_watcher",
        name: "Crossroads Watcher",
        mana_cost: "{2}{G}",
        types: &["Creature", "Kithkin", "Ranger"],
        stats: (Some(3), Some(3)),
        keywords: &[Keyword::Trample],
    },
    ExpectedCard {
        id: "thawbringer",
        name: "Thawbringer",
        mana_cost: "{2}{G}",
        types: &["Creature", "Insect", "Scout"],
        stats: (Some(4), Some(2)),
        keywords: &[],
    },
    ExpectedCard {
        id: "abandon_attachments",
        name: "Abandon Attachments",
        mana_cost: "{1}{U/R}",
        types: &["Instant", "Lesson"],
        stats: (None, None),
        keywords: &[],
    },
    ExpectedCard {
        id: "rowdy_snowballers",
        name: "Rowdy Snowballers",
        mana_cost: "{2}{U}",
        types: &["Creature", "Human", "Peasant", "Ally"],
        stats: (Some(2), Some(2)),
        keywords: &[],
    },
    ExpectedCard {
        id: "wandering_musicians",
        name: "Wandering Musicians",
        mana_cost: "{3}{R/W}",
        types: &["Creature", "Human", "Bard", "Ally"],
        stats: (Some(2), Some(5)),
        keywords: &[],
    },
    ExpectedCard {
        id: "mongoose_lizard",
        name: "Mongoose Lizard",
        mana_cost: "{4}{R}{R}",
        types: &["Creature", "Mongoose", "Lizard"],
        stats: (Some(5), Some(6)),
        keywords: &[Keyword::Menace],
    },
];

#[test]
fn issue_163_cohort_has_exact_oracle_characteristics() {
    let registry = CardRegistry::global();
    for expected in COHORT {
        let definition = registry
            .get(expected.id)
            .unwrap_or_else(|| panic!("missing issue #163 card {}", expected.id));
        assert_eq!(definition.name, expected.name, "{}", expected.id);
        assert_eq!(registry.id_for_name(expected.name), Some(expected.id));
        assert!(
            definition.partial.is_none(),
            "{} must be complete",
            expected.id
        );
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
        assert_eq!(
            (face.power, face.toughness),
            expected.stats,
            "{}",
            expected.id
        );
        assert_eq!(
            face.keywords.as_slice(),
            expected.keywords,
            "{}",
            expected.id
        );
    }
}

#[test]
fn issue_163_tokens_have_exact_registry_definitions() {
    let registry = CardRegistry::global();
    let lander = registry.get("lander").expect("Lander token");
    assert!(registry.is_token("lander"));
    assert_eq!(lander.primary_face().types, ["Artifact", "Lander"]);
    let ability = &lander.primary_face().activated_abilities[0];
    assert!(
        matches!(ability.costs.as_slice(), [AbilityCost::Mana(cost), AbilityCost::Tap, AbilityCost::SacrificeSelf] if cost.to_string() == "{2}")
    );
    assert!(matches!(
        ability.effect.as_slice(),
        [SpellEffectKind::SearchLibrary {
            destination: SearchDestination::Battlefield { tapped: true },
            shuffle: true,
            reveal: false,
            ..
        }]
    ));

    let ally = registry.get("ally_w_1_1").expect("white 1/1 Ally token");
    assert!(registry.is_token("ally_w_1_1"));
    let face = ally.primary_face();
    assert_eq!(face.types, ["Creature", "Ally"]);
    assert_eq!(face.colors_override.as_ref(), Some(&vec![Color::White]));
    assert_eq!((face.power, face.toughness), (Some(1), Some(1)));
}

#[test]
fn issue_163_uses_generic_target_modal_and_trigger_shapes() {
    let registry = CardRegistry::global();
    let radiant = registry.get("radiant_strike").unwrap().primary_face();
    assert!(
        matches!(radiant.spell_effect.as_slice(), [SpellEffectKind::Destroy { subject: EffectSubject::Chosen(target) }, SpellEffectKind::GainLife { .. }] if target.any_of.as_ref().is_some_and(|filters| filters.len() == 2))
    );

    let azula = registry.get("azula_always_lies").unwrap().primary_face();
    let modal = azula.modal_spell.as_ref().expect("one-or-both modal spell");
    assert_eq!(
        (modal.min_modes, modal.max_modes, modal.modes.len()),
        (1, 2, 2)
    );
    assert!(matches!(
        modal.modes[0].effects.as_slice(),
        [SpellEffectKind::PumpTarget {
            power: -1,
            toughness: -1,
            ..
        }]
    ));
    assert!(matches!(
        modal.modes[1].effects.as_slice(),
        [SpellEffectKind::PutCounters {
            counter: CounterKind::PlusOnePlusOne,
            count: 1,
            ..
        }]
    ));

    let boggart = registry.get("boggart_prankster").unwrap().primary_face();
    assert!(matches!(
        boggart.triggered_abilities[0].trigger,
        TriggerCondition::WheneverControllerAttacks {
            min_attackers: None,
            max_attackers: None
        }
    ));
    assert!(
        matches!(boggart.triggered_abilities[0].effect.as_slice(), [SpellEffectKind::PumpTarget { subject: EffectSubject::Chosen(target), .. }] if target.controller == TargetController::You && target.required_subtypes == ["Goblin"] && target.attacking_or_blocking)
    );

    let thawbringer = registry.get("thawbringer").unwrap().primary_face();
    assert_eq!(
        thawbringer
            .triggered_abilities
            .iter()
            .map(|ability| ability.trigger.clone())
            .collect::<Vec<_>>(),
        [
            TriggerCondition::WhenSelfEntersBattlefield,
            TriggerCondition::WhenSelfDies
        ]
    );
    assert!(thawbringer
        .triggered_abilities
        .iter()
        .all(|ability| matches!(
            ability.effect.as_slice(),
            [SpellEffectKind::LibraryPartition {
                kind: LibraryPartitionKind::Surveil,
                count: 1,
                ..
            }]
        )));
}

#[test]
fn issue_163_uses_generic_typecycling_condition_and_ordinal_shapes() {
    let registry = CardRegistry::global();
    let mongoose = registry.get("mongoose_lizard").unwrap().primary_face();
    let typecycling = &mongoose.activated_abilities[0];
    assert_eq!(typecycling.source_zone, AbilitySourceZone::Hand);
    assert!(
        matches!(typecycling.costs.as_slice(), [AbilityCost::Mana(cost), AbilityCost::DiscardSelf] if cost.to_string() == "{2}")
    );
    assert!(matches!(
        typecycling.effect.as_slice(),
        [SpellEffectKind::SearchLibrary {
            destination: SearchDestination::Hand,
            reveal: true,
            ..
        }]
    ));

    let cloudsculpt = registry
        .get("cloudsculpt_technician")
        .unwrap()
        .primary_face();
    assert!(matches!(
        cloudsculpt.static_abilities.as_slice(),
        [
            tricerules_cards::primitives::StaticAbilityDef::ConditionalSelfModifier {
                condition: GameCondition::BattlefieldAggregate { min: Some(1), .. },
                delta_power: 1,
                ..
            }
        ]
    ));

    let council = registry.get("mistmeadow_council").unwrap().primary_face();
    assert!(matches!(
        council.cost_modifiers.as_slice(),
        [SpellCostModifier::ConditionalGenericReduction {
            amount: 1,
            condition: GameCondition::BattlefieldCreatureCount { min: Some(1), .. }
        }]
    ));

    let otter = registry.get("otter-penguin").unwrap().primary_face();
    assert!(matches!(
        otter.triggered_abilities[0].trigger,
        TriggerCondition::WheneverPlayerDrawsNthCard { ordinal: 2, .. }
    ));
    assert!(
        matches!(otter.triggered_abilities[0].effect.as_slice(), [SpellEffectKind::PumpTarget { power: 1, toughness: 2, subject: EffectSubject::Source, .. }, SpellEffectKind::ApplyCombatRestriction { scope: CombatRestrictionScope::Source, restriction }] if restriction.cant_be_blocked)
    );

    let farrier = registry.get("surly_farrier").unwrap().primary_face();
    assert_eq!(
        farrier.activated_abilities[0].timing,
        ActivationTiming::SorcerySpeed
    );
    assert!(
        matches!(farrier.activated_abilities[0].effect.as_slice(), [SpellEffectKind::PumpTarget { subject: EffectSubject::Chosen(target), .. }, SpellEffectKind::GrantKeywords { .. }] if target.kind == TargetKind::Creature && target.controller == TargetController::You)
    );

    let abandon = registry.get("abandon_attachments").unwrap().primary_face();
    assert!(matches!(
        abandon.spell_effect.as_slice(),
        [SpellEffectKind::DrawDiscard {
            draw_count: 2,
            discard_count: 1,
            order: DrawDiscardOrder::DiscardThenDraw,
            optional: true,
            ..
        }]
    ));
}
