#![allow(unused_imports)]
//! Registry conformance for the queue-v34 self first-strike grants cohort.
use tricerules_cards::primitives::{
    AbilityCost, Amount, CombatRole, EffectSubject, GameCondition, LifeAmount, PlayerRecipient,
    PowerComparison, RelativePlayerSet, SpellEffectKind, StaticAbilityDef, TargetKind,
    TriggerCondition,
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
        id: "saberclaw_golem",
        name: "Saberclaw Golem",
        mana_cost: "{5}",
        face_id: "saberclaw_golem",
        fingerprint: "f634f751d0af6e1e68e35a6cd411849029abfe6590ca9326f19ca44a6a43ebec",
        types: &["Artifact", "Creature", "Golem"],
        body: Some((4, 2)),
        keywords: &[],
    },
    Card {
        id: "noble_panther",
        name: "Noble Panther",
        mana_cost: "{1}{G}{W}",
        face_id: "noble_panther",
        fingerprint: "c8c29fb504e2868442d934c6baed4b83dd20d74504ca5e40b39ab32ef3c1a7fa",
        types: &["Creature", "Cat"],
        body: Some((3, 3)),
        keywords: &[],
    },
    Card {
        id: "disciple_of_the_old_ways",
        name: "Disciple of the Old Ways",
        mana_cost: "{1}{G}",
        face_id: "disciple_of_the_old_ways",
        fingerprint: "f634f751d0af6e1e68e35a6cd411849029abfe6590ca9326f19ca44a6a43ebec",
        types: &["Creature", "Human", "Warrior"],
        body: Some((2, 2)),
        keywords: &[],
    },
];

#[test]
fn q34_self_first_strike_batch_maps_definitions() {
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
            "saberclaw_golem" => {
                assert_eq!(f.activated_abilities.len(), 1);
                let ability = &f.activated_abilities[0];
                assert_eq!(
                    ability.presentation,
                    AbilityPresentation::OracleLines(vec![1])
                );
                assert!(
                    matches!(ability.costs.as_slice(), [AbilityCost::Mana(cost)] if cost.to_string() == "{R}")
                );
                assert!(
                    matches!(ability.effect.as_slice(), [SpellEffectKind::GrantKeywords { subject: EffectSubject::Source, keywords }] if keywords.as_slice() == [Keyword::FirstStrike])
                );
            }
            "noble_panther" => {
                assert_eq!(f.activated_abilities.len(), 1);
                let ability = &f.activated_abilities[0];
                assert_eq!(
                    ability.presentation,
                    AbilityPresentation::OracleLines(vec![1])
                );
                assert!(
                    matches!(ability.costs.as_slice(), [AbilityCost::Mana(cost)] if cost.to_string() == "{1}")
                );
                assert!(
                    matches!(ability.effect.as_slice(), [SpellEffectKind::GrantKeywords { subject: EffectSubject::Source, keywords }] if keywords.as_slice() == [Keyword::FirstStrike])
                );
            }
            "disciple_of_the_old_ways" => {
                assert_eq!(f.activated_abilities.len(), 1);
                let ability = &f.activated_abilities[0];
                assert_eq!(
                    ability.presentation,
                    AbilityPresentation::OracleLines(vec![1])
                );
                assert!(
                    matches!(ability.costs.as_slice(), [AbilityCost::Mana(cost)] if cost.to_string() == "{R}")
                );
                assert!(
                    matches!(ability.effect.as_slice(), [SpellEffectKind::GrantKeywords { subject: EffectSubject::Source, keywords }] if keywords.as_slice() == [Keyword::FirstStrike])
                );
            }
            other => panic!("unexpected {other}"),
        }
    }
}

#[test]
fn q34_self_first_strike_batch_fingerprints_match_the_pinned_oracle_text() {
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
