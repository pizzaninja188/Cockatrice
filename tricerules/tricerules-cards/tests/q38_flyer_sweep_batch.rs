//! Registry conformance for the queue-v38 04-flyer-sweep cohort.
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
        id: "gale_force",
        name: "Gale Force",
        mana_cost: "{4}{G}",
        types: &["Sorcery"],
        power: None,
        toughness: None,
        amount: 5,
        fingerprint: "05f2e3a651580d372443abd891db66c756b8523e8c03ef1076d3849dfe58b48b",
    },
    Card {
        id: "needle_storm",
        name: "Needle Storm",
        mana_cost: "{2}{G}",
        types: &["Sorcery"],
        power: None,
        toughness: None,
        amount: 4,
        fingerprint: "356a1c71bbd4adfaeb798da21764b34cda92c3ef266bdcd70251ef89961e4f41",
    },
    Card {
        id: "squall",
        name: "Squall",
        mana_cost: "{2}{G}",
        types: &["Sorcery"],
        power: None,
        toughness: None,
        amount: 2,
        fingerprint: "e80141d9b6353c59840d300613c58006a9320886f50b53231e10ec3a5c4121f7",
    },
];

use tricerules_cards::Layout;
#[test]
fn q38_flyer_sweep_batch_maps_definitions() {
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
        assert!(f.targeting.is_none());
        assert!(matches!(
            f.spell_effect.as_slice(),
            [SpellEffectKind::DamageAll { amount: Amount::Fixed(damage), kind, .. }]
                if *damage == card.amount
                    && kind.kind == TargetKind::Creature
                    && kind.required_keywords == [Keyword::Flying]
        ));
    }
}

#[test]
fn q38_flyer_sweep_batch_fingerprints_match_the_pinned_oracle_text() {
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
