//! Registry conformance for the reviewed landfall +1/+1 creature family.
//!
//! Every card is a normal-layout creature whose only printed ability is one
//! landfall trigger that pumps the source by +1/+1 until end of turn. Each
//! fingerprint is SHA-256 of the exact pinned normalized Oracle text.

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
        "hedron_scrabbler",
        "Hedron Scrabbler",
        "{2}",
        &["Artifact", "Creature", "Construct"],
        Some((1, 1)),
        "735949e5422f42668ea4dc8ddbde9a42f2b427407a287cb22226401eb3e099b3",
    ),
    (
        "planetary_patrol",
        "Planetary Patrol",
        "{1}{R}",
        &["Creature", "Scout"],
        Some((2, 2)),
        "735949e5422f42668ea4dc8ddbde9a42f2b427407a287cb22226401eb3e099b3",
    ),
    (
        "scythe_leopard",
        "Scythe Leopard",
        "{G}",
        &["Creature", "Cat"],
        Some((1, 1)),
        "735949e5422f42668ea4dc8ddbde9a42f2b427407a287cb22226401eb3e099b3",
    ),
    (
        "snapping_gnarlid",
        "Snapping Gnarlid",
        "{1}{G}",
        &["Creature", "Beast"],
        Some((2, 2)),
        "735949e5422f42668ea4dc8ddbde9a42f2b427407a287cb22226401eb3e099b3",
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
fn landfall_pump_remnant_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, mana, types, body, _) in CARDS {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        assert_eq!(registry.id_for_name(name), Some(id), "{id}");
        let f = face(registry, id);
        assert_eq!(f.face_id.as_str(), id.replace('-', "_"), "{id} face id");
        assert_eq!(f.name, name, "{id} name");
        assert_eq!(f.mana_cost.to_string(), mana, "{id} mana cost");
        assert_eq!(f.types, types, "{id} types");
        assert_eq!(
            (f.power, f.toughness),
            (body.map(|b| b.0), body.map(|b| b.1)),
            "{id} body"
        );
        assert!(f.keywords.is_empty(), "{id}");
        assert!(f.activated_abilities.is_empty(), "{id}");
        assert!(f.static_abilities.is_empty(), "{id}");
        assert!(f.spell_effect.is_empty(), "{id}");
        let [ability] = f.triggered_abilities.as_slice() else {
            panic!("{id}: exactly one triggered ability")
        };
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{id}"
        );
        let TriggerCondition::WheneverPermanentEntersBattlefield {
            controller, filter, ..
        } = &ability.trigger
        else {
            panic!("{id}: land entry trigger")
        };
        assert_eq!(*controller, CastTriggerPlayer::Controller, "{id}");
        assert_eq!(
            filter.permanent_type,
            Some(PermanentTypeFilter::Land),
            "{id}"
        );
        assert!(
            matches!(
                ability.effect.as_slice(),
                [SpellEffectKind::PumpTarget {
                    power: 1,
                    toughness: 1,
                    subject: EffectSubject::Source,
                    ..
                }]
            ),
            "{id}: {:?}",
            ability.effect
        );
    }
}

#[test]
fn landfall_pump_remnant_batch_fingerprints_match_the_pinned_oracle_text() {
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
