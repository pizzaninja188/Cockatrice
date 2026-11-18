//! Registry conformance for the queue-v31 Treasure loot cohort.
use tricerules_cards::primitives::{Amount, CastCostOptionDef, SpellEffectKind};
use tricerules_cards::Layout;

struct Card {
    id: &'static str,
    name: &'static str,
    mana: &'static str,
    types: &'static [&'static str],
    fingerprint: &'static str,
}

const CARDS: &[Card] = &[
    Card {
        id: "big_score",
        name: "Big Score",
        mana: "{3}{R}",
        types: &["Instant"],
        fingerprint: "7d087de48ce4863f061067e510e87d87975201efc82260e61fab1a38b56cfaf7",
    },
    Card {
        id: "pirates_pillage",
        name: "Pirate's Pillage",
        mana: "{3}{R}",
        types: &["Sorcery"],
        fingerprint: "7d087de48ce4863f061067e510e87d87975201efc82260e61fab1a38b56cfaf7",
    },
    Card {
        id: "unexpected_windfall",
        name: "Unexpected Windfall",
        mana: "{2}{R}{R}",
        types: &["Instant"],
        fingerprint: "7d087de48ce4863f061067e510e87d87975201efc82260e61fab1a38b56cfaf7",
    },
];

#[test]
fn q31_treasure_loot_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for card in CARDS {
        let definition = registry
            .get(card.id)
            .unwrap_or_else(|| panic!("missing {}", card.id));
        let f = definition.primary_face();
        assert_eq!(registry.id_for_name(card.name), Some(card.id));
        assert_eq!(f.face_id.as_str(), card.id.replace('-', "_"));
        assert_eq!(f.name, card.name);
        assert_eq!(f.mana_cost.to_string(), card.mana);
        assert_eq!(f.types, card.types, "{}", card.id);
        assert_eq!(definition.layout, Layout::Normal);
        let [group] = f.cast_cost_groups.as_slice() else {
            panic!("{}: one cost group", card.id)
        };
        assert_eq!(group.min, 1);
        let [CastCostOptionDef::DiscardCard { .. }] = group.options.as_slice() else {
            panic!("{}: discard cost", card.id);
        };
        let [SpellEffectKind::Draw {
            count: Amount::Fixed(2),
            ..
        }, SpellEffectKind::CreateTokens {
            token,
            count: Amount::Fixed(2),
            ..
        }] = f.spell_effect.as_slice()
        else {
            panic!("{}: draw two and two Treasures", card.id);
        };
        assert_eq!(token.as_str(), "treasure");
    }
}

#[test]
fn q31_treasure_loot_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for card in CARDS {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{}\t", card.id)))
            .unwrap();
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5);
        assert_eq!(fields[0], card.id);
        assert_eq!(fields[1], card.name);
        assert_eq!(
            fields[4], card.fingerprint,
            "fingerprint drift for {}",
            card.id
        );
    }
}
