//! Registry conformance for the reviewed Flying-only creature family.
//!
//! Every included card is a normal-layout creature whose only rules text is
//! Flying. Card names, costs, types and bodies are printed data; the shared
//! scenario executes every card. Each fingerprint is SHA-256 of the exact
//! pinned normalized Oracle text.

use tricerules_cards::{Keyword, Layout};

type Card = (
    &'static str,
    &'static str,
    &'static str,
    &'static [&'static str],
    (u32, u32),
    &'static str,
);

const CARDS: &[Card] = &[
    (
        "roc_of_kher_ridges",
        "Roc of Kher Ridges",
        "{3}{R}",
        &["Creature", "Bird"],
        (3, 3),
        "75a4c2fe3b9e4d3cd2bd3d7787c7c138adf405fad5c27011227787947baf9b14",
    ),
    (
        "spotted_griffin",
        "Spotted Griffin",
        "{3}{W}",
        &["Creature", "Griffin"],
        (2, 3),
        "75a4c2fe3b9e4d3cd2bd3d7787c7c138adf405fad5c27011227787947baf9b14",
    ),
    (
        "scryb_sprites",
        "Scryb Sprites",
        "{G}",
        &["Creature", "Faerie"],
        (1, 1),
        "75a4c2fe3b9e4d3cd2bd3d7787c7c138adf405fad5c27011227787947baf9b14",
    ),
    (
        "fighting_drake",
        "Fighting Drake",
        "{2}{U}{U}",
        &["Creature", "Drake"],
        (2, 4),
        "75a4c2fe3b9e4d3cd2bd3d7787c7c138adf405fad5c27011227787947baf9b14",
    ),
    (
        "bird_maiden",
        "Bird Maiden",
        "{2}{R}",
        &["Creature", "Human", "Bird"],
        (1, 2),
        "75a4c2fe3b9e4d3cd2bd3d7787c7c138adf405fad5c27011227787947baf9b14",
    ),
    (
        "armored_pegasus",
        "Armored Pegasus",
        "{1}{W}",
        &["Creature", "Pegasus"],
        (1, 2),
        "75a4c2fe3b9e4d3cd2bd3d7787c7c138adf405fad5c27011227787947baf9b14",
    ),
    (
        "exocomp",
        "Exocomp",
        "{2}{U}",
        &["Artifact", "Creature", "Construct"],
        (3, 1),
        "3d1fb46cfb4fd830be5434dc8e5718e457ba550c24ef9c684bb2e0532012fe5a",
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
fn flying_creatures_batch_maps_definitions() {
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
            (Some(body.0), Some(body.1)),
            "{id} body"
        );
        assert_eq!(f.keywords, [Keyword::Flying], "{id} keyword");
        assert!(f.activated_abilities.is_empty(), "{id}");
        assert!(f.triggered_abilities.is_empty(), "{id}");
        assert!(f.static_abilities.is_empty(), "{id}");
        assert!(f.spell_effect.is_empty(), "{id}");
        assert!(f.targeting.is_none(), "{id}");
    }
}

#[test]
fn flying_creatures_batch_fingerprints_match_the_pinned_oracle_text() {
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
