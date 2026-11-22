//! Registry conformance for the reviewed "{T}: deal 1 damage to any target" family.
//!
//! Every included card is a normal-layout card with the same creatures that tap for one damage to any target
//! shape. Card names, costs, types and bodies are printed data; the shared
//! scenario executes every card. Each fingerprint is SHA-256 of the exact
//! pinned normalized Oracle text.

use tricerules_cards::primitives::{AbilityCost, Amount, SpellEffectKind, TargetKind};
use tricerules_cards::{AbilityPresentation, Layout};

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
        "razorfin_hunter",
        "Razorfin Hunter",
        "{U}{R}",
        &["Creature", "Merfolk", "Goblin"],
        Some((1, 1)),
        "c748ae1d32f1d9b397452e3a6adfe2ace17cca136e6d7660c05ce48d98298ad9",
    ),
    (
        "rootwater_hunter",
        "Rootwater Hunter",
        "{2}{U}",
        &["Creature", "Merfolk"],
        Some((1, 1)),
        "c748ae1d32f1d9b397452e3a6adfe2ace17cca136e6d7660c05ce48d98298ad9",
    ),
    (
        "viashino_fangtail",
        "Viashino Fangtail",
        "{2}{R}{R}",
        &["Creature", "Lizard", "Warrior"],
        Some((3, 3)),
        "c748ae1d32f1d9b397452e3a6adfe2ace17cca136e6d7660c05ce48d98298ad9",
    ),
    (
        "zuran_spellcaster",
        "Zuran Spellcaster",
        "{2}{U}",
        &["Creature", "Human", "Wizard"],
        Some((1, 1)),
        "c748ae1d32f1d9b397452e3a6adfe2ace17cca136e6d7660c05ce48d98298ad9",
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
fn pinger_batch_maps_definitions() {
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
        assert!(f.spell_effect.is_empty(), "{id}");
        assert!(f.targeting.is_none(), "{id}");
        let [ability] = f.activated_abilities.as_slice() else {
            panic!("{id}: exactly one activated ability");
        };
        assert_eq!(ability.ability_id.as_str(), "activated_01", "{id}");
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{id} presentation"
        );
        assert_eq!(ability.costs, [AbilityCost::Tap], "{id} costs");
        let [SpellEffectKind::DamageTarget { amount, target }] = ability.effect.as_slice() else {
            panic!("{id}: one damage effect");
        };
        assert_eq!(*amount, Amount::Fixed(1), "{id}");
        assert_eq!(target.kind, TargetKind::AnyTarget, "{id}");
    }
}

#[test]
fn pinger_batch_fingerprints_match_the_pinned_oracle_text() {
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
