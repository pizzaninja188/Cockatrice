//! Registry conformance for the reviewed colorless vanilla artifact-creature cohort.
//!
//! Every included card is a normal-layout artifact creature with no rules
//! text; its printed characteristics are the complete card.
//! Each fingerprint is SHA-256 of the exact pinned normalized Oracle text.

use tricerules_cards::Layout;

type Card = (
    &'static str,
    &'static str,
    &'static str,
    &'static [&'static str],
    &'static [&'static str],
    Option<(u32, u32)>,
    &'static str,
);

const CARDS: &[Card] = &[
    (
        "memnite",
        "Memnite",
        "{0}",
        &["Artifact", "Creature", "Construct"],
        &[],
        Some((1, 1)),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
    ),
    (
        "metallic_sliver",
        "Metallic Sliver",
        "{1}",
        &["Artifact", "Creature", "Sliver"],
        &[],
        Some((1, 1)),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
    ),
    (
        "obsianus_golem",
        "Obsianus Golem",
        "{6}",
        &["Artifact", "Creature", "Golem"],
        &[],
        Some((4, 6)),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
    ),
    (
        "phyrexian_hulk",
        "Phyrexian Hulk",
        "{6}",
        &["Artifact", "Creature", "Phyrexian", "Golem"],
        &[],
        Some((5, 4)),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
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
fn vanilla_colorless_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, mana, types, supertypes, body, _) in CARDS {
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
            f.supertypes.iter().map(String::as_str).collect::<Vec<_>>(),
            supertypes,
            "{id} supertypes"
        );
        assert_eq!(
            (f.power, f.toughness),
            body.map_or((None, None), |(p, t)| (Some(p), Some(t))),
            "{id} body"
        );
        assert!(f.spell_effect.is_empty(), "{id}");
        assert!(f.targeting.is_none(), "{id}");
        assert!(f.keywords.is_empty(), "{id}");
        assert!(f.protections.is_empty(), "{id}");
        assert!(f.triggered_abilities.is_empty(), "{id}");
        assert!(f.activated_abilities.is_empty(), "{id}");
        assert!(f.static_abilities.is_empty(), "{id}");
    }
}

#[test]
fn vanilla_colorless_batch_fingerprints_match_the_pinned_oracle_text() {
    const EMPTY_TEXT_SHA256: &str =
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for &(id, _, _, _, _, _, expected) in CARDS {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")));
        if let Some(row) = row {
            let fields: Vec<&str> = row.split('\t').collect();
            assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
            assert_eq!(fields[0], id);
            assert_eq!(fields[4], expected, "fingerprint drift for {id}");
        }
        assert_eq!(expected, EMPTY_TEXT_SHA256, "{id}: no Oracle text");
    }
}
