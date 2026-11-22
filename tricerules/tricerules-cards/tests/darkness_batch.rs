//! Registry conformance for the reviewed Darkness combat-damage-prevention family.
//!
//! Every included card is a normal-layout instant whose only effect prevents all combat
//! damage this turn. Card names, costs and types are data; the shared scenario executes
//! every card. Each fingerprint is SHA-256 of the exact pinned normalized Oracle text.

use tricerules_cards::primitives::SpellEffectKind;
use tricerules_cards::Layout;

type Card = (&'static str, &'static str, &'static str, &'static str);

const CARDS: &[Card] = &[
    (
        "darkness",
        "Darkness",
        "{B}",
        "0dcdee85ce7dc3d36f90c6a73819712bb38c10bcbe03dd5e56d0fcb459ef0ad5",
    ),
    (
        "holy_day",
        "Holy Day",
        "{W}",
        "0dcdee85ce7dc3d36f90c6a73819712bb38c10bcbe03dd5e56d0fcb459ef0ad5",
    ),
    (
        "root_snare",
        "Root Snare",
        "{1}{G}",
        "0dcdee85ce7dc3d36f90c6a73819712bb38c10bcbe03dd5e56d0fcb459ef0ad5",
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
fn darkness_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, mana, _) in CARDS {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        assert_eq!(registry.id_for_name(name), Some(id), "{id}");
        let f = face(registry, id);
        assert_eq!(f.face_id.as_str(), id, "{id} face id");
        assert_eq!(f.name, name, "{id} name");
        assert_eq!(f.mana_cost.to_string(), mana, "{id} mana cost");
        assert_eq!(f.types, vec!["Instant".to_string()], "{id} type");
        assert!(f.keywords.is_empty(), "{id}");
        assert!(f.activated_abilities.is_empty(), "{id}");
        assert!(f.triggered_abilities.is_empty(), "{id}");
        assert!(f.static_abilities.is_empty(), "{id}");
        assert_eq!(
            f.spell_effect,
            vec![SpellEffectKind::PreventAllCombatDamageTurn],
            "{id}"
        );
        assert!(f.targeting.is_none(), "{id}: no targets");
    }
}

#[test]
fn darkness_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for &(id, name, _, expected) in CARDS {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")))
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}"));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[0], id);
        assert_eq!(fields[1], name);
        assert_eq!(fields[2], id);
        assert_eq!(fields[3], name);
        assert_eq!(fields[4], expected, "fingerprint drift for {id}");
    }
}
