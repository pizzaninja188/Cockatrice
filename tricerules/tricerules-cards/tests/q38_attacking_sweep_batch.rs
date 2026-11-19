//! Registry conformance for the queue-v38 05-attacking-sweep cohort.
//! Printed identity, costs, typed shape and pinned fingerprints.

use tricerules_cards::primitives::{Amount, CombatRole, SpellEffectKind, TargetKind};

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
        id: "marrow_shards",
        name: "Marrow Shards",
        mana_cost: "{W/P}",
        types: &["Instant"],
        power: None,
        toughness: None,
        fingerprint: "b86971e1aab4ee2000254fd9c97d2a2115a08c6f3587041a8775eee85cb13350",
    },
    Card {
        id: "rain_of_blades",
        name: "Rain of Blades",
        mana_cost: "{W}",
        types: &["Instant"],
        power: None,
        toughness: None,
        fingerprint: "8d4bebe96dd97b154ec9ffb5366e2737921d5d3f6bfdbcbc4d181d915e891a75",
    },
    Card {
        id: "sandstorm",
        name: "Sandstorm",
        mana_cost: "{G}",
        types: &["Instant"],
        power: None,
        toughness: None,
        fingerprint: "76fc34e7b84c9d7274ff24ad13b4ae70b5b61076953d93be39fcde37789c9741",
    },
];

use tricerules_cards::Layout;
#[test]
fn q38_attacking_sweep_batch_maps_definitions() {
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
            [SpellEffectKind::DamageAll { amount: Amount::Fixed(1), kind, .. }]
                if kind.kind == TargetKind::Creature
                    && kind.combat_role == Some(CombatRole::Attacking)
        ));
    }
}

#[test]
fn q38_attacking_sweep_batch_fingerprints_match_the_pinned_oracle_text() {
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
