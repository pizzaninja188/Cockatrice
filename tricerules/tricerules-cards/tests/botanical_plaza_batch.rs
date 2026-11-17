//! Registry conformance for the reviewed Botanical Plaza land cycle.
//!
//! Every included card is a normal-layout land that enters tapped, has a
//! two-option `{T}` mana ability and a `{2}{X}{Y}, {T}, Sacrifice this land:
//! Draw a card` ability. Each fingerprint is SHA-256 of the exact pinned
//! Oracle lines.

use tricerules_cards::primitives::{
    AbilityCost, Amount, EntersTappedAffected, SpellEffectKind, StaticAbilityDef,
};
use tricerules_cards::{AbilityPresentation, Layout};

type Mana = (u32, u32, u32, u32, u32, u32);

struct Card {
    id: &'static str,
    name: &'static str,
    options: [Mana; 2],
    draw_cost: &'static str,
    fingerprint: &'static str,
}

const CARDS: &[Card] = &[
    Card {
        id: "botanical_plaza",
        name: "Botanical Plaza",
        options: [(0, 0, 0, 0, 1, 0), (1, 0, 0, 0, 0, 0)],
        draw_cost: "{2}{G}{W}",
        fingerprint: "73dc8e289d2bccc30162dbf352ed4ca7dc384660708ef087752078f4590e46df",
    },
    Card {
        id: "racers_ring",
        name: "Racers' Ring",
        options: [(0, 0, 0, 1, 0, 0), (0, 0, 0, 0, 1, 0)],
        draw_cost: "{2}{R}{G}",
        fingerprint: "86a07223292b4c90336c0717dc1eaf551689bbe8b44baa817873580b86b24abb",
    },
    Card {
        id: "skybridge_towers",
        name: "Skybridge Towers",
        options: [(1, 0, 0, 0, 0, 0), (0, 1, 0, 0, 0, 0)],
        draw_cost: "{2}{W}{U}",
        fingerprint: "41917ecd9983ccdbf61f12599f4eba38c84df497e63e9f9dbfb17a449cf03dce",
    },
    Card {
        id: "tramway_station",
        name: "Tramway Station",
        options: [(0, 0, 1, 0, 0, 0), (0, 0, 0, 1, 0, 0)],
        draw_cost: "{2}{B}{R}",
        fingerprint: "84501d3c97312fb71b75efe6f850f23bcfe937572cd4bcc0e3d7f7c05c61b06b",
    },
    Card {
        id: "waterfront_district",
        name: "Waterfront District",
        options: [(0, 1, 0, 0, 0, 0), (0, 0, 1, 0, 0, 0)],
        draw_cost: "{2}{U}{B}",
        fingerprint: "7354b1fe959ed7f3bcc58b3e1b24d6848d8b3496000da6b96b738e5236575efa",
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
fn botanical_plaza_batch_maps_definitions() {
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

        let [mana, draw] = f.activated_abilities.as_slice() else {
            panic!("{}: exactly two activated abilities", card.id);
        };
        assert_eq!(mana.ability_id.as_str(), "activated_01", "{}", card.id);
        assert_eq!(
            mana.presentation,
            AbilityPresentation::OracleLines(vec![2]),
            "{} mana presentation",
            card.id
        );
        assert_eq!(mana.costs, [AbilityCost::Tap], "{} mana cost", card.id);
        assert!(
            matches!(
                mana.effect.as_slice(),
                [SpellEffectKind::ProduceMana { options, .. }]
                    if options.len() == 2
                        && [(options[0].w, options[0].u, options[0].b,
                             options[0].r, options[0].g, options[0].c),
                            (options[1].w, options[1].u, options[1].b,
                             options[1].r, options[1].g, options[1].c)] == card.options
            ),
            "{} mana options",
            card.id
        );
        assert!(mana.targeting.is_none(), "{}", card.id);

        assert_eq!(draw.ability_id.as_str(), "activated_02", "{}", card.id);
        assert_eq!(
            draw.presentation,
            AbilityPresentation::OracleLines(vec![3]),
            "{} draw presentation",
            card.id
        );
        assert_eq!(draw.costs.len(), 3, "{} draw costs", card.id);
        assert!(
            matches!(&draw.costs[0], AbilityCost::Mana(cost) if cost.to_string() == card.draw_cost),
            "{} draw mana cost",
            card.id
        );
        assert_eq!(draw.costs[1], AbilityCost::Tap, "{}", card.id);
        assert_eq!(draw.costs[2], AbilityCost::SacrificeSelf, "{}", card.id);
        assert!(
            matches!(
                draw.effect.as_slice(),
                [SpellEffectKind::Draw {
                    count: Amount::Fixed(1),
                    ..
                }]
            ),
            "{} draw effect",
            card.id
        );
        assert!(draw.targeting.is_none(), "{}", card.id);
    }
}

#[test]
fn botanical_plaza_batch_fingerprints_match_the_pinned_oracle_text() {
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
