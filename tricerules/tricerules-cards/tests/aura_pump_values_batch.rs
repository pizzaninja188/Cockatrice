//! Registry conformance for the reviewed Aura power/toughness family.
//!
//! Every card is an Aura that enchants a creature and gives the enchanted creature a printed power/toughness bonus.
//! Each fingerprint is SHA-256 of the exact pinned normalized Oracle text.

use tricerules_cards::primitives::{SpellEffectKind, StaticAbilityDef, TargetFilter};
use tricerules_cards::{AbilityPresentation, Layout};

type Card = (
    &'static str,
    &'static str,
    &'static str,
    i32,
    i32,
    &'static str,
);

const CARDS: &[Card] = &[
    (
        "divine_transformation",
        "Divine Transformation",
        "{2}{W}{W}",
        3,
        3,
        "5924f5233ef0cb06601b8c4557300be4dde3937e2fdf3b8483969fdbf2e4dc65",
    ),
    (
        "feast_of_the_unicorn",
        "Feast of the Unicorn",
        "{3}{B}",
        4,
        0,
        "acdab6ebe3797be00d1c72bde7a30d8c333a9e9d444546e617a44d788513021e",
    ),
    (
        "giant_strength",
        "Giant Strength",
        "{R}{R}",
        2,
        2,
        "9ae89c1d319fc6263189a760540a5bd7ada556d7651da6949ecacb31ce93181c",
    ),
    (
        "hardened-scale_armor",
        "Hardened-Scale Armor",
        "{2}{G}",
        3,
        3,
        "5924f5233ef0cb06601b8c4557300be4dde3937e2fdf3b8483969fdbf2e4dc65",
    ),
    (
        "heros_resolve",
        "Hero's Resolve",
        "{1}{W}",
        1,
        5,
        "f1fb502220b037e23f675c4c0eb3a6b33205a08c05df3b01f1fc93bd9af348fc",
    ),
    (
        "knights_pledge",
        "Knight's Pledge",
        "{1}{W}",
        2,
        2,
        "9ae89c1d319fc6263189a760540a5bd7ada556d7651da6949ecacb31ce93181c",
    ),
    (
        "siegecraft",
        "Siegecraft",
        "{3}{W}",
        2,
        4,
        "aecda9bc2ca217c36899342de663edf0567dcf8b03e00775edd3477059b55ff1",
    ),
];

#[test]
fn aura_pump_values_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, mana, dp, dt, _) in CARDS {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        assert_eq!(registry.id_for_name(name), Some(id), "{id}");
        let f = face(registry, id);
        assert_eq!(f.face_id.as_str(), id.replace('-', "_"), "{id} face id");
        assert_eq!(f.name, name, "{id} name");
        assert_eq!(f.mana_cost.to_string(), mana, "{id} mana cost");
        assert_eq!(f.types, ["Enchantment", "Aura"], "{id} types");
        assert_eq!((f.power, f.toughness), (None, None), "{id} body");
        assert!(f.keywords.is_empty(), "{id}");
        assert!(f.activated_abilities.is_empty(), "{id}");
        assert!(f.triggered_abilities.is_empty(), "{id}");
        assert!(f.targeting.is_none(), "{id}");
        assert_eq!(
            f.spell_effect,
            [SpellEffectKind::AuraAttach {
                target: TargetFilter::default_creature()
            }],
            "{id}: enchant creature"
        );

        let [ability] = f.static_abilities.as_slice() else {
            panic!("{id}: exactly one attached modifier");
        };
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![2]),
            "{id}"
        );
        assert!(
            matches!(
                ability.definition,
                StaticAbilityDef::AttachedModifier { delta_power: p, delta_toughness: t, .. }
                    if p == dp && t == dt
            ),
            "{id}: exact attached pump"
        );
    }
}

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
fn aura_pump_values_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for &(id, name, .., expected) in CARDS {
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
