//! Registry and presentation conformance for issue #352.
//!
//! The eight generated Standard identities must register with their complete typed payloads:
//! CR 611.2a/514.2/702.7 the +2/+2 first-strike pump; CR 701.8/115.1 the creature-or-Vehicle
//! destroy union; CR 602.2/509.1b the mana activation making a creature unblockable; CR
//! 508.1/603.2/119.3 the self-attack lifegain; CR 602.2/701.9/701.21 the discard-and-sacrifice
//! draw; CR 602.2/701.9/702.12/701.26 the discard-to-grant-indestructible-and-tap; CR
//! 603.6a/611.2a the Alliance source-excluding pump; and CR 614.1c/122.6 the conditional entry
//! counter.

use tricerules_cards::primitives::{
    CombatRestriction, CombatRestrictionScope, EffectSubject, EntersWithCountersAffected,
    GameCondition, PermanentEventFilter, StaticAbilityDef, TargetFilter, TargetKind,
};
use tricerules_cards::{
    AbilityCost, AbilityPresentation, Amount, CardRegistry, CastTriggerPlayer, Color, CounterKind,
    Keyword, ManaCost, PermanentTypeFilter, SpellEffectKind, TriggerCondition,
};

#[test]
fn issue_352_registers_the_eight_reviewed_identities() {
    let registry = CardRegistry::global();
    for (id, name) in [
        ("interjection", "Interjection"),
        ("spin_out", "Spin Out"),
        ("elvenkings_harper", "Elvenking's Harper"),
        ("moonrise_cleric", "Moonrise Cleric"),
        ("masked_meower", "Masked Meower"),
        ("iron-shield_elf", "Iron-Shield Elf"),
        ("east_wind_avatar", "East Wind Avatar"),
        ("cackling_slasher", "Cackling Slasher"),
    ] {
        let definition = registry
            .get(id)
            .unwrap_or_else(|| panic!("missing reviewed card {id}"));
        assert_eq!(definition.name, name);
        assert_eq!(registry.id_for_name(name), Some(id));
    }
}

#[test]
fn issue_352_interjection_pumps_two_two_and_first_strike() {
    let definition = CardRegistry::global()
        .get("interjection")
        .expect("Interjection");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{W}");
    assert_eq!(face.types, ["Instant"]);
    assert_eq!(face.colors(), vec![Color::White]);
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
                subject: chosen,
                keywords: vec![Keyword::FirstStrike],
            },
        ]
    );
    assert!(
        face.targeting.is_none(),
        "one shared mandatory creature target uses the implicit contract"
    );
}

#[test]
fn issue_352_spin_out_destroys_a_creature_or_vehicle() {
    let definition = CardRegistry::global().get("spin_out").expect("Spin Out");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{1}{B}{B}");
    assert_eq!(face.types, ["Instant"]);
    assert_eq!(face.colors(), vec![Color::Black]);
    assert_eq!(
        face.spell_effect,
        [SpellEffectKind::Destroy {
            subject: EffectSubject::Chosen(Box::new(TargetFilter {
                any_of: Some(vec![
                    TargetFilter {
                        kind: TargetKind::Creature,
                        ..TargetFilter::default()
                    },
                    TargetFilter {
                        kind: TargetKind::AnyPermanent,
                        required_subtypes: vec!["Vehicle".into()],
                        ..TargetFilter::default()
                    },
                ]),
                ..TargetFilter::default()
            })),
        }]
    );
    assert!(face.targeting.is_none());
}

