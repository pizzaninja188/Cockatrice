//! Registry conformance for the reviewed negative-pump Aura family.
//!
//! Every included card is a normal-layout single-face Enchantment — Aura with `AuraAttach()` and
//! one `AttachedModifier(delta_power, delta_toughness)` static ability. Card names, costs and
//! numbers are printed data; the shared scenario executes every card. Each fingerprint is SHA-256
//! of the exact pinned normalized Oracle text.

use tricerules_cards::primitives::{
    SpellEffectKind, StaticAbilityDef, TargetController, TargetKind,
};
use tricerules_cards::{AbilityPresentation, Layout};

type Card = (&'static str, &'static str, &'static str, i32, i32);

const CARDS: &[Card] = &[
    (
        "chant_of_the_skifsang",
        "Chant of the Skifsang",
        "{2}{U}",
        -13,
        0,
    ),
    ("clinging_darkness", "Clinging Darkness", "{1}{B}", -4, -1),
    (
        "debilitating_injury",
        "Debilitating Injury",
        "{1}{B}",
        -2,
        -2,
    ),
    ("enfeeblement", "Enfeeblement", "{B}{B}", -2, -2),
    ("mires_grasp", "Mire's Grasp", "{1}{B}", -3, -3),
    ("pin_to_the_earth", "Pin to the Earth", "{1}{U}", -6, 0),
    ("torment", "Torment", "{1}{B}", -3, 0),
    ("weakness", "Weakness", "{B}", -2, -1),
    (
        "weight_of_the_underworld",
        "Weight of the Underworld",
        "{3}{B}",
        -3,
        -2,
    ),
    ("sensory_deprivation", "Sensory Deprivation", "{U}", -3, 0),
];

fn face<'a>(
    registry: &'a tricerules_cards::CardRegistry,
    id: &str,
) -> &'a tricerules_cards::CardFace {
    registry
        .get(id)
        .unwrap_or_else(|| panic!("missing {id}"))
        .primary_face()
}

#[test]
fn pump_aura_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, mana, delta_power, delta_toughness) in CARDS {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        let f = face(registry, id);
        assert_eq!(f.face_id.as_str(), id, "{id} face id");
        assert_eq!(f.name, name, "{id} name");
        assert_eq!(f.mana_cost.to_string(), mana, "{id} mana cost");
        assert_eq!(f.types, ["Enchantment", "Aura"], "{id} types");
        assert_eq!(f.power, None, "{id}: Auras have no power");
        assert_eq!(f.toughness, None, "{id}: Auras have no toughness");
        assert!(f.keywords.is_empty(), "{id} has no printed keyword");
        assert!(
            f.activated_abilities.is_empty(),
            "{id} has no activated ability"
        );
        assert!(
            f.triggered_abilities.is_empty(),
            "{id} has no triggered ability"
        );

        let [attach] = f.spell_effect.as_slice() else {
            panic!("{id}: exactly one Aura attach effect");
        };
        assert!(
            matches!(
                attach,
                SpellEffectKind::AuraAttach { target }
                    if target.kind == TargetKind::Creature
                        && target.controller == TargetController::Any
            ),
            "{id}: {:?}",
            f.spell_effect
        );

        let [static_ability] = f.static_abilities.as_slice() else {
            panic!("{id}: exactly one static ability");
        };
        assert_eq!(static_ability.ability_id.as_str(), "static_01", "{id}");
        assert_eq!(
            static_ability.presentation,
            AbilityPresentation::OracleLines(vec![2]),
            "{id} presentation"
        );
        assert!(
            matches!(
                static_ability.definition,
                StaticAbilityDef::AttachedModifier {
                    delta_power: p,
                    delta_toughness: t,
                    ..
                } if p == delta_power && t == delta_toughness
            ),
            "{id}: {:?}",
            static_ability.definition
        );
    }
}

type Fingerprint = (&'static str, &'static str, &'static str);

const FINGERPRINTS: &[Fingerprint] = &[
    (
        "chant_of_the_skifsang",
        "Chant of the Skifsang",
        "9453f414bbb5a76a9b7086f14d9adadd750c9c2f76e1ecfc325b90b46bad87b0",
    ),
    (
        "clinging_darkness",
        "Clinging Darkness",
        "26ef98dfa5a946bf9d7ac57564f06d4b778bc51f40d9002baad23f2a3db68d71",
    ),
    (
        "debilitating_injury",
        "Debilitating Injury",
        "0581cffdac39fec94dbe733ac91b2a69020c35c8d847ef4e1f8a4228f3c31c95",
    ),
    (
        "enfeeblement",
        "Enfeeblement",
        "cc251cafde2f13f6a70f6ecc5d4769e8b04a378bf821e29503b7a470dc11e7a2",
    ),
    (
        "mires_grasp",
        "Mire's Grasp",
        "755833cee83c632ab01659e61bf90bc84ad55670b9083a56b6684151098498b9",
    ),
    (
        "pin_to_the_earth",
        "Pin to the Earth",
        "3e929df5470221cfd401ead2515cba554030b76422abddb57efe5cf6b7e321e1",
    ),
    (
        "sensory_deprivation",
        "Sensory Deprivation",
        "68a164a4164ce5e9e16ca1ec249358f25abfc4a1fb7b830368470185c828f62a",
    ),
    (
        "torment",
        "Torment",
        "68a164a4164ce5e9e16ca1ec249358f25abfc4a1fb7b830368470185c828f62a",
    ),
    (
        "weakness",
        "Weakness",
        "6f7f4e89abdbf9d4c0f0c5459fa8940d178a76e32b23b78197c5b7d4625ddc50",
    ),
    (
        "weight_of_the_underworld",
        "Weight of the Underworld",
        "f9e5dc384f0ead33feaa9f659695da54c251ae66b88a3c5a42afeb2c6c79df9c",
    ),
];

#[test]
fn pump_aura_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for &(id, name, fingerprint) in FINGERPRINTS {
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
