//! Issue #338 registry and presentation conformance for the reviewed additional-cost cohort.
//!
//! Every identity prints exactly one `As an additional cost to cast this spell, ...` instruction
//! in the pinned Scryfall snapshot `27bf3214-1271-490b-bdfe-c0be6c23d02e`; exact-name records and
//! `rulings_uri` responses were fetched 2026-09-20 into `build/campaign/scryfall-batch338/`. The
//! returned rulings were reviewed and do not change these assertions (Bogslither's Embrace:
//! all -1/-1 counters on one creature and no blight without a creature; Corrupted Conviction:
//! exactly one creature; Guardian of the Great Door: any four untapped artifacts/creatures/lands;
//! Kinsbaile Aspirant and Silvergill Mentor: a revealed card stays revealed until the spell
//! leaves the stack; Laughing Mad: a flashback cast still pays additional costs). The expectations
//! below are the reviewed printed Oracle behavior and the shipped cast-cost vocabulary, not a copy
//! of generator output. CR 601.2b (announced additional costs), 701.4 (behold), 701.30 (blight),
//! 701.16 (discard), and 602.5 (announced cost receipts) govern the asserted shapes.

mod common;

use common::FaceExpectation;
use tricerules_cards::primitives::{
    CastCostGroupDef, CastCostOptionDef, ManaCostChoiceKind, ObjectCastCostKind,
    ObjectPaymentConstraint, PermanentTypeFilter, TargetController, TargetFilter, TargetKind,
    ZoneCardFilter,
};
use tricerules_cards::{AbilityPresentation, CardFace, CardRegistry, ChoiceId, Keyword, ManaCost};

fn face(id: &str) -> &'static CardFace {
    CardRegistry::global()
        .get(id)
        .unwrap_or_else(|| panic!("missing reviewed card {id}"))
        .primary_face()
}

fn group(id: &str) -> &'static CastCostGroupDef {
    let face = face(id);
    let [group] = face.cast_cost_groups.as_slice() else {
        panic!("{id} must print exactly one additional-cost group");
    };
    assert_eq!(group.group_id.as_str(), "additional_cost", "{id}");
    assert_eq!(
        group.presentation,
        AbilityPresentation::OracleLines(vec![1]),
        "{id}"
    );
    assert_eq!((group.min, group.max), (1, 1), "{id}");
    group
}

fn creature() -> TargetFilter {
    TargetFilter {
        kind: TargetKind::Creature,
        controller: TargetController::You,
        ..TargetFilter::default()
    }
}

fn artifact() -> TargetFilter {
    TargetFilter {
        kind: TargetKind::AnyPermanent,
        controller: TargetController::You,
        permanent_types: vec![PermanentTypeFilter::Artifact],
        ..TargetFilter::default()
    }
}

fn land() -> TargetFilter {
    TargetFilter {
        kind: TargetKind::AnyPermanent,
        controller: TargetController::You,
        permanent_types: vec![PermanentTypeFilter::Land],
        ..TargetFilter::default()
    }
}

fn sacrifice(option_id: &str, filter: TargetFilter) -> CastCostOptionDef {
    CastCostOptionDef::SacrificePermanent {
        option_id: ChoiceId::new(option_id).unwrap(),
        presentation: AbilityPresentation::OracleLines(vec![1]),
        kind: ObjectCastCostKind::AdditionalPayment,
        filter: Box::new(filter),
    }
}

fn discard() -> CastCostOptionDef {
    CastCostOptionDef::DiscardCard {
        option_id: ChoiceId::new("discard_card").unwrap(),
        presentation: AbilityPresentation::OracleLines(vec![1]),
    }
}

fn mana(option_id: &str, cost: &str) -> CastCostOptionDef {
    CastCostOptionDef::Mana {
        option_id: ChoiceId::new(option_id).unwrap(),
        presentation: AbilityPresentation::OracleLines(vec![1]),
        kind: ManaCostChoiceKind::AdditionalPayment,
        cost: ManaCost::parse(cost).unwrap(),
    }
}

