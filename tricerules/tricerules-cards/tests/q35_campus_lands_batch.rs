#![allow(unused_imports)]
//! Registry conformance for the queue-v35 campus scry lands cohort.
use tricerules_cards::primitives::{
    AbilityCost, Amount, CombatRestrictionScope, EffectSubject, EntersTappedAffected,
    SpellEffectKind, StaticAbilityDef, TargetKind,
};
use tricerules_cards::{AbilityPresentation, Keyword, Layout};

struct Card {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    face_id: &'static str,
    fingerprint: &'static str,
    types: &'static [&'static str],
    body: Option<(u32, u32)>,
    keywords: &'static [Keyword],
}

const CARDS: &[Card] = &[
    Card {
        id: "lorehold_campus",
        name: "Lorehold Campus",
        mana_cost: "",
        face_id: "lorehold_campus",
        fingerprint: "e1169cc128fe4843e7c8542ce64d2ad14f3c5107b5c99bf515e7d0351c3594eb",
        types: &["Land"],
        body: None,
        keywords: &[],
    },
    Card {
        id: "prismari_campus",
        name: "Prismari Campus",
        mana_cost: "",
        face_id: "prismari_campus",
        fingerprint: "228fe1e006b5eedbecad6110d71fdca6e6f07bdf43f078a62c1f7e33ba78771e",
        types: &["Land"],
        body: None,
        keywords: &[],
    },
    Card {
        id: "quandrix_campus",
        name: "Quandrix Campus",
        mana_cost: "",
        face_id: "quandrix_campus",
        fingerprint: "f73d7a0fcb40cd1a73913d4a7e72818fd6ee263cf40acc5b197c372deb6ea9f6",
        types: &["Land"],
        body: None,
        keywords: &[],
    },
    Card {
        id: "silverquill_campus",
        name: "Silverquill Campus",
        mana_cost: "",
        face_id: "silverquill_campus",
        fingerprint: "b37ac1bde384cc008c770885d9643b4b256d74a8a45be4bac56f4451359a953a",
        types: &["Land"],
        body: None,
        keywords: &[],
    },
    Card {
        id: "witherbloom_campus",
        name: "Witherbloom Campus",
        mana_cost: "",
        face_id: "witherbloom_campus",
        fingerprint: "06efad690d4821e5bdfb371fddee89dad13f879aa8479eac912bf4304bcb414d",
        types: &["Land"],
        body: None,
        keywords: &[],
    },
];

