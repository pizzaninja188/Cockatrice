//! Registry conformance for the reviewed three-color mana cohort.
//!
//! Every included card is a normal-layout permanent with one `{T}` mana ability
//! producing exactly one mana of each printed color in the printed option order.
//! Each fingerprint is SHA-256 of the exact pinned Oracle lines.

use tricerules_cards::primitives::{AbilityCost, SpellEffectKind};
use tricerules_cards::{AbilityPresentation, CardRegistry, Layout};

type Mana = (u32, u32, u32, u32, u32, u32);

struct Card {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    types: &'static [&'static str],
    power: Option<u32>,
    toughness: Option<u32>,
    options: [Mana; 3],
    fingerprint: &'static str,
}

const CARDS: &[Card] = &[
    Card {
        id: "obelisk_of_bant",
        name: "Obelisk of Bant",
        mana_cost: "{3}",
        types: &["Artifact"],
        power: None,
        toughness: None,
        options: [(0, 0, 0, 0, 1, 0), (1, 0, 0, 0, 0, 0), (0, 1, 0, 0, 0, 0)],
        fingerprint: "0d7ddf034d0304267eaedb1202a7e74d8986aad1c8f14ae855d4696bcac49d3c",
    },
    Card {
        id: "obelisk_of_esper",
        name: "Obelisk of Esper",
        mana_cost: "{3}",
        types: &["Artifact"],
        power: None,
        toughness: None,
        options: [(1, 0, 0, 0, 0, 0), (0, 1, 0, 0, 0, 0), (0, 0, 1, 0, 0, 0)],
        fingerprint: "ac635a6415565acd71bc96f66f6228ff6a510f7cf764ae2b9708a3328f5f8201",
    },
    Card {
        id: "obelisk_of_grixis",
        name: "Obelisk of Grixis",
        mana_cost: "{3}",
        types: &["Artifact"],
        power: None,
        toughness: None,
        options: [(0, 1, 0, 0, 0, 0), (0, 0, 1, 0, 0, 0), (0, 0, 0, 1, 0, 0)],
        fingerprint: "ac0d1c2e85f4487b01832146f8a3a0a6dfaeb4dbf89cccedb326fc9e99754b15",
    },
    Card {
        id: "obelisk_of_jund",
        name: "Obelisk of Jund",
        mana_cost: "{3}",
        types: &["Artifact"],
        power: None,
        toughness: None,
        options: [(0, 0, 1, 0, 0, 0), (0, 0, 0, 1, 0, 0), (0, 0, 0, 0, 1, 0)],
        fingerprint: "1e266cfa528539b29b24b71087c83d368c7abfac736a3f9ce30e48d596b599e8",
    },
    Card {
        id: "obelisk_of_naya",
        name: "Obelisk of Naya",
        mana_cost: "{3}",
        types: &["Artifact"],
        power: None,
        toughness: None,
        options: [(0, 0, 0, 1, 0, 0), (0, 0, 0, 0, 1, 0), (1, 0, 0, 0, 0, 0)],
        fingerprint: "c14c8c6c7b30b99b132ae88e102f299b863e232df1ae0ced5b6b13a5d8bd529d",
    },
    Card {
        id: "druid_of_the_anima",
        name: "Druid of the Anima",
        mana_cost: "{1}{G}",
        types: &["Creature", "Elf", "Druid"],
        power: Some(1),
        toughness: Some(1),
        options: [(0, 0, 0, 1, 0, 0), (0, 0, 0, 0, 1, 0), (1, 0, 0, 0, 0, 0)],
        fingerprint: "c14c8c6c7b30b99b132ae88e102f299b863e232df1ae0ced5b6b13a5d8bd529d",
    },
    Card {
        id: "urborg_elf",
        name: "Urborg Elf",
        mana_cost: "{1}{G}",
        types: &["Creature", "Elf", "Druid"],
        power: Some(1),
        toughness: Some(1),
        options: [(0, 0, 1, 0, 0, 0), (0, 0, 0, 0, 1, 0), (0, 1, 0, 0, 0, 0)],
        fingerprint: "95fed9b259fe64a674fbbc5bfa3afaf250e122da24ccc7f0672941c9f6076fd7",
    },
];

fn face<'a>(registry: &'a CardRegistry, id: &str) -> &'a tricerules_cards::CardFace {
    registry
        .get(id)
        .unwrap_or_else(|| panic!("missing {id}"))
        .primary_face()
}

#[test]
fn tri_mana_obelisks_batch_maps_definitions() {
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
        assert_eq!(
            f.mana_cost.to_string(),
            card.mana_cost,
            "{} mana cost",
            card.id
        );
        assert_eq!(f.types, card.types, "{} types", card.id);
        assert!(f.supertypes.is_empty(), "{} no supertype", card.id);
        assert_eq!(f.power, card.power, "{} power", card.id);
        assert_eq!(f.toughness, card.toughness, "{} toughness", card.id);
        assert!(f.keywords.is_empty(), "{} no keyword", card.id);
        assert!(
            f.static_abilities.is_empty(),
            "{} no static ability",
            card.id
        );
        assert!(f.triggered_abilities.is_empty(), "{} no trigger", card.id);
        assert!(f.spell_effect.is_empty(), "{} no spell effect", card.id);

        let [mana] = f.activated_abilities.as_slice() else {
            panic!("{}: exactly one activated ability", card.id);
        };
        assert_eq!(mana.ability_id.as_str(), "activated_01", "{}", card.id);
        assert_eq!(
            mana.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{} presentation",
            card.id
        );
        assert_eq!(mana.costs, [AbilityCost::Tap], "{} cost", card.id);
        assert!(
            matches!(
                mana.effect.as_slice(),
                [SpellEffectKind::ProduceMana { options, .. }]
                    if options.len() == 3
                        && (options[0].w, options[0].u, options[0].b,
                            options[0].r, options[0].g, options[0].c) == card.options[0]
                        && (options[1].w, options[1].u, options[1].b,
                            options[1].r, options[1].g, options[1].c) == card.options[1]
                        && (options[2].w, options[2].u, options[2].b,
                            options[2].r, options[2].g, options[2].c) == card.options[2]
            ),
            "{}: printed three-color option order",
            card.id
        );
        assert!(mana.targeting.is_none(), "{} targetless", card.id);
    }
}

#[test]
fn tri_mana_obelisks_batch_fingerprints_match_the_pinned_oracle_text() {
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
