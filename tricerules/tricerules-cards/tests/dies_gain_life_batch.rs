//! Registry conformance for the reviewed "dies, you gain N life" cohort.
//!
//! Every card is a normal-layout artifact or green creature with exactly one
//! dies trigger that gains its printed amount of life. Each fingerprint is
//! SHA-256 of the exact pinned normalized Oracle text.

use tricerules_cards::primitives::Amount;
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
        "anodet_lurker",
        "Anodet Lurker",
        "{5}",
        &["Artifact", "Creature", "Construct"],
        3,
        3,
        3,
        "cb34b92935f72ee7275565bf12abfb026c5ac6173f0afdbc90cca3cb59b12fd6",
    ),
    (
        "enatu_golem",
        "Enatu Golem",
        "{6}",
        &["Artifact", "Creature", "Golem"],
        3,
        5,
        4,
        "3d2ee54b5f80de381e3382e74d70bb878c08eda4a10757c6987e22d4288df5e3",
    ),
    (
        "guardian_automaton",
        "Guardian Automaton",
        "{4}",
        &["Artifact", "Creature", "Construct"],
        3,
        3,
        3,
        "cb34b92935f72ee7275565bf12abfb026c5ac6173f0afdbc90cca3cb59b12fd6",
    ),
    (
        "onulet",
        "Onulet",
        "{3}",
        &["Artifact", "Creature", "Construct"],
        2,
        2,
        2,
        "5313362ebe2adc13b91be0d522cbb3b6aa66337e2aecc455f5adb601f7f78066",
    ),
    (
        "tarpan",
        "Tarpan",
        "{G}",
        &["Creature", "Horse"],
        1,
        1,
        1,
        "0eae1a56e49438c3f5f5c2c5d21ebdf0ab80105f843c8b58fb52e1d2bca3c5e0",
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
fn dies_gain_life_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, mana, types, power, toughness, life, _) in CARDS {
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
        let [SpellEffectKind::GainLife { amount, .. }] = triggered.effect.as_slice() else {
            panic!("{id}: one life-gain instruction");
        };
        assert_eq!(*amount, Amount::Fixed(life), "{id}: printed life");
        assert!(triggered.targeting.is_none(), "{id}: no target");
    }
}

#[test]
fn dies_gain_life_batch_fingerprints_match_the_pinned_oracle_text() {
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
