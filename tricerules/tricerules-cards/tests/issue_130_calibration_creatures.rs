use tricerules_cards::primitives::{
    CombatRestrictionScope, EffectSubject, GameCondition, LifeAmount, PlayerRecipient,
    PowerComparison, SpellEffectKind, TargetController, TargetKind,
};
use tricerules_cards::{AbilityCost, CardRegistry, Color, Keyword, TriggerCondition};

struct ExpectedCard {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    types: &'static [&'static str],
    power: u32,
    toughness: u32,
    keywords: &'static [Keyword],
    triggers: usize,
    activations: usize,
}

const COHORT: &[ExpectedCard] = &[
    ExpectedCard {
        id: "watcher_of_the_wayside",
        name: "Watcher of the Wayside",
        mana_cost: "{3}",
        types: &["Artifact", "Creature", "Golem"],
        power: 3,
        toughness: 2,
        keywords: &[],
        triggers: 1,
        activations: 0,
    },
    ExpectedCard {
        id: "sanguine_syphoner",
        name: "Sanguine Syphoner",
        mana_cost: "{1}{B}",
        types: &["Creature", "Vampire", "Warlock"],
        power: 1,
        toughness: 3,
        keywords: &[],
        triggers: 1,
        activations: 0,
    },
    ExpectedCard {
        id: "underfoot_underdogs",
        name: "Underfoot Underdogs",
        mana_cost: "{2}{R}",
        types: &["Creature", "Goblin", "Warrior"],
        power: 1,
        toughness: 2,
        keywords: &[],
        triggers: 1,
        activations: 1,
    },
    ExpectedCard {
        id: "flesh_burrower",
        name: "Flesh Burrower",
        mana_cost: "{1}{G}",
        types: &["Creature", "Insect"],
        power: 2,
        toughness: 2,
        keywords: &[Keyword::Deathtouch],
        triggers: 1,
        activations: 0,
    },
    ExpectedCard {
        id: "prideful_parent",
        name: "Prideful Parent",
        mana_cost: "{2}{W}",
        types: &["Creature", "Cat"],
        power: 2,
        toughness: 2,
        keywords: &[Keyword::Vigilance],
        triggers: 1,
        activations: 0,
    },
    ExpectedCard {
        id: "apothecary_stomper",
        name: "Apothecary Stomper",
        mana_cost: "{4}{G}{G}",
        types: &["Creature", "Elephant"],
        power: 4,
        toughness: 4,
        keywords: &[Keyword::Vigilance],
        triggers: 1,
        activations: 0,
    },
    ExpectedCard {
        id: "elfsworn_giant",
        name: "Elfsworn Giant",
        mana_cost: "{3}{G}{G}",
        types: &["Creature", "Giant"],
        power: 5,
        toughness: 3,
        keywords: &[Keyword::Reach],
        triggers: 1,
        activations: 0,
    },
    ExpectedCard {
        id: "helpful_hunter",
        name: "Helpful Hunter",
        mana_cost: "{1}{W}",
        types: &["Creature", "Cat"],
        power: 1,
        toughness: 1,
        keywords: &[],
        triggers: 1,
        activations: 0,
    },
    ExpectedCard {
        id: "kin-tree_nurturer",
        name: "Kin-Tree Nurturer",
        mana_cost: "{2}{B}",
        types: &["Creature", "Human", "Druid"],
        power: 2,
        toughness: 1,
        keywords: &[Keyword::Lifelink],
        triggers: 1,
        activations: 0,
    },
    ExpectedCard {
        id: "dusyut_earthcarver",
        name: "Dusyut Earthcarver",
        mana_cost: "{5}{G}",
        types: &["Creature", "Elephant", "Druid"],
        power: 4,
        toughness: 4,
        keywords: &[Keyword::Reach],
        triggers: 1,
        activations: 0,
    },
    ExpectedCard {
        id: "sandskitter_outrider",
        name: "Sandskitter Outrider",
        mana_cost: "{3}{B}",
        types: &["Creature", "Goblin", "Soldier"],
        power: 2,
        toughness: 1,
        keywords: &[Keyword::Menace],
        triggers: 1,
        activations: 0,
    },
    ExpectedCard {
        id: "beast-kin_ranger",
        name: "Beast-Kin Ranger",
        mana_cost: "{2}{G}",
        types: &["Creature", "Elf", "Ranger"],
        power: 3,
        toughness: 3,
        keywords: &[Keyword::Trample],
        triggers: 1,
        activations: 0,
    },
    ExpectedCard {
        id: "dwynens_elite",
        name: "Dwynen's Elite",
        mana_cost: "{1}{G}",
        types: &["Creature", "Elf", "Warrior"],
        power: 2,
        toughness: 2,
        keywords: &[],
        triggers: 1,
        activations: 0,
    },
    ExpectedCard {
        id: "humbling_elder",
        name: "Humbling Elder",
        mana_cost: "{U}",
        types: &["Creature", "Human", "Monk"],
        power: 1,
        toughness: 2,
        keywords: &[Keyword::Flash],
        triggers: 1,
        activations: 0,
    },
    ExpectedCard {
        id: "reputable_merchant",
        name: "Reputable Merchant",
        mana_cost: "{2/W}{2/B}{2/G}",
        types: &["Creature", "Human", "Citizen"],
        power: 2,
        toughness: 2,
        keywords: &[],
        triggers: 2,
        activations: 0,
    },
    ExpectedCard {
        id: "delta_bloodflies",
        name: "Delta Bloodflies",
        mana_cost: "{1}{B}",
        types: &["Creature", "Insect"],
        power: 1,
        toughness: 2,
        keywords: &[Keyword::Flying],
        triggers: 1,
        activations: 0,
    },
    ExpectedCard {
        id: "iceridge_serpent",
        name: "Iceridge Serpent",
        mana_cost: "{4}{U}",
        types: &["Creature", "Serpent"],
        power: 3,
        toughness: 3,
        keywords: &[],
        triggers: 1,
        activations: 0,
    },
    ExpectedCard {
        id: "felidar_savior",
        name: "Felidar Savior",
        mana_cost: "{3}{W}",
        types: &["Creature", "Cat", "Beast"],
        power: 2,
        toughness: 3,
        keywords: &[Keyword::Lifelink],
        triggers: 1,
        activations: 0,
    },
    ExpectedCard {
        id: "infestation_sage",
        name: "Infestation Sage",
        mana_cost: "{B}",
        types: &["Creature", "Elf", "Warlock"],
        power: 1,
        toughness: 1,
        keywords: &[],
        triggers: 1,
        activations: 0,
    },
    ExpectedCard {
        id: "summit_intimidator",
        name: "Summit Intimidator",
        mana_cost: "{3}{R}",
        types: &["Creature", "Yeti"],
        power: 4,
        toughness: 3,
        keywords: &[Keyword::Reach],
        triggers: 1,
        activations: 0,
    },
];

