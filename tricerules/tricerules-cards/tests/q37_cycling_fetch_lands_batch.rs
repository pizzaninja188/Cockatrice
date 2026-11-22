//! Registry conformance for the queue-v37 02-cycling-fetch-lands cohort.
//! Printed identity, costs, typed shape and pinned fingerprints.

use tricerules_cards::primitives::{
    AbilityCost, Amount, CardTypeFilter, PlayerRecipient, SearchDestination, SpellEffectKind,
};
use tricerules_cards::{AbilityPresentation, AbilitySourceZone, Layout};

struct Card {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    types: &'static [&'static str],
    power: Option<u32>,
    toughness: Option<u32>,
    cost: &'static str,
    fingerprint: &'static str,
}

const CARDS: &[Card] = &[
    Card {
        id: "bountiful_landscape",
        cost: "{G}{U}{R}",
        name: "Bountiful Landscape",
        mana_cost: "",
        types: &["Land"],
        power: None,
        toughness: None,
        fingerprint: "f1e4078f972d8a506397876a6c530f35b7e0326258f818ec2ec783f433650fb6",
    },
    Card {
        id: "contaminated_landscape",
        cost: "{W}{U}{B}",
        name: "Contaminated Landscape",
        mana_cost: "",
        types: &["Land"],
        power: None,
        toughness: None,
        fingerprint: "ab83a07ace0617e83c58a40a9444f61cd27ac21c2b8bd474a07f0e58688731be",
    },
    Card {
        id: "deceptive_landscape",
        cost: "{W}{B}{G}",
        name: "Deceptive Landscape",
        mana_cost: "",
        types: &["Land"],
        power: None,
        toughness: None,
        fingerprint: "8ed42a65ab2f72d4e47f09f04acea87eec3a72eb0b6fedefc3069f0fcf014fcb",
    },
    Card {
        id: "foreboding_landscape",
        cost: "{B}{G}{U}",
        name: "Foreboding Landscape",
        mana_cost: "",
        types: &["Land"],
        power: None,
        toughness: None,
        fingerprint: "a6cd82534c1648169bb32b8fa03fb3c9af18abf9c7c35b466f5c02234a90a56d",
    },
    Card {
        id: "perilous_landscape",
        cost: "{U}{R}{W}",
        name: "Perilous Landscape",
        mana_cost: "",
        types: &["Land"],
        power: None,
        toughness: None,
        fingerprint: "0393deb94498826d1d8a1ca54af6f8d406a23ceaea360f7220e9aafcf02c8423",
    },
    Card {
        id: "seething_landscape",
        cost: "{U}{B}{R}",
        name: "Seething Landscape",
        mana_cost: "",
        types: &["Land"],
        power: None,
        toughness: None,
        fingerprint: "b3c134b07a96854042904e473718263a7c90da4ea505ecb708cafb9de766acf7",
    },
    Card {
        id: "shattered_landscape",
        cost: "{R}{W}{B}",
        name: "Shattered Landscape",
        mana_cost: "",
        types: &["Land"],
        power: None,
        toughness: None,
        fingerprint: "9d544ddcd7cafa362b68c82c2f55871bf98767960df47f5fd2d0d784b8560c90",
    },
    Card {
        id: "sheltering_landscape",
        cost: "{R}{G}{W}",
        name: "Sheltering Landscape",
        mana_cost: "",
        types: &["Land"],
        power: None,
        toughness: None,
        fingerprint: "8f6effd82ff072bcb6ef8219c08882cdf27d00c797e7dd9134cd26d99baf2430",
    },
    Card {
        id: "tranquil_landscape",
        cost: "{G}{W}{U}",
        name: "Tranquil Landscape",
        mana_cost: "",
        types: &["Land"],
        power: None,
        toughness: None,
        fingerprint: "a7bd22ae16b9a47f240214a1b94feb29198386b98272cb1e9e35c04254e4332c",
    },
    Card {
        id: "twisted_landscape",
        cost: "{B}{R}{G}",
        name: "Twisted Landscape",
        mana_cost: "",
        types: &["Land"],
        power: None,
        toughness: None,
        fingerprint: "0676afe5815f74af0f2194b9eadf4603443b22bb147916bd88f6b4930eda6f02",
    },
];

#[test]
fn q37_cycling_fetch_lands_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for card in CARDS {
        let definition = registry
            .get(card.id)
            .unwrap_or_else(|| panic!("missing {}", card.id));
        assert_eq!(definition.layout, Layout::Normal, "{}", card.id);
        assert_eq!(registry.id_for_name(card.name), Some(card.id));
        let f = definition.primary_face();
        assert_eq!(f.face_id.as_str(), card.id.replace('-', "_"));
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
        let [mana, fetch, cycling] = f.activated_abilities.as_slice() else {
            panic!("{}: exactly three activated abilities", card.id);
        };
        assert_eq!(mana.ability_id.as_str(), "activated_01");
        assert_eq!(mana.presentation, AbilityPresentation::OracleLines(vec![1]));
        assert_eq!(mana.costs, [AbilityCost::Tap]);
        assert!(matches!(
            mana.effect.as_slice(),
            [SpellEffectKind::ProduceMana { options, .. }]
                if options.len() == 1 && options[0].c == 1
        ));
        assert_eq!(fetch.ability_id.as_str(), "activated_02");
        assert_eq!(
            fetch.presentation,
            AbilityPresentation::OracleLines(vec![2])
        );
        assert_eq!(fetch.costs, [AbilityCost::Tap, AbilityCost::SacrificeSelf]);
        assert!(matches!(
            fetch.effect.as_slice(),
            [SpellEffectKind::SearchLibrary {
                who: PlayerRecipient::Controller,
                optional: false,
                count: 1,
                filter: Some(filter),
                destination: SearchDestination::Battlefield { tapped: true },
                shuffle: true,
                reveal: false,
                ..
            }] if filter.any_of.as_ref().is_some_and(|branches|
                branches.len() == 3
                    && branches.iter().all(|branch|
                        branch.card_type == Some(CardTypeFilter::BasicLand)
                            && branch.required_subtypes.len() == 1))
        ));
        assert_eq!(cycling.ability_id.as_str(), "activated_03");
        assert_eq!(
            cycling.presentation,
            AbilityPresentation::OracleLines(vec![3])
        );
        assert_eq!(cycling.source_zone, AbilitySourceZone::Hand);
        assert!(matches!(
            cycling.costs.as_slice(),
            [AbilityCost::Mana(mana), AbilityCost::DiscardSelf]
                if mana.to_string() == card.cost
        ));
        assert!(matches!(
            cycling.effect.as_slice(),
            [SpellEffectKind::Draw {
                who: PlayerRecipient::Controller,
                count: Amount::Fixed(1)
            }]
        ));
    }
}

#[test]
fn q37_cycling_fetch_lands_batch_fingerprints_match_the_pinned_oracle_text() {
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
