//! Registry conformance for the queue-v38 07-tap-power cohort.
//! Printed identity, costs, typed shape and pinned fingerprints.

use tricerules_cards::primitives::{
    AbilityCost, EffectSubject, PowerComparison, SpellEffectKind, TargetKind,
};

struct Card {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    types: &'static [&'static str],
    power: Option<u32>,
    toughness: Option<u32>,
    limit: u32,
    fingerprint: &'static str,
}

const CARDS: &[Card] = &[
    Card {
        id: "aysen_bureaucrats",
        name: "Aysen Bureaucrats",
        mana_cost: "{1}{W}",
        types: &["Creature", "Human", "Advisor"],
        power: Some(1),
        toughness: Some(1),
        limit: 2,
        fingerprint: "86871413195bd39ad2b7e21d94bf630da63c65ea90835626f4cd5c7facf8998b",
    },
    Card {
        id: "epharas_warden",
        name: "Ephara's Warden",
        mana_cost: "{3}{W}",
        types: &["Creature", "Human", "Cleric"],
        power: Some(1),
        toughness: Some(2),
        limit: 3,
        fingerprint: "3c8cc66f12449f78094c2cb286ee89a60d8182a888bb8f8e7627204c163bdd51",
    },
    Card {
        id: "kor_line-slinger",
        name: "Kor Line-Slinger",
        mana_cost: "{1}{W}",
        types: &["Creature", "Kor", "Scout"],
        power: Some(0),
        toughness: Some(1),
        limit: 3,
        fingerprint: "3c8cc66f12449f78094c2cb286ee89a60d8182a888bb8f8e7627204c163bdd51",
    },
];

use tricerules_cards::Layout;
#[test]
fn q38_tap_power_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for card in CARDS {
        let definition = registry
            .get(card.id)
            .unwrap_or_else(|| panic!("missing {}", card.id));
        assert_eq!(definition.layout, Layout::Normal, "{}", card.id);
        assert_eq!(registry.id_for_name(card.name), Some(card.id));
        let f = definition.primary_face();
        assert_eq!(f.face_id.as_str(), card.id.replace('-', "_"));
        assert_eq!(f.name, card.name);
        assert_eq!(f.mana_cost.to_string(), card.mana_cost);
        assert_eq!(
            f.types.iter().map(String::as_str).collect::<Vec<_>>(),
            card.types
        );
        assert_eq!((f.power, f.toughness), (card.power, card.toughness));
        assert!(f.keywords.is_empty());
        assert!(f.static_abilities.is_empty());
        assert!(f.triggered_abilities.is_empty());
        let [ability] = f.activated_abilities.as_slice() else {
            panic!("{}: exactly one activated ability", card.id)
        };
        assert_eq!(ability.costs, [AbilityCost::Tap]);
        assert!(matches!(
            ability.effect.as_slice(),
            [SpellEffectKind::Tap { subject: EffectSubject::Chosen(filter) }]
                if filter.kind == TargetKind::Creature
                    && filter.power == Some(PowerComparison::AtMost(card.limit))
        ));
    }
}

#[test]
fn q38_tap_power_batch_fingerprints_match_the_pinned_oracle_text() {
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
        assert_eq!(fields[2], card.id.replace('-', "_"));
        assert_eq!(
            fields[4], card.fingerprint,
            "fingerprint drift for {}",
            card.id
        );
    }
}
