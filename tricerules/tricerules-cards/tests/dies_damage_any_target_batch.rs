//! Registry conformance for the reviewed "dies, deal N damage to any target"
//! cohort.
//!
//! Every card is a normal-layout creature with exactly one dies trigger that
//! deals its printed amount to any target. Each fingerprint is SHA-256 of the
//! exact pinned normalized Oracle text.

use tricerules_cards::primitives::{Amount, TargetKind};
use tricerules_cards::{Layout, SpellEffectKind, TriggerCondition};

type Card = (
    &'static str,
    &'static str,
    &'static str,
    &'static [&'static str],
    u32,
    u32,
    u32,
    &'static str,
);

const CARDS: &[Card] = &[
    (
        "footlight_fiend",
        "Footlight Fiend",
        "{B/R}",
        &["Creature", "Devil"],
        1,
        1,
        1,
        "c85654c0e4d70bb7de639f472aa7de0323b4d9b0f294b328c41f653378f0783f",
    ),
    (
        "mudbutton_torchrunner",
        "Mudbutton Torchrunner",
        "{2}{R}",
        &["Creature", "Goblin", "Warrior"],
        1,
        1,
        3,
        "1aed45da8c6727791d21a8e1a72b4646933fe3c0b99854826300ffc545f44336",
    ),
    (
        "perilous_myr",
        "Perilous Myr",
        "{2}",
        &["Artifact", "Creature", "Phyrexian", "Myr"],
        1,
        1,
        2,
        "e871e94b13f9b29122e171e7973316e7ec1c44a3b34e2f6c00ced020332a6043",
    ),
    (
        "pyre_spawn",
        "Pyre Spawn",
        "{4}{R}{R}",
        &["Creature", "Elemental"],
        6,
        4,
        3,
        "1aed45da8c6727791d21a8e1a72b4646933fe3c0b99854826300ffc545f44336",
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
fn dies_damage_any_target_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, mana, types, power, toughness, damage, _) in CARDS {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        assert_eq!(registry.id_for_name(name), Some(id), "{id}");
        let f = face(registry, id);
        assert_eq!(f.face_id.as_str(), id, "{id} face id");
        assert_eq!(f.name, name, "{id} name");
        assert_eq!(f.mana_cost.to_string(), mana, "{id} mana cost");
        assert_eq!(f.types, types, "{id} types");
        assert_eq!(
            (f.power, f.toughness),
            (Some(power), Some(toughness)),
            "{id} body"
        );
        assert!(f.keywords.is_empty(), "{id}");
        assert!(f.activated_abilities.is_empty(), "{id}");
        assert!(f.static_abilities.is_empty(), "{id}");
        assert!(f.spell_effect.is_empty(), "{id}");
        assert!(f.targeting.is_none(), "{id}");

        let [triggered] = f.triggered_abilities.as_slice() else {
            panic!("{id}: exactly one triggered ability");
        };
        assert_eq!(triggered.ability_id.as_str(), "triggered_01", "{id}");
        assert_eq!(
            triggered.presentation,
            tricerules_cards::AbilityPresentation::OracleLines(vec![1]),
            "{id}"
        );
        assert_eq!(triggered.trigger, TriggerCondition::WhenSelfDies, "{id}");
        let [SpellEffectKind::DamageTarget { amount, target }] = triggered.effect.as_slice() else {
            panic!("{id}: one any-target damage instruction");
        };
        assert_eq!(*amount, Amount::Fixed(damage), "{id}: printed damage");
        assert_eq!(target.kind, TargetKind::AnyTarget, "{id}: any target");
        assert!(
            triggered.targeting.is_none(),
            "{id}: no separate target def"
        );
    }
}

#[test]
fn dies_damage_any_target_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for &(id, name, _, _, _, _, _, expected) in CARDS {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")))
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}"));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[0], id);
        assert_eq!(fields[1], name);
        assert_eq!(fields[2], id);
        assert_eq!(fields[4], expected, "fingerprint drift for {id}");
    }
}
