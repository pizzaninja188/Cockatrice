use tricerules_cards::primitives::{
    CastTriggerPlayer, EffectSubject, LibraryPartitionKind, SpellEffectKind, StaticAbilityDef,
    TargetController, TargetKind,
};
use tricerules_cards::{AbilityCost, CardRegistry, CounterKind, Keyword, TriggerCondition};

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
        id: "chrome_companion",
        name: "Chrome Companion",
        mana_cost: "{2}",
        types: &["Artifact", "Creature", "Dog"],
        stats: (Some(2), Some(1)),
        keywords: &[],
    },
    ExpectedCard {
        id: "starfighter_pilot",
        name: "Starfighter Pilot",
        mana_cost: "{1}{W}",
        types: &["Creature", "Human", "Pilot"],
        stats: (Some(2), Some(2)),
        keywords: &[],
    },
    ExpectedCard {
        id: "compassionate_healer",
        name: "Compassionate Healer",
        mana_cost: "{1}{W}",
        types: &["Creature", "Human", "Cleric", "Ally"],
        stats: (Some(2), Some(2)),
        keywords: &[],
    },
    ExpectedCard {
        id: "tributary_vaulter",
        name: "Tributary Vaulter",
        mana_cost: "{2}{W}",
        types: &["Creature", "Merfolk", "Warrior"],
        stats: (Some(1), Some(3)),
        keywords: &[Keyword::Flying],
    },
    ExpectedCard {
        id: "wanderbrine_preacher",
        name: "Wanderbrine Preacher",
        mana_cost: "{1}{W}",
        types: &["Creature", "Merfolk", "Cleric"],
        stats: (Some(2), Some(2)),
        keywords: &[],
    },
    ExpectedCard {
        id: "cryoshatter",
        name: "Cryoshatter",
        mana_cost: "{U}",
        types: &["Enchantment", "Aura"],
        stats: (None, None),
        keywords: &[],
    },
    ExpectedCard {
        id: "cryogen_relic",
        name: "Cryogen Relic",
        mana_cost: "{1}{U}",
        types: &["Artifact"],
        stats: (None, None),
        keywords: &[],
    },
    ExpectedCard {
        id: "pirate_peddlers",
        name: "Pirate Peddlers",
        mana_cost: "{2}{B}",
        types: &["Creature", "Human", "Pirate"],
        stats: (Some(2), Some(2)),
        keywords: &[Keyword::Deathtouch],
    },
];

