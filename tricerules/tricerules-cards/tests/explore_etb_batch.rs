//! Registry conformance for the reviewed ETB Explore cohort.
//!
//! Every card is a normal-layout creature with exactly one entry trigger whose
//! effect makes the source explore. Each fingerprint is SHA-256 of the exact
//! pinned normalized Oracle text.

use tricerules_cards::primitives::EffectSubject;
use tricerules_cards::{Layout, SpellEffectKind, TriggerCondition};

type Card = (
    &'static str,
    &'static str,
    &'static str,
    &'static [&'static str],
    u32,
    u32,
    &'static str,
);

const CARDS: &[Card] = &[
    (
        "ixallis_diviner",
        "Ixalli's Diviner",
        "{1}{G}",
        &["Creature", "Human", "Druid"],
        0,
        3,
        "f6cb65c0432d5a24773d30d9dbc9d9d77210e8ecd0e8b7e7096bb238fba60e71",
    ),
    (
        "merfolk_branchwalker",
        "Merfolk Branchwalker",
        "{1}{G}",
        &["Creature", "Merfolk", "Scout"],
        2,
        1,
        "f6cb65c0432d5a24773d30d9dbc9d9d77210e8ecd0e8b7e7096bb238fba60e71",
    ),
    (
        "seekers_squire",
        "Seekers' Squire",
        "{1}{B}",
        &["Creature", "Human", "Scout"],
        1,
        2,
        "f6cb65c0432d5a24773d30d9dbc9d9d77210e8ecd0e8b7e7096bb238fba60e71",
    ),
    (
        "tishanas_wayfinder",
        "Tishana's Wayfinder",
        "{2}{G}",
        &["Creature", "Merfolk", "Scout"],
        2,
        2,
        "f6cb65c0432d5a24773d30d9dbc9d9d77210e8ecd0e8b7e7096bb238fba60e71",
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
fn explore_etb_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, mana, types, power, toughness, _) in CARDS {
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
        assert_eq!(
            triggered.trigger,
            TriggerCondition::WhenSelfEntersBattlefield,
            "{id}"
        );
        assert_eq!(
            triggered.effect,
            [SpellEffectKind::Explore {
                subject: EffectSubject::Source
            }],
            "{id}: the source explores"
        );
        assert!(triggered.targeting.is_none(), "{id}: no target");
        assert!(
            matches!(
                triggered.effect.as_slice(),
                [SpellEffectKind::Explore { .. }]
            ),
            "{id}: one explore instruction"
        );
    }
}

#[test]
fn explore_etb_batch_fingerprints_match_the_pinned_oracle_text() {
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
        assert_eq!(fields[2], id);
        assert_eq!(fields[4], expected, "fingerprint drift for {id}");
    }
}
