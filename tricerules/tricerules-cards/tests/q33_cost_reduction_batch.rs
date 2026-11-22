#![allow(unused_imports)]
//! Registry conformance for the queue-v33 fixed generic cost reduction cohort.
use tricerules_cards::primitives::{
    AbilityCost, Amount, CardTypeFilter, CastTriggerPlayer, Color, CounterKind,
    CreatureScopeController, EffectSubject, RelativePlayerSet, SpellEffectKind, StaticAbilityDef,
    TriggerCondition,
};
use tricerules_cards::{AbilityPresentation, AbilitySourceZone, Keyword, Layout};

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
        id: "emerald_medallion",
        name: "Emerald Medallion",
        mana_cost: "{2}",
        face_id: "emerald_medallion",
        fingerprint: "8557e245ca35f3af76521704b4fd0a39acfdcf7d55b21f258b85b6912e0ca29c",
        types: &["Artifact"],
        body: None,
        keywords: &[],
    },
    Card {
        id: "etherium_sculptor",
        name: "Etherium Sculptor",
        mana_cost: "{1}{U}",
        face_id: "etherium_sculptor",
        fingerprint: "4ec5b9e9396460eb97813593ad5a7fa958d8c029e05b3f7cd14360c0e629408f",
        types: &["Artifact", "Creature", "Vedalken", "Artificer"],
        body: Some((1, 2)),
        keywords: &[],
    },
    Card {
        id: "foundry_inspector",
        name: "Foundry Inspector",
        mana_cost: "{3}",
        face_id: "foundry_inspector",
        fingerprint: "4ec5b9e9396460eb97813593ad5a7fa958d8c029e05b3f7cd14360c0e629408f",
        types: &["Artifact", "Creature", "Construct"],
        body: Some((3, 2)),
        keywords: &[],
    },
    Card {
        id: "enthusiastic_mechanaut",
        name: "Enthusiastic Mechanaut",
        mana_cost: "{U}{R}",
        face_id: "enthusiastic_mechanaut",
        fingerprint: "c9b8379af2d0f4126001dd32b8f3a7c24661e2d721566c8b30707fa4cfd79595",
        types: &["Artifact", "Creature", "Goblin", "Artificer"],
        body: Some((2, 2)),
        keywords: &[Keyword::Flying],
    },
    Card {
        id: "jet_medallion",
        name: "Jet Medallion",
        mana_cost: "{2}",
        face_id: "jet_medallion",
        fingerprint: "935bd912fcfe9bdc8ecaf21e016e2cf7607111acee6ffd082111baecdd4cb4e9",
        types: &["Artifact"],
        body: None,
        keywords: &[],
    },
    Card {
        id: "pearl_medallion",
        name: "Pearl Medallion",
        mana_cost: "{2}",
        face_id: "pearl_medallion",
        fingerprint: "c518095411646325cc51e5be55ca6a379c01203bf983f10597be3149b030616c",
        types: &["Artifact"],
        body: None,
        keywords: &[],
    },
    Card {
        id: "ruby_medallion",
        name: "Ruby Medallion",
        mana_cost: "{2}",
        face_id: "ruby_medallion",
        fingerprint: "7af2ea6b3982b379adc635fe694983fbe2fb768e7510aca738c2e65b0f517ed1",
        types: &["Artifact"],
        body: None,
        keywords: &[],
    },
    Card {
        id: "sapphire_medallion",
        name: "Sapphire Medallion",
        mana_cost: "{2}",
        face_id: "sapphire_medallion",
        fingerprint: "1e18466aef53a0191ea5d511706bd02b236f811acf62f1faaff968fe08295cfc",
        types: &["Artifact"],
        body: None,
        keywords: &[],
    },
];

