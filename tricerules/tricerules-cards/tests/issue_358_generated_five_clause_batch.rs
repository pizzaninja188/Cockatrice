//! Registry and presentation conformance for issue #358.
//!
//! The five generated Standard identities must register with their complete typed payloads: CR
//! 614.1c/122.6/119.3 the opponent-life-loss conditional entry counter; CR 119.4/602.2b/602.5b
//! the pay-two-life once-per-turn self pump; CR 601.2f/700.4 the conditional {3} cost reduction;
//! CR 113.6b/602.2/701.13/121.2 the graveyard exile-self draw-then-lose-life; and CR
//! 603.6c/603.10a/700.4/115.1 the self-inclusive creature-or-artifact leaves-the-battlefield
//! drain.

use tricerules_cards::primitives::{
    ActivationLimit, ConditionPlayerSet, EffectSubject, EntersWithCountersAffected, EventZone,
    GameCondition, LifeAmount, LifeChangeKind, PermanentEventFilter, PlayerQuantifier,
    PlayerRecipient, StaticAbilityDef, TargetFilter, TargetKind, ZoneEventCardinality,
    ZoneEventDestination,
};
use tricerules_cards::{
    AbilityCost, AbilityPresentation, AbilitySourceZone, ActivationTiming, Amount, CardRegistry,
    CastTriggerPlayer, Color, CounterKind, Keyword, LibraryPartitionKind, ManaCost,
    PermanentTypeFilter, SpellCostModifier, SpellEffectKind, TriggerCondition,
};

#[test]
fn issue_358_registers_the_five_reviewed_identities() {
    let registry = CardRegistry::global();
    for (id, name, face_id) in [
        (
            "frilled_sparkshooter",
            "Frilled Sparkshooter",
            "frilled_sparkshooter",
        ),
        (
            "desolation_prowler",
            "Desolation Prowler",
            "desolation_prowler",
        ),
        (
            "dreaded_bat-cloud",
            "Dreaded Bat-Cloud",
            "dreaded_bat_cloud",
        ),
        (
            "faerie_dreamthief",
            "Faerie Dreamthief",
            "faerie_dreamthief",
        ),
        (
            "susurian_voidborn",
            "Susurian Voidborn",
            "susurian_voidborn",
        ),
    ] {
        let definition = registry
            .get(id)
            .unwrap_or_else(|| panic!("missing reviewed card {id}"));
        assert_eq!(definition.name, name);
        assert_eq!(registry.id_for_name(name), Some(id));
        assert_eq!(definition.layout, tricerules_cards::Layout::Normal);
        assert_eq!(definition.face_count(), 1);
        assert_eq!(definition.primary_face().face_id.as_str(), face_id);
    }
}

#[test]
fn issue_358_frilled_sparkshooter_keeps_keywords_and_opponent_life_counter() {
    let definition = CardRegistry::global()
        .get("frilled_sparkshooter")
        .expect("Frilled Sparkshooter");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{3}{R}");
    assert_eq!(face.types, ["Creature", "Lizard", "Archer"]);
    assert_eq!((face.power, face.toughness), (Some(3), Some(3)));
    assert_eq!(face.colors(), vec![Color::Red]);
    assert_eq!(face.keywords, [Keyword::Reach, Keyword::Menace]);
    let [ability] = face.static_abilities.as_slice() else {
        panic!("Frilled Sparkshooter must have exactly one static ability");
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
                condition: GameCondition::LifeChangedThisTurn {
                    players: ConditionPlayerSet::Relative(
                        tricerules_cards::RelativePlayerSet::Opponents
                    ),
                    change: LifeChangeKind::Loss,
                    quantifier: PlayerQuantifier::Any,
                },
                when_true: 1,
                otherwise: 0,
            },
            cast_cost_condition: None,
        }
    );
}

#[test]
fn issue_358_desolation_prowler_pays_two_life_once_per_turn_to_pump_itself() {
    let definition = CardRegistry::global()
        .get("desolation_prowler")
        .expect("Desolation Prowler");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{1}{B}");
    assert_eq!(face.types, ["Creature", "Wolf"]);
    assert_eq!((face.power, face.toughness), (Some(2), Some(2)));
    assert_eq!(face.colors(), vec![Color::Black]);
    let [ability] = face.activated_abilities.as_slice() else {
        panic!("Desolation Prowler must have exactly one activated ability");
    };
    assert_eq!(ability.source_zone, AbilitySourceZone::Battlefield);
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(ability.costs, [AbilityCost::PayLife { amount: 2 }]);
    assert_eq!(ability.timing, ActivationTiming::Normal);
    assert_eq!(
        ability.activation_limit,
        Some(ActivationLimit::PerTurn { max_activations: 1 })
    );
    assert!(ability.targeting.is_none());
    assert_eq!(
        ability.effect,
        [SpellEffectKind::PumpTarget {
            power: 2,
            toughness: 2,
            scale: None,
            subject: EffectSubject::Source,
        }]
    );
}

