//! Registry conformance for the queue-v37 05-target-keyword-activated cohort.
//! Printed identity, costs, typed keyword shape and pinned fingerprints.

use tricerules_cards::primitives::{AbilityCost, EffectSubject, SpellEffectKind, TargetKind};
use tricerules_cards::{AbilityPresentation, AbilitySourceZone, Keyword, Layout};

struct Card {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    types: &'static [&'static str],
    power: Option<u32>,
    toughness: Option<u32>,
    cost: &'static str,
    keywords: &'static [Keyword],
    fingerprint: &'static str,
}

const CARDS: &[Card] = &[
    Card {
        id: "whip_sergeant",
        name: "Whip Sergeant",
        mana_cost: "{2}{R}",
        types: &["Creature", "Human", "Soldier"],
        power: Some(2),
        toughness: Some(1),
        cost: "{R}",
        keywords: &[Keyword::Haste],
        fingerprint: "870080411d54bbf9603736cf10a7990b7d55becf8ccf1f07cf98c0fb42d7062f",
    },
    Card {
        id: "zephyr_charge",
        name: "Zephyr Charge",
        mana_cost: "{1}{U}",
        types: &["Enchantment"],
        power: None,
        toughness: None,
        cost: "{1}{U}",
        keywords: &[Keyword::Flying],
        fingerprint: "8637586d1ff7389237d3ad4c72689e59b6fbc7a36b7c66685a8fabce967b0680",
    },
    Card {
        id: "elvish_herder",
        name: "Elvish Herder",
        mana_cost: "{G}",
        types: &["Creature", "Elf"],
        power: Some(1),
        toughness: Some(1),
        cost: "{G}",
        keywords: &[Keyword::Trample],
        fingerprint: "f1466222023f62d0d3196b3a3b0d2730de7cd8a418cc437ef61db15324cd0d27",
    },
    Card {
        id: "keen_glidemaster",
        name: "Keen Glidemaster",
        mana_cost: "{1}{U}",
        types: &["Creature", "Human", "Soldier"],
        power: Some(2),
        toughness: Some(1),
        cost: "{2}{U}",
        keywords: &[Keyword::Flying],
        fingerprint: "8b00ccd24ccf61d3a5f2061cd8edb90d45d8e02a0cdefe703346b5627523ff0e",
    },
    Card {
        id: "noble_steeds",
        name: "Noble Steeds",
        mana_cost: "{2}{W}",
        types: &["Enchantment"],
        power: None,
        toughness: None,
        cost: "{1}{W}",
        keywords: &[Keyword::FirstStrike],
        fingerprint: "cec00bf52cb4009ae0c503d25fde57fa1d5f8b8851980adf68a6a1594ac3ca0f",
    },
];

#[test]
fn q37_target_keyword_activated_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for card in CARDS {
        let definition = registry
            .get(card.id)
            .unwrap_or_else(|| panic!("missing {}", card.id));
        assert_eq!(definition.layout, Layout::Normal, "{}", card.id);
        assert_eq!(registry.id_for_name(card.name), Some(card.id));
        let f = definition.primary_face();
        assert_eq!(f.name, card.name);
        assert_eq!(f.mana_cost.to_string(), card.mana_cost);
        assert_eq!(
            f.types.iter().map(String::as_str).collect::<Vec<_>>(),
            card.types
        );
        assert_eq!((f.power, f.toughness), (card.power, card.toughness));
        assert!(f.keywords.is_empty(), "{}: no printed keyword", card.id);
        assert!(f.static_abilities.is_empty(), "{}", card.id);
        assert!(f.triggered_abilities.is_empty(), "{}", card.id);
        let [ability] = f.activated_abilities.as_slice() else {
            panic!("{}: exactly one activated ability", card.id);
        };
        assert_eq!(ability.ability_id.as_str(), "activated_01");
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![1])
        );
        assert_eq!(ability.source_zone, AbilitySourceZone::Battlefield);
        assert!(
            matches!(
                ability.costs.as_slice(),
                [AbilityCost::Mana(mana)] if mana.to_string() == card.cost
            ),
            "{}: printed activation cost",
            card.id
        );
        assert!(
            matches!(
                ability.effect.as_slice(),
                [SpellEffectKind::GrantKeywords { subject, keywords }]
                    if matches!(subject, EffectSubject::Chosen(filter)
                        if filter.kind == TargetKind::Creature)
                        && keywords == card.keywords
            ),
            "{}: targeted keyword grant",
            card.id
        );
        let targeting = ability.targeting.as_ref().expect("targeting");
        assert_eq!(targeting.groups.len(), 1);
        assert_eq!((targeting.groups[0].min, targeting.groups[0].max), (1, 1));
        assert_eq!(targeting.groups[0].effect_indices, [0]);
    }
}

#[test]
fn q37_target_keyword_activated_batch_fingerprints_match_the_pinned_oracle_text() {
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
        assert_eq!(fields[2], card.id.replace('-', "_"));
        assert_eq!(
            fields[4], card.fingerprint,
            "fingerprint drift for {}",
            card.id
        );
    }
}
