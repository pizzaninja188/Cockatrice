//! Registry conformance for the queue-v38 08-mountain-damage cohort.
//! Printed identity, costs, typed shape and pinned fingerprints.

use tricerules_cards::primitives::{Amount, CountExpression, SpellEffectKind, TargetKind};

struct Card {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    types: &'static [&'static str],
    power: Option<u32>,
    toughness: Option<u32>,

    fingerprint: &'static str,
}

const CARDS: &[Card] = &[
    Card {
        id: "rockslide_ambush",
        name: "Rockslide Ambush",
        mana_cost: "{1}{R}",
        types: &["Sorcery"],
        power: None,
        toughness: None,
        fingerprint: "f0fbca28a0ca925f8ebfe494406e30972d759cc83e69494338c056251c35a8ad",
    },
    Card {
        id: "seismic_strike",
        name: "Seismic Strike",
        mana_cost: "{2}{R}",
        types: &["Instant"],
        power: None,
        toughness: None,
        fingerprint: "814e06c410dcd8bbc041094196ce58b0adbabdb25d86e45cbb38eccf20edd566",
    },
    Card {
        id: "spitting_earth",
        name: "Spitting Earth",
        mana_cost: "{1}{R}",
        types: &["Sorcery"],
        power: None,
        toughness: None,
        fingerprint: "49544f81f5d3ec35eff2f83425a29aaa992e0aa57c4860ea2f0288fe102a130b",
    },
];

use tricerules_cards::Layout;
#[test]
fn q38_mountain_damage_batch_maps_definitions() {
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
            [SpellEffectKind::DamageTarget { amount: Amount::Count(
                CountExpression::BattlefieldPermanents { filter }), target }]
                if target.kind == TargetKind::Creature
                    && filter.matching.permanent_types == [tricerules_cards::primitives::PermanentTypeFilter::Land]
                    && filter.matching.required_subtypes == ["Mountain"]
        ));
    }
}

#[test]
fn q38_mountain_damage_batch_fingerprints_match_the_pinned_oracle_text() {
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