#[test]
fn issue_130_cohort_has_exact_oracle_characteristics() {
    let registry = CardRegistry::global();
    for expected in COHORT {
        let definition = registry
            .get(expected.id)
            .unwrap_or_else(|| panic!("missing issue #130 card {}", expected.id));
        assert_eq!(definition.name, expected.name, "{}", expected.id);
        assert_eq!(registry.id_for_name(expected.name), Some(expected.id));
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
            (Some(expected.power), Some(expected.toughness)),
            "{}",
            expected.id
        );
        assert_eq!(
            face.keywords.as_slice(),
            expected.keywords,
            "{}",
            expected.id
        );
        assert_eq!(
            face.triggered_abilities.len(),
            expected.triggers,
            "{}",
            expected.id
        );
        assert_eq!(
            face.activated_abilities.len(),
            expected.activations,
            "{}",
            expected.id
        );
    }
}

#[test]
fn issue_130_token_definitions_have_exact_characteristics() {
    let registry = CardRegistry::global();
    for (id, name, colors, keywords) in [
        ("cat_w_1_1", "Cat", vec![Color::White], vec![]),
        (
            "elf_warrior_g_1_1",
            "Elf Warrior",
            vec![Color::Green],
            vec![],
        ),
        (
            "insect_bg_1_1_flying",
            "Insect",
            vec![Color::Black, Color::Green],
            vec![Keyword::Flying],
        ),
    ] {
        let definition = registry
            .get(id)
            .unwrap_or_else(|| panic!("missing token {id}"));
        assert!(registry.is_token(id));
        let face = definition.primary_face();
        assert_eq!(definition.name, name, "{id}");
        assert_eq!((face.power, face.toughness), (Some(1), Some(1)), "{id}");
        assert_eq!(face.colors_override.as_ref(), Some(&colors), "{id}");
        assert_eq!(face.keywords, keywords, "{id}");
    }
}

