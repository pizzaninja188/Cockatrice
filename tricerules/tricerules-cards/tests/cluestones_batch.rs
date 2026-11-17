//! Registry conformance for the queue-v26 Cluestone sacrifice-draw cycle.
//!
//! Every card is a normal-layout `{3}` artifact with a two-option tap mana
//! ability and a two-color, tap-and-sacrifice draw ability. Each fingerprint is
//! SHA-256 of the exact pinned normalized Oracle text.

use tricerules_cards::primitives::{AbilityCost, Amount, PlayerRecipient, SpellEffectKind};
use tricerules_cards::{AbilityPresentation, Layout};

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
        id: "azorius_cluestone",
        name: "Azorius Cluestone",
        first: [1, 0, 0, 0, 0, 0],
        second: [0, 1, 0, 0, 0, 0],
        cost: "{W}{U}",
        fingerprint: "d7abf07c1c4d859487e1cbdb2fba9c7b284a13262237a48b42782e835f97308f",
    },
    Card {
        id: "boros_cluestone",
        name: "Boros Cluestone",
        first: [0, 0, 0, 1, 0, 0],
        second: [1, 0, 0, 0, 0, 0],
        cost: "{R}{W}",
        fingerprint: "7b1cef1a6a71e4a9dbbd883f5b96a05d3ead48ddc063de60e97a33830c45c6b1",
    },
    Card {
        id: "dimir_cluestone",
        name: "Dimir Cluestone",
        first: [0, 1, 0, 0, 0, 0],
        second: [0, 0, 1, 0, 0, 0],
        cost: "{U}{B}",
        fingerprint: "f8f683fcee62e4884b955e4c4e427724209116cf0801ac93c7cbffaa5ef3baaf",
    },
    Card {
        id: "golgari_cluestone",
        name: "Golgari Cluestone",
        first: [0, 0, 1, 0, 0, 0],
        second: [0, 0, 0, 0, 1, 0],
        cost: "{B}{G}",
        fingerprint: "c3d9652c7ed1c10e9377c23196d301ff1db96241e5b45020a66bc96182f705b3",
    },
    Card {
        id: "gruul_cluestone",
        name: "Gruul Cluestone",
        first: [0, 0, 0, 1, 0, 0],
        second: [0, 0, 0, 0, 1, 0],
        cost: "{R}{G}",
        fingerprint: "d67c7f9333fd94b1cd3aba046519f3177665d88a6cb066f454ef8ec4206b41ea",
    },
    Card {
        id: "izzet_cluestone",
        name: "Izzet Cluestone",
        first: [0, 1, 0, 0, 0, 0],
        second: [0, 0, 0, 1, 0, 0],
        cost: "{U}{R}",
        fingerprint: "8e06a463cc41fd1b05f630c20fbf4233cdcda1fa600069da396fdf5a48ddfc28",
    },
    Card {
        id: "orzhov_cluestone",
        name: "Orzhov Cluestone",
        first: [1, 0, 0, 0, 0, 0],
        second: [0, 0, 1, 0, 0, 0],
        cost: "{W}{B}",
        fingerprint: "fb0c6bf2f639aa653158783b9f269b71d95797a4b57131b3bb9dba21574171a7",
    },
    Card {
        id: "rakdos_cluestone",
        name: "Rakdos Cluestone",
        first: [0, 0, 1, 0, 0, 0],
        second: [0, 0, 0, 1, 0, 0],
        cost: "{B}{R}",
        fingerprint: "152cbc69d9827dbb1b5931193399d0fc15060fca9c18729d032f274c27eb14ac",
    },
    Card {
        id: "selesnya_cluestone",
        name: "Selesnya Cluestone",
        first: [0, 0, 0, 0, 1, 0],
        second: [1, 0, 0, 0, 0, 0],
        cost: "{G}{W}",
        fingerprint: "1ce69e7f6c422aa823856f05607b634d908a71d4f92ff677bc58eb4fd3cbe8f4",
    },
    Card {
        id: "simic_cluestone",
        name: "Simic Cluestone",
        first: [0, 0, 0, 0, 1, 0],
        second: [0, 1, 0, 0, 0, 0],
        cost: "{G}{U}",
        fingerprint: "06f5c0a9a5babfc4ae0e088098ec31b647ca17891cbf92dae493494972d30445",
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
fn cluestones_batch_maps_definitions() {
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
            AbilityCost::Mana(tricerules_cards::mana::ManaCost::parse(card.cost).unwrap()),
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
                    count: Amount::Fixed(1),
                }]
            ),
            "{}: draw exactly one",
            card.id
        );
    }
}

#[test]
fn cluestones_batch_fingerprints_match_the_pinned_oracle_text() {
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
