//! Registry conformance for the reviewed Islandwalk landwalk creature family.
//!
//! Every card is a normal-layout creature whose only rules text is Islandwalk landwalk.
//! Each fingerprint is SHA-256 of the exact pinned normalized Oracle text.

use tricerules_cards::primitives::Evasion;
use tricerules_cards::Layout;

type Card = (
    &'static str,
    &'static str,
    &'static str,
    &'static [&'static str],
    u32,
    u32,
    &'static str,
);

const CARDS: &[Card] = &[
    (
        "benthic_behemoth",
        "Benthic Behemoth",
        "{5}{U}{U}{U}",
        &["Creature", "Serpent"],
        7,
        6,
        "4eb795027ae8c7f496841106509d8eb16d2d779ae7d225260dc3561dacc06bc1",
    ),
    (
        "bull_hippo",
        "Bull Hippo",
        "{3}{G}",
        &["Creature", "Hippo"],
        3,
        3,
        "4eb795027ae8c7f496841106509d8eb16d2d779ae7d225260dc3561dacc06bc1",
    ),
    (
        "devouring_deep",
        "Devouring Deep",
        "{2}{U}",
        &["Creature", "Fish"],
        1,
        2,
        "4eb795027ae8c7f496841106509d8eb16d2d779ae7d225260dc3561dacc06bc1",
    ),
    (
        "grayscaled_gharial",
        "Grayscaled Gharial",
        "{U}",
        &["Creature", "Crocodile"],
        1,
        1,
        "4eb795027ae8c7f496841106509d8eb16d2d779ae7d225260dc3561dacc06bc1",
    ),
    (
        "pale_bears",
        "Pale Bears",
        "{2}{G}",
        &["Creature", "Bear"],
        2,
        2,
        "4eb795027ae8c7f496841106509d8eb16d2d779ae7d225260dc3561dacc06bc1",
    ),
    (
        "river_bear",
        "River Bear",
        "{3}{G}",
        &["Creature", "Bear"],
        3,
        3,
        "4eb795027ae8c7f496841106509d8eb16d2d779ae7d225260dc3561dacc06bc1",
    ),
    (
        "rootwater_commando",
        "Rootwater Commando",
        "{2}{U}",
        &["Creature", "Merfolk"],
        2,
        2,
        "4eb795027ae8c7f496841106509d8eb16d2d779ae7d225260dc3561dacc06bc1",
    ),
    (
        "segovian_leviathan",
        "Segovian Leviathan",
        "{4}{U}",
        &["Creature", "Leviathan"],
        3,
        3,
        "4eb795027ae8c7f496841106509d8eb16d2d779ae7d225260dc3561dacc06bc1",
    ),
    (
        "zodiac_horse",
        "Zodiac Horse",
        "{3}{G}",
        &["Creature", "Horse"],
        3,
        3,
        "4eb795027ae8c7f496841106509d8eb16d2d779ae7d225260dc3561dacc06bc1",
    ),
];

#[test]
fn islandwalk_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, mana, types, power, toughness, _) in CARDS {
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
            (Some(power), Some(toughness)),
            "{id} body"
        );
        assert_eq!(
            f.evasions,
            [Evasion::Landwalk {
                land_subtype: "Island".to_string()
            }],
            "{id} Islandwalk landwalk"
        );
        assert!(f.keywords.is_empty(), "{id}");
        assert!(f.static_abilities.is_empty(), "{id}");
        assert!(f.activated_abilities.is_empty(), "{id}");
        assert!(f.triggered_abilities.is_empty(), "{id}");
    }
}

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
fn islandwalk_batch_fingerprints_match_the_pinned_oracle_text() {
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
