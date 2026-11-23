//! Registry conformance for the reviewed Battlebond opponent-count land cycle.
//!
//! Every included card is a normal-layout land that enters tapped unless its
//! controller has two or more opponents still in the game, then has a
//! two-option `{T}` mana ability. Each fingerprint is SHA-256 of the exact
//! pinned Oracle lines.

use tricerules_cards::primitives::{
    AbilityCost, EntersTappedAffected, GameCondition, RelativePlayerSet, SpellEffectKind,
    StaticAbilityDef,
};
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
        id: "bountiful_promenade",
        name: "Bountiful Promenade",
        options: [(0, 0, 0, 0, 1, 0), (1, 0, 0, 0, 0, 0)],
        fingerprint: "b2da03d150868fe5d46ca40062a7f017b84610a2d110d697baf52a970a532ac5",
    },
    Card {
        id: "luxury_suite",
        name: "Luxury Suite",
        options: [(0, 0, 1, 0, 0, 0), (0, 0, 0, 1, 0, 0)],
        fingerprint: "b42d510b781035079ea7b960f2c5c7f339a300761d9d37c72bff7a5eaa19949e",
    },
    Card {
        id: "morphic_pool",
        name: "Morphic Pool",
        options: [(0, 1, 0, 0, 0, 0), (0, 0, 1, 0, 0, 0)],
        fingerprint: "d2e0b2b7f9a642ee3e0be6471e62e53f000144e2e41079c706dc29d7f6caa289",
    },
    Card {
        id: "rejuvenating_springs",
        name: "Rejuvenating Springs",
        options: [(0, 0, 0, 0, 1, 0), (0, 1, 0, 0, 0, 0)],
        fingerprint: "1d60a348b15ce378de0a446a44baeb027c06d61d0177e1e40226cc4cd8828a52",
    },
    Card {
        id: "sea_of_clouds",
        name: "Sea of Clouds",
        options: [(1, 0, 0, 0, 0, 0), (0, 1, 0, 0, 0, 0)],
        fingerprint: "45879cfed7071ce4b419e076cfb9d652cb73ee5d9ca21b49d1d38eeb3385d5fe",
    },
    Card {
        id: "spectator_seating",
        name: "Spectator Seating",
        options: [(0, 0, 0, 1, 0, 0), (1, 0, 0, 0, 0, 0)],
        fingerprint: "0833a4386ff6e76d8b688f33877065f067ec106380f4bb22b0b3aad085fb2046",
    },
    Card {
        id: "spire_garden",
        name: "Spire Garden",
        options: [(0, 0, 0, 1, 0, 0), (0, 0, 0, 0, 1, 0)],
        fingerprint: "cd41f726294383cd361eb76956e8fc923fa2347906da03325169dedb4ab8c8dd",
    },
    Card {
        id: "training_center",
        name: "Training Center",
        options: [(0, 1, 0, 0, 0, 0), (0, 0, 0, 1, 0, 0)],
        fingerprint: "78d344aaaf33164598c431af53c56cd4ba923087eea560116cfc8b13d1cf6ea7",
    },
    Card {
        id: "undergrowth_stadium",
        name: "Undergrowth Stadium",
        options: [(0, 0, 1, 0, 0, 0), (0, 0, 0, 0, 1, 0)],
        fingerprint: "0083218c53dad6b78126491b9cd5e617f3759ca9872dd9e2dee6217d406f396b",
    },
    Card {
        id: "vault_of_champions",
        name: "Vault of Champions",
        options: [(1, 0, 0, 0, 0, 0), (0, 0, 1, 0, 0, 0)],
        fingerprint: "e8f6b4d116a6f1f36ff563e3a80ea8c15b8587320eac0db15cbe66c33169c5d3",
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
fn battlebond_lands_batch_maps_definitions() {
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
                condition: Some(GameCondition::PlayerCount {
                    players: RelativePlayerSet::Opponents,
                    min: None,
                    max: Some(1),
                }),
                unless_cost: None,
            },
            "{}: enters tapped unless its controller has two or more opponents",
            card.id
        );

        let [mana] = f.activated_abilities.as_slice() else {
            panic!("{}: exactly one activated ability", card.id);
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
    }
}

#[test]
fn battlebond_lands_batch_fingerprints_match_the_pinned_oracle_text() {
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
