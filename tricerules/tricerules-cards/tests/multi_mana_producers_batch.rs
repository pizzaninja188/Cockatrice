//! Registry conformance for the queue-v28 multi-mana producer cohort.
//!
//! Each card is a normal-layout printed creature whose complete typed shape is asserted
//! against the pinned Oracle text. Fingerprints are SHA-256 of the exact normalized
//! Oracle lines.

use tricerules_cards::primitives::{AbilityCost, SpellEffectKind};
use tricerules_cards::Layout;

struct Card {
    id: &'static str,
    name: &'static str,
    mana: &'static str,
    types: &'static [&'static str],
    power: u32,
    toughness: u32,
    fingerprint: &'static str,
}

const CARDS: &[Card] = &[
    Card {
        id: "fyndhorn_elder",
        name: "Fyndhorn Elder",
        mana: "{2}{G}",
        types: &["Creature", "Elf", "Druid"],
        power: 1,
        toughness: 1,
        fingerprint: "6fa4efbae51e3a8893c0ca5a61be515233f4381b1cd151ba71b367a4114a8789",
    },
    Card {
        id: "greenweaver_druid",
        name: "Greenweaver Druid",
        mana: "{2}{G}",
        types: &["Creature", "Elf", "Druid"],
        power: 1,
        toughness: 1,
        fingerprint: "6fa4efbae51e3a8893c0ca5a61be515233f4381b1cd151ba71b367a4114a8789",
    },
    Card {
        id: "gyre_engineer",
        name: "Gyre Engineer",
        mana: "{1}{G}{U}",
        types: &["Creature", "Vedalken", "Wizard"],
        power: 1,
        toughness: 1,
        fingerprint: "496602dee1500fed7439fb77721eba8ccd1dfb3f07cbbcb226483ca0d831607d",
    },
    Card {
        id: "nantuko_elder",
        name: "Nantuko Elder",
        mana: "{2}{G}",
        types: &["Creature", "Insect", "Druid"],
        power: 1,
        toughness: 2,
        fingerprint: "360d9eb0dcbf7d6c49ebe7e4fe4b3ced9415f916fd8949878192de24de635b61",
    },
    Card {
        id: "llanowar_tribe",
        name: "Llanowar Tribe",
        mana: "{G}{G}{G}",
        types: &["Creature", "Elf", "Druid"],
        power: 3,
        toughness: 3,
        fingerprint: "5ca021e082b82b1c312bd9ef30287e4746b49fe90deba6a566eac1448449e9f1",
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
fn multi_mana_producers_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for card in CARDS {
        let definition = registry
            .get(card.id)
            .unwrap_or_else(|| panic!("missing {}", card.id));
        assert_eq!(
            registry.id_for_name(card.name),
            Some(card.id),
            "{}",
            card.id
        );
        let f = face(registry, card.id);
        assert_eq!(f.face_id.as_str(), card.id, "{} face id", card.id);
        assert_eq!(f.name, card.name, "{} name", card.id);
        assert_eq!(f.mana_cost.to_string(), card.mana, "{} mana", card.id);
        assert_eq!(f.types, card.types, "{} types", card.id);
        assert_eq!(f.power, Some(card.power), "{} power", card.id);
        assert_eq!(f.toughness, Some(card.toughness), "{} toughness", card.id);
        assert_eq!(definition.layout, Layout::Normal, "{}", card.id);
        assert!(f.triggered_abilities.is_empty(), "{}", card.id);
        assert!(f.static_abilities.is_empty(), "{}", card.id);
        assert!(f.spell_effect.is_empty(), "{}", card.id);
        assert!(f.targeting.is_none(), "{}", card.id);
        let [ability] = f.activated_abilities.as_slice() else {
            panic!("{}: exactly one activated ability", card.id);
        };
        assert_eq!(ability.costs, [AbilityCost::Tap], "{}", card.id);
        assert!(ability.targeting.is_none(), "{}", card.id);
        assert!(ability.conditions.is_empty(), "{}", card.id);
        let [effect] = ability.effect.as_slice() else {
            panic!("{}: exactly one mana effect", card.id);
        };
        assert!(
            matches!(effect, SpellEffectKind::ProduceMana { .. }),
            "{}: produces mana",
            card.id
        );
    }
}

#[test]
fn multi_mana_producers_batch_fingerprints_match_the_pinned_oracle_text() {
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
