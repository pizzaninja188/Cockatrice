//! Registry conformance for the reviewed attack-triggered +2/+0 family.
//!
//! Every included card is a normal-layout creature whose only printed ability is
//! one attack trigger that pumps the source by +2/+0 until end of turn. Card
//! names, costs, types and bodies are printed data; the shared scenario executes
//! every card. Each fingerprint is SHA-256 of the exact pinned normalized Oracle
//! text.

use tricerules_cards::primitives::{EffectSubject, SpellEffectKind};
use tricerules_cards::{AbilityPresentation, Layout, TriggerCondition};

type Card = (
    &'static str,
    &'static str,
    &'static str,
    &'static [&'static str],
    Option<(u32, u32)>,
    &'static str,
);

const CARDS: &[Card] = &[
    (
        "lurking_nightstalker",
        "Lurking Nightstalker",
        "{B}{B}",
        &["Creature", "Nightstalker"],
        Some((1, 1)),
        "024a8ccf72ff8b013a6c257bccc1e915f30cc44395aa10a70f689903b3ea03ee",
    ),
    (
        "wei_ambush_force",
        "Wei Ambush Force",
        "{1}{B}",
        &["Creature", "Human", "Soldier"],
        Some((1, 1)),
        "024a8ccf72ff8b013a6c257bccc1e915f30cc44395aa10a70f689903b3ea03ee",
    ),
    (
        "borderland_marauder",
        "Borderland Marauder",
        "{1}{R}",
        &["Creature", "Human", "Warrior"],
        Some((1, 2)),
        "024a8ccf72ff8b013a6c257bccc1e915f30cc44395aa10a70f689903b3ea03ee",
    ),
    (
        "brazen_wolves",
        "Brazen Wolves",
        "{2}{R}",
        &["Creature", "Wolf"],
        Some((2, 3)),
        "024a8ccf72ff8b013a6c257bccc1e915f30cc44395aa10a70f689903b3ea03ee",
    ),
    (
        "vicious_kavu",
        "Vicious Kavu",
        "{1}{B}{R}",
        &["Creature", "Kavu"],
        Some((2, 2)),
        "024a8ccf72ff8b013a6c257bccc1e915f30cc44395aa10a70f689903b3ea03ee",
    ),
    (
        "charging_bandits",
        "Charging Bandits",
        "{4}{B}",
        &["Creature", "Human", "Rogue"],
        Some((3, 3)),
        "024a8ccf72ff8b013a6c257bccc1e915f30cc44395aa10a70f689903b3ea03ee",
    ),
    (
        "hollow_dogs",
        "Hollow Dogs",
        "{4}{B}",
        &["Creature", "Phyrexian", "Zombie", "Dog"],
        Some((3, 3)),
        "024a8ccf72ff8b013a6c257bccc1e915f30cc44395aa10a70f689903b3ea03ee",
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
fn attack_pump_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, mana, types, body, _) in CARDS {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        assert_eq!(registry.id_for_name(name), Some(id), "{id}");
        let f = face(registry, id);
        assert_eq!(f.face_id.as_str(), id, "{id} face id");
        assert_eq!(f.name, name, "{id} name");
        assert_eq!(f.mana_cost.to_string(), mana, "{id} mana cost");
        assert_eq!(
            f.types.iter().map(String::as_str).collect::<Vec<_>>(),
            types,
            "{id} types"
        );
        assert!(f.supertypes.is_empty(), "{id} supertypes");
        assert_eq!(
            (f.power, f.toughness),
            body.map_or((None, None), |(p, t)| (Some(p), Some(t))),
            "{id} body"
        );

        assert!(f.keywords.is_empty(), "{id}");
        assert!(f.activated_abilities.is_empty(), "{id}");
        assert!(f.static_abilities.is_empty(), "{id}");
        assert!(f.spell_effect.is_empty(), "{id}");
        assert!(f.targeting.is_none(), "{id}");

        let [ability] = f.triggered_abilities.as_slice() else {
            panic!("{id}: exactly one triggered ability");
        };
        assert_eq!(ability.ability_id.as_str(), "triggered_01", "{id}");
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{id} presentation"
        );
        assert_eq!(
            ability.trigger,
            TriggerCondition::WheneverSelfAttacks {
                minimum_other_attackers: 0,
            },
            "{id}"
        );
        let [SpellEffectKind::PumpTarget {
            power,
            toughness,
            subject: EffectSubject::Source,
            ..
        }] = ability.effect.as_slice()
        else {
            panic!("{id}: source-only +2/+0 pump");
        };
        assert_eq!((*power, *toughness), (2, 0), "{id}");
    }
}

#[test]
fn attack_pump_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for &(id, name, _, _, _, expected) in CARDS {
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
