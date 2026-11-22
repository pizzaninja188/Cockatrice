//! Registry conformance for the reviewed `{2}, {T}: Tap target creature` family.
//!
//! Every card is a normal-layout creature or artifact with exactly one
//! `{2}, {T}` activated ability that taps any target creature. Each
//! fingerprint is SHA-256 of the exact pinned normalized Oracle text.

use tricerules_cards::primitives::{AbilityCost, EffectSubject, SpellEffectKind, TargetKind};
use tricerules_cards::{AbilityPresentation, Layout, ManaCost};

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
        "fan_bearer",
        "Fan Bearer",
        "{W}",
        &["Creature", "Zombie"],
        Some((1, 2)),
        "9b2875f85f8bfdf46f0584595b55fe8975966f63e33f9f4244f299375356f64b",
    ),
    (
        "gavony_trapper",
        "Gavony Trapper",
        "{W}",
        &["Creature", "Human", "Soldier"],
        Some((0, 2)),
        "9b2875f85f8bfdf46f0584595b55fe8975966f63e33f9f4244f299375356f64b",
    ),
    (
        "trip_noose",
        "Trip Noose",
        "{2}",
        &["Artifact"],
        None,
        "9b2875f85f8bfdf46f0584595b55fe8975966f63e33f9f4244f299375356f64b",
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
fn tap_target_two_batch_maps_definitions() {
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
        assert!(f.triggered_abilities.is_empty(), "{id}");
        assert!(f.static_abilities.is_empty(), "{id}");
        assert!(f.spell_effect.is_empty(), "{id}");
        assert!(f.targeting.is_none(), "{id}");
        let [ability] = f.activated_abilities.as_slice() else {
            panic!("{id}: exactly one activated ability")
        };
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{id}"
        );
        assert_eq!(
            ability.costs,
            [
                AbilityCost::Mana(ManaCost::parse("{2}").unwrap()),
                AbilityCost::Tap
            ],
            "{id} costs"
        );
        assert!(!ability.is_mana_ability(), "{id}");
        assert!(
            matches!(
                ability.effect.as_slice(),
                [SpellEffectKind::Tap {
                    subject: EffectSubject::Chosen(filter),
                }] if filter.kind == TargetKind::Creature
            ),
            "{id}: {:?}",
            ability.effect
        );
    }
}

#[test]
fn tap_target_two_batch_fingerprints_match_the_pinned_oracle_text() {
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