#[test]
fn issue_130_target_choice_and_condition_shapes_use_generic_vocabulary() {
    let registry = CardRegistry::global();
    let face = |id: &str| {
        registry
            .get(id)
            .unwrap_or_else(|| panic!("missing {id}"))
            .primary_face()
    };

    assert!(matches!(
        face("iceridge_serpent").triggered_abilities[0].effect.as_slice(),
        [SpellEffectKind::ReturnToOwnersHand {
            subject: EffectSubject::Chosen(target),
        }]
            if target.kind == TargetKind::Creature && target.controller == TargetController::Opponent
    ));

    assert!(matches!(
        face("sanguine_syphoner").triggered_abilities[0]
            .effect
            .as_slice(),
        [
            SpellEffectKind::LoseLife {
                amount: LifeAmount::Fixed(1),
                who: PlayerRecipient::EachOpponent
            },
            SpellEffectKind::GainLife { .. }
        ]
    ));

    for id in ["dwynens_elite", "delta_bloodflies"] {
        assert!(
            matches!(
                face(id).triggered_abilities[0].intervening_if,
                Some(GameCondition::BattlefieldCreatureCount { min: Some(1), .. })
            ),
            "{id}"
        );
    }

    let felidar = &face("felidar_savior").triggered_abilities[0];
    let group = &felidar.targeting.as_ref().expect("grouped targets").groups[0];
    assert_eq!((group.min, group.max), (0, 2));
    assert!(matches!(
        felidar.effect.as_slice(),
        [SpellEffectKind::PutCounters { subject: EffectSubject::Chosen(target), .. }]
            if target.controller == TargetController::You && target.excluded_objects.contains(&tricerules_cards::TargetObjectExclusion::Source)
    ));

    let apothecary = &face("apothecary_stomper").triggered_abilities[0];
    assert_eq!(apothecary.modal.as_ref().expect("modal ETB").modes.len(), 2);

    let underdogs = &face("underfoot_underdogs").activated_abilities[0];
    assert!(matches!(
        underdogs.costs.as_slice(),
        [AbilityCost::Mana(_), AbilityCost::Tap]
    ));
    assert!(matches!(
        underdogs.effect.as_slice(),
        [SpellEffectKind::ApplyCombatRestriction { scope: CombatRestrictionScope::Chosen(target), restriction }]
            if target.controller == TargetController::You
                && target.power == Some(PowerComparison::AtMost(2))
                && restriction.cant_be_blocked
    ));

    assert_eq!(
        face("reputable_merchant")
            .triggered_abilities
            .iter()
            .map(|ability| ability.trigger.clone())
            .collect::<Vec<_>>(),
        [
            TriggerCondition::WhenSelfEntersBattlefield,
            TriggerCondition::WhenSelfDies
        ]
    );
}
