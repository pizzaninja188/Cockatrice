//! Registry conformance for the reviewed Flying, protection from red family.
//!
//! Every card is a normal-layout creature whose only rules text is Flying and
//! protection from red. Each fingerprint is SHA-256 of the exact pinned
//! normalized Oracle text.

use tricerules_cards::primitives::ProtectionQuality;
use tricerules_cards::{Color, Keyword, Layout};

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
        "abbey_gargoyles",
        "Abbey Gargoyles",
        "{2}{W}{W}{W}",
        &["Creature", "Gargoyle"],
        (3, 4),
        "d24e48fdd016760394b489518a6d45f68189ac99dfcd58ce20b3dd4a606fd8ed",
    ),
    (
        "aven_smokeweaver",
        "Aven Smokeweaver",
        "{2}{U}{U}",
        &["Creature", "Bird", "Soldier"],
        (2, 3),
        "d24e48fdd016760394b489518a6d45f68189ac99dfcd58ce20b3dd4a606fd8ed",
    ),
    (
        "freewind_falcon",
        "Freewind Falcon",
        "{1}{W}",
        &["Creature", "Bird"],
        (1, 1),
        "d24e48fdd016760394b489518a6d45f68189ac99dfcd58ce20b3dd4a606fd8ed",
    ),
    (
        "hazerider_drake",
        "Hazerider Drake",
        "{2}{W}{U}",
        &["Creature", "Drake"],
        (2, 3),
        "d24e48fdd016760394b489518a6d45f68189ac99dfcd58ce20b3dd4a606fd8ed",
    ),
    (
        "sea_sprite",
        "Sea Sprite",
        "{1}{U}",
        &["Creature", "Faerie"],
        (1, 1),
        "d24e48fdd016760394b489518a6d45f68189ac99dfcd58ce20b3dd4a606fd8ed",
    ),
    (
        "thermal_glider",
        "Thermal Glider",
        "{2}{W}",
        &["Creature", "Human", "Rebel"],
        (2, 1),
        "d24e48fdd016760394b489518a6d45f68189ac99dfcd58ce20b3dd4a606fd8ed",
    ),
    (
        "voice_of_law",
        "Voice of Law",
        "{3}{W}",
        &["Creature", "Angel"],
        (2, 2),
        "d24e48fdd016760394b489518a6d45f68189ac99dfcd58ce20b3dd4a606fd8ed",
    ),
    (
        "weatherseed_faeries",
        "Weatherseed Faeries",
        "{2}{U}",
        &["Creature", "Faerie"],
        (2, 1),
        "d24e48fdd016760394b489518a6d45f68189ac99dfcd58ce20b3dd4a606fd8ed",
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
fn protection_red_flying_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, mana, types, (power, toughness), _) in CARDS {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        assert_eq!(registry.id_for_name(name), Some(id), "{id}");
        let f = face(registry, id);
        assert_eq!(f.face_id.as_str(), id.replace('-', "_"), "{id} face id");
        assert_eq!(f.name, name, "{id} name");
        assert_eq!(f.mana_cost.to_string(), mana, "{id} mana cost");
        assert_eq!(f.types, types, "{id} types");
        assert!(f.supertypes.is_empty(), "{id} supertypes");
        assert_eq!(
            (f.power, f.toughness),
            (Some(power), Some(toughness)),
            "{id} body"
        );
        assert_eq!(f.keywords, [Keyword::Flying], "{id} keyword");
        assert_eq!(
            f.protections,
            [ProtectionQuality::Color(Color::Red)],
            "{id} protection"
        );
        assert!(f.static_abilities.is_empty(), "{id}");
        assert!(f.activated_abilities.is_empty(), "{id}");
        assert!(f.triggered_abilities.is_empty(), "{id}");
        assert!(f.spell_effect.is_empty(), "{id}");
        assert!(f.targeting.is_none(), "{id}");
    }
}

#[test]
fn protection_red_flying_batch_fingerprints_match_the_pinned_oracle_text() {
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
