//! Registry conformance for the queue-v37 03-prevention-activated cohort.
//! Printed identity, costs, typed shape and pinned fingerprints.

use tricerules_cards::primitives::{AbilityCost, SpellEffectKind};
use tricerules_cards::{AbilityPresentation, Layout};

struct Card {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    types: &'static [&'static str],
    power: Option<u32>,
    toughness: Option<u32>,
    cost: &'static str,
    fingerprint: &'static str,
}

const CARDS: &[Card] = &[
    Card {
        id: "amulet_of_kroog",
        cost: "{2}",
        name: "Amulet of Kroog",
        mana_cost: "{2}",
        types: &["Artifact"],
        power: None,
        toughness: None,
        fingerprint: "01716b19218ea0f95059bef1a7180772968be923d3fed65e06984cf66e05a65d",
    },
    Card {
        id: "split-tail_miko",
        cost: "{W}",
        name: "Split-Tail Miko",
        mana_cost: "{1}{W}",
        types: &["Creature", "Fox", "Cleric"],
        power: Some(1),
        toughness: Some(1),
        fingerprint: "8d2c25cbc7450c54ae76e883b79972a8c7f7783f07ae59ea6546886b45d611e5",
    },
    Card {
        id: "treefolk_healer",
        cost: "{2}{W}",
        name: "Treefolk Healer",
        mana_cost: "{4}{G}",
        types: &["Creature", "Treefolk", "Cleric"],
        power: Some(2),
        toughness: Some(3),
        fingerprint: "1a21dd349da9b54edd740c47d16ab607299a0f868ea395a8a89c33f48a97bce6",
    },
];

#[test]
fn q37_prevention_activated_batch_maps_definitions() {
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
        assert!(f.keywords.is_empty(), "{}: no printed keyword", card.id);
        assert!(f.static_abilities.is_empty(), "{}", card.id);
        assert!(f.triggered_abilities.is_empty(), "{}", card.id);
        let [ability] = f.activated_abilities.as_slice() else {
            panic!("{}: exactly one activated ability", card.id);
        };
        assert_eq!(ability.ability_id.as_str(), "activated_01");
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![1])
        );
        assert!(matches!(
            ability.costs.as_slice(),
            [AbilityCost::Mana(mana), AbilityCost::Tap] if mana.to_string() == card.cost
        ));
        assert!(matches!(
            ability.effect.as_slice(),
            [SpellEffectKind::PreventNextDamage { target, .. }]
                if target.kind == tricerules_cards::primitives::TargetKind::AnyTarget
        ));
    }
}

#[test]
fn q37_prevention_activated_batch_fingerprints_match_the_pinned_oracle_text() {
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
