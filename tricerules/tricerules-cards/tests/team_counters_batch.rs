//! Registry conformance for the queue-v28 team +1/+1 counter cohort.
//!
//! Each card is a normal-layout printed sorcery whose complete typed shape is asserted
//! against the pinned Oracle text. Fingerprints are SHA-256 of the exact normalized
//! Oracle lines.

use tricerules_cards::primitives::{
    Amount, CreatureScopeController, CreatureScopeFilter, SpellEffectKind,
};
use tricerules_cards::{CounterKind, Layout};

struct Card {
    id: &'static str,
    name: &'static str,
    mana: &'static str,
    types: &'static [&'static str],
    fingerprint: &'static str,
}

const CARDS: &[Card] = &[
    Card {
        id: "basris_solidarity",
        name: "Basri's Solidarity",
        mana: "{1}{W}",
        types: &["Sorcery"],
        fingerprint: "eb20322f85ef9ba3cf2c4ef53fab8ab7bfc8099c181d56e3d5f7eac85367ad42",
    },
    Card {
        id: "titanias_boon",
        name: "Titania's Boon",
        mana: "{3}{G}",
        types: &["Sorcery"],
        fingerprint: "eb20322f85ef9ba3cf2c4ef53fab8ab7bfc8099c181d56e3d5f7eac85367ad42",
    },
    Card {
        id: "inspiring_roar",
        name: "Inspiring Roar",
        mana: "{3}{W}",
        types: &["Sorcery"],
        fingerprint: "eb20322f85ef9ba3cf2c4ef53fab8ab7bfc8099c181d56e3d5f7eac85367ad42",
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

fn expected_filter() -> CreatureScopeFilter {
    CreatureScopeFilter {
        controller: Some(CreatureScopeController::YouControl),
        ..CreatureScopeFilter::default()
    }
}

#[test]
fn team_counters_batch_maps_definitions() {
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
        assert_eq!(definition.layout, Layout::Normal, "{}", card.id);
        assert!(f.activated_abilities.is_empty(), "{}", card.id);
        assert!(f.triggered_abilities.is_empty(), "{}", card.id);
        assert!(f.static_abilities.is_empty(), "{}", card.id);
        assert!(f.targeting.is_none(), "{}", card.id);
        assert_eq!(
            f.spell_effect.as_slice(),
            [SpellEffectKind::PutCountersAll {
                counter: CounterKind::PlusOnePlusOne,
                count: Amount::Fixed(1),
                filter: expected_filter(),
            }],
            "{}",
            card.id
        );
    }
}

#[test]
fn team_counters_batch_fingerprints_match_the_pinned_oracle_text() {
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
