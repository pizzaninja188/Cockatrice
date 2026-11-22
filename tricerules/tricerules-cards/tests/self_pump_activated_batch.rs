//! Registry conformance for the queue-v20 activated self-pump cohort.
//!
//! Every included card is a normal-layout creature or artifact creature with one
//! `{cost}: This creature gets +X/+Y until end of turn.` activated ability and no
//! other ability. Card names, costs, types, stats and pump values are printed
//! data; the shared scenario executes every card. Each fingerprint is SHA-256 of
//! the exact pinned normalized Oracle text.

use tricerules_cards::primitives::{AbilityCost, EffectSubject, SpellEffectKind};
use tricerules_cards::{AbilityPresentation, ActivationTiming, Layout};

type Card = (
    &'static str,
    &'static str,
    &'static str,
    &'static [&'static str],
    u32,
    u32,
    &'static str,
    (i32, i32),
    &'static str,
);

const CARDS: &[Card] = &[
    (
        "dross_ripper",
        "Dross Ripper",
        "{4}",
        &["Artifact", "Creature", "Phyrexian", "Dog"],
        3,
        3,
        "{2}{B}",
        (1, 1),
        "09bb5d5d89faa0a3bb55fa7d401b82c394dacc9b4e0f5e0179e996a89a674e35",
    ),
    (
        "ebony_treefolk",
        "Ebony Treefolk",
        "{1}{B}{G}",
        &["Creature", "Treefolk"],
        3,
        3,
        "{B}{G}",
        (1, 1),
        "cd3836274679b2e9e2acb34f25aa7cde05fce498f652af67971ec5ca6287d164",
    ),
    (
        "folk_of_the_pines",
        "Folk of the Pines",
        "{4}{G}",
        &["Creature", "Dryad"],
        2,
        5,
        "{1}{G}",
        (1, 0),
        "d72d58b8a1d2fefa18221058b38fb86bb0cd3b493f6bf69a0fa71c7942653e8d",
    ),
    (
        "kranioceros",
        "Kranioceros",
        "{4}{R}",
        &["Creature", "Beast"],
        5,
        2,
        "{1}{W}",
        (0, 3),
        "759ac99b90ec0936e08c7f4d6eaf048b6f806a1028ccf1486fd6ba4c6db312f1",
    ),
    (
        "zof_shade",
        "Zof Shade",
        "{3}{B}",
        &["Creature", "Shade"],
        2,
        2,
        "{2}{B}",
        (2, 2),
        "af4194061e526f4a5ed8d0529092a8ff5d0d875b369e24d28f907cd05175b4df",
    ),
];

fn check_card(
    id: &str,
    name: &str,
    mana: &str,
    types: &[&str],
    body: (u32, u32),
    cost: &str,
    pump: (i32, i32),
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
    assert_eq!(
        (face.power, face.toughness),
        (Some(body.0), Some(body.1)),
        "{id}"
    );
    assert!(face.keywords.is_empty(), "{id}");
    assert!(face.spell_effect.is_empty(), "{id}");
    assert!(face.targeting.is_none(), "{id}");
    assert!(face.triggered_abilities.is_empty(), "{id}");
    assert!(face.static_abilities.is_empty(), "{id}");

    let [ability] = face.activated_abilities.as_slice() else {
        panic!("{id}: one activated ability");
    };
    assert_eq!(ability.ability_id.as_str(), "activated_01", "{id}");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1]),
        "{id} presentation"
    );
    assert_eq!(ability.timing, ActivationTiming::Normal, "{id} timing");
    assert!(
        matches!(
            ability.costs.as_slice(),
            [AbilityCost::Mana(mana_cost)] if mana_cost.to_string() == cost
        ),
        "{id}: {:?}",
        ability.costs
    );
    assert!(
        matches!(
            ability.effect.as_slice(),
            [SpellEffectKind::PumpTarget {
                power,
                toughness,
                subject: EffectSubject::Source,
                ..
            }] if (*power, *toughness) == pump
        ),
        "{id}: {:?}",
        ability.effect
    );
    assert!(ability.targeting.is_none(), "{id}: no targets");
}

#[test]
fn self_pump_activated_batch_canary_cards_match_their_typed_shape() {
    check_card(
        "dross_ripper",
        "Dross Ripper",
        "{4}",
        &["Artifact", "Creature", "Phyrexian", "Dog"],
        (3, 3),
        "{2}{B}",
        (1, 1),
    );
    check_card(
        "zof_shade",
        "Zof Shade",
        "{3}{B}",
        &["Creature", "Shade"],
        (2, 2),
        "{2}{B}",
        (2, 2),
    );
}

#[test]
fn self_pump_activated_batch_maps_definitions() {
    for &(id, name, mana, types, power, toughness, cost, pump, _fingerprint) in CARDS {
        check_card(id, name, mana, types, (power, toughness), cost, pump);
    }
}

#[test]
fn self_pump_activated_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for &(id, name, _mana, _types, _power, _toughness, _cost, _pump, fingerprint) in CARDS {
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
