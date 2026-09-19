//! Issue #427 — registry identities for the Firebending reminder batch.
//!
//! Oracle text verified against the pinned Scryfall `oracle_cards` snapshot
//! (`27bf3214-1271-490b-bdfe-c0be6c23d02e`, compressed SHA-256
//! `9bb0c7853625c76597fdee2f97d54d376feca71aa16645b7e38b214c74ef807a`, with the
//! canonical full-corpus refresh `9611b5d93b20478a0ee46bae8b20a9eb39ee980f0ef4f5f6f6aaa8f7ab010ab2`).
//! CR 702.189 defines Firebending N as a triggered ability that adds N red mana and retains it
//! until end of combat (the CR 106.4 exception); CR 508.1m governs the attack trigger, CR 603.6a
//! the entry trigger, CR 603.2 the sacrifice observer, CR 111.10f the Clue token, and CR 509.1b
//! the Crew activation.
//!
//! The four excluded cohort identities (Boiling Rock Rioter, Jeong Jeong, the Deserter, Azula,
//! Cunning Usurper, Fire Lord Azula) intentionally have no registry entry here; their unsupported
//! companion clauses stay fail-closed.

use tricerules_cards::primitives::{
    AbilityCost, AbilitySourceZone, ActivationTiming, Amount, CastTriggerPlayer,
    CreatureScopeController, CreatureScopeFilter, EffectSubject, LifeAmount,
    ObjectContributionKind, ObjectPaymentConstraint, PermanentEventFilter, PlayerRecipient,
    SpellEffectKind, TargetController, TargetFilter, TargetKind, TriggerCondition,
};
use tricerules_cards::{AbilityPresentation, CardFace, CardRegistry, CounterKind, ManaCost};

fn registered_face(card_id: &str) -> CardFace {
    CardRegistry::global()
        .get(card_id)
        .unwrap_or_else(|| panic!("{card_id} must be registered"))
        .primary_face()
        .clone()
}

fn firebending_ability(
    face: &CardFace,
    red: u32,
    oracle_line: u16,
) -> &tricerules_cards::TriggeredAbilityDef {
    let firebending = face
        .triggered_abilities
        .iter()
        .filter(|ability| {
            matches!(
                ability.effect.as_slice(),
                [SpellEffectKind::AddMana {
                    amount,
                    retention: tricerules_cards::primitives::ManaRetention::EndOfCombat,
                }] if amount.r == red
            )
        })
        .collect::<Vec<_>>();
    let [ability] = firebending.as_slice() else {
        panic!(
            "{} must have exactly one firebending {red} trigger",
            face.name
        );
    };
    assert_eq!(ability.ability_id.as_str(), "triggered_01");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![oracle_line])
    );
    assert_eq!(
        ability.trigger,
        TriggerCondition::WheneverSelfAttacks {
            minimum_other_attackers: 0,
        }
    );
    assert!(ability.targeting.is_none());
    assert!(!ability.may);
    assert!(ability.intervening_if.is_none());
    ability
}

