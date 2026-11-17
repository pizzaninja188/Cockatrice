//! Registry conformance for the queue-v29 basic Snow land cohort.
use tricerules_cards::primitives::{AbilityCost, ManaAmount, SpellEffectKind};
use tricerules_cards::Layout;

struct Card {
    id: &'static str,
    name: &'static str,
    types: &'static [&'static str],
    mana: ManaAmount,
}

const CARDS: &[Card] = &[
    Card {
        id: "snow-covered_plains",
        name: "Snow-Covered Plains",
        types: &["Land", "Plains"],
        mana: ManaAmount {
            w: 1,
            u: 0,
            b: 0,
            r: 0,
            g: 0,
            c: 0,
        },
    },
    Card {
        id: "snow-covered_island",
        name: "Snow-Covered Island",
        types: &["Land", "Island"],
        mana: ManaAmount {
            w: 0,
            u: 1,
            b: 0,
            r: 0,
            g: 0,
            c: 0,
        },
    },
    Card {
        id: "snow-covered_swamp",
        name: "Snow-Covered Swamp",
        types: &["Land", "Swamp"],
        mana: ManaAmount {
            w: 0,
            u: 0,
            b: 1,
            r: 0,
            g: 0,
            c: 0,
        },
    },
    Card {
        id: "snow-covered_mountain",
        name: "Snow-Covered Mountain",
        types: &["Land", "Mountain"],
        mana: ManaAmount {
            w: 0,
            u: 0,
            b: 0,
            r: 1,
            g: 0,
            c: 0,
        },
    },
    Card {
        id: "snow-covered_forest",
        name: "Snow-Covered Forest",
        types: &["Land", "Forest"],
        mana: ManaAmount {
            w: 0,
            u: 0,
            b: 0,
            r: 0,
            g: 1,
            c: 0,
        },
    },
];

#[test]
fn q29_snow_basics_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for card in CARDS {
        let definition = registry
            .get(card.id)
            .unwrap_or_else(|| panic!("missing {}", card.id));
        let f = definition.primary_face();
        assert_eq!(registry.id_for_name(card.name), Some(card.id));
        assert_eq!(f.face_id.as_str(), card.id.replace('-', "_"));
        assert_eq!(f.name, card.name);
        assert!(f.mana_cost.is_empty(), "{}: no mana cost", card.id);
        assert_eq!(f.types, card.types);
        assert_eq!(f.supertypes, ["Basic", "Snow"], "{}", card.id);
        assert_eq!(definition.layout, Layout::Normal);
        assert!(f.keywords.is_empty());
        assert!(f.triggered_abilities.is_empty());
        assert!(f.static_abilities.is_empty());
        assert!(f.spell_effect.is_empty());
        let [ability] = f.activated_abilities.as_slice() else {
            panic!("{}: exactly one activated ability", card.id);
        };
        assert!(ability.intrinsic_land_mana, "{}", card.id);
        assert_eq!(ability.costs, [AbilityCost::Tap], "{}", card.id);
        assert!(ability.targeting.is_none(), "{}", card.id);
        let [effect] = ability.effect.as_slice() else {
            panic!("{}: exactly one effect", card.id);
        };
        let SpellEffectKind::ProduceMana { options, .. } = effect else {
            panic!("{}: produce mana", card.id);
        };
        assert_eq!(options.as_slice(), [card.mana], "{}", card.id);
    }
}

#[test]
fn q29_snow_basics_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, expected) in [
        (
            "snow-covered_plains",
            "Snow-Covered Plains",
            "f3f8e4a29db20544bd9a5b0ae8e10a23a2ae52112da77fa9111d60fb9d409d22",
        ),
        (
            "snow-covered_island",
            "Snow-Covered Island",
            "a6050f1a6c1f21586486a1e21aa8be9bc41340fa0beb2ae1a5952cf26b7f4a57",
        ),
        (
            "snow-covered_swamp",
            "Snow-Covered Swamp",
            "5ad0718d1920ec4ca2dc64a9aff6383c8d801d418cfda034ed19aa016d16dfb4",
        ),
        (
            "snow-covered_mountain",
            "Snow-Covered Mountain",
            "91e8319d188a3c46e5c83ea1023ab6a453d5b028c8b00254c1168bdc23578e3c",
        ),
        (
            "snow-covered_forest",
            "Snow-Covered Forest",
            "6215eab3372e7128de713b41e7758fc9b179ade5bc47d33a5af8cef9c9c7493b",
        ),
    ] {
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
