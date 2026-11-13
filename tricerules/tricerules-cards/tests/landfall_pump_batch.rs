//! Registry conformance for the reviewed landfall +2/+2 creature family.
//!
//! Every included card is a normal-layout creature whose only printed ability is
//! one landfall trigger that pumps the source by +2/+2 until end of turn. Card
//! names, costs, types and bodies are printed data; the shared scenario executes
//! every card. Each fingerprint is SHA-256 of the exact pinned normalized Oracle
//! text.

use tricerules_cards::primitives::{
    CastTriggerPlayer, EffectSubject, PermanentTypeFilter, SpellEffectKind,
};
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
        "akoum_hellhound",
        "Akoum Hellhound",
        "{R}",
        &["Creature", "Elemental", "Dog"],
        Some((0, 1)),
        "cfbb09a5f99b1d8c1aa9c7f804e4d2f20ccb1dbf4151de4b23f2978b17fed75c",
    ),
    (
        "steppe_lynx",
        "Steppe Lynx",
        "{W}",
        &["Creature", "Cat"],
        Some((0, 1)),
        "cfbb09a5f99b1d8c1aa9c7f804e4d2f20ccb1dbf4151de4b23f2978b17fed75c",
    ),
    (
        "valakut_predator",
        "Valakut Predator",
        "{2}{R}",
        &["Creature", "Elemental"],
        Some((2, 2)),
        "cfbb09a5f99b1d8c1aa9c7f804e4d2f20ccb1dbf4151de4b23f2978b17fed75c",
    ),
    (
        "canopy_baloth",
        "Canopy Baloth",
        "{3}{G}",
        &["Creature", "Beast"],
        Some((4, 3)),
        "cfbb09a5f99b1d8c1aa9c7f804e4d2f20ccb1dbf4151de4b23f2978b17fed75c",
    ),
    (
        "territorial_baloth",
        "Territorial Baloth",
        "{4}{G}",
        &["Creature", "Beast"],
        Some((4, 4)),
        "cfbb09a5f99b1d8c1aa9c7f804e4d2f20ccb1dbf4151de4b23f2978b17fed75c",
    ),
    (
        "hedron_rover",
        "Hedron Rover",
        "{4}",
        &["Artifact", "Creature", "Construct"],
        Some((2, 2)),
        "cfbb09a5f99b1d8c1aa9c7f804e4d2f20ccb1dbf4151de4b23f2978b17fed75c",
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
fn landfall_pump_batch_maps_definitions() {
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
        let TriggerCondition::WheneverPermanentEntersBattlefield {
            controller, filter, ..
        } = &ability.trigger
        else {
            panic!("{id}: land entry trigger");
        };
        assert_eq!(*controller, CastTriggerPlayer::Controller, "{id}");
        assert_eq!(
            filter.permanent_type,
            Some(PermanentTypeFilter::Land),
            "{id}"
        );
        let [SpellEffectKind::PumpTarget {
            power,
            toughness,
            subject: EffectSubject::Source,
            ..
        }] = ability.effect.as_slice()
        else {
            panic!("{id}: source-only +2/+2 pump");
        };
        assert_eq!((*power, *toughness), (2, 2), "{id}");
    }
}

#[test]
fn landfall_pump_batch_fingerprints_match_the_pinned_oracle_text() {
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
