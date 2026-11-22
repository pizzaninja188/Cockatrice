//! Registry conformance for the reviewed indestructible artifact-land family.
//!
//! Every included card is a normal-layout artifact land with the printed
//! `Indestructible` keyword, an entered-tapped replacement effect and one `{T}`
//! dual mana ability with the printed option order. Each fingerprint is SHA-256
//! of the exact pinned Oracle lines.

use tricerules_cards::primitives::{
    AbilityCost, EntersTappedAffected, SpellEffectKind, StaticAbilityDef,
};
use tricerules_cards::{AbilityPresentation, CardRegistry, Keyword, Layout};

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
        id: "darkmoss_bridge",
        name: "Darkmoss Bridge",
        first: (0, 0, 1, 0, 0, 0),
        second: (0, 0, 0, 0, 1, 0),
        fingerprint: "0e27236e548512eee2ad180c0d2970dda65cd5120269329c10bd593522a26e47",
    },
    Card {
        id: "drossforge_bridge",
        name: "Drossforge Bridge",
        first: (0, 0, 1, 0, 0, 0),
        second: (0, 0, 0, 1, 0, 0),
        fingerprint: "b8580244d632a6d924239a09918df750e0f0d8c8ccbb2cda5d82b35bd595b5cf",
    },
    Card {
        id: "goldmire_bridge",
        name: "Goldmire Bridge",
        first: (1, 0, 0, 0, 0, 0),
        second: (0, 0, 1, 0, 0, 0),
        fingerprint: "a8680eb764a0a8a840443faadcc3c7bfe5077b2baa587c7c84e1d7109d67f9cf",
    },
    Card {
        id: "mistvault_bridge",
        name: "Mistvault Bridge",
        first: (0, 1, 0, 0, 0, 0),
        second: (0, 0, 1, 0, 0, 0),
        fingerprint: "2586f125aae4ede9e7dbe98c6e8b36999cb77e21d53d20351568ea0fd5021a31",
    },
    Card {
        id: "razortide_bridge",
        name: "Razortide Bridge",
        first: (1, 0, 0, 0, 0, 0),
        second: (0, 1, 0, 0, 0, 0),
        fingerprint: "88b7fe095492930a3ffed44a473d29dd24046a3aa52b3205600ac7078453950c",
    },
    Card {
        id: "rustvale_bridge",
        name: "Rustvale Bridge",
        first: (0, 0, 0, 1, 0, 0),
        second: (1, 0, 0, 0, 0, 0),
        fingerprint: "f3a90b736b51807bfe487b377cdd6d2e4b1d2a146fe4251304dc2d07480b8af6",
    },
    Card {
        id: "silverbluff_bridge",
        name: "Silverbluff Bridge",
        first: (0, 1, 0, 0, 0, 0),
        second: (0, 0, 0, 1, 0, 0),
        fingerprint: "0e2e4035bdddc88ed1e80e173fbbfccb3a0fd7ae2bd186b01ce9f8898612292c",
    },
    Card {
        id: "slagwoods_bridge",
        name: "Slagwoods Bridge",
        first: (0, 0, 0, 1, 0, 0),
        second: (0, 0, 0, 0, 1, 0),
        fingerprint: "d5ad3f1a096c68a0d058002d853ff897ae26a95791716c476b1d177a7681d5c1",
    },
    Card {
        id: "tanglepool_bridge",
        name: "Tanglepool Bridge",
        first: (0, 0, 0, 0, 1, 0),
        second: (0, 1, 0, 0, 0, 0),
        fingerprint: "8cfbbc653570c6f22c747ad550c80d00b281378e365dae95a62d3b5a3385c6a5",
    },
    Card {
        id: "thornglint_bridge",
        name: "Thornglint Bridge",
        first: (0, 0, 0, 0, 1, 0),
        second: (1, 0, 0, 0, 0, 0),
        fingerprint: "0a239720e951c819ec63eccc338eacd2b4ee7eb7431dddcda12936c1ff6c2436",
    },
];

fn face<'a>(registry: &'a CardRegistry, id: &str) -> &'a tricerules_cards::CardFace {
    registry
        .get(id)
        .unwrap_or_else(|| panic!("missing {id}"))
        .primary_face()
}

#[test]
fn artifact_lands_bridge_batch_maps_definitions() {
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
        assert_eq!(f.types, ["Artifact", "Land"], "{} types", card.id);
        assert!(f.supertypes.is_empty(), "{} no supertype", card.id);
        assert_eq!(f.power, None, "{} has no power", card.id);
        assert_eq!(f.toughness, None, "{} has no toughness", card.id);
        assert_eq!(f.keywords, [Keyword::Indestructible], "{} keyword", card.id);
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

        let [mana] = f.activated_abilities.as_slice() else {
            panic!("{}: exactly one activated ability", card.id);
        };
        assert_eq!(mana.ability_id.as_str(), "activated_01", "{}", card.id);
        assert_eq!(
            mana.presentation,
            AbilityPresentation::OracleLines(vec![3]),
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
        assert!(mana.targeting.is_none(), "{} targetless", card.id);
    }
}

#[test]
fn artifact_lands_bridge_batch_fingerprints_match_the_pinned_oracle_text() {
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
