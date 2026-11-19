//! Registry conformance for the queue-v38 02-hand-damage cohort.
//! Printed identity, costs, typed shape and pinned fingerprints.

use tricerules_cards::primitives::{
    Amount, ConditionPlayerSet, CountExpression, SpellEffectKind, TargetKind,
};

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
        id: "gaze_of_adamaro",
        name: "Gaze of Adamaro",
        mana_cost: "{2}{R}{R}",
        types: &["Instant", "Arcane"],
        power: None,
        toughness: None,
        fingerprint: "5609c9f998f238afae861eecb0c5111ed5983172a78481e08c13350b43f75c34",
    },
    Card {
        id: "storm_seeker",
        name: "Storm Seeker",
        mana_cost: "{3}{G}",
        types: &["Instant"],
        power: None,
        toughness: None,
        fingerprint: "208929b9adbed1b86e23daf6ab2cda298fb7e232f196af4c1514bed2f72611a2",
    },
    Card {
        id: "sudden_impact",
        name: "Sudden Impact",
        mana_cost: "{3}{R}",
        types: &["Instant"],
        power: None,
        toughness: None,
        fingerprint: "d870904bfbf56070f095d641fce1d4033a90e0db4d83ef6deeafd81610663e6d",
    },
];

use tricerules_cards::Layout;
#[test]
fn q38_hand_damage_batch_maps_definitions() {
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
            [SpellEffectKind::DamageTarget { amount: Amount::Count(CountExpression::CardsInHand {
                players: ConditionPlayerSet::ChosenTarget { group_index: 0, target_index: 0 },
            }), target }] if target.kind == TargetKind::AnyPlayer
        ));
        let groups = f
            .targeting
            .as_ref()
            .expect("player target group")
            .groups
            .as_slice();
        assert_eq!(groups.len(), 1);
        assert_eq!((groups[0].min, groups[0].max), (1, 1));
    }
}

#[test]
fn q38_hand_damage_batch_fingerprints_match_the_pinned_oracle_text() {
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
