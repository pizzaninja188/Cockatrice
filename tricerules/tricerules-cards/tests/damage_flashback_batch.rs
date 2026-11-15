//! Registry conformance for the queue-v18 damage-with-flashback cohort.
//!
//! Every included card is a normal-layout instant or sorcery that deals a fixed
//! amount of damage to any target and has a printed flashback cost.
use tricerules_cards::primitives::{SpellEffectKind, TargetKind};
use tricerules_cards::Layout;

struct ExpectedCard {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    card_type: &'static str,
    damage: u32,
    flashback_cost: &'static str,
    oracle_text_sha256: &'static str,
}

const CARDS: &[ExpectedCard] = &[
    ExpectedCard {
        id: "firebolt",
        name: "Firebolt",
        mana_cost: "{R}",
        card_type: "Sorcery",
        damage: 2,
        flashback_cost: "{4}{R}",
        oracle_text_sha256: "74151002b7e994d985ce0dfe652b01cb6935699654131c52dc0d7668e1b05fa8",
    },
    ExpectedCard {
        id: "fires_of_undeath",
        name: "Fires of Undeath",
        mana_cost: "{2}{R}",
        card_type: "Instant",
        damage: 2,
        flashback_cost: "{5}{B}",
        oracle_text_sha256: "c022e9d0505da54257c159c35793e08590242f502f1e4ad9ba4d49a376f07ef2",
    },
    ExpectedCard {
        id: "geistflame",
        name: "Geistflame",
        mana_cost: "{R}",
        card_type: "Instant",
        damage: 1,
        flashback_cost: "{3}{R}",
        oracle_text_sha256: "bf51782fe49f46ec65fa00eb1ff4df9fffaf739cb1059cd01c4f80158eee1d8e",
    },
    ExpectedCard {
        id: "tome_blast",
        name: "Tome Blast",
        mana_cost: "{1}{R}",
        card_type: "Sorcery",
        damage: 2,
        flashback_cost: "{4}{R}",
        oracle_text_sha256: "1dbee19bfbe3bfb1eb2853451857ed447badb3b959224f4ab2af1b1c11340dfd",
    },
];

#[test]
fn damage_flashback_batch_maps_definitions() {
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
        assert_eq!(face.types, [expected.card_type], "{id}");
        assert_eq!(
            face.flashback_cost.as_ref().map(ToString::to_string),
            Some(expected.flashback_cost.to_string()),
            "{id}"
        );
        assert!(face.keywords.is_empty(), "{id}");
        assert!(face.activated_abilities.is_empty(), "{id}");
        assert!(face.static_abilities.is_empty(), "{id}");
        assert!(face.triggered_abilities.is_empty(), "{id}");
        assert!(face.targeting.is_none(), "{id}");
        let [SpellEffectKind::DamageTarget { amount, target }] = face.spell_effect.as_slice()
        else {
            panic!("{id}: exactly one DamageTarget");
        };
        assert_eq!(
            *amount,
            tricerules_cards::Amount::Fixed(expected.damage),
            "{id}"
        );
        assert_eq!(target.kind, TargetKind::AnyTarget, "{id}");
    }
}

#[test]
fn damage_flashback_batch_fingerprints_match_the_pinned_oracle_text() {
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
