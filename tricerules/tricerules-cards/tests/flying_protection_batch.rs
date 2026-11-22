//! Registry conformance for the reviewed Flying / protection keyword cohort.
//!
//! Every included card is a normal-layout creature whose only rules text is
//! Flying and one printed protection-from-a-color quality.
//! Each fingerprint is SHA-256 of the exact pinned normalized Oracle text.

use tricerules_cards::primitives::ProtectionQuality;
use tricerules_cards::{Keyword, Layout};

type Card = (
    &'static str,
    &'static str,
    &'static str,
    &'static [&'static str],
    &'static [&'static str],
    Option<(u32, u32)>,
    ProtectionQuality,
    &'static str,
);

const CARDS: &[Card] = &[
    (
        "coast_watcher",
        "Coast Watcher",
        "{1}{U}",
        &["Creature", "Bird", "Soldier"],
        &[],
        Some((1, 1)),
        ProtectionQuality::Color(tricerules_cards::Color::Green),
        "baab29e18c433a31288cf50dfb1bf341ba3d9acc745a7d9d3bd0f491182f05a6",
    ),
    (
        "treetop_sentinel",
        "Treetop Sentinel",
        "{2}{U}{U}",
        &["Creature", "Bird", "Soldier"],
        &[],
        Some((2, 3)),
        ProtectionQuality::Color(tricerules_cards::Color::Green),
        "baab29e18c433a31288cf50dfb1bf341ba3d9acc745a7d9d3bd0f491182f05a6",
    ),
    (
        "cerulean_wyvern",
        "Cerulean Wyvern",
        "{4}{U}",
        &["Creature", "Drake"],
        &[],
        Some((3, 3)),
        ProtectionQuality::Color(tricerules_cards::Color::Green),
        "baab29e18c433a31288cf50dfb1bf341ba3d9acc745a7d9d3bd0f491182f05a6",
    ),
    (
        "voice_of_duty",
        "Voice of Duty",
        "{3}{W}",
        &["Creature", "Angel"],
        &[],
        Some((2, 2)),
        ProtectionQuality::Color(tricerules_cards::Color::Green),
        "baab29e18c433a31288cf50dfb1bf341ba3d9acc745a7d9d3bd0f491182f05a6",
    ),
    (
        "melesse_spirit",
        "Melesse Spirit",
        "{3}{W}{W}",
        &["Creature", "Angel", "Spirit"],
        &[],
        Some((3, 3)),
        ProtectionQuality::Color(tricerules_cards::Color::Black),
        "c3a800b101454dc096b3ce2602bda8764e54f438c35551a458b78acff851b27d",
    ),
    (
        "voice_of_grace",
        "Voice of Grace",
        "{3}{W}",
        &["Creature", "Angel"],
        &[],
        Some((2, 2)),
        ProtectionQuality::Color(tricerules_cards::Color::Black),
        "c3a800b101454dc096b3ce2602bda8764e54f438c35551a458b78acff851b27d",
    ),
    (
        "nightwind_glider",
        "Nightwind Glider",
        "{2}{W}",
        &["Creature", "Human", "Rebel"],
        &[],
        Some((2, 1)),
        ProtectionQuality::Color(tricerules_cards::Color::Black),
        "c3a800b101454dc096b3ce2602bda8764e54f438c35551a458b78acff851b27d",
    ),
    (
        "duskrider_falcon",
        "Duskrider Falcon",
        "{1}{W}",
        &["Creature", "Bird"],
        &[],
        Some((1, 1)),
        ProtectionQuality::Color(tricerules_cards::Color::Black),
        "c3a800b101454dc096b3ce2602bda8764e54f438c35551a458b78acff851b27d",
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
fn flying_protection_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, mana, types, supertypes, body, expected, _) in CARDS {
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
        assert_eq!(f.keywords, [Keyword::Flying], "{id} keyword");
        assert_eq!(
            f.protections,
            [expected],
            "{id}: exactly the printed protection quality"
        );
        assert!(f.triggered_abilities.is_empty(), "{id}");
        assert!(f.activated_abilities.is_empty(), "{id}");
        assert!(f.static_abilities.is_empty(), "{id}");
    }
}

#[test]
fn flying_protection_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for &(id, name, _, _, _, _, _, expected) in CARDS {
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
