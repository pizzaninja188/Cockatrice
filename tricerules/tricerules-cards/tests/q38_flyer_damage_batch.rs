//! Registry conformance for the queue-v38 03-flyer-damage cohort.
//! Printed identity, costs, typed shape and pinned fingerprints.

use tricerules_cards::primitives::{Amount, Keyword, SpellEffectKind, TargetKind};

struct Card {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    types: &'static [&'static str],
    power: Option<u32>,
    toughness: Option<u32>,
    amount: u32,
    fingerprint: &'static str,
}

const CARDS: &[Card] = &[
    Card {
        id: "leaf_arrow",
        name: "Leaf Arrow",
        mana_cost: "{G}",
        types: &["Instant"],
        power: None,
        toughness: None,
        amount: 3,
        fingerprint: "13ae8dd0f2d48599b83e177caadc56d890faf940ed19de44a45257bcb36739d6",
    },
    Card {
        id: "pierce_the_sky",
        name: "Pierce the Sky",
        mana_cost: "{1}{G}",
        types: &["Instant"],
        power: None,
        toughness: None,
        amount: 7,
        fingerprint: "caaa4fd1351d17a6a3207c0b431c70f3472cdc0eb83defdbdc2677e5e9730ac0",
    },
    Card {
        id: "shredding_winds",
        name: "Shredding Winds",
        mana_cost: "{2}{G}",
        types: &["Instant"],
        power: None,
        toughness: None,
        amount: 7,
        fingerprint: "e4281dd885e1d67080a6e1d07167ad7e48ac7b73e5447fa1068e8729f2edf6e6",
    },
];

use tricerules_cards::Layout;
#[test]
fn q38_flyer_damage_batch_maps_definitions() {
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
        assert!(f.activated_abilities.is_empty());
        assert!(matches!(
            f.spell_effect.as_slice(),
            [SpellEffectKind::DamageTarget { amount: Amount::Fixed(damage), target }]
                if *damage == card.amount
                    && target.kind == TargetKind::Creature
                    && target.required_keywords == [Keyword::Flying]
        ));
    }
}

#[test]
fn q38_flyer_damage_batch_fingerprints_match_the_pinned_oracle_text() {
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
