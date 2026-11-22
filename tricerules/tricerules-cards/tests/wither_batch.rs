//! Registry conformance for the reviewed Wither keyword cohort.
//!
//! Every included card is a normal-layout creature whose only rules text is
//! the Wither keyword.
//! Each fingerprint is SHA-256 of the exact pinned normalized Oracle text.

use tricerules_cards::{Keyword, Layout};

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
        "harvest_gwyllion",
        "Harvest Gwyllion",
        "{2}{W/B}{W/B}",
        &["Creature", "Hag"],
        &[],
        Some((2, 4)),
        "a35eb47df4d2b6a3a254afcb3abee617a6a1c6136d8fb3c5a7d186f808a3b90d",
    ),
    (
        "scuzzback_scrapper",
        "Scuzzback Scrapper",
        "{R/G}",
        &["Creature", "Goblin", "Warrior"],
        &[],
        Some((1, 1)),
        "a35eb47df4d2b6a3a254afcb3abee617a6a1c6136d8fb3c5a7d186f808a3b90d",
    ),
    (
        "sickle_ripper",
        "Sickle Ripper",
        "{1}{B}",
        &["Creature", "Elemental", "Warrior"],
        &[],
        Some((2, 1)),
        "a35eb47df4d2b6a3a254afcb3abee617a6a1c6136d8fb3c5a7d186f808a3b90d",
    ),
    (
        "smoldering_butcher",
        "Smoldering Butcher",
        "{3}{B}",
        &["Creature", "Elemental", "Warrior"],
        &[],
        Some((4, 2)),
        "a35eb47df4d2b6a3a254afcb3abee617a6a1c6136d8fb3c5a7d186f808a3b90d",
    ),
    (
        "wildslayer_elves",
        "Wildslayer Elves",
        "{3}{G}",
        &["Creature", "Elf", "Warrior"],
        &[],
        Some((3, 3)),
        "a35eb47df4d2b6a3a254afcb3abee617a6a1c6136d8fb3c5a7d186f808a3b90d",
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
fn wither_batch_maps_definitions() {
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
        assert_eq!(f.keywords, [Keyword::Wither], "{id} keyword");
        assert!(f.protections.is_empty(), "{id}");
        assert!(f.triggered_abilities.is_empty(), "{id}");
        assert!(f.activated_abilities.is_empty(), "{id}");
        assert!(f.static_abilities.is_empty(), "{id}");
    }
}

#[test]
fn wither_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for &(id, name, _, _, _, _, expected) in CARDS {
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
