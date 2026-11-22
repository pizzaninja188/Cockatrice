//! Registry conformance for the reviewed creature-only damage spell family.
//!
//! Every card is a normal-layout Instant or Sorcery with one
//! `DamageTarget(amount, target: Creature)` effect. Each fingerprint is SHA-256
//! of the exact pinned normalized Oracle text.

use tricerules_cards::primitives::{Amount, SpellEffectKind, TargetKind};
use tricerules_cards::Layout;

type Card = (
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    u32,
    &'static str,
);

const CARDS: &[Card] = &[
    (
        "bathe_in_dragonfire",
        "Bathe in Dragonfire",
        "{2}{R}",
        "Sorcery",
        4,
        "1a8dd66d7ff37ebe5a93ea0e77c7fc2956888d604b8ccaac7296c2d9e2d9f962",
    ),
    (
        "breath_of_fire",
        "Breath of Fire",
        "{1}{R}",
        "Instant",
        2,
        "d8cab65688a697c88ce0f3225028b5de00f2d13f0849b9481363a92486430d11",
    ),
    (
        "electrify",
        "Electrify",
        "{3}{R}",
        "Instant",
        4,
        "04c5ff574fdad3e1b9de8a88b90f80b340e3367f329f95d80ac34d9626136321",
    ),
    (
        "explosive_shot",
        "Explosive Shot",
        "{1}{R}",
        "Sorcery",
        4,
        "1ac3370fe16c1619c48e8cff05d903060def2e62c79aeba6a014e15da3dcc612",
    ),
    (
        "fiery_finish",
        "Fiery Finish",
        "{4}{R}{R}",
        "Sorcery",
        7,
        "ab9a86f04e06861714222a39ee82b19758e081176ab13307599121fcbda94ec4",
    ),
    (
        "flame_slash",
        "Flame Slash",
        "{R}",
        "Sorcery",
        4,
        "daa607dc2a5c3da5f9a25e6be59a43805f3b7f6f68982ee126205cb4760a78be",
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
fn damage_target_creature_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, mana, card_type, amount, _) in CARDS {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        assert_eq!(registry.id_for_name(name), Some(id), "{id}");
        let f = face(registry, id);
        assert_eq!(f.face_id.as_str(), id.replace('-', "_"), "{id} face id");
        assert_eq!(f.name, name, "{id} name");
        assert_eq!(f.mana_cost.to_string(), mana, "{id} mana cost");
        assert_eq!(f.types, [card_type], "{id} type");
        assert_eq!(f.power, None, "{id}");
        assert_eq!(f.toughness, None, "{id}");
        assert!(f.keywords.is_empty(), "{id}");
        assert!(f.activated_abilities.is_empty(), "{id}");
        assert!(f.triggered_abilities.is_empty(), "{id}");
        assert!(f.static_abilities.is_empty(), "{id}");
        let [effect] = f.spell_effect.as_slice() else {
            panic!("{id}: one damage effect")
        };
        assert!(
            matches!(
                effect,
                SpellEffectKind::DamageTarget { amount: a, target }
                    if *a == Amount::Fixed(amount) && target.kind == TargetKind::Creature
            ),
            "{id}: {effect:?}"
        );
    }
}

#[test]
fn damage_target_creature_batch_fingerprints_match_the_pinned_oracle_text() {
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