#[test]
fn issue_156_cohort_has_exact_oracle_characteristics() {
    let registry = CardRegistry::global();
    for expected in COHORT {
        let definition = registry
            .get(expected.id)
            .unwrap_or_else(|| panic!("missing issue #156 card {}", expected.id));
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
fn issue_156_tap_cards_use_shared_trigger_and_effect_shapes() {
    let registry = CardRegistry::global();
    for id in [
        "chrome_companion",
        "starfighter_pilot",
        "compassionate_healer",
        "tributary_vaulter",
        "wanderbrine_preacher",
    ] {
        assert_eq!(
            registry.get(id).unwrap().primary_face().triggered_abilities[0].trigger,
            TriggerCondition::WheneverSelfBecomesTapped,
            "{id}",
        );
    }

    let pilot = registry.get("starfighter_pilot").unwrap().primary_face();
    assert!(matches!(
        pilot.triggered_abilities[0].effect.as_slice(),
        [SpellEffectKind::LibraryPartition {
            count: 1,
            kind: LibraryPartitionKind::Surveil,
            ..
        }]
    ));

    let vaulter = registry.get("tributary_vaulter").unwrap().primary_face();
    assert!(matches!(
        vaulter.triggered_abilities[0].effect.as_slice(),
        [SpellEffectKind::PumpTarget { power: 2, toughness: 0, subject: EffectSubject::Chosen(filter), .. }]
            if filter.kind == TargetKind::Creature
                && filter.controller == TargetController::You
                && filter.exclude_source
                && filter.required_subtypes == ["Merfolk"]
    ));
}

#[test]
fn issue_156_attachment_leave_and_sacrifice_cards_use_shared_shapes() {
    let registry = CardRegistry::global();

    let cryoshatter = registry.get("cryoshatter").unwrap().primary_face();
    assert_eq!(
        cryoshatter.triggered_abilities[0].trigger,
        TriggerCondition::WheneverAttachedObjectBecomesTapped
    );
    assert!(matches!(
        cryoshatter.triggered_abilities[0].effect.as_slice(),
        [SpellEffectKind::Destroy {
            subject: EffectSubject::TriggerObject
        }]
    ));
    assert!(matches!(
        cryoshatter.static_abilities.as_slice(),
        [StaticAbilityDef::AttachedModifier {
            delta_power: -5,
            delta_toughness: 0,
            ..
        }]
    ));

    let relic = registry.get("cryogen_relic").unwrap().primary_face();
    assert_eq!(
        relic
            .triggered_abilities
            .iter()
            .map(|ability| ability.trigger.clone())
            .collect::<Vec<_>>(),
        [
            TriggerCondition::WhenSelfEntersBattlefield,
            TriggerCondition::WhenSelfLeavesBattlefield
        ]
    );
    let activation = &relic.activated_abilities[0];
    assert!(
        matches!(activation.costs.as_slice(), [AbilityCost::Mana(cost), AbilityCost::SacrificeSelf] if cost.to_string() == "{1}{U}")
    );
    assert!(
        matches!(activation.effect.as_slice(), [SpellEffectKind::PutCounters { counter: CounterKind::Stun, count: 1, subject: EffectSubject::Chosen(filter) }] if filter.tapped == Some(true))
    );
    let group = &activation.targeting.as_ref().unwrap().groups[0];
    assert_eq!((group.min, group.max), (0, 1));

    let peddlers = registry.get("pirate_peddlers").unwrap().primary_face();
    assert!(matches!(
        peddlers.triggered_abilities[0].trigger,
        TriggerCondition::WheneverPlayerSacrificesPermanent {
            player: CastTriggerPlayer::Controller,
            filter: tricerules_cards::primitives::PermanentEventFilter {
                exclude_source: true,
                ..
            },
        }
    ));
}

#[test]
fn issue_168_complete_cohort_has_exact_oracle_characteristics() {
    let registry = CardRegistry::global();
    for (id, name, mana, types, stats, keywords) in [
        (
            "warehouse_tabby",
            "Warehouse Tabby",
            "{B}",
            vec!["Creature", "Cat"],
            (Some(1), Some(1)),
            vec![],
        ),
        (
            "vengeful_tracker",
            "Vengeful Tracker",
            "{1}{R}",
            vec!["Creature", "Human", "Detective"],
            (Some(2), Some(2)),
            vec![],
        ),
        (
            "rakish_crew",
            "Rakish Crew",
            "{2}{B}",
            vec!["Enchantment"],
            (None, None),
            vec![],
        ),
        (
            "vial_smasher,_gleeful_grenadier",
            "Vial Smasher, Gleeful Grenadier",
            "{B}{R}",
            vec!["Legendary", "Creature", "Goblin", "Mercenary"],
            (Some(3), Some(2)),
            vec![],
        ),
        (
            "carrot_cake",
            "Carrot Cake",
            "{1}{W}",
            vec!["Artifact", "Food"],
            (None, None),
            vec![],
        ),
        (
            "knightfisher",
            "Knightfisher",
            "{3}{U}{U}",
            vec!["Creature", "Bird", "Knight"],
            (Some(4), Some(5)),
            vec![Keyword::Flying],
        ),
        (
            "three_tree_scribe",
            "Three Tree Scribe",
            "{1}{G}",
            vec!["Creature", "Frog", "Druid"],
            (Some(2), Some(3)),
            vec![],
        ),
        (
            "armory_mice",
            "Armory Mice",
            "{1}{W}",
            vec!["Creature", "Mouse"],
            (Some(3), Some(1)),
            vec![],
        ),
        (
            "gallant_pie-wielder",
            "Gallant Pie-Wielder",
            "{2}{W}",
            vec!["Creature", "Dwarf", "Knight"],
            (Some(2), Some(3)),
            vec![Keyword::FirstStrike],
        ),
    ] {
        let card = registry
            .get(id)
            .unwrap_or_else(|| panic!("missing complete issue #168 card {id}"));
        assert!(card.partial.is_none(), "{id}");
        assert_eq!(card.name, name);
        let face = card.primary_face();
        assert_eq!(face.mana_cost.to_string(), mana, "{id}");
        assert_eq!(face.types, types, "{id}");
        assert_eq!((face.power, face.toughness), stats, "{id}");
        assert_eq!(face.keywords, keywords, "{id}");
    }
}
