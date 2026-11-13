//! Registry conformance for the reviewed "Exile target creature" family.
//!
//! Card names, costs, types and bodies are printed data; the shared scenario
//! executes every card. Each fingerprint is SHA-256 of the exact pinned
//! normalized Oracle text.

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
        "final_death",
        "Final Death",
        "{4}{B}",
        &["Instant"],
        None,
        "a4b23ac12daed915d5cde308b56f7b7d6bfa41f7953bed991a3c40e0a7c928dc",
    ),
    (
        "final_reward",
        "Final Reward",
        "{4}{B}",
        &["Instant"],
        None,
        "a4b23ac12daed915d5cde308b56f7b7d6bfa41f7953bed991a3c40e0a7c928dc",
    ),
    (
        "unmake",
        "Unmake",
        "{W/B}{W/B}{W/B}",
        &["Instant"],
        None,
        "a4b23ac12daed915d5cde308b56f7b7d6bfa41f7953bed991a3c40e0a7c928dc",
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
fn exile_target_creature_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, mana, types, body, _) in CARDS {
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
        assert!(f.supertypes.is_empty(), "{id} supertypes");
        assert_eq!(
            (f.power, f.toughness),
            body.map_or((None, None), |(p, t)| (Some(p), Some(t))),
            "{id} body"
        );
        assert!(f.keywords.is_empty(), "{id}");
        assert!(f.triggered_abilities.is_empty(), "{id}");
        assert!(f.static_abilities.is_empty(), "{id}");
        assert!(f.targeting.is_none(), "{id}");
        assert!(f.activated_abilities.is_empty(), "{id}");
        let [SpellEffectKind::Exile {
            subject: EffectSubject::Chosen(filter),
            linked_exile_id,
        }] = f.spell_effect.as_slice()
        else {
            panic!("{id}: one targeted exile effect");
        };
        assert_eq!(*linked_exile_id, None, "{id}: no linked exile");
        assert_eq!(filter.kind, TargetKind::Creature, "{id}: creature target");
    }
}

#[test]
fn exile_target_creature_batch_fingerprints_match_the_pinned_oracle_text() {
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
        assert_eq!(fields[2], id.replace('-', "_"));
        assert_eq!(fields[4], expected, "fingerprint drift for {id}");
    }
}
