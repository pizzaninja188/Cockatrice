//! Registry conformance for the queue-v26 Locket sacrifice-draw cycle.
//!
//! Every card is a normal-layout `{3}` artifact with a two-option tap mana
//! ability and a four-hybrid-symbol, tap-and-sacrifice draw-two ability. Each
//! fingerprint is SHA-256 of the exact pinned normalized Oracle text.

use tricerules_cards::primitives::{AbilityCost, Amount, PlayerRecipient, SpellEffectKind};
use tricerules_cards::{AbilityPresentation, Layout, ManaCost};

struct Card {
    id: &'static str,
    name: &'static str,
    first: [u32; 6],
    second: [u32; 6],
    cost: &'static str,
    fingerprint: &'static str,
}

const CARDS: &[Card] = &[
    Card {
        id: "azorius_locket",
        name: "Azorius Locket",
        first: [1, 0, 0, 0, 0, 0],
        second: [0, 1, 0, 0, 0, 0],
        cost: "{W/U}{W/U}{W/U}{W/U}",
        fingerprint: "99d89857828159b02d876755e08f2d2fb3bf5250a9fe8a196f859cff02e669d5",
    },
    Card {
        id: "boros_locket",
        name: "Boros Locket",
        first: [0, 0, 0, 1, 0, 0],
        second: [1, 0, 0, 0, 0, 0],
        cost: "{R/W}{R/W}{R/W}{R/W}",
        fingerprint: "4b8e63510f829db6c10b4f4977776a6481fd6e2f7501d3baa60666933b53f221",
    },
    Card {
        id: "dimir_locket",
        name: "Dimir Locket",
        first: [0, 1, 0, 0, 0, 0],
        second: [0, 0, 1, 0, 0, 0],
        cost: "{U/B}{U/B}{U/B}{U/B}",
        fingerprint: "e3bf809f121f74514ac992a779b5a2990b80e6af27d628f090f562f85c709183",
    },
    Card {
        id: "golgari_locket",
        name: "Golgari Locket",
        first: [0, 0, 1, 0, 0, 0],
        second: [0, 0, 0, 0, 1, 0],
        cost: "{B/G}{B/G}{B/G}{B/G}",
        fingerprint: "c39b1487817155120cb2c733966cdd172153b0ef4fa0411c1a97f85340d6f39d",
    },
    Card {
        id: "gruul_locket",
        name: "Gruul Locket",
        first: [0, 0, 0, 1, 0, 0],
        second: [0, 0, 0, 0, 1, 0],
        cost: "{R/G}{R/G}{R/G}{R/G}",
        fingerprint: "f22b36485e260dc497e102e4fb2b53812c36b8e5225741686e123f93bef5dd4d",
    },
    Card {
        id: "izzet_locket",
        name: "Izzet Locket",
        first: [0, 1, 0, 0, 0, 0],
        second: [0, 0, 0, 1, 0, 0],
        cost: "{U/R}{U/R}{U/R}{U/R}",
        fingerprint: "4d86561f0c42eb582167bde2c88b6f1505b3f45a3dc4505b3b8883d1101596cd",
    },
    Card {
        id: "orzhov_locket",
        name: "Orzhov Locket",
        first: [1, 0, 0, 0, 0, 0],
        second: [0, 0, 1, 0, 0, 0],
        cost: "{W/B}{W/B}{W/B}{W/B}",
        fingerprint: "2335eddb18a0214548724997b63ac07c0455f9e240fd7540ee609aca2991d1f0",
    },
    Card {
        id: "rakdos_locket",
        name: "Rakdos Locket",
        first: [0, 0, 1, 0, 0, 0],
        second: [0, 0, 0, 1, 0, 0],
        cost: "{B/R}{B/R}{B/R}{B/R}",
        fingerprint: "8d4b6832c2a72f665be7b5bd0b36b58d7c8dd33d19dec525ffbfe762084e1ea2",
    },
    Card {
        id: "selesnya_locket",
        name: "Selesnya Locket",
        first: [0, 0, 0, 0, 1, 0],
        second: [1, 0, 0, 0, 0, 0],
        cost: "{G/W}{G/W}{G/W}{G/W}",
        fingerprint: "967662ed1ef8102d825f53152b98e69999fed7aa44232914d2eb7a68c1052a6c",
    },
    Card {
        id: "simic_locket",
        name: "Simic Locket",
        first: [0, 0, 0, 0, 1, 0],
        second: [0, 1, 0, 0, 0, 0],
        cost: "{G/U}{G/U}{G/U}{G/U}",
        fingerprint: "fe86f6982af1e787789e16f185ee0b85557f9cf4c438dfba8a5832f17f1747de",
    },
];

