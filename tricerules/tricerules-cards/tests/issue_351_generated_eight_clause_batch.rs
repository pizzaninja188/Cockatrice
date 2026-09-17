//! Registry and presentation conformance for issue #351.
//!
//! The eight generated Standard identities must register with their complete typed payloads:
//! CR 611.2a/514.2/702.17 the +2/+2 reach pump with untap; CR 611.2a/702.7 the +3/+0 reach and
//! first-strike pump; CR 508.1/603.2/119.3 the defending-player attack drain; CR 603.2/115.9b the
//! Repartee instant-or-sorcery cast trigger; CR 602.1/702.17 the activated reach grant; CR
//! 602.1/509.1b/208 the power-bounded unblockable activation; CR 603.6a/611.3 the source-excluding
//! mass -2/-2; and CR 604.1/611.3 the live control-an-artifact self modifier.

use tricerules_cards::primitives::{
    BattlefieldAggregate, BattlefieldPermanentFilter, CardTypeFilter, CombatRestriction,
    CombatRestrictionScope, CreatureScopeFilter, EffectSubject, GameCondition, LifeAmount,
    PlayerRecipient, PowerComparison, RelativePlayerSet, SpellCastFilter, StaticAbilityDef,
    TargetFilter, TargetKind, TypeLineAddition,
};
use tricerules_cards::{
    AbilityCost, AbilityPresentation, Amount, CardRegistry, CastTriggerPlayer, Color, CounterKind,
    Keyword, ManaCost, PermanentTypeFilter, SpellEffectKind, TriggerCondition,
};

#[test]
fn issue_351_registers_the_eight_reviewed_identities() {
    let registry = CardRegistry::global();
    for (id, name) in [
        ("pillar_launch", "Pillar Launch"),
        ("smaugs_fury", "Smaug's Fury"),
        ("agate-blade_assassin", "Agate-Blade Assassin"),
        ("lecturing_scornmage", "Lecturing Scornmage"),
        ("frog_butler", "Frog Butler"),
        ("ragged_playmate", "Ragged Playmate"),
        ("shefet_archfiend", "Shefet Archfiend"),
        ("gravblade_heavy", "Gravblade Heavy"),
    ] {
        let definition = registry
            .get(id)
            .unwrap_or_else(|| panic!("missing reviewed card {id}"));
        assert_eq!(definition.name, name);
        assert_eq!(registry.id_for_name(name), Some(id));
    }
}

#[test]
fn issue_351_pillar_launch_pumps_reach_and_untaps() {
    let definition = CardRegistry::global()
        .get("pillar_launch")
        .expect("Pillar Launch");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{G}");
    assert_eq!(face.types, ["Instant"]);
    assert_eq!(face.colors(), vec![Color::Green]);
    let chosen = EffectSubject::Chosen(Box::new(TargetFilter::default_creature()));
    assert_eq!(
        face.spell_effect,
        [
            SpellEffectKind::PumpTarget {
                power: 2,
                toughness: 2,
                scale: None,
                subject: chosen.clone(),
            },
            SpellEffectKind::GrantKeywords {
                subject: chosen.clone(),
                keywords: vec![Keyword::Reach],
            },
            SpellEffectKind::Untap { subject: chosen },
        ]
    );
    assert!(
        face.targeting.is_none(),
        "one shared mandatory creature target uses the implicit contract"
    );
}

#[test]
fn issue_351_smaugs_fury_pumps_reach_and_first_strike() {
    let definition = CardRegistry::global()
        .get("smaugs_fury")
        .expect("Smaug's Fury");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{1}{R}");
    assert_eq!(face.types, ["Instant"]);
    assert_eq!(face.colors(), vec![Color::Red]);
    let chosen = EffectSubject::Chosen(Box::new(TargetFilter::default_creature()));
    assert_eq!(
        face.spell_effect,
        [
            SpellEffectKind::PumpTarget {
                power: 3,
                toughness: 0,
                scale: None,
                subject: chosen.clone(),
            },
            SpellEffectKind::GrantKeywords {
                subject: chosen,
                keywords: vec![Keyword::Reach, Keyword::FirstStrike],
            },
        ]
    );
    assert!(face.targeting.is_none());
}

