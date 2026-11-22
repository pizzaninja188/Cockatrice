//! Registry conformance for the queue-v37 01-panorama-lands cohort.
//! Printed identity, costs, typed shape and pinned fingerprints.

use tricerules_cards::primitives::{
    AbilityCost, CardTypeFilter, PlayerRecipient, SearchDestination, SpellEffectKind,
};
use tricerules_cards::{AbilityPresentation, AbilitySourceZone, Layout};

struct Card {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    types: &'static [&'static str],
    power: Option<u32>,
    toughness: Option<u32>,
    fingerprint: &'static str,
}

const CARDS: &[Card] = &[
    Card {
        id: "bant_panorama",
        name: "Bant Panorama",
        mana_cost: "",
        types: &["Land"],
        power: None,
        toughness: None,
        fingerprint: "2b1438dd2b13e3f29917452a0e75a7e212bf13def32bf818081f7c15cd104d3c",
    },
    Card {
        id: "esper_panorama",
        name: "Esper Panorama",
        mana_cost: "",
        types: &["Land"],
        power: None,
        toughness: None,
        fingerprint: "38397e805cb377851ad276d006fc59686fda77d542fa1d4e5899250372786ecc",
    },
    Card {
        id: "grixis_panorama",
        name: "Grixis Panorama",
        mana_cost: "",
        types: &["Land"],
        power: None,
        toughness: None,
        fingerprint: "ad565584830510266787cf5b83bfd50f839f4cf75caf4060cc7f6840ddc49744",
    },
    Card {
        id: "jund_panorama",
        name: "Jund Panorama",
        mana_cost: "",
        types: &["Land"],
        power: None,
        toughness: None,
        fingerprint: "d8da196eb2f89f65e910ad70e517c6c7c5b2df3fb3e1ef9baea5be58eaa63020",
    },
    Card {
        id: "naya_panorama",
        name: "Naya Panorama",
        mana_cost: "",
        types: &["Land"],
        power: None,
        toughness: None,
        fingerprint: "3fcb6ba58593548834f31951da7bc5af12d35bcd8b770acb938956c6558b578f",
    },
];

#[test]
fn q37_panorama_lands_batch_maps_definitions() {
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
        let [mana, search] = f.activated_abilities.as_slice() else {
            panic!("{}: exactly two activated abilities", card.id);
        };
        assert_eq!(mana.ability_id.as_str(), "activated_01");
        assert_eq!(mana.presentation, AbilityPresentation::OracleLines(vec![1]));
        assert_eq!(mana.source_zone, AbilitySourceZone::Battlefield);
        assert_eq!(mana.costs, [AbilityCost::Tap]);
        assert!(matches!(
            mana.effect.as_slice(),
            [SpellEffectKind::ProduceMana { options, .. }]
                if options.len() == 1 && options[0].c == 1
        ));
        assert_eq!(search.ability_id.as_str(), "activated_02");
        assert_eq!(
            search.presentation,
            AbilityPresentation::OracleLines(vec![2])
        );
        assert!(matches!(
            search.costs.as_slice(),
            [AbilityCost::Mana(mana), AbilityCost::Tap, AbilityCost::SacrificeSelf]
                if mana.to_string() == "{1}"
        ));
        assert!(matches!(
            search.effect.as_slice(),
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
    }
}

#[test]
fn q37_panorama_lands_batch_fingerprints_match_the_pinned_oracle_text() {
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
