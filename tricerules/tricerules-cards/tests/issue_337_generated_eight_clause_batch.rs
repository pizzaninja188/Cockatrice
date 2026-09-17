//! Registry and presentation conformance for issue #337.
//!
//! The eight generated Standard identities must register with their complete typed payloads:
//! CR 611.2a/514.2/701.26 the +1/+3 flying pump with untap; CR 601.2f/118.7a the {2}
//! attacking-target reduction; CR 603.6 the gain-one-then-draw ETB ordering; CR 702.2b the
//! source-excluding deathtouch grant; CR 603.2/701.22a the self-becomes-tapped scry; CR 111.10a
//! the two-Treasure ETB; CR 301.5/701.3 the Equipment entry attach plus untap alongside Flash,
//! +1/+2, and Equip {2}; and CR 610.3 the linked tapped-creature exile alongside Flash.

use tricerules_cards::primitives::{
    CombatRole, EffectSubject, PlayerRecipient, SpellCostModifier, StaticAbilityDef,
    TargetController, TargetFilter, TargetKind, TargetMatchFilter, TargetObjectExclusion,
};
use tricerules_cards::{
    AbilityPresentation, Amount, CardRegistry, Color, Keyword, SpellEffectKind, TriggerCondition,
};

#[test]
fn issue_337_registers_the_eight_reviewed_identities() {
    let registry = CardRegistry::global();
    for (id, name) in [
        ("acrobatic_leap", "Acrobatic Leap"),
        ("attentive_sunscribe", "Attentive Sunscribe"),
        ("blooming_stinger", "Blooming Stinger"),
        ("depower", "Depower"),
        ("inspiring_overseer", "Inspiring Overseer"),
        ("rapacious_dragon", "Rapacious Dragon"),
        ("super_suit", "Super Suit"),
        ("super_villain_lockup", "Super Villain Lockup"),
    ] {
        let definition = registry
            .get(id)
            .unwrap_or_else(|| panic!("missing reviewed card {id}"));
        assert_eq!(definition.name, name);
        assert_eq!(registry.id_for_name(name), Some(id));
    }
}

#[test]
fn issue_337_acrobatic_leap_pumps_grants_flying_and_untaps_one_creature() {
    let definition = CardRegistry::global()
        .get("acrobatic_leap")
        .expect("Acrobatic Leap");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{W}");
    assert_eq!(face.types, ["Instant"]);
    assert_eq!(face.colors(), vec![Color::White]);
    let chosen = EffectSubject::Chosen(Box::new(TargetFilter::default_creature()));
    assert_eq!(
        face.spell_effect,
        [
            SpellEffectKind::PumpTarget {
                power: 1,
                toughness: 3,
                scale: None,
                subject: chosen.clone(),
            },
            SpellEffectKind::GrantKeywords {
                subject: chosen.clone(),
                keywords: vec![Keyword::Flying],
            },
            SpellEffectKind::Untap { subject: chosen },
        ]
    );
    assert!(
        face.targeting.is_none(),
        "the single targeted instruction uses the implicit one-target contract"
    );
}

#[test]
fn issue_337_depower_reduces_two_against_an_attacking_target() {
    let definition = CardRegistry::global().get("depower").expect("Depower");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{2}{U}");
    assert_eq!(face.types, ["Instant"]);
    assert_eq!(face.colors(), vec![Color::Blue]);
    assert_eq!(
        face.cost_modifiers,
        [SpellCostModifier::TargetMatchGenericReduction {
            amount: 2,
            filter: TargetMatchFilter::Battlefield(TargetFilter {
                kind: TargetKind::Creature,
                combat_role: Some(CombatRole::Attacking),
                ..TargetFilter::default()
            }),
        }]
    );
    assert_eq!(
        face.spell_effect,
        [
            SpellEffectKind::PumpTarget {
                power: -4,
                toughness: 0,
                scale: None,
                subject: EffectSubject::Chosen(Box::new(TargetFilter::default_creature())),
            },
            SpellEffectKind::Draw {
                who: PlayerRecipient::Controller,
                count: Amount::Fixed(1),
            },
        ]
    );
}