#[test]
fn issue_351_agate_blade_assassin_drains_the_defender() {
    let definition = CardRegistry::global()
        .get("agate-blade_assassin")
        .expect("Agate-Blade Assassin");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{1}{B}");
    assert_eq!(face.types, ["Creature", "Lizard", "Assassin"]);
    assert_eq!((face.power, face.toughness), (Some(1), Some(3)));
    assert_eq!(face.colors(), vec![Color::Black]);
    let [ability] = face.triggered_abilities.as_slice() else {
        panic!("Agate-Blade Assassin must have exactly one triggered ability");
    };
    assert_eq!(
        ability.trigger,
        TriggerCondition::WheneverSelfAttacks {
            minimum_other_attackers: 0
        }
    );
    assert!(!ability.may);
    assert!(ability.targeting.is_none());
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(
        ability.effect,
        [
            SpellEffectKind::LoseLife {
                amount: LifeAmount::Fixed(1),
                who: PlayerRecipient::DefendingPlayer,
            },
            SpellEffectKind::GainLife {
                amount: Amount::Fixed(1),
            },
        ]
    );
}

#[test]
fn issue_351_lecturing_scornmage_keeps_the_targeted_repartee() {
    let definition = CardRegistry::global()
        .get("lecturing_scornmage")
        .expect("Lecturing Scornmage");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{B}");
    assert_eq!(face.types, ["Creature", "Human", "Warlock"]);
    assert_eq!((face.power, face.toughness), (Some(1), Some(1)));
    assert_eq!(face.colors(), vec![Color::Black]);
    let [ability] = face.triggered_abilities.as_slice() else {
        panic!("Lecturing Scornmage must have exactly one triggered ability");
    };
    assert_eq!(
        ability.trigger,
        TriggerCondition::WheneverPlayerCastsSpell {
            caster: CastTriggerPlayer::Controller,
            filter: SpellCastFilter {
                card_type: Some(CardTypeFilter::InstantOrSorcery),
                targeted_permanent_type: Some(PermanentTypeFilter::Creature),
                ..SpellCastFilter::default()
            },
            ordinal: None,
            ordinal_scope: Default::default(),
        }
    );
    assert!(!ability.may);
    assert!(ability.targeting.is_none());
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(
        ability.effect,
        [SpellEffectKind::PutCounters {
            counter: CounterKind::PlusOnePlusOne,
            count: Amount::Fixed(1),
            subject: EffectSubject::Source,
        }]
    );
}

#[test]
fn issue_351_frog_butler_keeps_deathtouch_mana_and_reach() {
    let definition = CardRegistry::global()
        .get("frog_butler")
        .expect("Frog Butler");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{1}{G}");
    assert_eq!(face.types, ["Creature", "Frog", "Spirit"]);
    assert_eq!((face.power, face.toughness), (Some(1), Some(1)));
    assert_eq!(face.colors(), vec![Color::Green]);
    assert_eq!(face.keywords, [Keyword::Deathtouch]);
    let [mana, reach] = face.activated_abilities.as_slice() else {
        panic!("Frog Butler must keep its mana ability and gain the reach grant");
    };
    assert_eq!(mana.presentation, AbilityPresentation::OracleLines(vec![2]));
    assert!(mana
        .effect
        .iter()
        .any(|effect| matches!(effect, SpellEffectKind::ProduceMana { .. })));
    assert_eq!(
        reach.presentation,
        AbilityPresentation::OracleLines(vec![3])
    );
    assert_eq!(
        reach.costs,
        [AbilityCost::Mana(
            ManaCost::parse("{2}").expect("valid cost")
        )]
    );
    assert_eq!(
        reach.effect,
        [SpellEffectKind::GrantKeywords {
            subject: EffectSubject::Source,
            keywords: vec![Keyword::Reach],
        }]
    );
    assert!(reach.targeting.is_none());
}

