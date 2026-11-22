//! Registry conformance for the queue-v36 enters-the-battlefield mill cohort.
use tricerules_cards::primitives::{Amount, SpellEffectKind, TriggerCondition};
use tricerules_cards::Layout;

struct Card {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    fingerprint: &'static str,
    types: &'static [&'static str],
    power: u32,
    toughness: u32,
    mill: u32,
}

const CARDS: &[Card] = &[
    Card {
        id: "armored_skaab",
        name: "Armored Skaab",
        mana_cost: "{2}{U}",
        fingerprint: "0ae83c579e549aaf607e387c681da54c5c7e5b70f5a5d6040306c906c53105aa",
        types: &["Creature", "Zombie", "Warrior"],
        power: 1,
        toughness: 4,
        mill: 4,
    },
    Card {
        id: "laboratory_brute",
        name: "Laboratory Brute",
        mana_cost: "{3}{U}",
        fingerprint: "0ae83c579e549aaf607e387c681da54c5c7e5b70f5a5d6040306c906c53105aa",
        types: &["Creature", "Zombie", "Horror"],
        power: 3,
        toughness: 3,
        mill: 4,
    },
    Card {
        id: "necromancers_assistant",
        name: "Necromancer's Assistant",
        mana_cost: "{2}{B}",
        fingerprint: "508640cb4aa9f286ae8a03aff458b123c7480ccaa4c8d7fe5f47b32b70a4c5ec",
        types: &["Creature", "Zombie"],
        power: 3,
        toughness: 1,
        mill: 3,
    },
    Card {
        id: "screeching_skaab",
        name: "Screeching Skaab",
        mana_cost: "{1}{U}",
        fingerprint: "93e3ec099dcf8d056ccd9c4afbc0ca1ade78b9021cb03659fae801924737e2e9",
        types: &["Creature", "Zombie"],
        power: 2,
        toughness: 1,
        mill: 2,
    },
    Card {
        id: "sultai_skullkeeper",
        name: "Sultai Skullkeeper",
        mana_cost: "{1}{U}",
        fingerprint: "93e3ec099dcf8d056ccd9c4afbc0ca1ade78b9021cb03659fae801924737e2e9",
        types: &["Creature", "Snake", "Shaman"],
        power: 2,
        toughness: 1,
        mill: 2,
    },
    Card {
        id: "wailing_ghoul",
        name: "Wailing Ghoul",
        mana_cost: "{1}{B}",
        fingerprint: "f74044e86692718c8d70828bf8e6a1a7b41b2ce939a33052c45a7ec07611685b",
        types: &["Creature", "Zombie"],
        power: 1,
        toughness: 3,
        mill: 2,
    },
];

#[test]
fn q36_etb_mill_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for card in CARDS {
        let definition = registry
            .get(card.id)
            .unwrap_or_else(|| panic!("missing {}", card.id));
        assert_eq!(definition.layout, Layout::Normal, "{}", card.id);
        assert_eq!(registry.id_for_name(card.name), Some(card.id));
        let f = definition.primary_face();
        assert_eq!(f.face_id.as_str(), card.id);
        assert_eq!(f.name, card.name);
        assert_eq!(f.mana_cost.to_string(), card.mana_cost);
        assert_eq!(
            f.types.iter().map(String::as_str).collect::<Vec<_>>(),
            card.types.to_vec()
        );
        assert_eq!(
            (f.power, f.toughness),
            (Some(card.power), Some(card.toughness)),
            "{}",
            card.id
        );
        assert!(f.keywords.is_empty(), "{}", card.id);
        assert!(f.activated_abilities.is_empty(), "{}", card.id);
        assert_eq!(f.triggered_abilities.len(), 1, "{}", card.id);
        let trigger = &f.triggered_abilities[0];
        assert_eq!(trigger.trigger, TriggerCondition::WhenSelfEntersBattlefield);
        assert_eq!(trigger.effect.len(), 1, "{}", card.id);
        assert!(
            matches!(
                &trigger.effect[0],
                SpellEffectKind::Mill { count: Amount::Fixed(n), .. } if *n == card.mill
            ),
            "{}: printed mill count",
            card.id
        );
    }
}

#[test]
fn q36_etb_mill_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for card in CARDS {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{}\t", card.id)))
            .unwrap_or_else(|| panic!("missing fingerprint row for {}", card.id));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "{}", card.id);
        assert_eq!(fields[0], card.id);
        assert_eq!(fields[1], card.name);
        assert_eq!(fields[2], card.id, "face id drift for {}", card.id);
        assert_eq!(
            fields[4], card.fingerprint,
            "fingerprint drift for {}",
            card.id
        );
    }
}