#[test]
fn q35_campus_lands_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for card in CARDS {
        let definition = registry
            .get(card.id)
            .unwrap_or_else(|| panic!("missing {}", card.id));
        assert_eq!(definition.layout, Layout::Normal, "{}", card.id);
        assert_eq!(registry.id_for_name(card.name), Some(card.id));
        let f = definition.primary_face();
        assert_eq!(f.face_id.as_str(), card.face_id);
        assert_eq!(f.name, card.name);
        assert_eq!(f.mana_cost.to_string(), card.mana_cost);
        assert_eq!(
            f.types.iter().map(String::as_str).collect::<Vec<_>>(),
            card.types.to_vec()
        );
        match card.body {
            Some((power, toughness)) => {
                assert_eq!((f.power, f.toughness), (Some(power), Some(toughness)))
            }
            None => assert_eq!((f.power, f.toughness), (None, None)),
        }
        assert_eq!(f.keywords, card.keywords);
        match card.id {
            "lorehold_campus" => {
                assert_eq!(f.activated_abilities.len(), 2);
                assert_eq!(f.static_abilities.len(), 1);
                let mana = &f.activated_abilities[0];
                assert_eq!(mana.presentation, AbilityPresentation::OracleLines(vec![2]));
                assert!(matches!(mana.costs.as_slice(), [AbilityCost::Tap]));
                assert!(
                    matches!(mana.effect.as_slice(), [SpellEffectKind::ProduceMana { options, .. }] if options.len() == 2)
                );
                let scry = &f.activated_abilities[1];
                assert_eq!(scry.presentation, AbilityPresentation::OracleLines(vec![3]));
                assert!(
                    matches!(scry.costs.as_slice(), [AbilityCost::Mana(cost), AbilityCost::Tap] if cost.to_string() == "{4}")
                );
                assert!(matches!(
                    scry.effect.as_slice(),
                    [SpellEffectKind::Scry {
                        count: Amount::Fixed(1)
                    }]
                ));
                assert_eq!(
                    f.static_abilities[0].presentation,
                    AbilityPresentation::OracleLines(vec![1])
                );
                assert!(matches!(
                    &f.static_abilities[0].definition,
                    StaticAbilityDef::EntersTapped {
                        affected: EntersTappedAffected::Self_,
                        ..
                    }
                ));
            }
            "prismari_campus" => {
                assert_eq!(f.activated_abilities.len(), 2);
                assert_eq!(f.static_abilities.len(), 1);
                let mana = &f.activated_abilities[0];
                assert_eq!(mana.presentation, AbilityPresentation::OracleLines(vec![2]));
                assert!(matches!(mana.costs.as_slice(), [AbilityCost::Tap]));
                assert!(
                    matches!(mana.effect.as_slice(), [SpellEffectKind::ProduceMana { options, .. }] if options.len() == 2)
                );
                let scry = &f.activated_abilities[1];
                assert_eq!(scry.presentation, AbilityPresentation::OracleLines(vec![3]));
                assert!(
                    matches!(scry.costs.as_slice(), [AbilityCost::Mana(cost), AbilityCost::Tap] if cost.to_string() == "{4}")
                );
                assert!(matches!(
                    scry.effect.as_slice(),
                    [SpellEffectKind::Scry {
                        count: Amount::Fixed(1)
                    }]
                ));
                assert_eq!(
                    f.static_abilities[0].presentation,
                    AbilityPresentation::OracleLines(vec![1])
                );
                assert!(matches!(
                    &f.static_abilities[0].definition,
                    StaticAbilityDef::EntersTapped {
                        affected: EntersTappedAffected::Self_,
                        ..
                    }
                ));
            }
            "quandrix_campus" => {
                assert_eq!(f.activated_abilities.len(), 2);
                assert_eq!(f.static_abilities.len(), 1);
                let mana = &f.activated_abilities[0];
                assert_eq!(mana.presentation, AbilityPresentation::OracleLines(vec![2]));
                assert!(matches!(mana.costs.as_slice(), [AbilityCost::Tap]));
                assert!(
                    matches!(mana.effect.as_slice(), [SpellEffectKind::ProduceMana { options, .. }] if options.len() == 2)
                );
                let scry = &f.activated_abilities[1];
                assert_eq!(scry.presentation, AbilityPresentation::OracleLines(vec![3]));
                assert!(
                    matches!(scry.costs.as_slice(), [AbilityCost::Mana(cost), AbilityCost::Tap] if cost.to_string() == "{4}")
                );
                assert!(matches!(
                    scry.effect.as_slice(),
                    [SpellEffectKind::Scry {
                        count: Amount::Fixed(1)
                    }]
                ));
                assert_eq!(
                    f.static_abilities[0].presentation,
                    AbilityPresentation::OracleLines(vec![1])
                );
                assert!(matches!(
                    &f.static_abilities[0].definition,
                    StaticAbilityDef::EntersTapped {
                        affected: EntersTappedAffected::Self_,
                        ..
                    }
                ));
            }
            "silverquill_campus" => {
                assert_eq!(f.activated_abilities.len(), 2);
                assert_eq!(f.static_abilities.len(), 1);
                let mana = &f.activated_abilities[0];
                assert_eq!(mana.presentation, AbilityPresentation::OracleLines(vec![2]));
                assert!(matches!(mana.costs.as_slice(), [AbilityCost::Tap]));
                assert!(
                    matches!(mana.effect.as_slice(), [SpellEffectKind::ProduceMana { options, .. }] if options.len() == 2)
                );
                let scry = &f.activated_abilities[1];
                assert_eq!(scry.presentation, AbilityPresentation::OracleLines(vec![3]));
                assert!(
                    matches!(scry.costs.as_slice(), [AbilityCost::Mana(cost), AbilityCost::Tap] if cost.to_string() == "{4}")
                );
                assert!(matches!(
                    scry.effect.as_slice(),
                    [SpellEffectKind::Scry {
                        count: Amount::Fixed(1)
                    }]
                ));
                assert_eq!(
                    f.static_abilities[0].presentation,
                    AbilityPresentation::OracleLines(vec![1])
                );
                assert!(matches!(
                    &f.static_abilities[0].definition,
                    StaticAbilityDef::EntersTapped {
                        affected: EntersTappedAffected::Self_,
                        ..
                    }
                ));
            }
            "witherbloom_campus" => {
                assert_eq!(f.activated_abilities.len(), 2);
                assert_eq!(f.static_abilities.len(), 1);
                let mana = &f.activated_abilities[0];
                assert_eq!(mana.presentation, AbilityPresentation::OracleLines(vec![2]));
                assert!(matches!(mana.costs.as_slice(), [AbilityCost::Tap]));
                assert!(
                    matches!(mana.effect.as_slice(), [SpellEffectKind::ProduceMana { options, .. }] if options.len() == 2)
                );
                let scry = &f.activated_abilities[1];
                assert_eq!(scry.presentation, AbilityPresentation::OracleLines(vec![3]));
                assert!(
                    matches!(scry.costs.as_slice(), [AbilityCost::Mana(cost), AbilityCost::Tap] if cost.to_string() == "{4}")
                );
                assert!(matches!(
                    scry.effect.as_slice(),
                    [SpellEffectKind::Scry {
                        count: Amount::Fixed(1)
                    }]
                ));
                assert_eq!(
                    f.static_abilities[0].presentation,
                    AbilityPresentation::OracleLines(vec![1])
                );
                assert!(matches!(
                    &f.static_abilities[0].definition,
                    StaticAbilityDef::EntersTapped {
                        affected: EntersTappedAffected::Self_,
                        ..
                    }
                ));
            }
            other => panic!("unexpected {other}"),
        }
    }
}

#[test]
fn q35_campus_lands_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for card in CARDS {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{}\t", card.id)))
            .unwrap_or_else(|| panic!("missing fingerprint row for {}", card.id));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "{}", card.id);
        assert_eq!(fields[0], card.id);
        assert_eq!(fields[1], card.name);
        assert_eq!(fields[2], card.face_id, "face id drift for {}", card.id);
        assert_eq!(
            fields[4], card.fingerprint,
            "fingerprint drift for {}",
            card.id
        );
    }
}