fn tap(option_id: &str, count: u32, filter: TargetFilter) -> CastCostOptionDef {
    CastCostOptionDef::TapPermanents {
        option_id: ChoiceId::new(option_id).unwrap(),
        presentation: AbilityPresentation::OracleLines(vec![1]),
        kind: ObjectCastCostKind::AdditionalPayment,
        constraint: ObjectPaymentConstraint::ExactCount(count),
        filter: Box::new(filter),
    }
}

#[test]
fn issue_338_registers_exactly_the_reviewed_eleven() {
    let registry = CardRegistry::global();
    for (id, name, face_id, mana_cost, types, keywords, power_toughness) in [
        (
            "bogslithers_embrace",
            "Bogslither's Embrace",
            "bogslither_s_embrace",
            "{1}{B}",
            &["Sorcery"][..],
            &[][..],
            None,
        ),
        (
            "corrupted_conviction",
            "Corrupted Conviction",
            "corrupted_conviction",
            "{B}",
            &["Instant"][..],
            &[][..],
            None,
        ),
        (
            "deadly_precision",
            "Deadly Precision",
            "deadly_precision",
            "{B}",
            &["Sorcery"][..],
            &[][..],
            None,
        ),
        (
            "demand_answers",
            "Demand Answers",
            "demand_answers",
            "{1}{R}",
            &["Instant"][..],
            &[][..],
            None,
        ),
        (
            "fear_of_exposure",
            "Fear of Exposure",
            "fear_of_exposure",
            "{2}{G}",
            &["Enchantment", "Creature", "Nightmare"][..],
            &[Keyword::Trample][..],
            Some((5, 4)),
        ),
        (
            "final_vengeance",
            "Final Vengeance",
            "final_vengeance",
            "{B}",
            &["Sorcery"][..],
            &[][..],
            None,
        ),
        (
            "guardian_of_the_great_door",
            "Guardian of the Great Door",
            "guardian_of_the_great_door",
            "{W}{W}",
            &["Creature", "Angel"][..],
            &[Keyword::Flying][..],
            Some((4, 4)),
        ),
        (
            "kinsbaile_aspirant",
            "Kinsbaile Aspirant",
            "kinsbaile_aspirant",
            "{W}",
            &["Creature", "Kithkin", "Citizen"][..],
            &[][..],
            Some((2, 1)),
        ),
        (
            "laughing_mad",
            "Laughing Mad",
            "laughing_mad",
            "{2}{R}",
            &["Instant"][..],
            &[][..],
            None,
        ),
        (
            "pumpkin_bombardment",
            "Pumpkin Bombardment",
            "pumpkin_bombardment",
            "{B/R}",
            &["Sorcery"][..],
            &[][..],
            None,
        ),
        (
            "silvergill_mentor",
            "Silvergill Mentor",
            "silvergill_mentor",
            "{1}{U}",
            &["Creature", "Merfolk", "Wizard"][..],
            &[][..],
            Some((2, 1)),
        ),
    ] {
        assert_eq!(registry.id_for_name(name), Some(id), "{id}");
        FaceExpectation {
            id,
            name,
            face_id,
            mana_cost,
            types,
            keywords,
            power_toughness,
        }
        .check();
    }
}

#[test]
fn issue_338_excluded_additional_cost_forms_stay_unregistered() {
    let registry = CardRegistry::global();
    for (id, name) in [
        ("feed_the_cycle", "Feed the Cycle"),
        ("fear_of_isolation", "Fear of Isolation"),
        ("soaring_stoneglider", "Soaring Stoneglider"),
        ("champion_of_the_clachan", "Champion of the Clachan"),
    ] {
        assert!(
            registry.get(id).is_none(),
            "{id} still prints unsupported clauses and must stay unregistered"
        );
        assert!(
            registry.id_for_name(name).is_none(),
            "{id} must not resolve by name"
        );
    }
}