#[test]
fn q33_cost_reduction_batch_maps_definitions() {
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
            "emerald_medallion" => {
                assert_eq!(f.static_abilities.len(), 1);
                let ability = &f.static_abilities[0];
                assert_eq!(
                    ability.presentation,
                    AbilityPresentation::OracleLines(vec![1])
                );
                assert!(
                    matches!(&ability.definition, StaticAbilityDef::SpellGenericReduction { casters: RelativePlayerSet::Controller, spell_filter: Some(filter), amount: Amount::Fixed(1), .. } if filter.is_color == Some(Color::Green) && filter.card_type.is_none())
                );
            }
            "etherium_sculptor" => {
                assert_eq!(f.static_abilities.len(), 1);
                let ability = &f.static_abilities[0];
                assert_eq!(
                    ability.presentation,
                    AbilityPresentation::OracleLines(vec![1])
                );
                assert!(
                    matches!(&ability.definition, StaticAbilityDef::SpellGenericReduction { casters: RelativePlayerSet::Controller, spell_filter: Some(filter), amount: Amount::Fixed(1), .. } if filter.card_type == Some(CardTypeFilter::Artifact) && filter.is_color.is_none())
                );
            }
            "foundry_inspector" => {
                assert_eq!(f.static_abilities.len(), 1);
                let ability = &f.static_abilities[0];
                assert_eq!(
                    ability.presentation,
                    AbilityPresentation::OracleLines(vec![1])
                );
                assert!(
                    matches!(&ability.definition, StaticAbilityDef::SpellGenericReduction { casters: RelativePlayerSet::Controller, spell_filter: Some(filter), amount: Amount::Fixed(1), .. } if filter.card_type == Some(CardTypeFilter::Artifact) && filter.is_color.is_none())
                );
            }
            "enthusiastic_mechanaut" => {
                assert_eq!(f.static_abilities.len(), 1);
                let ability = &f.static_abilities[0];
                assert_eq!(
                    ability.presentation,
                    AbilityPresentation::OracleLines(vec![2])
                );
                assert!(
                    matches!(&ability.definition, StaticAbilityDef::SpellGenericReduction { casters: RelativePlayerSet::Controller, spell_filter: Some(filter), amount: Amount::Fixed(1), .. } if filter.card_type == Some(CardTypeFilter::Artifact) && filter.is_color.is_none())
                );
            }
            "jet_medallion" => {
                assert_eq!(f.static_abilities.len(), 1);
                let ability = &f.static_abilities[0];
                assert_eq!(
                    ability.presentation,
                    AbilityPresentation::OracleLines(vec![1])
                );
                assert!(
                    matches!(&ability.definition, StaticAbilityDef::SpellGenericReduction { casters: RelativePlayerSet::Controller, spell_filter: Some(filter), amount: Amount::Fixed(1), .. } if filter.is_color == Some(Color::Black) && filter.card_type.is_none())
                );
            }
            "pearl_medallion" => {
                assert_eq!(f.static_abilities.len(), 1);
                let ability = &f.static_abilities[0];
                assert_eq!(
                    ability.presentation,
                    AbilityPresentation::OracleLines(vec![1])
                );
                assert!(
                    matches!(&ability.definition, StaticAbilityDef::SpellGenericReduction { casters: RelativePlayerSet::Controller, spell_filter: Some(filter), amount: Amount::Fixed(1), .. } if filter.is_color == Some(Color::White) && filter.card_type.is_none())
                );
            }
            "ruby_medallion" => {
                assert_eq!(f.static_abilities.len(), 1);
                let ability = &f.static_abilities[0];
                assert_eq!(
                    ability.presentation,
                    AbilityPresentation::OracleLines(vec![1])
                );
                assert!(
                    matches!(&ability.definition, StaticAbilityDef::SpellGenericReduction { casters: RelativePlayerSet::Controller, spell_filter: Some(filter), amount: Amount::Fixed(1), .. } if filter.is_color == Some(Color::Red) && filter.card_type.is_none())
                );
            }
            "sapphire_medallion" => {
                assert_eq!(f.static_abilities.len(), 1);
                let ability = &f.static_abilities[0];
                assert_eq!(
                    ability.presentation,
                    AbilityPresentation::OracleLines(vec![1])
                );
                assert!(
                    matches!(&ability.definition, StaticAbilityDef::SpellGenericReduction { casters: RelativePlayerSet::Controller, spell_filter: Some(filter), amount: Amount::Fixed(1), .. } if filter.is_color == Some(Color::Blue) && filter.card_type.is_none())
                );
            }
            other => panic!("unexpected {other}"),
        }
    }
}

#[test]
fn q33_cost_reduction_batch_fingerprints_match_the_pinned_oracle_text() {
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