#[test]
fn issue_351_ragged_playmate_activates_the_unblockable_grant() {
    let definition = CardRegistry::global()
        .get("ragged_playmate")
        .expect("Ragged Playmate");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{1}{R}");
    assert_eq!(face.types, ["Artifact", "Creature", "Toy"]);
    assert_eq!((face.power, face.toughness), (Some(2), Some(2)));
    assert_eq!(face.colors(), vec![Color::Red]);
    let [ability] = face.activated_abilities.as_slice() else {
        panic!("Ragged Playmate must have exactly one activated ability");
    };
    assert_eq!(
        ability.costs,
        [
            AbilityCost::Mana(ManaCost::parse("{1}").expect("valid cost")),
            AbilityCost::Tap,
        ]
    );
    assert_eq!(
        ability.effect,
        [SpellEffectKind::ApplyCombatRestriction {
            scope: CombatRestrictionScope::Chosen(TargetFilter {
                kind: TargetKind::Creature,
                power: Some(PowerComparison::AtMost(2)),
                ..TargetFilter::default()
            }),
            restriction: CombatRestriction {
                cant_be_blocked: true,
                ..CombatRestriction::default()
            },
        }]
    );
    let targeting = ability.targeting.as_ref().expect("one target group");
    let [group] = targeting.groups.as_slice() else {
        panic!("Ragged Playmate must own exactly one target group");
    };
    assert_eq!((group.min, group.max), (1, 1));
    assert_eq!(group.effect_indices, [0]);
    assert_eq!(group.prompt, "Choose target creature with power 2 or less");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
}

#[test]
fn issue_351_shefet_archfiend_keeps_flying_cycling_and_the_sweep() {
    let definition = CardRegistry::global()
        .get("shefet_archfiend")
        .expect("Shefet Archfiend");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{5}{B}{B}");
    assert_eq!(face.types, ["Creature", "Demon"]);
    assert_eq!((face.power, face.toughness), (Some(5), Some(5)));
    assert_eq!(face.colors(), vec![Color::Black]);
    assert_eq!(face.keywords, [Keyword::Flying]);
    let [cycling] = face.activated_abilities.as_slice() else {
        panic!("Shefet Archfiend must keep exactly the Cycling activation");
    };
    assert_eq!(
        cycling.costs,
        [
            AbilityCost::Mana(ManaCost::parse("{2}").expect("valid cost")),
            AbilityCost::DiscardSelf,
        ]
    );
    assert_eq!(
        cycling.presentation,
        AbilityPresentation::OracleLines(vec![3])
    );
    let [ability] = face.triggered_abilities.as_slice() else {
        panic!("Shefet Archfiend must have exactly one triggered ability");
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
        [SpellEffectKind::PumpAll {
            filter: CreatureScopeFilter {
                exclude_self: true,
                ..CreatureScopeFilter::default()
            },
            power: -2,
            toughness: -2,
        }]
    );
}

#[test]
fn issue_351_gravblade_heavy_keeps_the_artifact_condition() {
    let definition = CardRegistry::global()
        .get("gravblade_heavy")
        .expect("Gravblade Heavy");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{3}{B}");
    assert_eq!(face.types, ["Creature", "Human", "Soldier"]);
    assert_eq!((face.power, face.toughness), (Some(3), Some(4)));
    assert_eq!(face.colors(), vec![Color::Black]);
    assert!(face.keywords.is_empty());
    let [modifier] = face.static_abilities.as_slice() else {
        panic!("Gravblade Heavy must have exactly one static ability");
    };
    assert_eq!(
        modifier.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(
        modifier.definition,
        StaticAbilityDef::ConditionalSelfModifier {
            condition: GameCondition::BattlefieldAggregate {
                filter: BattlefieldPermanentFilter {
                    token: None,
                    any_of: None,
                    controllers: RelativePlayerSet::Controller,
                    card_type: Some(CardTypeFilter::Artifact),
                    color: None,
                    name: None,
                    required_subtypes: Vec::new(),
                    exclude_source: false,
                },
                aggregate: BattlefieldAggregate::Count,
                min: Some(1),
                max: None,
            },
            set_types: None,
            add_types: TypeLineAddition::default(),
            base_power: None,
            base_toughness: None,
            delta_power: 1,
            delta_toughness: 0,
            keywords: vec![Keyword::Deathtouch],
            activated_abilities: Vec::new(),
            triggered_abilities: Vec::new(),
            can_attack_as_though_without_defender: false,
        }
    );
}

#[test]
fn issue_351_fingerprint_rows_match_the_presentation_registry() {
    let registry = CardRegistry::global();
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, face_id) in [
        ("pillar_launch", "pillar_launch"),
        ("smaugs_fury", "smaug_s_fury"),
        ("agate-blade_assassin", "agate_blade_assassin"),
        ("lecturing_scornmage", "lecturing_scornmage"),
        ("frog_butler", "frog_butler"),
        ("ragged_playmate", "ragged_playmate"),
        ("shefet_archfiend", "shefet_archfiend"),
        ("gravblade_heavy", "gravblade_heavy"),
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
