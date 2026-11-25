//! Registry conformance for the queue-v18 affinity-for-artifacts flier cohort.
//!
//! Every included card is a normal-layout artifact-affinity spell with flying and a
//! printed generic cost reduced by one for each artifact its controller controls.
use tricerules_cards::primitives::RelativePlayerSet;
use tricerules_cards::{BattlefieldAggregate, Keyword, Layout, SpellCostModifier};

struct ExpectedCard {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    types: &'static [&'static str],
    body: (u32, u32),
    oracle_text_sha256: &'static str,
}

const CARDS: &[ExpectedCard] = &[
    ExpectedCard {
        id: "qumulox",
        name: "Qumulox",
        mana_cost: "{6}{U}{U}",
        types: &["Creature", "Beast"],
        body: (5, 4),
        oracle_text_sha256: "0b00e43faf1b4fd3ee00cf24fd0c87ceb116a258a9bec9ab75c2eebf24f943ed",
    },
    ExpectedCard {
        id: "somber_hoverguard",
        name: "Somber Hoverguard",
        mana_cost: "{5}{U}",
        types: &["Creature", "Drone"],
        body: (3, 2),
        oracle_text_sha256: "0b00e43faf1b4fd3ee00cf24fd0c87ceb116a258a9bec9ab75c2eebf24f943ed",
    },
    ExpectedCard {
        id: "utrom_monitor",
        name: "Utrom Monitor",
        mana_cost: "{4}{U}",
        types: &["Artifact", "Creature", "Utrom", "Scientist"],
        body: (3, 3),
        oracle_text_sha256: "6ec374f0a219254a1a33e486123b6d04bfcc09acb968ccb64f0e61fbc44677b1",
    },
];

#[test]
fn affinity_flying_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for expected in CARDS {
        let id = expected.id;
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        assert_eq!(definition.name, expected.name);
        assert_eq!(registry.id_for_name(expected.name), Some(id));
        let face = definition.primary_face();
        assert_eq!(face.face_id.as_str(), id.replace('-', "_"), "{id}");
        assert_eq!(face.mana_cost.to_string(), expected.mana_cost, "{id}");
        assert_eq!(face.types, expected.types, "{id}");
        assert_eq!(
            (face.power, face.toughness),
            (Some(expected.body.0), Some(expected.body.1)),
            "{id}"
        );
        assert_eq!(face.keywords, [Keyword::Flying], "{id}");
        assert!(face.spell_effect.is_empty(), "{id}");
        assert!(face.targeting.is_none(), "{id}");
        assert!(face.activated_abilities.is_empty(), "{id}");
        assert!(face.triggered_abilities.is_empty(), "{id}");
        assert!(face.static_abilities.is_empty(), "{id}");
        let [SpellCostModifier::BattlefieldCountGenericReduction {
            amount_per_match,
            filter,
            aggregate,
        }] = face.cost_modifiers.as_slice()
        else {
            panic!("{id}: exactly one affinity cost modifier");
        };
        assert_eq!(*amount_per_match, 1, "{id}");
        assert_eq!(filter.controllers, RelativePlayerSet::Controller, "{id}");
        assert!(
            filter.matching.permanent_types
                == [tricerules_cards::primitives::PermanentTypeFilter::Artifact],
            "{id}"
        );
        assert_eq!(*aggregate, BattlefieldAggregate::Count, "{id}");
        assert!(filter.matching.required_subtypes.is_empty(), "{id}");
    }
}

#[test]
fn affinity_flying_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for expected in CARDS {
        let id = expected.id;
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")))
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}"));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[0], id);
        assert_eq!(fields[1], expected.name);
        assert_eq!(fields[2], id.replace('-', "_"));
        assert_eq!(
            fields[4], expected.oracle_text_sha256,
            "fingerprint drift for {id}"
        );
    }
}