#[test]
fn issue_427_fire_sages_registers_the_keyword_and_printed_activation() {
    let face = registered_face("fire_sages");
    assert_eq!(face.face_id.as_str(), "fire_sages");
    assert_eq!(face.name, "Fire Sages");
    assert_eq!(
        face.mana_cost,
        ManaCost::parse("{1}{R}").expect("printed cost")
    );
    assert_eq!(face.types, ["Creature", "Human", "Cleric"]);
    assert_eq!((face.power, face.toughness), (Some(2), Some(2)));
    assert_eq!(face.triggered_abilities.len(), 1);
    assert_eq!(face.activated_abilities.len(), 1);

    let firebending = firebending_ability(&face, 1, 1);
    assert_eq!(firebending.ability_id.as_str(), "triggered_01");

    let [activation] = face.activated_abilities.as_slice() else {
        panic!("Fire Sages must have one activated ability");
    };
    assert_eq!(activation.ability_id.as_str(), "activated_01");
    assert_eq!(
        activation.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(activation.source_zone, AbilitySourceZone::Battlefield);
    assert_eq!(
        activation.costs,
        [AbilityCost::Mana(
            ManaCost::parse("{1}{R}{R}").expect("printed cost")
        )]
    );
    assert_eq!(
        activation.effect,
        [SpellEffectKind::PutCounters {
            counter: CounterKind::PlusOnePlusOne,
            count: Amount::Fixed(1),
            subject: EffectSubject::Source,
        }]
    );
    assert!(activation.targeting.is_none());
    assert_eq!(activation.timing, ActivationTiming::Normal);
}

#[test]
fn issue_427_tundra_tank_registers_crew_etb_and_the_keyword() {
    let face = registered_face("tundra_tank");
    assert_eq!(face.face_id.as_str(), "tundra_tank");
    assert_eq!(face.name, "Tundra Tank");
    assert_eq!(
        face.mana_cost,
        ManaCost::parse("{2}{B}").expect("printed cost")
    );
    assert_eq!(face.types, ["Artifact", "Vehicle"]);
    assert_eq!((face.power, face.toughness), (Some(4), Some(4)));
    assert_eq!(face.triggered_abilities.len(), 2);
    assert_eq!(face.activated_abilities.len(), 1);

    firebending_ability(&face, 1, 1);

    let [crew] = face.activated_abilities.as_slice() else {
        panic!("Tundra Tank must have one Crew ability");
    };
    assert_eq!(crew.ability_id.as_str(), "activated_01");
    assert_eq!(crew.presentation, AbilityPresentation::OracleLines(vec![3]));
    assert_eq!(
        crew.costs,
        [AbilityCost::TapPermanents {
            constraint: ObjectPaymentConstraint::AggregateMinimum {
                minimum: 1,
                contribution: ObjectContributionKind::CurrentPower,
            },
            filter: TargetFilter {
                kind: TargetKind::Creature,
                controller: TargetController::You,
                ..TargetFilter::default()
            },
            exclude_source: true,
        }]
    );

    let etb = face
        .triggered_abilities
        .iter()
        .find(|ability| ability.trigger == TriggerCondition::WhenSelfEntersBattlefield)
        .expect("Tundra Tank must have the entry trigger");
    assert_eq!(etb.ability_id.as_str(), "triggered_02");
    assert_eq!(etb.presentation, AbilityPresentation::OracleLines(vec![2]));
    assert_eq!(
        etb.effect,
        [SpellEffectKind::GrantKeywords {
            subject: EffectSubject::Chosen(Box::new(TargetFilter {
                kind: TargetKind::Creature,
                controller: TargetController::You,
                ..TargetFilter::default()
            })),
            keywords: vec![tricerules_cards::Keyword::Indestructible],
        }]
    );
    let targeting = etb.targeting.as_ref().expect("targeted ETB");
    let [group] = targeting.groups.as_slice() else {
        panic!("the ETB must publish one target group");
    };
    assert_eq!((group.min, group.max), (1, 1));
    assert_eq!(group.prompt, "Choose target creature you control");
    assert_eq!(group.effect_indices, [0]);
}

#[test]
fn issue_427_azula_on_the_hunt_registers_both_attack_triggers() {
    let face = registered_face("azula,_on_the_hunt");
    assert_eq!(face.face_id.as_str(), "azula_on_the_hunt");
    assert_eq!(face.name, "Azula, On the Hunt");
    assert_eq!(
        face.mana_cost,
        ManaCost::parse("{3}{B}").expect("printed cost")
    );
    assert_eq!(face.supertypes, ["Legendary"]);
    assert_eq!(face.types, ["Creature", "Human", "Noble"]);
    assert_eq!((face.power, face.toughness), (Some(4), Some(3)));
    assert_eq!(face.triggered_abilities.len(), 2);

    firebending_ability(&face, 2, 1);

    let attack = face
        .triggered_abilities
        .iter()
        .find(|ability| {
            matches!(
                ability.effect.as_slice(),
                [
                    SpellEffectKind::LoseLife { .. },
                    SpellEffectKind::CreateTokens { .. }
                ]
            )
        })
        .expect("Azula must have the life-loss plus Clue trigger");
    assert_eq!(attack.ability_id.as_str(), "triggered_02");
    assert_eq!(
        attack.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(
        attack.trigger,
        TriggerCondition::WheneverSelfAttacks {
            minimum_other_attackers: 0,
        }
    );
    assert_eq!(
        attack.effect,
        [
            SpellEffectKind::LoseLife {
                amount: LifeAmount::Fixed(1),
                who: PlayerRecipient::Controller,
            },
            SpellEffectKind::CreateTokens {
                token: "clue".into(),
                count: Amount::Fixed(1),
                who: PlayerRecipient::Controller,
                tapped: false,
                sacrifice_timing: None,
            },
        ]
    );
    assert!(attack.targeting.is_none());
    assert!(!attack.may);
}

#[test]
fn issue_427_zhao_registers_the_sacrifice_pump_and_the_keyword() {
    let face = registered_face("zhao,_ruthless_admiral");
    assert_eq!(face.face_id.as_str(), "zhao_ruthless_admiral");
    assert_eq!(face.name, "Zhao, Ruthless Admiral");
    assert_eq!(
        face.mana_cost,
        ManaCost::parse("{2}{B/R}{B/R}").expect("printed cost")
    );
    assert_eq!(face.supertypes, ["Legendary"]);
    assert_eq!(face.types, ["Creature", "Human", "Soldier"]);
    assert_eq!((face.power, face.toughness), (Some(3), Some(4)));
    assert_eq!(face.triggered_abilities.len(), 2);

    firebending_ability(&face, 2, 1);

    let sacrifice = face
        .triggered_abilities
        .iter()
        .find(|ability| matches!(ability.effect.as_slice(), [SpellEffectKind::PumpAll { .. }]))
        .expect("Zhao must have the sacrifice pump trigger");
    assert_eq!(sacrifice.ability_id.as_str(), "triggered_02");
    assert_eq!(
        sacrifice.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(
        sacrifice.trigger,
        TriggerCondition::WheneverPlayerSacrificesPermanent {
            player: CastTriggerPlayer::Controller,
            filter: PermanentEventFilter {
                exclude_source: true,
                ..PermanentEventFilter::default()
            },
        }
    );
    assert_eq!(
        sacrifice.effect,
        [SpellEffectKind::PumpAll {
            filter: CreatureScopeFilter {
                controller: Some(CreatureScopeController::YouControl),
                ..CreatureScopeFilter::default()
            },
            power: 1,
            toughness: 0,
        }]
    );
    assert!(sacrifice.targeting.is_none());
    assert!(!sacrifice.may);
}

#[test]
fn issue_427_excluded_cohort_identities_stay_unregistered() {
    for card_id in [
        "boiling_rock_rioter",
        "jeong_jeong,_the_deserter",
        "azula,_cunning_usurper",
        "fire_lord_azula",
    ] {
        assert!(
            CardRegistry::global().get(card_id).is_none(),
            "{card_id} must stay unregistered while a companion clause is unsupported"
        );
    }
}
