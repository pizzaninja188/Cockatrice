//! Registry conformance for the queue-v31 turbulent land cycle.
use tricerules_cards::primitives::{
    BattlefieldAggregate, GameCondition, RelativePlayerSet, SpellEffectKind, StaticAbilityDef,
};
use tricerules_cards::Layout;

struct Card {
    id: &'static str,
    name: &'static str,
    types: &'static [&'static str],
    fingerprint: &'static str,
}

const CARDS: &[Card] = &[
    Card {
        id: "turbulent_fen",
        name: "Turbulent Fen",
        types: &["Land", "Swamp", "Forest"],
        fingerprint: "5b8a65972f075f51f9aca7616fc229f872c05aa9c611d069b82027fff9f632eb",
    },
    Card {
        id: "turbulent_moor",
        name: "Turbulent Moor",
        types: &["Land", "Plains", "Swamp"],
        fingerprint: "2c484f2dbdf6aab7756d06767425e7ea2d6e9ad245310ceb12512c6607300ae3",
    },
    Card {
        id: "turbulent_springs",
        name: "Turbulent Springs",
        types: &["Land", "Island", "Mountain"],
        fingerprint: "67ddef006f9a077a85cdf61307543ebfd17284f6463a7a7afca83a2bf95e4385",
    },
    Card {
        id: "turbulent_wilderness",
        name: "Turbulent Wilderness",
        types: &["Land", "Forest", "Island"],
        fingerprint: "072d04a497061dda0593835c311f42312c551ddda54238103f1af0603bd618ca",
    },
    Card {
        id: "turbulent_steppe",
        name: "Turbulent Steppe",
        types: &["Land", "Mountain", "Plains"],
        fingerprint: "ae28c51ee389f5155a835780b864ce6692abf0ad9c2b79042c852208b08e2f4a",
    },
];

#[test]
fn q31_turbulent_lands_batch_maps_definitions() {
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
        assert_eq!(f.types, card.types, "{}", card.id);
        assert_eq!(definition.layout, Layout::Normal, "{}", card.id);
        assert!(f.keywords.is_empty(), "{}", card.id);
        assert!(f.triggered_abilities.is_empty(), "{}", card.id);
        assert!(f.spell_effect.is_empty(), "{}", card.id);
        let [mana] = f.activated_abilities.as_slice() else {
            panic!("{}: one mana ability", card.id);
        };
        assert!(mana.intrinsic_land_mana, "{}", card.id);
        let [SpellEffectKind::ProduceMana { options, .. }] = mana.effect.as_slice() else {
            panic!("{}: produce mana", card.id);
        };
        assert_eq!(
            options.len(),
            2,
            "{}: one option per basic land type",
            card.id
        );
        let [static_ability] = f.static_abilities.as_slice() else {
            panic!("{}: one static ability", card.id);
        };
        let StaticAbilityDef::EntersTapped {
            affected: tricerules_cards::primitives::EntersTappedAffected::Self_,
            condition:
                Some(GameCondition::BattlefieldAggregate {
                    filter,
                    aggregate,
                    max,
                    ..
                }),
            ..
        } = &static_ability.definition
        else {
            panic!("{}: conditional enters tapped", card.id);
        };
        assert_eq!(*aggregate, BattlefieldAggregate::Count, "{}", card.id);
        assert_eq!(*max, Some(7), "{}", card.id);
        assert_eq!(
            filter.controllers,
            RelativePlayerSet::Opponents,
            "{}",
            card.id
        );
        assert_eq!(
            filter.matching.permanent_types,
            [tricerules_cards::primitives::PermanentTypeFilter::Land],
            "{}",
            card.id
        );
    }
}

#[test]
fn q31_turbulent_lands_batch_fingerprints_match_the_pinned_oracle_text() {
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