fn face<'a>(
    registry: &'a tricerules_cards::CardRegistry,
    id: &str,
) -> &'a tricerules_cards::CardFace {
    registry
        .get(id)
        .unwrap_or_else(|| panic!("missing {id}"))
        .primary_face()
}

fn mask(amount: &tricerules_cards::ManaAmount) -> [u32; 6] {
    [amount.w, amount.u, amount.b, amount.r, amount.g, amount.c]
}

#[test]
fn lockets_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for card in CARDS {
        let definition = registry
            .get(card.id)
            .unwrap_or_else(|| panic!("missing {}", card.id));
        assert_eq!(definition.layout, Layout::Normal, "{}", card.id);
        assert_eq!(
            registry.id_for_name(card.name),
            Some(card.id),
            "{}",
            card.id
        );
        let f = face(registry, card.id);
        assert_eq!(f.face_id.as_str(), card.id, "{} face id", card.id);
        assert_eq!(f.name, card.name, "{} name", card.id);
        assert_eq!(f.mana_cost.to_string(), "{3}", "{} mana cost", card.id);
        assert_eq!(f.types, ["Artifact"], "{} types", card.id);
        assert!(f.supertypes.is_empty(), "{}", card.id);
        assert_eq!((f.power, f.toughness), (None, None), "{} body", card.id);
        assert!(f.keywords.is_empty(), "{}", card.id);
        assert!(f.triggered_abilities.is_empty(), "{}", card.id);
        assert!(f.static_abilities.is_empty(), "{}", card.id);
        assert!(f.spell_effect.is_empty(), "{}", card.id);
        assert!(f.targeting.is_none(), "{}", card.id);

        let [mana, draw] = f.activated_abilities.as_slice() else {
            panic!("{}: exactly two activated abilities", card.id);
        };
        assert_eq!(mana.ability_id.as_str(), "activated_01", "{}", card.id);
        assert_eq!(
            mana.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{} mana presentation",
            card.id
        );
        assert_eq!(mana.costs, [AbilityCost::Tap], "{}", card.id);
        assert!(mana.is_mana_ability(), "{}", card.id);
        let [SpellEffectKind::ProduceMana { options, .. }] = mana.effect.as_slice() else {
            panic!("{}: one mana instruction", card.id);
        };
        assert_eq!(options.len(), 2, "{}", card.id);
        assert_eq!(mask(&options[0]), card.first, "{} first option", card.id);
        assert_eq!(mask(&options[1]), card.second, "{} second option", card.id);

        assert_eq!(draw.ability_id.as_str(), "activated_02", "{}", card.id);
        assert_eq!(
            draw.presentation,
            AbilityPresentation::OracleLines(vec![2]),
            "{} draw presentation",
            card.id
        );
        assert_eq!(draw.costs.len(), 3, "{}", card.id);
        assert_eq!(
            draw.costs[0],
            AbilityCost::Mana(ManaCost::parse(card.cost).unwrap()),
            "{}",
            card.id
        );
        assert_eq!(draw.costs[1], AbilityCost::Tap, "{}", card.id);
        assert_eq!(draw.costs[2], AbilityCost::SacrificeSelf, "{}", card.id);
        assert!(
            matches!(
                draw.effect.as_slice(),
                [SpellEffectKind::Draw {
                    who: PlayerRecipient::Controller,
                    count: Amount::Fixed(2),
                }]
            ),
            "{}: draw exactly two",
            card.id
        );
    }
}

#[test]
fn lockets_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for card in CARDS {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{}\t", card.id)))
            .unwrap_or_else(|| panic!("missing fingerprint row for {}", card.id));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[0], card.id);
        assert_eq!(fields[1], card.name);
        assert_eq!(fields[2], card.id);
        assert_eq!(
            fields[4], card.fingerprint,
            "fingerprint drift for {}",
            card.id
        );
    }
}
