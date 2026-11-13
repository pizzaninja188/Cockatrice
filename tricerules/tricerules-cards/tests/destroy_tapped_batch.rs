//! Registry conformance for the reviewed `Destroy target tapped creature.` family.
//!
//! Every included card is a normal-layout instant or sorcery whose only effect
//! destroys one target tapped creature. Card names and costs are printed data;
//! the shared scenario executes every card. Each fingerprint is SHA-256 of the
//! exact pinned normalized Oracle text.

use tricerules_cards::primitives::{EffectSubject, SpellEffectKind, TargetKind};
use tricerules_cards::Layout;

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
        "death_stroke",
        "Death Stroke",
        "{B}{B}",
        &["Sorcery"],
        None,
        "920ec903f1db0308b6641b24afd514bef2ecf149364f543a9ff9356b4d3a84e2",
    ),
    (
        "swift_response",
        "Swift Response",
        "{1}{W}",
        &["Instant"],
        None,
        "920ec903f1db0308b6641b24afd514bef2ecf149364f543a9ff9356b4d3a84e2",
    ),
    (
        "assassinate",
        "Assassinate",
        "{2}{B}",
        &["Sorcery"],
        None,
        "920ec903f1db0308b6641b24afd514bef2ecf149364f543a9ff9356b4d3a84e2",
    ),
    (
        "take_vengeance",
        "Take Vengeance",
        "{1}{W}",
        &["Sorcery"],
        None,
        "920ec903f1db0308b6641b24afd514bef2ecf149364f543a9ff9356b4d3a84e2",
    ),
    (
        "vengeance",
        "Vengeance",
        "{3}{W}",
        &["Sorcery"],
        None,
        "920ec903f1db0308b6641b24afd514bef2ecf149364f543a9ff9356b4d3a84e2",
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
fn destroy_tapped_batch_maps_definitions() {
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
        assert!(f.triggered_abilities.is_empty(), "{id}");
        assert!(f.static_abilities.is_empty(), "{id}");

        let [SpellEffectKind::Destroy {
            subject: EffectSubject::Chosen(filter),
        }] = f.spell_effect.as_slice()
        else {
            panic!("{id}: one chosen destroy effect");
        };
        assert_eq!(filter.kind, TargetKind::Creature, "{id}");
        assert_eq!(filter.tapped, Some(true), "{id}");

        let Some(targeting) = &f.targeting else {
            panic!("{id}: one target group");
        };
        assert_eq!(targeting.groups.len(), 1, "{id}");
        assert_eq!(
            (targeting.groups[0].min, targeting.groups[0].max),
            (1, 1),
            "{id}"
        );
        assert_eq!(targeting.groups[0].effect_indices, [0], "{id}");
        assert_eq!(
            targeting.groups[0].prompt, "Choose target tapped creature",
            "{id}"
        );
    }
}

#[test]
fn destroy_tapped_batch_fingerprints_match_the_pinned_oracle_text() {
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
