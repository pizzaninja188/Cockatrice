//! Registry conformance for the reviewed investigate dual-land family.
//!
//! Every included card is a normal-layout land that enters tapped, has one
//! `{T}` dual mana ability with the printed option order and one
//! `{4}, {T}: Investigate` ability that creates the shipped Clue token.
//! Each fingerprint is SHA-256 of the exact pinned Oracle lines.

use tricerules_cards::primitives::{
    AbilityCost, Amount, EntersTappedAffected, PlayerRecipient, SpellEffectKind, StaticAbilityDef,
};
use tricerules_cards::{AbilityPresentation, CardRegistry, Layout};

type Mana = (u32, u32, u32, u32, u32, u32);

struct Card {
    id: &'static str,
    name: &'static str,
    first: Mana,
    second: Mana,
    fingerprint: &'static str,
}

const CARDS: &[Card] = &[
    Card {
        id: "ballroom",
        name: "Ballroom",
        first: (1, 0, 0, 0, 0, 0),
        second: (0, 0, 1, 0, 0, 0),
        fingerprint: "cfc087f2f5c02e9e2fcfc94f5b4213d15680af5594170dc09b8bf398f42d401f",
    },
    Card {
        id: "billiard_room",
        name: "Billiard Room",
        first: (0, 0, 1, 0, 0, 0),
        second: (0, 0, 0, 1, 0, 0),
        fingerprint: "2f2723f99a86d6c315733b9bf443a0b29995c8c35c04367d871c34a06c0528a4",
    },
    Card {
        id: "conservatory",
        name: "Conservatory",
        first: (0, 0, 0, 0, 1, 0),
        second: (1, 0, 0, 0, 0, 0),
        fingerprint: "f3ad07b2fde280406afe7942de30f00f4b318c0527876b3c38805493537b30fb",
    },
    Card {
        id: "dining_room",
        name: "Dining Room",
        first: (0, 0, 0, 1, 0, 0),
        second: (0, 0, 0, 0, 1, 0),
        fingerprint: "ceef2f7930f00b0d92bfff50f3022efccf97a1f493f5eaebd3592094624136e4",
    },
    Card {
        id: "hall",
        name: "Hall",
        first: (0, 0, 0, 1, 0, 0),
        second: (1, 0, 0, 0, 0, 0),
        fingerprint: "3018e63b23c485397e813f5272c3224c139c919621a793cf2998f29ae7551a6f",
    },
    Card {
        id: "kitchen",
        name: "Kitchen",
        first: (0, 0, 0, 0, 1, 0),
        second: (0, 1, 0, 0, 0, 0),
        fingerprint: "b7da707659d4ca46bb45f2dc66a84d1cc0f9f9515d5188bcdbcd85b995f8fc15",
    },
    Card {
        id: "library",
        name: "Library",
        first: (0, 1, 0, 0, 0, 0),
        second: (0, 0, 0, 1, 0, 0),
        fingerprint: "1e8e25ee4a47a475ae729448029067c253dc3f317f2230c3a020d464973060ad",
    },
    Card {
        id: "lounge",
        name: "Lounge",
        first: (0, 0, 1, 0, 0, 0),
        second: (0, 0, 0, 0, 1, 0),
        fingerprint: "2e0dac517189a58bf5c6107c44cf2ff47788b0f73ac56e14aebeeb6ced80d0af",
    },
    Card {
        id: "secret_passage",
        name: "Secret Passage",
        first: (0, 1, 0, 0, 0, 0),
        second: (0, 0, 1, 0, 0, 0),
        fingerprint: "727fccc74df4c73c3633498c2e0b81469250933888a42d4ed8435d730c818e29",
    },
    Card {
        id: "study",
        name: "Study",
        first: (1, 0, 0, 0, 0, 0),
        second: (0, 1, 0, 0, 0, 0),
        fingerprint: "862b6924bd348ccbb1d82be1c1a872f9d79ef7d45a583ffee8dfd474d5de694c",
    },
];

fn face<'a>(registry: &'a CardRegistry, id: &str) -> &'a tricerules_cards::CardFace {
    registry
        .get(id)
        .unwrap_or_else(|| panic!("missing {id}"))
        .primary_face()
}

#[test]
fn investigate_lands_batch_maps_definitions() {
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
        assert!(f.keywords.is_empty(), "{} has no printed keyword", card.id);
        assert!(f.triggered_abilities.is_empty(), "{}", card.id);
        assert!(f.spell_effect.is_empty(), "{}", card.id);

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

        let [mana, investigate] = f.activated_abilities.as_slice() else {
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
                        && (options[0].w, options[0].u, options[0].b,
                            options[0].r, options[0].g, options[0].c) == card.first
                        && (options[1].w, options[1].u, options[1].b,
                            options[1].r, options[1].g, options[1].c) == card.second
            ),
            "{}: printed dual option order",
            card.id
        );
        assert!(mana.targeting.is_none(), "{} mana targetless", card.id);

        assert_eq!(
            investigate.ability_id.as_str(),
            "activated_02",
            "{}",
            card.id
        );
        assert_eq!(
            investigate.presentation,
            AbilityPresentation::OracleLines(vec![3]),
            "{} investigate presentation",
            card.id
        );
        assert!(
            matches!(
                investigate.costs.as_slice(),
                [AbilityCost::Mana(cost), AbilityCost::Tap] if cost.to_string() == "{4}"
            ),
            "{} investigate cost",
            card.id
        );
        assert!(
            matches!(
                investigate.effect.as_slice(),
                [SpellEffectKind::Investigate {
                    count: Amount::Fixed(1),
                    who: PlayerRecipient::Controller,
                }]
            ),
            "{}: investigate creates exactly one Clue",
            card.id
        );
        assert!(
            investigate.targeting.is_none(),
            "{} investigate targetless",
            card.id
        );
    }
}

#[test]
fn investigate_lands_batch_fingerprints_match_the_pinned_oracle_text() {
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
