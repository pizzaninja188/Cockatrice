#![allow(unused_imports)]
//! Registry conformance for the queue-v35 self first-strike grants cohort.
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
        id: "kessig_wolf",
        name: "Kessig Wolf",
        mana_cost: "{2}{R}",
        face_id: "kessig_wolf",
        fingerprint: "aeae591a486c9d9c5ead02e506b9b102b809e92b33b705af861c54c8152e5bee",
        types: &["Creature", "Wolf"],
        body: Some((3, 1)),
        keywords: &[],
    },
    Card {
        id: "titanium_golem",
        name: "Titanium Golem",
        mana_cost: "{5}",
        face_id: "titanium_golem",
        fingerprint: "17c3ceaf4a673cb8cb5dc20d7fd302f18b7a4d2c24a1bdd5b37e58bc8e52799b",
        types: &["Artifact", "Creature", "Golem"],
        body: Some((3, 3)),
        keywords: &[],
    },
    Card {
        id: "unyielding_krumar",
        name: "Unyielding Krumar",
        mana_cost: "{3}{B}",
        face_id: "unyielding_krumar",
        fingerprint: "17c3ceaf4a673cb8cb5dc20d7fd302f18b7a4d2c24a1bdd5b37e58bc8e52799b",
        types: &["Creature", "Orc", "Warrior"],
        body: Some((3, 3)),
        keywords: &[],
    },
];

#[test]
fn q35_self_first_strike_batch_maps_definitions() {
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
            "kessig_wolf" => {
                assert_eq!(f.activated_abilities.len(), 1);
                let ability = &f.activated_abilities[0];
                assert_eq!(
                    ability.presentation,
                    AbilityPresentation::OracleLines(vec![1])
                );
                assert!(
                    matches!(ability.costs.as_slice(), [AbilityCost::Mana(cost)] if cost.to_string() == "{1}{R}")
                );
                assert!(
                    matches!(ability.effect.as_slice(), [SpellEffectKind::GrantKeywords { subject: EffectSubject::Source, keywords }] if keywords.as_slice() == [Keyword::FirstStrike])
                );
                assert!(ability.targeting.is_none());
            }
            "titanium_golem" => {
                assert_eq!(f.activated_abilities.len(), 1);
                let ability = &f.activated_abilities[0];
                assert_eq!(
                    ability.presentation,
                    AbilityPresentation::OracleLines(vec![1])
                );
                assert!(
                    matches!(ability.costs.as_slice(), [AbilityCost::Mana(cost)] if cost.to_string() == "{1}{W}")
                );
                assert!(
                    matches!(ability.effect.as_slice(), [SpellEffectKind::GrantKeywords { subject: EffectSubject::Source, keywords }] if keywords.as_slice() == [Keyword::FirstStrike])
                );
                assert!(ability.targeting.is_none());
            }
            "unyielding_krumar" => {
                assert_eq!(f.activated_abilities.len(), 1);
                let ability = &f.activated_abilities[0];
                assert_eq!(
                    ability.presentation,
                    AbilityPresentation::OracleLines(vec![1])
                );
                assert!(
                    matches!(ability.costs.as_slice(), [AbilityCost::Mana(cost)] if cost.to_string() == "{1}{W}")
                );
                assert!(
                    matches!(ability.effect.as_slice(), [SpellEffectKind::GrantKeywords { subject: EffectSubject::Source, keywords }] if keywords.as_slice() == [Keyword::FirstStrike])
                );
                assert!(ability.targeting.is_none());
            }
            other => panic!("unexpected {other}"),
        }
    }
}

#[test]
fn q35_self_first_strike_batch_fingerprints_match_the_pinned_oracle_text() {
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
