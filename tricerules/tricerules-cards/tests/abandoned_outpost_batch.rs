//! Registry conformance for the reviewed Abandoned Outpost land cycle.
//!
//! Every included card is a normal-layout land that enters tapped, has one
//! `{T}` single-color mana ability and one `{T}, Sacrifice this land` ability
//! producing one mana of any color. Each fingerprint is SHA-256 of the exact
//! pinned Oracle lines.

use tricerules_cards::primitives::{
    AbilityCost, EntersTappedAffected, SpellEffectKind, StaticAbilityDef,
};
use tricerules_cards::{AbilityPresentation, Layout};

type Mana = (u32, u32, u32, u32, u32, u32);

const ANY_COLOR: [Mana; 5] = [
    (1, 0, 0, 0, 0, 0),
    (0, 1, 0, 0, 0, 0),
    (0, 0, 1, 0, 0, 0),
    (0, 0, 0, 1, 0, 0),
    (0, 0, 0, 0, 1, 0),
];

struct Card {
    id: &'static str,
    name: &'static str,
    color: Mana,
    fingerprint: &'static str,
}

const CARDS: &[Card] = &[
    Card {
        id: "abandoned_outpost",
        name: "Abandoned Outpost",
        color: (1, 0, 0, 0, 0, 0),
        fingerprint: "95950cc1a2f5dd0f8542f865af2657401165d6e40c7ae6bf54cca622d4f47530",
    },
    Card {
        id: "bog_wreckage",
        name: "Bog Wreckage",
        color: (0, 0, 1, 0, 0, 0),
        fingerprint: "e5f3fd5a5f07f53a560cd7232eb7e842c8febd5ae12c72b663bfdc3d8e332730",
    },
    Card {
        id: "ravaged_highlands",
        name: "Ravaged Highlands",
        color: (0, 0, 0, 1, 0, 0),
        fingerprint: "1ca8350c687fe0460ba4eaf27d4905e2bb5a055bfa8ecc8ca86ce170fe7207dd",
    },
    Card {
        id: "seafloor_debris",
        name: "Seafloor Debris",
        color: (0, 1, 0, 0, 0, 0),
        fingerprint: "e8b1702531ae7583202675fbf5e85da0c2f8f2619503a2803e120da95deefdbd",
    },
    Card {
        id: "timberland_ruins",
        name: "Timberland Ruins",
        color: (0, 0, 0, 0, 1, 0),
        fingerprint: "abe05becff10202eded755c6481cb58fed537b139042623830ddfe86945c388a",
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

#[test]
fn abandoned_outpost_batch_maps_definitions() {
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
        assert_eq!(f.mana_cost.to_string(), "", "{} mana cost", card.id);
        assert_eq!(f.types, ["Land"], "{} types", card.id);
        assert!(f.supertypes.is_empty(), "{}", card.id);
        assert_eq!(f.power, None, "{}", card.id);
        assert_eq!(f.toughness, None, "{}", card.id);
        assert!(f.keywords.is_empty(), "{}", card.id);
        assert!(f.triggered_abilities.is_empty(), "{}", card.id);
        assert!(f.spell_effect.is_empty(), "{}", card.id);
        assert!(f.targeting.is_none(), "{}", card.id);

        let [entry] = f.static_abilities.as_slice() else {
            panic!("{}: exactly one static ability", card.id);
        };
        assert_eq!(entry.ability_id.as_str(), "static_01", "{}", card.id);
        assert_eq!(
            entry.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{} entry presentation",
            card.id
        );
        assert_eq!(
            entry.definition,
            StaticAbilityDef::EntersTapped {
                affected: EntersTappedAffected::Self_,
                condition: None,
                unless_cost: None,
            },
            "{}",
            card.id
        );

        let [single, any] = f.activated_abilities.as_slice() else {
            panic!("{}: exactly two activated abilities", card.id);
        };
        assert_eq!(single.ability_id.as_str(), "activated_01", "{}", card.id);
        assert_eq!(
            single.presentation,
            AbilityPresentation::OracleLines(vec![2]),
            "{} single presentation",
            card.id
        );
        assert_eq!(single.costs, [AbilityCost::Tap], "{} single cost", card.id);
        assert!(
            matches!(
                single.effect.as_slice(),
                [SpellEffectKind::ProduceMana { options, .. }]
                    if options.len() == 1
                        && (options[0].w, options[0].u, options[0].b,
                            options[0].r, options[0].g, options[0].c) == card.color
            ),
            "{} single mana",
            card.id
        );
        assert!(single.targeting.is_none(), "{}", card.id);

        assert_eq!(any.ability_id.as_str(), "activated_02", "{}", card.id);
        assert_eq!(
            any.presentation,
            AbilityPresentation::OracleLines(vec![3]),
            "{} any-color presentation",
            card.id
        );
        assert_eq!(
            any.costs,
            [AbilityCost::Tap, AbilityCost::SacrificeSelf],
            "{} any-color costs",
            card.id
        );
        assert!(
            matches!(
                any.effect.as_slice(),
                [SpellEffectKind::ProduceMana { options, .. }]
                    if options.len() == 5
                        && ANY_COLOR.iter().all(|expected| options.iter().any(|option|
                            (option.w, option.u, option.b, option.r, option.g, option.c) == *expected))
            ),
            "{} any-color mana",
            card.id
        );
        assert!(any.targeting.is_none(), "{}", card.id);
    }
}

#[test]
fn abandoned_outpost_batch_fingerprints_match_the_pinned_oracle_text() {
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
