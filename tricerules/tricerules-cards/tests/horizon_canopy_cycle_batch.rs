//! Registry conformance for the reviewed Horizon Canopy land cycle.
//!
//! Every included card is a normal-layout land with a `{T}, Pay 1 life`
//! two-option mana ability and a `{1}, {T}, Sacrifice this land: Draw a card`
//! ability. Each fingerprint is SHA-256 of the exact pinned Oracle lines.

use tricerules_cards::primitives::{AbilityCost, Amount, PlayerRecipient, SpellEffectKind};
use tricerules_cards::{AbilityPresentation, Layout};

type Mana = (u32, u32, u32, u32, u32, u32);

struct Card {
    id: &'static str,
    name: &'static str,
    options: [Mana; 2],
    fingerprint: &'static str,
}

const CARDS: &[Card] = &[
    Card {
        id: "fiery_islet",
        name: "Fiery Islet",
        options: [(0, 1, 0, 0, 0, 0), (0, 0, 0, 1, 0, 0)],
        fingerprint: "1bac8e2e32b12b577300cb8f4e8c603c7ffe090bce9e2caf12a9e6afe1b4b7d6",
    },
    Card {
        id: "horizon_canopy",
        name: "Horizon Canopy",
        options: [(0, 0, 0, 0, 1, 0), (1, 0, 0, 0, 0, 0)],
        fingerprint: "5f520ec4df627d5c4dd2848edb91060bdc8c834544edaf9e64cd0ca08fee18d1",
    },
    Card {
        id: "nurturing_peatland",
        name: "Nurturing Peatland",
        options: [(0, 0, 1, 0, 0, 0), (0, 0, 0, 0, 1, 0)],
        fingerprint: "f32413e4c695eefeeebb1e0ba09123900e21013fe0b35b444ad34c6e3d4e42ba",
    },
    Card {
        id: "silent_clearing",
        name: "Silent Clearing",
        options: [(1, 0, 0, 0, 0, 0), (0, 0, 1, 0, 0, 0)],
        fingerprint: "eab5f44f1876b7ba27c2dca414137afe09bcff5564785aaa921be65166000438",
    },
    Card {
        id: "sunbaked_canyon",
        name: "Sunbaked Canyon",
        options: [(0, 0, 0, 1, 0, 0), (1, 0, 0, 0, 0, 0)],
        fingerprint: "8b51a4288ebe83aac343bd4addc008f8bc20a50f42a6258a24dfd969255235ff",
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
fn horizon_canopy_cycle_batch_maps_definitions() {
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
        assert!(f.supertypes.is_empty(), "{} no supertype", card.id);
        assert_eq!(f.power, None, "{} has no power", card.id);
        assert_eq!(f.toughness, None, "{} has no toughness", card.id);
        assert!(f.keywords.is_empty(), "{} no keyword", card.id);
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
        assert!(
            matches!(
                mana.costs.as_slice(),
                [AbilityCost::Tap, AbilityCost::PayLife { amount: 1 }]
            ),
            "{} mana costs",
            card.id
        );
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
        assert!(mana.targeting.is_none(), "{} mana targetless", card.id);

        assert_eq!(draw.ability_id.as_str(), "activated_02", "{}", card.id);
        assert_eq!(
            draw.presentation,
            AbilityPresentation::OracleLines(vec![2]),
            "{} draw presentation",
            card.id
        );
        assert_eq!(draw.costs.len(), 3, "{} draw costs", card.id);
        assert!(
            matches!(&draw.costs[0], AbilityCost::Mana(cost) if cost.to_string() == "{1}"),
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
            "{} draw effect",
            card.id
        );
        assert!(draw.targeting.is_none(), "{} draw targetless", card.id);
    }
}

#[test]
fn horizon_canopy_cycle_batch_fingerprints_match_the_pinned_oracle_text() {
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
