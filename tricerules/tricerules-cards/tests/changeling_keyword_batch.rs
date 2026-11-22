//! Registry conformance for the queue-v20 changeling keyword cohort.
//!
//! Every included card is a normal-layout creature (or artifact creature) whose
//! only printed ability is the changeling characteristic-defining ability. Card
//! names, costs, types and bodies are printed data; the shared scenario executes
//! every card. Each fingerprint is SHA-256 of the exact pinned normalized Oracle
//! text.

use tricerules_cards::primitives::SpellEffectKind;
use tricerules_cards::{AbilityPresentation, CharacteristicDefiningAbility, Layout};

type Card = (
    &'static str,
    &'static str,
    &'static str,
    &'static [&'static str],
    u32,
    u32,
    bool,
    &'static str,
);

const CARDS: &[Card] = &[
    (
        "impostor_of_the_sixth_pride",
        "Impostor of the Sixth Pride",
        "{1}{W}",
        &["Creature", "Shapeshifter"],
        3,
        1,
        false,
        "70c30518730ca6fd1274af12f70f83347fa383cccc86cea53d55372b8ffcc670",
    ),
    (
        "universal_automaton",
        "Universal Automaton",
        "{1}",
        &["Artifact", "Creature", "Shapeshifter"],
        1,
        1,
        true,
        "70c30518730ca6fd1274af12f70f83347fa383cccc86cea53d55372b8ffcc670",
    ),
    (
        "woodland_changeling",
        "Woodland Changeling",
        "{1}{G}",
        &["Creature", "Shapeshifter"],
        2,
        2,
        false,
        "70c30518730ca6fd1274af12f70f83347fa383cccc86cea53d55372b8ffcc670",
    ),
];

fn check_card(
    id: &str,
    name: &str,
    mana: &str,
    types: &[&str],
    power: u32,
    toughness: u32,
    artifact: bool,
) {
    let registry = tricerules_cards::registry::global();
    let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
    assert_eq!(definition.layout, Layout::Normal, "{id}");
    assert_eq!(definition.name, name, "{id}");
    assert_eq!(registry.id_for_name(name), Some(id), "{id}");
    let face = definition.primary_face();
    assert_eq!(face.face_id.as_str(), id, "{id} face id");
    assert_eq!(face.mana_cost.to_string(), mana, "{id} mana cost");
    assert_eq!(face.types, types, "{id} types");
    assert_eq!(face.is_artifact, artifact, "{id} artifact flag");
    assert_eq!(
        (face.power, face.toughness),
        (Some(power), Some(toughness)),
        "{id}"
    );
    assert!(face.keywords.is_empty(), "{id}");
    assert!(face.spell_effect.is_empty(), "{id}");
    assert!(face.targeting.is_none(), "{id}");
    assert!(face.activated_abilities.is_empty(), "{id}");
    assert!(face.triggered_abilities.is_empty(), "{id}");
    assert!(face.static_abilities.is_empty(), "{id}");

    let [ability] = face.characteristic_defining_abilities.as_slice() else {
        panic!("{id}: one characteristic-defining ability");
    };
    assert_eq!(ability.ability_id.as_str(), "characteristic_01", "{id}");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1]),
        "{id} presentation"
    );
    assert_eq!(
        ability.definition,
        CharacteristicDefiningAbility::Changeling,
        "{id}: changeling CDA"
    );
    assert!(
        !matches!(face.spell_effect.as_slice(), [SpellEffectKind::Draw { .. }]),
        "{id}: no unrelated spell effect"
    );
}

#[test]
fn changeling_keyword_batch_canary_cards_match_their_typed_shape() {
    check_card(
        "universal_automaton",
        "Universal Automaton",
        "{1}",
        &["Artifact", "Creature", "Shapeshifter"],
        1,
        1,
        true,
    );
    check_card(
        "impostor_of_the_sixth_pride",
        "Impostor of the Sixth Pride",
        "{1}{W}",
        &["Creature", "Shapeshifter"],
        3,
        1,
        false,
    );
}

#[test]
fn changeling_keyword_batch_maps_definitions() {
    for &(id, name, mana, types, power, toughness, artifact, _fingerprint) in CARDS {
        check_card(id, name, mana, types, power, toughness, artifact);
    }
}

#[test]
fn changeling_keyword_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for &(id, name, _mana, _types, _power, _toughness, _artifact, fingerprint) in CARDS {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")))
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}"));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[0], id);
        assert_eq!(fields[1], name);
        assert_eq!(fields[2], id);
        assert_eq!(fields[4], fingerprint, "fingerprint drift for {id}");
    }
}
