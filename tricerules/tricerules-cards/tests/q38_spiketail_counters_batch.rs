//! Registry conformance for the queue-v38 06-spiketail-counters cohort.
//! Printed identity, costs, typed shape and pinned fingerprints.

use tricerules_cards::primitives::{AbilityCost, Amount, Keyword, SpellEffectKind};

struct Card {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    types: &'static [&'static str],
    power: Option<u32>,
    toughness: Option<u32>,
    payment: u32,
    fingerprint: &'static str,
}

const CARDS: &[Card] = &[
    Card {
        id: "spiketail_drake",
        name: "Spiketail Drake",
        mana_cost: "{3}{U}{U}",
        types: &["Creature", "Drake"],
        power: Some(3),
        toughness: Some(3),
        payment: 3,
        fingerprint: "bf144876b6dc5f6b36c33f69326053f6fa4599e328740a62460bdf7fd9e889cf",
    },
    Card {
        id: "spiketail_drakeling",
        name: "Spiketail Drakeling",
        mana_cost: "{1}{U}{U}",
        types: &["Creature", "Drake"],
        power: Some(2),
        toughness: Some(2),
        payment: 2,
        fingerprint: "9c06455e65dd0ec15e9a766bb29c74964df4f8533e46fae274be4c09f281c01b",
    },
    Card {
        id: "spiketail_hatchling",
        name: "Spiketail Hatchling",
        mana_cost: "{1}{U}",
        types: &["Creature", "Drake"],
        power: Some(1),
        toughness: Some(1),
        payment: 1,
        fingerprint: "c50561584622b7f7dc857000dd62db339ffa7e1e51a8a83f89ad3fcc45c4852a",
    },
];

use tricerules_cards::{AbilityPresentation, Layout};
#[test]
fn q38_spiketail_counters_batch_maps_definitions() {
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
        assert_eq!(f.keywords, [Keyword::Flying]);
        assert!(f.static_abilities.is_empty());
        assert!(f.triggered_abilities.is_empty());
        let [ability] = f.activated_abilities.as_slice() else {
            panic!("{}: exactly one activated ability", card.id)
        };
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![2])
        );
        assert_eq!(ability.costs, [AbilityCost::SacrificeSelf]);
        assert!(matches!(
            ability.effect.as_slice(),
            [SpellEffectKind::CounterTargetSpell { unless_controller_pays: Some(Amount::Fixed(payment)), .. }]
                if *payment == card.payment
        ));
    }
}

#[test]
fn q38_spiketail_counters_batch_fingerprints_match_the_pinned_oracle_text() {
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
