//! Registry conformance for the queue-v26 Banner sacrifice-draw cycle.
//!
//! Every card is a normal-layout `{3}` artifact with a three-option tap mana
//! ability and a three-color, tap-and-sacrifice draw ability. Each fingerprint
//! is SHA-256 of the exact pinned normalized Oracle text.

use tricerules_cards::primitives::{AbilityCost, Amount, PlayerRecipient, SpellEffectKind};
use tricerules_cards::{AbilityPresentation, Layout, ManaCost};

struct Card {
    id: &'static str,
    name: &'static str,
    options: [[u32; 6]; 3],
    cost: &'static str,
    fingerprint: &'static str,
}

const CARDS: &[Card] = &[
    Card {
        id: "abzan_banner",
        name: "Abzan Banner",
        options: [[1, 0, 0, 0, 0, 0], [0, 0, 1, 0, 0, 0], [0, 0, 0, 0, 1, 0]],
        cost: "{W}{B}{G}",
        fingerprint: "78818ea12894aefb32ac80c04b8c3b34560eeafa131003c2b5ec147970093f05",
    },
    Card {
        id: "jeskai_banner",
        name: "Jeskai Banner",
        options: [[0, 1, 0, 0, 0, 0], [0, 0, 0, 1, 0, 0], [1, 0, 0, 0, 0, 0]],
        cost: "{U}{R}{W}",
        fingerprint: "a476f8a89f78b70783a24abffe981b53b5ce218c3ab5b0bb694c0e29acef8d58",
    },
    Card {
        id: "mardu_banner",
        name: "Mardu Banner",
        options: [[0, 0, 0, 1, 0, 0], [1, 0, 0, 0, 0, 0], [0, 0, 1, 0, 0, 0]],
        cost: "{R}{W}{B}",
        fingerprint: "cf46076dc6c506a41317f0f103797ec1a1c86ef9941bd046ccdc240df03a879c",
    },
    Card {
        id: "sultai_banner",
        name: "Sultai Banner",
        options: [[0, 0, 1, 0, 0, 0], [0, 0, 0, 0, 1, 0], [0, 1, 0, 0, 0, 0]],
        cost: "{B}{G}{U}",
        fingerprint: "3ca2b60938ba9d76626c8dbf34a44e61b7037ece4e82541f7bad0511f80835a7",
    },
    Card {
        id: "temur_banner",
        name: "Temur Banner",
        options: [[0, 0, 0, 0, 1, 0], [0, 1, 0, 0, 0, 0], [0, 0, 0, 1, 0, 0]],
        cost: "{G}{U}{R}",
        fingerprint: "dd915ad783762fcf5055c5b4878f62f73ac49eaa6f2b0ca142eec14874c5331e",
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
fn banners_batch_maps_definitions() {
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
        assert_eq!(options.len(), 3, "{}", card.id);
        for (option, expected) in options.iter().zip(card.options) {
            assert_eq!(mask(option), expected, "{}", card.id);
        }

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
                    count: Amount::Fixed(1),
                }]
            ),
            "{}: draw exactly one",
            card.id
        );
    }
}

#[test]
fn banners_batch_fingerprints_match_the_pinned_oracle_text() {
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
