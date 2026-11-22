//! Registry conformance for the reviewed "dies, shrink an opponent's creature"
//! family.

use tricerules_cards::primitives::{
    EffectSubject, SpellEffectKind, TargetController, TargetKind, TriggerCondition,
};
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
        "grim_physician",
        "Grim Physician",
        "{B}",
        &["Creature", "Zombie"],
        Some((1, 1)),
        "06a05e7ea5361501c9cea2b5cb3b8bb6b12d4c9c7a0bd63a7452d38f0356bcc2",
    ),
    (
        "shambling_goblin",
        "Shambling Goblin",
        "{B}",
        &["Creature", "Zombie", "Goblin"],
        Some((1, 1)),
        "06a05e7ea5361501c9cea2b5cb3b8bb6b12d4c9c7a0bd63a7452d38f0356bcc2",
    ),
    (
        "splatter_goblin",
        "Splatter Goblin",
        "{1}{B}",
        &["Creature", "Phyrexian", "Goblin"],
        Some((2, 1)),
        "06a05e7ea5361501c9cea2b5cb3b8bb6b12d4c9c7a0bd63a7452d38f0356bcc2",
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
fn dies_shrink_opponent_batch_maps_definitions() {
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
        assert_eq!(
            (f.power, f.toughness),
            body.map_or((None, None), |(p, t)| (Some(p), Some(t))),
            "{id} body"
        );
        assert!(f.keywords.is_empty(), "{id}");
        assert!(f.static_abilities.is_empty(), "{id}");
        assert!(f.spell_effect.is_empty(), "{id}");
        assert!(f.targeting.is_none(), "{id}");
        assert!(f.activated_abilities.is_empty(), "{id}");
        let [trigger] = f.triggered_abilities.as_slice() else {
            panic!("{id}: exactly one triggered ability");
        };
        assert_eq!(trigger.trigger, TriggerCondition::WhenSelfDies, "{id}");
        let [SpellEffectKind::PumpTarget {
            power,
            toughness,
            subject,
            ..
        }] = trigger.effect.as_slice()
        else {
            panic!("{id}: one shrink effect");
        };
        assert_eq!((*power, *toughness), (-1, -1), "{id}");
        let EffectSubject::Chosen(filter) = subject else {
            panic!("{id}: chosen target");
        };
        assert_eq!(filter.kind, TargetKind::Creature, "{id}");
        assert_eq!(filter.controller, TargetController::Opponent, "{id}");
    }
}

#[test]
fn dies_shrink_opponent_batch_fingerprints_match_the_pinned_oracle_text() {
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