#[test]
fn issue_358_dreaded_bat_cloud_reduces_its_cost_after_a_creature_death() {
    let definition = CardRegistry::global()
        .get("dreaded_bat-cloud")
        .expect("Dreaded Bat-Cloud");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{4}{B}");
    assert_eq!(face.types, ["Creature", "Bat"]);
    assert_eq!((face.power, face.toughness), (Some(4), Some(2)));
    assert_eq!(face.colors(), vec![Color::Black]);
    assert_eq!(face.keywords, [Keyword::Flying, Keyword::Deathtouch]);
    assert_eq!(
        face.cost_modifiers,
        [SpellCostModifier::ConditionalGenericReduction {
            amount: 3,
            condition: GameCondition::CreatureDeathsThisTurn {
                min: Some(1),
                max: None,
            },
        }]
    );
}

#[test]
fn issue_358_faerie_dreamthief_surveils_then_exiles_itself_to_draw_and_lose() {
    let definition = CardRegistry::global()
        .get("faerie_dreamthief")
        .expect("Faerie Dreamthief");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{B}");
    assert_eq!(face.types, ["Creature", "Faerie", "Warlock"]);
    assert_eq!((face.power, face.toughness), (Some(1), Some(1)));
    assert_eq!(face.colors(), vec![Color::Black]);
    assert_eq!(face.keywords, [Keyword::Flying]);
    let [triggered] = face.triggered_abilities.as_slice() else {
        panic!("Faerie Dreamthief must have exactly one triggered ability");
    };
    assert_eq!(
        triggered.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(
        triggered.trigger,
        TriggerCondition::WhenSelfEntersBattlefield
    );
    assert_eq!(
        triggered.effect,
        [SpellEffectKind::LibraryPartition {
            count: 1,
            top_min: 0,
            top_max: None,
            kind: LibraryPartitionKind::Surveil,
        }]
    );
    let [ability] = face.activated_abilities.as_slice() else {
        panic!("Faerie Dreamthief must have exactly one activated ability");
    };
    assert_eq!(ability.source_zone, AbilitySourceZone::Graveyard);
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![3])
    );
    assert_eq!(
        ability.costs,
        [
            AbilityCost::Mana(ManaCost::parse("{2}{B}").expect("valid cost")),
            AbilityCost::ExileSelf,
        ]
    );
    assert!(ability.targeting.is_none());
    assert_eq!(
        ability.effect,
        [
            SpellEffectKind::Draw {
                who: PlayerRecipient::Controller,
                count: Amount::Fixed(1),
            },
            SpellEffectKind::LoseLife {
                amount: LifeAmount::Fixed(1),
                who: PlayerRecipient::Controller,
            },
        ]
    );
}

#[test]
fn issue_358_susurian_voidborn_drains_when_self_creature_or_artifact_dies() {
    let definition = CardRegistry::global()
        .get("susurian_voidborn")
        .expect("Susurian Voidborn");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{2}{B}");
    assert_eq!(face.types, ["Creature", "Vampire", "Soldier"]);
    assert_eq!((face.power, face.toughness), (Some(2), Some(2)));
    assert_eq!(face.colors(), vec![Color::Black]);
    assert_eq!(
        face.warp_cost.as_ref().map(ToString::to_string),
        Some("{B}".to_string())
    );
    let [ability] = face.triggered_abilities.as_slice() else {
        panic!("Susurian Voidborn must have exactly one triggered ability");
    };
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert!(!ability.may);
    assert_eq!(
        ability.trigger,
        TriggerCondition::WheneverPermanentLeavesBattlefield {
            controller: CastTriggerPlayer::Controller,
            filter: PermanentEventFilter {
                any_of: Some(vec![
                    PermanentEventFilter {
                        permanent_type: Some(PermanentTypeFilter::Creature),
                        ..PermanentEventFilter::default()
                    },
                    PermanentEventFilter {
                        permanent_type: Some(PermanentTypeFilter::Artifact),
                        ..PermanentEventFilter::default()
                    },
                ]),
                ..PermanentEventFilter::default()
            },
            destination: ZoneEventDestination::OneOf(vec![EventZone::Graveyard]),
            cardinality: ZoneEventCardinality::EachObject,
        }
    );
    assert_eq!(
        ability.effect,
        [
            SpellEffectKind::TargetPlayerLosesLife {
                amount: 1,
                target: TargetFilter {
                    kind: TargetKind::OpponentPlayer,
                    ..TargetFilter::default()
                },
            },
            SpellEffectKind::GainLife {
                amount: Amount::Fixed(1),
            },
        ]
    );
    let targeting = ability.targeting.as_ref().expect("one target group");
    let [group] = targeting.groups.as_slice() else {
        panic!("Susurian Voidborn must own exactly one target group");
    };
    assert_eq!((group.min, group.max), (1, 1));
    assert_eq!(group.effect_indices, [0]);
    assert_eq!(group.prompt, "Choose target opponent");
}

#[test]
fn issue_358_fingerprint_rows_match_the_presentation_registry() {
    let registry = CardRegistry::global();
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, face_id) in [
        ("frilled_sparkshooter", "frilled_sparkshooter"),
        ("desolation_prowler", "desolation_prowler"),
        ("dreaded_bat-cloud", "dreaded_bat_cloud"),
        ("faerie_dreamthief", "faerie_dreamthief"),
        ("susurian_voidborn", "susurian_voidborn"),
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
