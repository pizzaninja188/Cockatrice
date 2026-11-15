//! Registry conformance for the queue-v18 flying Aura pump cohort.
//!
//! Every included card is a normal-layout Aura that enchants a creature and gives
//! the enchanted creature a printed P/T bonus and flying.
use tricerules_cards::primitives::{SpellEffectKind, StaticAbilityDef};
use tricerules_cards::{AbilityPresentation, Keyword, Layout};

struct ExpectedCard {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    pump: (i32, i32),
    oracle_text_sha256: &'static str,
}

const CARDS: &[ExpectedCard] = &[
    ExpectedCard {
        id: "arcane_flight",
        name: "Arcane Flight",
        mana_cost: "{U}",
        pump: (1, 1),
        oracle_text_sha256: "a4bb538cfc76e99592a96fe83f62f9cb61360db8d02b44eb1a76f5c1c70ade0d",
    },
    ExpectedCard {
        id: "magefire_wings",
        name: "Magefire Wings",
        mana_cost: "{U}{R}",
        pump: (2, 0),
        oracle_text_sha256: "43cdca8401a44c60bb0c51a14e54dfb3bccafafa16ce1da82e1baa407c60beb4",
    },
    ExpectedCard {
        id: "nimbus_wings",
        name: "Nimbus Wings",
        mana_cost: "{1}{W}",
        pump: (1, 2),
        oracle_text_sha256: "5f06d6a7c94535682db4893cb9ad5dad59b18845b5f8bbb397ac23c39a9e7e3d",
    },
    ExpectedCard {
        id: "one_with_the_wind",
        name: "One With the Wind",
        mana_cost: "{1}{U}",
        pump: (2, 2),
        oracle_text_sha256: "f954babb9b7294f50198631b6886493b696bba4f286fd669f35f809e42520f9c",
    },
    ExpectedCard {
        id: "spectral_flight",
        name: "Spectral Flight",
        mana_cost: "{1}{U}",
        pump: (2, 2),
        oracle_text_sha256: "f954babb9b7294f50198631b6886493b696bba4f286fd669f35f809e42520f9c",
    },
    ExpectedCard {
        id: "wings_of_hope",
        name: "Wings of Hope",
        mana_cost: "{W}{U}",
        pump: (1, 3),
        oracle_text_sha256: "02eb6da03e00793406ef492e1970ce3b8117768b3bb57e92177fc3c96c1a3ddb",
    },
];

#[test]
fn aura_flying_pump_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for expected in CARDS {
        let id = expected.id;
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        assert_eq!(definition.name, expected.name);
        assert_eq!(registry.id_for_name(expected.name), Some(id));
        assert_eq!(definition.face_count(), 1);
        let face = definition.primary_face();
        assert_eq!(face.face_id.as_str(), id.replace('-', "_"), "{id}");
        assert_eq!(face.mana_cost.to_string(), expected.mana_cost, "{id}");
        assert_eq!(face.types, ["Enchantment", "Aura"], "{id}");
        assert!(face.keywords.is_empty(), "{id}");
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
            AbilityPresentation::OracleLines(vec![2]),
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
                } if *delta_power == expected.pump.0
                    && *delta_toughness == expected.pump.1
                    && keywords == &[Keyword::Flying]
            ),
            "{id}: attached printed pump and flying"
        );
    }
}

#[test]
fn aura_flying_pump_batch_fingerprints_match_the_pinned_oracle_text() {
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
