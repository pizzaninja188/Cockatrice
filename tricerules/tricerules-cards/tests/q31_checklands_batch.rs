//! Registry conformance for the queue-v31 checkland cycle.
use tricerules_cards::primitives::{
    EntersTappedAffected, GameCondition, SpellEffectKind, StaticAbilityDef,
};
use tricerules_cards::Layout;

struct Card {
    id: &'static str,
    name: &'static str,
    first_subtype: &'static str,
    second_subtype: &'static str,
    fingerprint: &'static str,
}

const CARDS: &[Card] = &[
    Card {
        id: "glacial_fortress",
        name: "Glacial Fortress",
        first_subtype: "Plains",
        second_subtype: "Island",
        fingerprint: "9cf4af362b7c152c6b6d4583ede608e59eea8b66ccdc07ce35e6561437e383f7",
    },
    Card {
        id: "drowned_catacomb",
        name: "Drowned Catacomb",
        first_subtype: "Island",
        second_subtype: "Swamp",
        fingerprint: "01a52f64f95d150989a9de9ab348e5ac0d3114946a18e8152534a2bbb74329c4",
    },
    Card {
        id: "dragonskull_summit",
        name: "Dragonskull Summit",
        first_subtype: "Swamp",
        second_subtype: "Mountain",
        fingerprint: "01e2e8a19098b09c25ba702d4bbc68d9e17ee7af23713db54c854b2faa598f83",
    },
    Card {
        id: "isolated_chapel",
        name: "Isolated Chapel",
        first_subtype: "Plains",
        second_subtype: "Swamp",
        fingerprint: "a5220c580152b636e6031e944ff9afa838c9f725213a9bf5e252376fbdb57515",
    },
    Card {
        id: "woodland_cemetery",
        name: "Woodland Cemetery",
        first_subtype: "Swamp",
        second_subtype: "Forest",
        fingerprint: "313bc63074494af27f332aaa391ef150f6c72dc7d1b7394ff5dd5cc295c895b0",
    },
    Card {
        id: "clifftop_retreat",
        name: "Clifftop Retreat",
        first_subtype: "Mountain",
        second_subtype: "Plains",
        fingerprint: "81d48e369f7c4d1b7d48729b615db74fb7aecfe9deab97d88c23d0e06e9fb2aa",
    },
    Card {
        id: "sunpetal_grove",
        name: "Sunpetal Grove",
        first_subtype: "Forest",
        second_subtype: "Plains",
        fingerprint: "3c3c38cad1eeb40fbe47c7b9bdd4b56b95589caebf6bca2ca55f5a1d364da310",
    },
    Card {
        id: "sulfur_falls",
        name: "Sulfur Falls",
        first_subtype: "Island",
        second_subtype: "Mountain",
        fingerprint: "750213f293e367ebe44e8c8840550c5618bd4606909dd1779397139b51e819e9",
    },
];

fn condition_subtypes(condition: &GameCondition) -> Option<Vec<String>> {
    let GameCondition::BattlefieldAggregate {
        filter,
        aggregate,
        max,
        ..
    } = condition
    else {
        return None;
    };
    assert_eq!(
        *aggregate,
        tricerules_cards::primitives::BattlefieldAggregate::Count
    );
    assert_eq!(*max, Some(0));
    assert_eq!(
        filter.card_type,
        Some(tricerules_cards::primitives::CardTypeFilter::Land)
    );
    let mut out = Vec::new();
    for branch in filter.any_of.as_ref()? {
        out.extend(branch.required_subtypes.iter().cloned());
    }
    Some(out)
}

#[test]
fn q31_checklands_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for card in CARDS {
        let definition = registry
            .get(card.id)
            .unwrap_or_else(|| panic!("missing {}", card.id));
        let f = definition.primary_face();
        assert_eq!(registry.id_for_name(card.name), Some(card.id));
        assert_eq!(f.face_id.as_str(), card.id.replace('-', "_"), "{}", card.id);
        assert_eq!(f.name, card.name, "{}", card.id);
        assert_eq!(f.mana_cost.to_string(), "", "{}", card.id);
        assert_eq!(f.types, ["Land"], "{}", card.id);
        assert_eq!(definition.layout, Layout::Normal, "{}", card.id);
        assert!(f.keywords.is_empty(), "{}", card.id);
        assert!(f.triggered_abilities.is_empty(), "{}", card.id);
        assert!(f.spell_effect.is_empty(), "{}", card.id);
        let [mana] = f.activated_abilities.as_slice() else {
            panic!("{}: one mana ability", card.id);
        };
        let [SpellEffectKind::ProduceMana { options, .. }] = mana.effect.as_slice() else {
            panic!("{}: produce mana", card.id);
        };
        assert_eq!(options.len(), 2, "{}", card.id);
        let [static_ability] = f.static_abilities.as_slice() else {
            panic!("{}: one static ability", card.id);
        };
        let StaticAbilityDef::EntersTapped {
            affected,
            condition,
            ..
        } = &static_ability.definition
        else {
            panic!("{}: entries tapped", card.id);
        };
        assert_eq!(*affected, EntersTappedAffected::Self_, "{}", card.id);
        let subtypes = condition_subtypes(condition.as_ref().expect("condition"))
            .unwrap_or_else(|| panic!("{}: any-of basic land condition", card.id));
        assert_eq!(
            subtypes,
            [card.first_subtype, card.second_subtype],
            "{}",
            card.id
        );
    }
}

#[test]
fn q31_checklands_batch_fingerprints_match_the_pinned_oracle_text() {
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
        assert_eq!(fields[2], card.id.replace('-', "_"));
        assert_eq!(
            fields[4], card.fingerprint,
            "fingerprint drift for {}",
            card.id
        );
    }
}
