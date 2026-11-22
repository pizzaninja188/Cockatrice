//! Registry conformance for the queue-v18 flash Aura shrink cohort.
//!
//! Every included card is a normal-layout Aura with Flash that enchants a creature
//! and applies a printed negative P/T modifier.
use tricerules_cards::primitives::{SpellEffectKind, StaticAbilityDef};
use tricerules_cards::{AbilityPresentation, Keyword, Layout};

struct ExpectedCard {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    shrink: (i32, i32),
    oracle_text_sha256: &'static str,
}

const CARDS: &[ExpectedCard] = &[
    ExpectedCard {
        id: "feebleness",
        name: "Feebleness",
        mana_cost: "{1}{B}",
        shrink: (-2, -1),
        oracle_text_sha256: "a9e8ccc6fd670d8a1e0059db142122e7d08245f5bfc5558198256ffba70929e0",
    },
    ExpectedCard {
        id: "greels_caress",
        name: "Greel's Caress",
        mana_cost: "{1}{B}",
        shrink: (-3, 0),
        oracle_text_sha256: "592eb0430f62fd95bca09c857b62993f756f52ff9cf3bc64d09f25f240c77f22",
    },
    ExpectedCard {
        id: "slimebind",
        name: "Slimebind",
        mana_cost: "{1}{U}",
        shrink: (-4, 0),
        oracle_text_sha256: "603c22973a84b62f1be9a23369ab2e295dea52d433a11ed114d53c8d1c05ce44",
    },
    ExpectedCard {
        id: "torpor_dust",
        name: "Torpor Dust",
        mana_cost: "{2}{U/B}",
        shrink: (-3, 0),
        oracle_text_sha256: "592eb0430f62fd95bca09c857b62993f756f52ff9cf3bc64d09f25f240c77f22",
    },
];

#[test]
fn flash_aura_shrink_batch_maps_definitions() {
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
        assert_eq!(face.types, ["Enchantment", "Aura"], "{id}");
        assert_eq!(face.keywords, [Keyword::Flash], "{id}");
        assert!(face.activated_abilities.is_empty(), "{id}");
        assert!(face.triggered_abilities.is_empty(), "{id}");
        assert!(face.targeting.is_none(), "{id}");
        let [SpellEffectKind::AuraAttach { target }] = face.spell_effect.as_slice() else {
            panic!("{id}: exactly one AuraAttach");
        };
        assert_eq!(
            target.kind,
            tricerules_cards::primitives::TargetKind::Creature,
            "{id}"
        );
        let [static_ability] = face.static_abilities.as_slice() else {
            panic!("{id}: exactly one static ability");
        };
        assert_eq!(
            static_ability.presentation,
            AbilityPresentation::OracleLines(vec![3]),
            "{id}"
        );
        assert!(
            matches!(
                &static_ability.definition,
                StaticAbilityDef::AttachedModifier {
                    delta_power,
                    delta_toughness,
                    keywords,
                    ..
                } if *delta_power == expected.shrink.0
                    && *delta_toughness == expected.shrink.1
                    && keywords.is_empty()
            ),
            "{id}: attached printed shrink"
        );
    }
}

#[test]
fn flash_aura_shrink_batch_fingerprints_match_the_pinned_oracle_text() {
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
