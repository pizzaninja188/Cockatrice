#![allow(unused_imports)]
//! Registry conformance for the queue-v35 target first-strike grants cohort.
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
        id: "fyndhorn_bow",
        name: "Fyndhorn Bow",
        mana_cost: "{2}",
        face_id: "fyndhorn_bow",
        fingerprint: "2eb9b6f9705cb811b6a0b186545565b441dcc05cab9b66bb893b00ce888a1990",
        types: &["Artifact"],
        body: None,
        keywords: &[],
    },
    Card {
        id: "icatian_scout",
        name: "Icatian Scout",
        mana_cost: "{W}",
        face_id: "icatian_scout",
        fingerprint: "22265786ec6f74b3eeee24aa562b9a703e678d6e89d46f6cd981f05e8fafd1c7",
        types: &["Creature", "Human", "Soldier", "Scout"],
        body: Some((1, 1)),
        keywords: &[],
    },
    Card {
        id: "iron_lance",
        name: "Iron Lance",
        mana_cost: "{2}",
        face_id: "iron_lance",
        fingerprint: "2eb9b6f9705cb811b6a0b186545565b441dcc05cab9b66bb893b00ce888a1990",
        types: &["Artifact"],
        body: None,
        keywords: &[],
    },
];

#[test]
fn q35_target_first_strike_batch_maps_definitions() {
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
            "fyndhorn_bow" => {
                assert_eq!(f.activated_abilities.len(), 1);
                let ability = &f.activated_abilities[0];
                assert_eq!(
                    ability.presentation,
                    AbilityPresentation::OracleLines(vec![1])
                );
                assert!(
                    matches!(ability.costs.as_slice(), [AbilityCost::Mana(cost), AbilityCost::Tap] if cost.to_string() == "{3}")
                );
                assert!(
                    matches!(ability.effect.as_slice(), [SpellEffectKind::GrantKeywords { subject: EffectSubject::Chosen(_), keywords }] if keywords.as_slice() == [Keyword::FirstStrike])
                );
                let groups = &ability.targeting.as_ref().expect("one target group").groups;
                assert_eq!(groups.len(), 1);
                assert_eq!((groups[0].min, groups[0].max), (1, 1));
                assert_eq!(groups[0].effect_indices, [0]);
            }
            "icatian_scout" => {
                assert_eq!(f.activated_abilities.len(), 1);
                let ability = &f.activated_abilities[0];
                assert_eq!(
                    ability.presentation,
                    AbilityPresentation::OracleLines(vec![1])
                );
                assert!(
                    matches!(ability.costs.as_slice(), [AbilityCost::Mana(cost), AbilityCost::Tap] if cost.to_string() == "{1}")
                );
                assert!(
                    matches!(ability.effect.as_slice(), [SpellEffectKind::GrantKeywords { subject: EffectSubject::Chosen(_), keywords }] if keywords.as_slice() == [Keyword::FirstStrike])
                );
                let groups = &ability.targeting.as_ref().expect("one target group").groups;
                assert_eq!(groups.len(), 1);
                assert_eq!((groups[0].min, groups[0].max), (1, 1));
                assert_eq!(groups[0].effect_indices, [0]);
            }
            "iron_lance" => {
                assert_eq!(f.activated_abilities.len(), 1);
                let ability = &f.activated_abilities[0];
                assert_eq!(
                    ability.presentation,
                    AbilityPresentation::OracleLines(vec![1])
                );
                assert!(
                    matches!(ability.costs.as_slice(), [AbilityCost::Mana(cost), AbilityCost::Tap] if cost.to_string() == "{3}")
                );
                assert!(
                    matches!(ability.effect.as_slice(), [SpellEffectKind::GrantKeywords { subject: EffectSubject::Chosen(_), keywords }] if keywords.as_slice() == [Keyword::FirstStrike])
                );
                let groups = &ability.targeting.as_ref().expect("one target group").groups;
                assert_eq!(groups.len(), 1);
                assert_eq!((groups[0].min, groups[0].max), (1, 1));
                assert_eq!(groups[0].effect_indices, [0]);
            }
            other => panic!("unexpected {other}"),
        }
    }
}

#[test]
fn q35_target_first_strike_batch_fingerprints_match_the_pinned_oracle_text() {
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
