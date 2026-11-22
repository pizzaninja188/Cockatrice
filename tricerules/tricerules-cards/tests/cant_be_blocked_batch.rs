//! Registry conformance for the reviewed "This creature can't be blocked" cohort.
//!
//! Every included card is a normal-layout creature whose only rules text is the
//! unconditional, source-only can't-be-blocked restriction.
//! Each fingerprint is SHA-256 of the exact pinned normalized Oracle text.

use tricerules_cards::primitives::StaticAbilityDef;
use tricerules_cards::{AbilityPresentation, Layout};

type Card = (
    &'static str,
    &'static str,
    &'static str,
    &'static [&'static str],
    &'static [&'static str],
    Option<(u32, u32)>,
    &'static str,
);

const CARDS: &[Card] = &[
    (
        "covert_operative",
        "Covert Operative",
        "{4}{U}",
        &["Creature", "Human", "Wizard"],
        &[],
        Some((3, 2)),
        "7540809bd7730a0a5bad46af0633452e5397286f07a579880cc8b31591e88883",
    ),
    (
        "jhessian_infiltrator",
        "Jhessian Infiltrator",
        "{G}{U}",
        &["Creature", "Human", "Rogue"],
        &[],
        Some((2, 2)),
        "7540809bd7730a0a5bad46af0633452e5397286f07a579880cc8b31591e88883",
    ),
    (
        "latch_seeker",
        "Latch Seeker",
        "{1}{U}{U}",
        &["Creature", "Spirit"],
        &[],
        Some((3, 1)),
        "7540809bd7730a0a5bad46af0633452e5397286f07a579880cc8b31591e88883",
    ),
    (
        "metathran_soldier",
        "Metathran Soldier",
        "{1}{U}",
        &["Creature", "Metathran", "Soldier"],
        &[],
        Some((1, 1)),
        "7540809bd7730a0a5bad46af0633452e5397286f07a579880cc8b31591e88883",
    ),
    (
        "mist-cloaked_herald",
        "Mist-Cloaked Herald",
        "{U}",
        &["Creature", "Merfolk", "Warrior"],
        &[],
        Some((1, 1)),
        "7540809bd7730a0a5bad46af0633452e5397286f07a579880cc8b31591e88883",
    ),
    (
        "phantom_ninja",
        "Phantom Ninja",
        "{1}{U}{U}",
        &["Creature", "Illusion", "Ninja"],
        &[],
        Some((2, 2)),
        "7540809bd7730a0a5bad46af0633452e5397286f07a579880cc8b31591e88883",
    ),
    (
        "phantom_warrior",
        "Phantom Warrior",
        "{1}{U}{U}",
        &["Creature", "Illusion", "Warrior"],
        &[],
        Some((2, 2)),
        "7540809bd7730a0a5bad46af0633452e5397286f07a579880cc8b31591e88883",
    ),
    (
        "plasma_elemental",
        "Plasma Elemental",
        "{5}{U}",
        &["Creature", "Elemental"],
        &[],
        Some((4, 1)),
        "7540809bd7730a0a5bad46af0633452e5397286f07a579880cc8b31591e88883",
    ),
    (
        "slither_blade",
        "Slither Blade",
        "{U}",
        &["Creature", "Snake", "Rogue"],
        &[],
        Some((1, 2)),
        "7540809bd7730a0a5bad46af0633452e5397286f07a579880cc8b31591e88883",
    ),
    (
        "talas_warrior",
        "Talas Warrior",
        "{1}{U}{U}",
        &["Creature", "Human", "Pirate", "Warrior"],
        &[],
        Some((2, 2)),
        "7540809bd7730a0a5bad46af0633452e5397286f07a579880cc8b31591e88883",
    ),
    (
        "tidal_kraken",
        "Tidal Kraken",
        "{5}{U}{U}{U}",
        &["Creature", "Kraken"],
        &[],
        Some((6, 6)),
        "7540809bd7730a0a5bad46af0633452e5397286f07a579880cc8b31591e88883",
    ),
    (
        "triton_shorestalker",
        "Triton Shorestalker",
        "{U}",
        &["Creature", "Merfolk", "Rogue"],
        &[],
        Some((1, 1)),
        "7540809bd7730a0a5bad46af0633452e5397286f07a579880cc8b31591e88883",
    ),
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
fn cant_be_blocked_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, mana, types, supertypes, body, _) in CARDS {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        assert_eq!(registry.id_for_name(name), Some(id), "{id}");
        let f = face(registry, id);
        assert_eq!(f.face_id.as_str(), id.replace('-', "_"), "{id} face id");
        assert_eq!(f.name, name, "{id} name");
        assert_eq!(f.mana_cost.to_string(), mana, "{id} mana cost");
        assert_eq!(
            f.types.iter().map(String::as_str).collect::<Vec<_>>(),
            types,
            "{id} types"
        );
        assert_eq!(
            f.supertypes.iter().map(String::as_str).collect::<Vec<_>>(),
            supertypes,
            "{id} supertypes"
        );
        assert_eq!(
            (f.power, f.toughness),
            body.map_or((None, None), |(p, t)| (Some(p), Some(t))),
            "{id} body"
        );
        assert!(f.spell_effect.is_empty(), "{id}");
        assert!(f.targeting.is_none(), "{id}");
        assert!(f.keywords.is_empty(), "{id}");
        assert!(f.protections.is_empty(), "{id}");
        assert!(f.triggered_abilities.is_empty(), "{id}");
        assert!(f.activated_abilities.is_empty(), "{id}");
        let [ability] = f.static_abilities.as_slice() else {
            panic!("{id}: exactly one static ability");
        };
        assert_eq!(ability.ability_id.as_str(), "static_01", "{id}");
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{id} presentation"
        );
        assert!(
            matches!(
                &ability.definition,
                StaticAbilityDef::SelfCombatRestriction {
                    restriction,
                    condition: None,
                } if restriction.cant_be_blocked && !restriction.cant_block
            ),
            "{id}: must publish the can't-be-blocked restriction"
        );
    }
}

#[test]
fn cant_be_blocked_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for &(id, name, _, _, _, _, expected) in CARDS {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")))
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}"));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[0], id);
        assert_eq!(fields[1], name);
        assert_eq!(fields[2], id.replace('-', "_"));
        assert_eq!(fields[4], expected, "fingerprint drift for {id}");
    }
}