#[test]
fn issue_337_inspiring_overseer_gains_one_then_draws_with_flying() {
    let definition = CardRegistry::global()
        .get("inspiring_overseer")
        .expect("Inspiring Overseer");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{2}{W}");
    assert_eq!(face.types, ["Creature", "Angel", "Cleric"]);
    assert_eq!((face.power, face.toughness), (Some(2), Some(1)));
    assert_eq!(face.colors(), vec![Color::White]);
    assert_eq!(face.keywords, [Keyword::Flying]);
    let [ability] = face.triggered_abilities.as_slice() else {
        panic!("Inspiring Overseer must have exactly one triggered ability");
    };
    assert_eq!(ability.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert!(!ability.may);
    assert!(ability.targeting.is_none());
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(
        ability.effect,
        [
            SpellEffectKind::GainLife {
                amount: Amount::Fixed(1),
            },
            SpellEffectKind::Draw {
                who: PlayerRecipient::Controller,
                count: Amount::Fixed(1),
            },
        ],
        "CR 603.6: the generated order is life then draw"
    );
}

#[test]
fn issue_337_blooming_stinger_grants_deathtouch_to_another_creature() {
    let definition = CardRegistry::global()
        .get("blooming_stinger")
        .expect("Blooming Stinger");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{1}{G}");
    assert_eq!(face.types, ["Creature", "Plant", "Scorpion"]);
    assert_eq!((face.power, face.toughness), (Some(2), Some(2)));
    assert_eq!(face.colors(), vec![Color::Green]);
    assert_eq!(face.keywords, [Keyword::Deathtouch]);
    let [ability] = face.triggered_abilities.as_slice() else {
        panic!("Blooming Stinger must have exactly one triggered ability");
    };
    assert_eq!(ability.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert!(!ability.may);
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(
        ability.effect,
        [SpellEffectKind::GrantKeywords {
            subject: EffectSubject::Chosen(Box::new(TargetFilter {
                kind: TargetKind::Creature,
                controller: TargetController::You,
                excluded_objects: vec![TargetObjectExclusion::Source],
                ..TargetFilter::default()
            })),
            keywords: vec![Keyword::Deathtouch],
        }]
    );
    let targeting = ability.targeting.as_ref().expect("deathtouch target group");
    let [group] = targeting.groups.as_slice() else {
        panic!("Blooming Stinger must own exactly one target group");
    };
    assert_eq!((group.min, group.max), (1, 1));
    assert_eq!(group.prompt, "Choose another target creature you control");
}

#[test]
fn issue_337_attentive_sunscribe_scries_on_becoming_tapped() {
    let definition = CardRegistry::global()
        .get("attentive_sunscribe")
        .expect("Attentive Sunscribe");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{1}{W}");
    assert_eq!(face.types, ["Artifact", "Creature", "Gnome"]);
    assert_eq!((face.power, face.toughness), (Some(2), Some(2)));
    let [ability] = face.triggered_abilities.as_slice() else {
        panic!("Attentive Sunscribe must have exactly one triggered ability");
    };
    assert_eq!(ability.trigger, TriggerCondition::WheneverSelfBecomesTapped);
    assert!(!ability.may);
    assert!(ability.targeting.is_none());
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(
        ability.effect,
        [SpellEffectKind::Scry {
            count: Amount::Fixed(1),
        }]
    );
}

#[test]
fn issue_337_rapacious_dragon_keeps_flying_and_makes_two_treasures() {
    let definition = CardRegistry::global()
        .get("rapacious_dragon")
        .expect("Rapacious Dragon");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{4}{R}");
    assert_eq!(face.types, ["Creature", "Dragon"]);
    assert_eq!((face.power, face.toughness), (Some(3), Some(3)));
    assert_eq!(face.colors(), vec![Color::Red]);
    assert_eq!(face.keywords, [Keyword::Flying]);
    let [ability] = face.triggered_abilities.as_slice() else {
        panic!("Rapacious Dragon must have exactly one triggered ability");
    };
    assert_eq!(ability.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert!(!ability.may);
    assert!(ability.targeting.is_none());
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(
        ability.effect,
        [SpellEffectKind::CreateTokens {
            token: "treasure".into(),
            count: Amount::Fixed(2),
            who: PlayerRecipient::Controller,
            tapped: false,
            sacrifice_timing: None,
        }]
    );
}

#[test]
fn issue_337_super_suit_keeps_flash_modifier_equip_and_attach_untap() {
    let definition = CardRegistry::global()
        .get("super_suit")
        .expect("Super Suit");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{1}{U}");
    assert_eq!(face.types, ["Artifact", "Equipment"]);
    assert_eq!(face.colors(), vec![Color::Blue]);
    assert_eq!(face.keywords, [Keyword::Flash]);

    let [trigger] = face.triggered_abilities.as_slice() else {
        panic!("Super Suit must have exactly one ETB trigger");
    };
    assert_eq!(trigger.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert!(!trigger.may);
    assert_eq!(
        trigger.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(
        trigger.effect,
        [
            SpellEffectKind::AttachSource {
                target: TargetFilter {
                    kind: TargetKind::Creature,
                    controller: TargetController::You,
                    ..TargetFilter::default()
                },
            },
            SpellEffectKind::Untap {
                subject: EffectSubject::Chosen(Box::new(TargetFilter {
                    kind: TargetKind::Creature,
                    controller: TargetController::You,
                    ..TargetFilter::default()
                })),
            },
        ]
    );
    let targeting = trigger.targeting.as_ref().expect("attach target group");
    let [group] = targeting.groups.as_slice() else {
        panic!("Super Suit must own exactly one target group");
    };
    assert_eq!((group.min, group.max), (1, 1));
    assert_eq!(group.effect_indices, [0, 1]);

    let [static_ability] = face.static_abilities.as_slice() else {
        panic!("Super Suit must keep exactly one attached modifier");
    };
    assert_eq!(
        static_ability.presentation,
        AbilityPresentation::OracleLines(vec![3])
    );
    assert!(matches!(
        static_ability.definition,
        StaticAbilityDef::AttachedModifier {
            delta_power: 1,
            delta_toughness: 2,
            ..
        }
    ));

    let [equip] = face.activated_abilities.as_slice() else {
        panic!("Super Suit must keep exactly one Equip activation");
    };
    assert_eq!(
        equip.presentation,
        AbilityPresentation::OracleLines(vec![4])
    );
    assert_eq!(
        equip.effect,
        [SpellEffectKind::Equip {
            target: TargetFilter {
                kind: TargetKind::Creature,
                controller: TargetController::You,
                ..TargetFilter::default()
            },
        }]
    );
}

#[test]
fn issue_337_super_villain_lockup_linked_exiles_a_tapped_creature() {
    let definition = CardRegistry::global()
        .get("super_villain_lockup")
        .expect("Super Villain Lockup");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{1}{W}");
    assert_eq!(face.types, ["Enchantment"]);
    assert_eq!(face.colors(), vec![Color::White]);
    assert_eq!(face.keywords, [Keyword::Flash]);
    let [ability] = face.triggered_abilities.as_slice() else {
        panic!("Super Villain Lockup must have exactly one triggered ability");
    };
    assert_eq!(ability.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert!(!ability.may);
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(
        ability.effect,
        [SpellEffectKind::ExileUntilSourceLeaves {
            target: TargetFilter {
                kind: TargetKind::Creature,
                controller: TargetController::Opponent,
                tapped: Some(true),
                ..TargetFilter::default()
            },
        }]
    );
    let targeting = ability.targeting.as_ref().expect("exile target group");
    let [group] = targeting.groups.as_slice() else {
        panic!("Super Villain Lockup must own exactly one target group");
    };
    assert_eq!((group.min, group.max), (1, 1));
    assert_eq!(
        group.prompt,
        "Choose target tapped creature an opponent controls"
    );
}

#[test]
fn issue_337_fingerprint_rows_match_the_presentation_registry() {
    let registry = CardRegistry::global();
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, face_id) in [
        ("acrobatic_leap", "acrobatic_leap"),
        ("attentive_sunscribe", "attentive_sunscribe"),
        ("blooming_stinger", "blooming_stinger"),
        ("depower", "depower"),
        ("inspiring_overseer", "inspiring_overseer"),
        ("rapacious_dragon", "rapacious_dragon"),
        ("super_suit", "super_suit"),
        ("super_villain_lockup", "super_villain_lockup"),
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
