//! Registry conformance for the queue-v18 flying self-pump cohort.
//!
//! Every included card is a normal-layout flier with one `{1}{colored}` activated
//! ability that pumps only the source until end of turn.
use tricerules_cards::primitives::{AbilityCost, EffectSubject, SpellEffectKind};
use tricerules_cards::{AbilityPresentation, Keyword, Layout};

struct ExpectedCard {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    types: &'static [&'static str],
    body: (u32, u32),
    activation_cost: &'static str,
    pump: (i32, i32),
    oracle_text_sha256: &'static str,
}

const CARDS: &[ExpectedCard] = &[
    ExpectedCard {
        id: "mesa_falcon",
        name: "Mesa Falcon",
        mana_cost: "{1}{W}",
        types: &["Creature", "Bird"],
        body: (1, 1),
        activation_cost: "{1}{W}",
        pump: (0, 1),
        oracle_text_sha256: "476b95dbb55df66ece2d01a3ef5095e57ba29c48e2211a821b376eb83e76cc81",
    },
    ExpectedCard {
        id: "nightwing_shade",
        name: "Nightwing Shade",
        mana_cost: "{4}{B}",
        types: &["Creature", "Shade"],
        body: (2, 2),
        activation_cost: "{1}{B}",
        pump: (1, 1),
        oracle_text_sha256: "04bf021c51ec010dc32c68d182a64ad97000d2f210fbf6c7f29205f5a1d71782",
    },
    ExpectedCard {
        id: "pearl_dragon",
        name: "Pearl Dragon",
        mana_cost: "{4}{W}{W}",
        types: &["Creature", "Dragon"],
        body: (4, 4),
        activation_cost: "{1}{W}",
        pump: (0, 1),
        oracle_text_sha256: "476b95dbb55df66ece2d01a3ef5095e57ba29c48e2211a821b376eb83e76cc81",
    },
    ExpectedCard {
        id: "tattered_apparition",
        name: "Tattered Apparition",
        mana_cost: "{3}{B}",
        types: &["Creature", "Shade"],
        body: (2, 2),
        activation_cost: "{1}{B}",
        pump: (1, 1),
        oracle_text_sha256: "04bf021c51ec010dc32c68d182a64ad97000d2f210fbf6c7f29205f5a1d71782",
    },
    ExpectedCard {
        id: "torch_drake",
        name: "Torch Drake",
        mana_cost: "{3}{U}",
        types: &["Creature", "Drake"],
        body: (2, 2),
        activation_cost: "{1}{R}",
        pump: (1, 0),
        oracle_text_sha256: "3c4c24dbb8512cf6f8ca955bee7cd31b1ee61b39064da7886122841fc6995735",
    },
];

#[test]
fn flying_self_pump_batch_maps_definitions() {
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
        assert_eq!(face.types, expected.types, "{id}");
        assert!(face.supertypes.is_empty(), "{id}");
        assert_eq!(
            (face.power, face.toughness),
            (Some(expected.body.0), Some(expected.body.1)),
            "{id}"
        );
        assert_eq!(face.keywords, [Keyword::Flying], "{id}");
        assert!(face.spell_effect.is_empty(), "{id}");
        assert!(face.targeting.is_none(), "{id}");
        assert!(face.static_abilities.is_empty(), "{id}");
        assert!(face.triggered_abilities.is_empty(), "{id}");
        let [ability] = face.activated_abilities.as_slice() else {
            panic!("{id}: exactly one activated ability");
        };
        assert_eq!(ability.ability_id.as_str(), "activated_01", "{id}");
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![2]),
            "{id} presentation"
        );
        assert!(
            matches!(ability.costs.as_slice(), [AbilityCost::Mana(cost)] if cost.to_string() == expected.activation_cost),
            "{id}: exact printed activation cost"
        );
        assert!(
            matches!(
                ability.effect.as_slice(),
                [SpellEffectKind::PumpTarget {
                    power,
                    toughness,
                    subject: EffectSubject::Source,
                    ..
                }] if *power == expected.pump.0 && *toughness == expected.pump.1
            ),
            "{id}: source-only printed pump"
        );
        assert!(ability.targeting.is_none(), "{id}");
    }
}

#[test]
fn flying_self_pump_batch_fingerprints_match_the_pinned_oracle_text() {
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
