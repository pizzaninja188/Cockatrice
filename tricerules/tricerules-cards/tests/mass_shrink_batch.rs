//! Registry conformance for the queue-v18 opponent-wide shrink cohort.
//!
//! Every included card is a normal-layout instant that applies a printed negative
//! P/T modifier to each creature your opponents control until end of turn.
use tricerules_cards::primitives::{CreatureScopeController, CreatureScopeFilter, SpellEffectKind};
use tricerules_cards::Layout;

struct ExpectedCard {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    shrink: (i32, i32),
    oracle_text_sha256: &'static str,
}

const CARDS: &[ExpectedCard] = &[
    ExpectedCard {
        id: "cower_in_fear",
        name: "Cower in Fear",
        mana_cost: "{1}{B}{B}",
        shrink: (-1, -1),
        oracle_text_sha256: "755c622b7ac54e3bd4eb3a60d0efc62db0d9f47c20922abea5425d937930a6b6",
    },
    ExpectedCard {
        id: "hysterical_blindness",
        name: "Hysterical Blindness",
        mana_cost: "{2}{U}",
        shrink: (-4, 0),
        oracle_text_sha256: "63303c4a1eac9dbd2538e97e5de1ae77aead185632fec15569d3d6de36711db1",
    },
    ExpectedCard {
        id: "turn_the_tide",
        name: "Turn the Tide",
        mana_cost: "{1}{U}",
        shrink: (-2, 0),
        oracle_text_sha256: "4ca9aec84384371ed9d2649f303f4e8dee6fa0c483628b00c909f0d1ea268365",
    },
];

#[test]
fn mass_shrink_batch_maps_definitions() {
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
        assert_eq!(face.types, ["Instant"], "{id}");
        assert!(face.keywords.is_empty(), "{id}");
        assert!(face.activated_abilities.is_empty(), "{id}");
        assert!(face.static_abilities.is_empty(), "{id}");
        assert!(face.triggered_abilities.is_empty(), "{id}");
        assert!(face.targeting.is_none(), "{id}");
        assert_eq!(
            face.spell_effect,
            [SpellEffectKind::PumpAll {
                filter: CreatureScopeFilter {
                    controller: Some(CreatureScopeController::Opponents),
                    ..CreatureScopeFilter::default()
                },
                power: expected.shrink.0,
                toughness: expected.shrink.1,
            }],
            "{id}: opponent-only printed shrink"
        );
    }
}

#[test]
fn mass_shrink_batch_fingerprints_match_the_pinned_oracle_text() {
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