#[test]
fn issue_338_emits_the_reviewed_cast_cost_groups() {
    assert_eq!(
        group("corrupted_conviction").options,
        [sacrifice("sacrifice_creature", creature())]
    );
    assert_eq!(group("laughing_mad").options, [discard()]);
    assert_eq!(
        group("final_vengeance").options,
        [sacrifice(
            "sacrifice_creature_or_enchantment",
            TargetFilter {
                any_of: Some(vec![
                    creature(),
                    TargetFilter {
                        kind: TargetKind::AnyPermanent,
                        controller: TargetController::You,
                        permanent_types: vec![PermanentTypeFilter::Enchantment],
                        ..TargetFilter::default()
                    },
                ]),
                ..TargetFilter::default()
            }
        )]
    );
    assert_eq!(
        group("demand_answers").options,
        [sacrifice("sacrifice_artifact", artifact()), discard()]
    );
    assert_eq!(
        group("pumpkin_bombardment").options,
        [discard(), mana("pay_mana", "{2}")]
    );
    assert_eq!(
        group("bogslithers_embrace").options,
        [
            CastCostOptionDef::Blight {
                option_id: ChoiceId::new("blight").unwrap(),
                presentation: AbilityPresentation::OracleLines(vec![1]),
                count: 1,
            },
            mana("pay_mana", "{3}"),
        ]
    );
    assert_eq!(
        group("kinsbaile_aspirant").options,
        [
            CastCostOptionDef::Behold {
                option_id: ChoiceId::new("behold").unwrap(),
                presentation: AbilityPresentation::OracleLines(vec![1]),
                hand_filter: ZoneCardFilter {
                    required_subtypes: vec!["Kithkin".to_string()],
                    ..ZoneCardFilter::default()
                },
                permanent_filter: Box::new(TargetFilter {
                    kind: TargetKind::AnyPermanent,
                    controller: TargetController::You,
                    required_subtypes: vec!["Kithkin".to_string()],
                    ..TargetFilter::default()
                }),
            },
            mana("pay_mana", "{2}"),
        ]
    );
    assert_eq!(
        group("silvergill_mentor").options,
        [
            CastCostOptionDef::Behold {
                option_id: ChoiceId::new("behold").unwrap(),
                presentation: AbilityPresentation::OracleLines(vec![1]),
                hand_filter: ZoneCardFilter {
                    required_subtypes: vec!["Merfolk".to_string()],
                    ..ZoneCardFilter::default()
                },
                permanent_filter: Box::new(TargetFilter {
                    kind: TargetKind::AnyPermanent,
                    controller: TargetController::You,
                    required_subtypes: vec!["Merfolk".to_string()],
                    ..TargetFilter::default()
                }),
            },
            mana("pay_mana", "{2}"),
        ]
    );
    assert_eq!(
        group("deadly_precision").options,
        [
            mana("pay_mana", "{4}"),
            sacrifice(
                "sacrifice_artifact_or_creature",
                TargetFilter {
                    any_of: Some(vec![artifact(), creature()]),
                    ..TargetFilter::default()
                }
            ),
        ]
    );
    assert_eq!(
        group("fear_of_exposure").options,
        [tap(
            "tap_permanents",
            2,
            TargetFilter {
                any_of: Some(vec![creature(), land()]),
                ..TargetFilter::default()
            }
        )]
    );
    assert_eq!(
        group("guardian_of_the_great_door").options,
        [tap(
            "tap_permanents",
            4,
            TargetFilter {
                any_of: Some(vec![artifact(), creature(), land()]),
                ..TargetFilter::default()
            }
        )]
    );
}

#[test]
fn issue_338_option_presentations_map_the_printed_line() {
    for id in [
        "bogslithers_embrace",
        "corrupted_conviction",
        "deadly_precision",
        "demand_answers",
        "fear_of_exposure",
        "final_vengeance",
        "guardian_of_the_great_door",
        "kinsbaile_aspirant",
        "laughing_mad",
        "pumpkin_bombardment",
        "silvergill_mentor",
    ] {
        let group = group(id);
        for option in &group.options {
            assert_eq!(
                option.presentation(),
                &AbilityPresentation::OracleLines(vec![1]),
                "{id}"
            );
        }
    }
}