#[test]
fn issue_352_elvenkings_harper_makes_a_creature_unblockable() {
    let definition = CardRegistry::global()
        .get("elvenkings_harper")
        .expect("Elvenking's Harper");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{1}{U}");
    assert_eq!(face.types, ["Creature", "Elf", "Bard"]);
    assert_eq!((face.power, face.toughness), (Some(2), Some(2)));
    assert_eq!(face.colors(), vec![Color::Blue]);
    let [ability] = face.activated_abilities.as_slice() else {
        panic!("Elvenking's Harper must have exactly one activated ability");
    };
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(
        ability.costs,
        [AbilityCost::Mana(
            ManaCost::parse("{4}{U}").expect("valid cost")
        )]
    );
    assert_eq!(
        ability.effect,
        [SpellEffectKind::ApplyCombatRestriction {
            scope: CombatRestrictionScope::Chosen(TargetFilter {
                kind: TargetKind::Creature,
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
        panic!("Elvenking's Harper must own exactly one target group");
    };
    assert_eq!((group.min, group.max), (1, 1));
    assert_eq!(group.effect_indices, [0]);
    assert_eq!(group.prompt, "Choose target creature");
}

#[test]
fn issue_352_moonrise_cleric_keeps_flying_and_gains_one_on_attack() {
    let definition = CardRegistry::global()
        .get("moonrise_cleric")
        .expect("Moonrise Cleric");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{1}{W/B}{W/B}");
    assert_eq!(face.types, ["Creature", "Bat", "Cleric"]);
    assert_eq!((face.power, face.toughness), (Some(2), Some(3)));
    assert_eq!(face.colors(), vec![Color::White, Color::Black]);
    assert_eq!(face.keywords, [Keyword::Flying]);
    let [ability] = face.triggered_abilities.as_slice() else {
        panic!("Moonrise Cleric must have exactly one triggered ability");
    };
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(
        ability.trigger,
        TriggerCondition::WheneverSelfAttacks {
            minimum_other_attackers: 0
        }
    );
    assert!(!ability.may);
    assert!(ability.targeting.is_none());
    assert_eq!(
        ability.effect,
        [SpellEffectKind::GainLife {
            amount: Amount::Fixed(1),
        }]
    );
}

#[test]
fn issue_352_masked_meower_keeps_haste_and_the_discard_sacrifice_loot() {
    let definition = CardRegistry::global()
        .get("masked_meower")
        .expect("Masked Meower");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{R}");
    assert_eq!(face.types, ["Creature", "Spider", "Cat", "Hero"]);
    assert_eq!((face.power, face.toughness), (Some(1), Some(1)));
    assert_eq!(face.colors(), vec![Color::Red]);
    assert_eq!(face.keywords, [Keyword::Haste]);
    let [ability] = face.activated_abilities.as_slice() else {
        panic!("Masked Meower must have exactly one activated ability");
    };
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(
        ability.costs,
        [AbilityCost::Discard, AbilityCost::SacrificeSelf]
    );
    assert_eq!(
        ability.effect,
        [SpellEffectKind::Draw {
            who: tricerules_cards::primitives::PlayerRecipient::Controller,
            count: Amount::Fixed(1),
        }]
    );
    assert!(ability.targeting.is_none());
}

#[test]
fn issue_352_iron_shield_elf_discards_for_indestructible_and_taps() {
    let definition = CardRegistry::global()
        .get("iron-shield_elf")
        .expect("Iron-Shield Elf");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{1}{B}");
    assert_eq!(face.types, ["Creature", "Elf", "Warrior"]);
    assert_eq!((face.power, face.toughness), (Some(3), Some(1)));
    assert_eq!(face.colors(), vec![Color::Black]);
    assert!(face.keywords.is_empty());
    let [ability] = face.activated_abilities.as_slice() else {
        panic!("Iron-Shield Elf must have exactly one activated ability");
    };
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(ability.costs, [AbilityCost::Discard]);
    assert_eq!(
        ability.effect,
        [
            SpellEffectKind::GrantKeywords {
                subject: EffectSubject::Source,
                keywords: vec![Keyword::Indestructible],
            },
            SpellEffectKind::Tap {
                subject: EffectSubject::Source,
            },
        ]
    );
    assert!(ability.targeting.is_none());
}

#[test]
fn issue_352_east_wind_avatar_keeps_keywords_and_alliance_pump() {
    let definition = CardRegistry::global()
        .get("east_wind_avatar")
        .expect("East Wind Avatar");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{3}{W}");
    assert_eq!(face.types, ["Creature", "Bird", "Spirit", "Avatar"]);
    assert_eq!((face.power, face.toughness), (Some(2), Some(4)));
    assert_eq!(face.colors(), vec![Color::White]);
    assert_eq!(face.keywords, [Keyword::Flying, Keyword::Vigilance]);
    let [ability] = face.triggered_abilities.as_slice() else {
        panic!("East Wind Avatar must have exactly one triggered ability");
    };
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(
        ability.trigger,
        TriggerCondition::WheneverPermanentEntersBattlefield {
            controller: CastTriggerPlayer::Controller,
            filter: PermanentEventFilter {
                permanent_type: Some(PermanentTypeFilter::Creature),
                exclude_source: true,
                ..PermanentEventFilter::default()
            },
            creature_filter: None,
        }
    );
    assert!(!ability.may);
    assert!(ability.targeting.is_none());
    assert_eq!(
        ability.effect,
        [SpellEffectKind::PumpTarget {
            power: 1,
            toughness: 0,
            scale: None,
            subject: EffectSubject::Source,
        }]
    );
}

#[test]
fn issue_352_cackling_slasher_keeps_deathtouch_and_the_conditional_entry_counter() {
    let definition = CardRegistry::global()
        .get("cackling_slasher")
        .expect("Cackling Slasher");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{3}{B}");
    assert_eq!(face.types, ["Creature", "Human", "Assassin"]);
    assert_eq!((face.power, face.toughness), (Some(3), Some(3)));
    assert_eq!(face.colors(), vec![Color::Black]);
    assert_eq!(face.keywords, [Keyword::Deathtouch]);
    let [ability] = face.static_abilities.as_slice() else {
        panic!("Cackling Slasher must have exactly one static ability");
    };
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(
        ability.definition,
        StaticAbilityDef::EntersWithCounters {
            affected: EntersWithCountersAffected::Self_,
            counter: CounterKind::PlusOnePlusOne,
            amount: Amount::Conditional {
                condition: GameCondition::CreatureDeathsThisTurn {
                    min: Some(1),
                    max: None,
                },
                when_true: 1,
                otherwise: 0,
            },
            cast_cost_condition: None,
        }
    );
}

#[test]
fn issue_352_fingerprint_rows_match_the_presentation_registry() {
    let registry = CardRegistry::global();
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, face_id) in [
        ("interjection", "interjection"),
        ("spin_out", "spin_out"),
        ("elvenkings_harper", "elvenking_s_harper"),
        ("moonrise_cleric", "moonrise_cleric"),
        ("masked_meower", "masked_meower"),
        ("iron-shield_elf", "iron_shield_elf"),
        ("east_wind_avatar", "east_wind_avatar"),
        ("cackling_slasher", "cackling_slasher"),
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
