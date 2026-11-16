//! Registry conformance for the queue-v23 tap prevention-shield cohort.
//!
//! Every included card is a vanilla creature body with exactly one tap ability
//! preventing the next N damage to any target. Each fingerprint is SHA-256 of
//! the exact pinned normalized Oracle text.

use tricerules_cards::primitives::{AbilityCost, SpellEffectKind, TargetKind};
use tricerules_cards::{AbilityPresentation, Layout};

struct ExpectedCard {
    id: &'static str,
    name: &'static str,
    face_id: &'static str,
    mana_cost: &'static str,
    types: &'static [&'static str],
    body: (u32, u32),
    amount: u32,
    oracle_text_sha256: &'static str,
}

const CARDS: &[ExpectedCard] = &[
    ExpectedCard {
        id: "sanctum_custodian",
        name: "Sanctum Custodian",
        face_id: "sanctum_custodian",
        mana_cost: "{2}{W}",
        types: &["Creature", "Human", "Cleric"],
        body: (1, 2),
        amount: 2,
        oracle_text_sha256: "ad6fd8d0fb75544256fa19ebbe4778b539fb3467f0a7fbaf8aaae0c15a89f776",
    },
    ExpectedCard {
        id: "master_healer",
        name: "Master Healer",
        face_id: "master_healer",
        mana_cost: "{4}{W}",
        types: &["Creature", "Human", "Cleric"],
        body: (1, 4),
        amount: 4,
        oracle_text_sha256: "3f4a1646f52965df2989b5a054eb7e659f22058dd28c5ae29ce7df90ff0fa775",
    },
    ExpectedCard {
        id: "samite_healer",
        name: "Samite Healer",
        face_id: "samite_healer",
        mana_cost: "{1}{W}",
        types: &["Creature", "Human", "Cleric"],
        body: (1, 1),
        amount: 1,
        oracle_text_sha256: "7eee5bba728689e1d99300d21fae92ce7dba92b11432bf9bde9a8d71ff6128a0",
    },
    ExpectedCard {
        id: "orim,_samite_healer",
        name: "Orim, Samite Healer",
        face_id: "orim_samite_healer",
        mana_cost: "{1}{W}{W}",
        types: &["Creature", "Human", "Cleric"],
        body: (1, 3),
        amount: 3,
        oracle_text_sha256: "e78fd8b6f4b6f1cc6fc532120acee6e874f97d51ff3eab67c3db1b0b5e43d4a7",
    },
    ExpectedCard {
        id: "clergy_en-vec",
        name: "Clergy en-Vec",
        face_id: "clergy_en_vec",
        mana_cost: "{1}{W}",
        types: &["Creature", "Human", "Cleric"],
        body: (1, 1),
        amount: 1,
        oracle_text_sha256: "7eee5bba728689e1d99300d21fae92ce7dba92b11432bf9bde9a8d71ff6128a0",
    },
    ExpectedCard {
        id: "loxodon_anchorite",
        name: "Loxodon Anchorite",
        face_id: "loxodon_anchorite",
        mana_cost: "{2}{W}{W}",
        types: &["Creature", "Elephant", "Cleric"],
        body: (2, 3),
        amount: 2,
        oracle_text_sha256: "ad6fd8d0fb75544256fa19ebbe4778b539fb3467f0a7fbaf8aaae0c15a89f776",
    },
    ExpectedCard {
        id: "femeref_healer",
        name: "Femeref Healer",
        face_id: "femeref_healer",
        mana_cost: "{1}{W}",
        types: &["Creature", "Human", "Cleric"],
        body: (1, 1),
        amount: 1,
        oracle_text_sha256: "7eee5bba728689e1d99300d21fae92ce7dba92b11432bf9bde9a8d71ff6128a0",
    },
    ExpectedCard {
        id: "kithkin_healer",
        name: "Kithkin Healer",
        face_id: "kithkin_healer",
        mana_cost: "{2}{W}",
        types: &["Creature", "Kithkin", "Cleric"],
        body: (2, 2),
        amount: 1,
        oracle_text_sha256: "7eee5bba728689e1d99300d21fae92ce7dba92b11432bf9bde9a8d71ff6128a0",
    },
];

fn check_card(expected: &ExpectedCard) {
    let id = expected.id;
    let registry = tricerules_cards::registry::global();
    let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
    assert_eq!(definition.layout, Layout::Normal, "{id}");
    assert_eq!(definition.name, expected.name, "{id}");
    assert_eq!(registry.id_for_name(expected.name), Some(id), "{id}");
    let face = definition.primary_face();
    assert_eq!(face.face_id.as_str(), expected.face_id, "{id} face id");
    assert_eq!(face.mana_cost.to_string(), expected.mana_cost, "{id}");
    assert_eq!(face.types, expected.types, "{id} types");
    let expected_supertypes: Vec<String> = if id == "orim,_samite_healer" {
        vec!["Legendary".to_string()]
    } else {
        Vec::new()
    };
    assert_eq!(face.supertypes, expected_supertypes, "{id} supertypes");
    assert!(face.keywords.is_empty(), "{id}");
    assert_eq!(
        (face.power, face.toughness),
        (Some(expected.body.0), Some(expected.body.1)),
        "{id} body"
    );
    assert!(face.triggered_abilities.is_empty(), "{id}");
    assert!(face.static_abilities.is_empty(), "{id}");
    assert!(face.spell_effect.is_empty(), "{id}");
    assert!(face.targeting.is_none(), "{id}");

    let [ability] = face.activated_abilities.as_slice() else {
        panic!("{id}: exactly one activated ability");
    };
    assert_eq!(ability.ability_id.as_str(), "activated_01", "{id}");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1]),
        "{id}"
    );
    assert_eq!(ability.costs, [AbilityCost::Tap], "{id} tap cost");
    assert!(ability.targeting.is_none(), "{id}");
    assert!(
        matches!(
            ability.effect.as_slice(),
            [SpellEffectKind::PreventNextDamage { amount, target }]
                if *amount == expected.amount && target.kind == TargetKind::AnyTarget
        ),
        "{id}: any-target prevention shield"
    );
}

#[test]
fn prevention_shield_batch_canary_cards_match_their_typed_shape() {
    for expected in CARDS
        .iter()
        .filter(|card| matches!(card.id, "samite_healer" | "master_healer"))
    {
        check_card(expected);
    }
}

#[test]
fn prevention_shield_batch_maps_definitions() {
    for expected in CARDS {
        check_card(expected);
    }
}

#[test]
fn prevention_shield_batch_fingerprints_match_the_pinned_oracle_text() {
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
        assert_eq!(fields[2], expected.face_id);
        assert_eq!(
            fields[4], expected.oracle_text_sha256,
            "fingerprint drift for {id}"
        );
    }
}
