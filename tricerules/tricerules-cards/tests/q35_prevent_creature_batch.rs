#![allow(unused_imports)]
//! Registry conformance for the queue-v35 prevention-shield creatures cohort.
use tricerules_cards::primitives::{
    AbilityCost, EffectSubject, EntersTappedAffected, SpellEffectKind, StaticAbilityDef, TargetKind,
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
        id: "kei_takahashi",
        name: "Kei Takahashi",
        mana_cost: "{2}{G}{W}",
        face_id: "kei_takahashi",
        fingerprint: "29731fa4a3fa09d3ab9667a851d3e7857ed8586ded61244676f4f3ccae05ead7",
        types: &["Creature", "Human", "Cleric"],
        body: Some((2, 2)),
        keywords: &[],
    },
    Card {
        id: "oasis",
        name: "Oasis",
        mana_cost: "",
        face_id: "oasis",
        fingerprint: "689f9660438368b50f9221f5491bc3ed49e6485da8d95e1384dfc264596afd40",
        types: &["Land"],
        body: None,
        keywords: &[],
    },
    Card {
        id: "squees_toy",
        name: "Squee's Toy",
        mana_cost: "{1}",
        face_id: "squees_toy",
        fingerprint: "689f9660438368b50f9221f5491bc3ed49e6485da8d95e1384dfc264596afd40",
        types: &["Artifact"],
        body: None,
        keywords: &[],
    },
];

#[test]
fn q35_prevent_creature_batch_maps_definitions() {
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
            "kei_takahashi" => {
                assert_eq!(f.activated_abilities.len(), 1);
                let ability = &f.activated_abilities[0];
                assert_eq!(
                    ability.presentation,
                    AbilityPresentation::OracleLines(vec![1])
                );
                assert!(matches!(ability.costs.as_slice(), [AbilityCost::Tap]));
                assert!(
                    matches!(ability.effect.as_slice(), [SpellEffectKind::PreventNextDamage { amount: 2, target }] if target.kind == TargetKind::Creature)
                );
                assert!(ability.targeting.is_none());
            }
            "oasis" => {
                assert_eq!(f.activated_abilities.len(), 1);
                let ability = &f.activated_abilities[0];
                assert_eq!(
                    ability.presentation,
                    AbilityPresentation::OracleLines(vec![1])
                );
                assert!(matches!(ability.costs.as_slice(), [AbilityCost::Tap]));
                assert!(
                    matches!(ability.effect.as_slice(), [SpellEffectKind::PreventNextDamage { amount: 1, target }] if target.kind == TargetKind::Creature)
                );
                assert!(ability.targeting.is_none());
            }
            "squees_toy" => {
                assert_eq!(f.activated_abilities.len(), 1);
                let ability = &f.activated_abilities[0];
                assert_eq!(
                    ability.presentation,
                    AbilityPresentation::OracleLines(vec![1])
                );
                assert!(matches!(ability.costs.as_slice(), [AbilityCost::Tap]));
                assert!(
                    matches!(ability.effect.as_slice(), [SpellEffectKind::PreventNextDamage { amount: 1, target }] if target.kind == TargetKind::Creature)
                );
                assert!(ability.targeting.is_none());
            }
            other => panic!("unexpected {other}"),
        }
    }
}

#[test]
fn q35_prevent_creature_batch_fingerprints_match_the_pinned_oracle_text() {
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
