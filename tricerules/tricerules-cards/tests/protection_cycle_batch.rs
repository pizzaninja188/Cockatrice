//! Registry conformance for the reviewed protection keyword cycle.
//!
//! Every included card is a normal-layout creature with one printed protection
//! quality and no other rules text.
//! Each fingerprint is SHA-256 of the exact pinned normalized Oracle text.

use tricerules_cards::primitives::ProtectionQuality;
use tricerules_cards::Layout;

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
        "galinas_knight",
        "Galina's Knight",
        "{W}{U}",
        &["Creature", "Merfolk", "Knight"],
        &[],
        Some((2, 2)),
        ProtectionQuality::Color(tricerules_cards::Color::Red),
        "42e9b6253e88c49ae253461eaf29952552d2be2c3117a7d78a8df01a90b39e1d",
    ),
    (
        "oraxid",
        "Oraxid",
        "{3}{U}",
        &["Creature", "Crab", "Beast"],
        &[],
        Some((2, 3)),
        ProtectionQuality::Color(tricerules_cards::Color::Red),
        "42e9b6253e88c49ae253461eaf29952552d2be2c3117a7d78a8df01a90b39e1d",
    ),
    (
        "repentant_blacksmith",
        "Repentant Blacksmith",
        "{1}{W}",
        &["Creature", "Human"],
        &[],
        Some((1, 2)),
        ProtectionQuality::Color(tricerules_cards::Color::Red),
        "42e9b6253e88c49ae253461eaf29952552d2be2c3117a7d78a8df01a90b39e1d",
    ),
    (
        "vedalken_outlander",
        "Vedalken Outlander",
        "{W}{U}",
        &["Artifact", "Creature", "Vedalken", "Scout"],
        &[],
        Some((2, 2)),
        ProtectionQuality::Color(tricerules_cards::Color::Red),
        "42e9b6253e88c49ae253461eaf29952552d2be2c3117a7d78a8df01a90b39e1d",
    ),
    (
        "vulshok_refugee",
        "Vulshok Refugee",
        "{1}{R}{R}",
        &["Creature", "Human", "Warrior"],
        &[],
        Some((3, 2)),
        ProtectionQuality::Color(tricerules_cards::Color::Red),
        "42e9b6253e88c49ae253461eaf29952552d2be2c3117a7d78a8df01a90b39e1d",
    ),
    (
        "guma",
        "Guma",
        "{2}{R}",
        &["Creature", "Cat"],
        &[],
        Some((2, 2)),
        ProtectionQuality::Color(tricerules_cards::Color::Blue),
        "180a404e731b4fa5562ef4b726b0f095e3f1aaf09a7254df7bcf73f2b554438d",
    ),
    (
        "karoo_meerkat",
        "Karoo Meerkat",
        "{1}{G}",
        &["Creature", "Mongoose"],
        &[],
        Some((2, 1)),
        ProtectionQuality::Color(tricerules_cards::Color::Blue),
        "180a404e731b4fa5562ef4b726b0f095e3f1aaf09a7254df7bcf73f2b554438d",
    ),
    (
        "nacatl_outlander",
        "Nacatl Outlander",
        "{R}{G}",
        &["Creature", "Cat", "Scout"],
        &[],
        Some((2, 2)),
        ProtectionQuality::Color(tricerules_cards::Color::Blue),
        "180a404e731b4fa5562ef4b726b0f095e3f1aaf09a7254df7bcf73f2b554438d",
    ),
    (
        "yavimaya_barbarian",
        "Yavimaya Barbarian",
        "{R}{G}",
        &["Creature", "Elf", "Barbarian"],
        &[],
        Some((2, 2)),
        ProtectionQuality::Color(tricerules_cards::Color::Blue),
        "180a404e731b4fa5562ef4b726b0f095e3f1aaf09a7254df7bcf73f2b554438d",
    ),
    (
        "death_speakers",
        "Death Speakers",
        "{W}",
        &["Creature", "Human", "Cleric"],
        &[],
        Some((1, 1)),
        ProtectionQuality::Color(tricerules_cards::Color::Black),
        "4fe118d9ae8b860e0eb9e44537c465be84d9310b4943a11a917e4572632ec56e",
    ),
    (
        "llanowar_knight",
        "Llanowar Knight",
        "{G}{W}",
        &["Creature", "Elf", "Knight"],
        &[],
        Some((2, 2)),
        ProtectionQuality::Color(tricerules_cards::Color::Black),
        "4fe118d9ae8b860e0eb9e44537c465be84d9310b4943a11a917e4572632ec56e",
    ),
    (
        "scalebanes_elite",
        "Scalebane's Elite",
        "{3}{G}{W}",
        &["Creature", "Human", "Soldier"],
        &[],
        Some((4, 4)),
        ProtectionQuality::Color(tricerules_cards::Color::Black),
        "4fe118d9ae8b860e0eb9e44537c465be84d9310b4943a11a917e4572632ec56e",
    ),
    (
        "valeron_outlander",
        "Valeron Outlander",
        "{G}{W}",
        &["Creature", "Human", "Scout"],
        &[],
        Some((2, 2)),
        ProtectionQuality::Color(tricerules_cards::Color::Black),
        "4fe118d9ae8b860e0eb9e44537c465be84d9310b4943a11a917e4572632ec56e",
    ),
    (
        "goblin_outlander",
        "Goblin Outlander",
        "{B}{R}",
        &["Creature", "Goblin", "Scout"],
        &[],
        Some((2, 2)),
        ProtectionQuality::Color(tricerules_cards::Color::White),
        "71902598bb3663c35563caf407e62891c108ec476a2d6143f33d6ec4d96dcf01",
    ),
    (
        "ihsans_shade",
        "Ihsan's Shade",
        "{3}{B}{B}{B}",
        &["Creature", "Shade", "Knight"],
        &["Legendary"],
        Some((5, 5)),
        ProtectionQuality::Color(tricerules_cards::Color::White),
        "71902598bb3663c35563caf407e62891c108ec476a2d6143f33d6ec4d96dcf01",
    ),
    (
        "shivan_zombie",
        "Shivan Zombie",
        "{B}{R}",
        &["Creature", "Phyrexian", "Barbarian", "Zombie"],
        &[],
        Some((2, 2)),
        ProtectionQuality::Color(tricerules_cards::Color::White),
        "71902598bb3663c35563caf407e62891c108ec476a2d6143f33d6ec4d96dcf01",
    ),
    (
        "nacatl_savage",
        "Nacatl Savage",
        "{1}{G}",
        &["Creature", "Cat", "Warrior"],
        &[],
        Some((2, 1)),
        ProtectionQuality::CardType(tricerules_cards::primitives::ProtectionCardType::Artifact),
        "1bd6efb27d050c8708c6b2be041e6c9cd9999b612539569fa91b4ec1c2ce2478",
    ),
    (
        "tel-jilad_chosen",
        "Tel-Jilad Chosen",
        "{1}{G}",
        &["Creature", "Elf", "Warrior"],
        &[],
        Some((2, 1)),
        ProtectionQuality::CardType(tricerules_cards::primitives::ProtectionCardType::Artifact),
        "1bd6efb27d050c8708c6b2be041e6c9cd9999b612539569fa91b4ec1c2ce2478",
    ),
    (
        "tel-jilad_outrider",
        "Tel-Jilad Outrider",
        "{3}{G}",
        &["Creature", "Elf", "Warrior"],
        &[],
        Some((3, 1)),
        ProtectionQuality::CardType(tricerules_cards::primitives::ProtectionCardType::Artifact),
        "1bd6efb27d050c8708c6b2be041e6c9cd9999b612539569fa91b4ec1c2ce2478",
    ),
    (
        "yavimaya_scion",
        "Yavimaya Scion",
        "{4}{G}",
        &["Creature", "Treefolk"],
        &[],
        Some((4, 4)),
        ProtectionQuality::CardType(tricerules_cards::primitives::ProtectionCardType::Artifact),
        "1bd6efb27d050c8708c6b2be041e6c9cd9999b612539569fa91b4ec1c2ce2478",
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
fn protection_cycle_batch_maps_definitions() {
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
        assert!(f.keywords.is_empty(), "{id}");
        assert!(f.triggered_abilities.is_empty(), "{id}");
        assert!(f.activated_abilities.is_empty(), "{id}");
        assert!(f.static_abilities.is_empty(), "{id}");
        assert_eq!(
            f.protections,
            [expected],
            "{id}: exactly the printed protection quality"
        );
    }
}

#[test]
fn protection_cycle_batch_fingerprints_match_the_pinned_oracle_text() {
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
