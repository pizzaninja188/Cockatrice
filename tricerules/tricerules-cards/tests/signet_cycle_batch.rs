//! Registry conformance for the reviewed Signet artifact cycle.
//!
//! Every card is a {2} artifact with one `{1}, {T}` mana ability that adds
//! its printed color pair. Each fingerprint is SHA-256 of the exact pinned
//! normalized Oracle text.

use tricerules_cards::primitives::{AbilityCost, SpellEffectKind};
use tricerules_cards::{AbilityPresentation, Layout, ManaAmount, ManaCost};

type Card = (
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    &'static str,
);

const CARDS: &[Card] = &[
    (
        "azorius_signet",
        "Azorius Signet",
        "w",
        "u",
        "162e9c2434f64310ce1c8ac328430c5307c592289346a046a7f26f76f3b3861f",
    ),
    (
        "boros_signet",
        "Boros Signet",
        "r",
        "w",
        "324a680fbbfa421309b523a1fe951e863dca1302a8aa58692ae1977f932282e0",
    ),
    (
        "dimir_signet",
        "Dimir Signet",
        "u",
        "b",
        "0dc995e30fce95de5c99984b457ad408a58d5aac1db57b87dd5f1be8077fc185",
    ),
    (
        "golgari_signet",
        "Golgari Signet",
        "b",
        "g",
        "acb0a020e21025ccde4321fcb890b29f6ad91a39ef04ddd3f5b17862623899e8",
    ),
    (
        "gruul_signet",
        "Gruul Signet",
        "r",
        "g",
        "73c8abc48f0fcb22becfb2689dca668bdbf468ddaaec7cb7ce078f478f4047bc",
    ),
    (
        "izzet_signet",
        "Izzet Signet",
        "u",
        "r",
        "84eb910b73d5d2f8209f2c6e8a15bd9ee1903efc7b63f32ea1bb418ececf958b",
    ),
    (
        "orzhov_signet",
        "Orzhov Signet",
        "w",
        "b",
        "c43c2e7bde1a3849159689cf8bc95ace1a88fa712779c084767e71e10d62dba8",
    ),
    (
        "rakdos_signet",
        "Rakdos Signet",
        "b",
        "r",
        "d432a422f30cbcfd33adec0673492c8760fae25e9e5ac3ef482059279647a666",
    ),
    (
        "selesnya_signet",
        "Selesnya Signet",
        "g",
        "w",
        "2624c89a7f30627af56f34df748df8d82df598b3756a4d9568340c3dd3f3f987",
    ),
    (
        "simic_signet",
        "Simic Signet",
        "g",
        "u",
        "499bfba7fc39112e8dd5f586669328ee70951a9d35978769f050dc3fc85e41a6",
    ),
];

fn mask(color: &str) -> [u32; 6] {
    match color {
        "w" => [1, 0, 0, 0, 0, 0],
        "u" => [0, 1, 0, 0, 0, 0],
        "b" => [0, 0, 1, 0, 0, 0],
        "r" => [0, 0, 0, 1, 0, 0],
        "g" => [0, 0, 0, 0, 1, 0],
        _ => unreachable!("printed color"),
    }
}

fn amounts(amount: &ManaAmount) -> [u32; 6] {
    [amount.w, amount.u, amount.b, amount.r, amount.g, amount.c]
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
fn signet_cycle_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, first, second, _) in CARDS {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        assert_eq!(registry.id_for_name(name), Some(id), "{id}");
        let f = face(registry, id);
        assert_eq!(f.face_id.as_str(), id.replace('-', "_"), "{id} face id");
        assert_eq!(f.name, name, "{id} name");
        assert_eq!(f.mana_cost.to_string(), "{2}", "{id} mana cost");
        assert_eq!(f.types, ["Artifact"], "{id} types");
        assert_eq!((f.power, f.toughness), (None, None), "{id} body");
        assert!(f.keywords.is_empty(), "{id}");
        assert!(f.triggered_abilities.is_empty(), "{id}");
        assert!(f.static_abilities.is_empty(), "{id}");
        assert!(f.spell_effect.is_empty(), "{id}");
        assert!(f.targeting.is_none(), "{id}");
        let [ability] = f.activated_abilities.as_slice() else {
            panic!("{id}: exactly one activated ability")
        };
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{id}"
        );
        assert_eq!(
            ability.costs,
            [
                AbilityCost::Mana(ManaCost::parse("{1}").unwrap()),
                AbilityCost::Tap
            ],
            "{id} costs"
        );
        assert!(ability.is_mana_ability(), "{id}");
        let [SpellEffectKind::ProduceMana { options, .. }] = ability.effect.as_slice() else {
            panic!("{id}: one mana effect")
        };
        assert_eq!(options.len(), 1, "{id}");
        let a = mask(first);
        let b = mask(second);
        assert_eq!(
            amounts(&options[0]),
            [
                a[0] + b[0],
                a[1] + b[1],
                a[2] + b[2],
                a[3] + b[3],
                a[4] + b[4],
                a[5] + b[5]
            ],
            "{id} printed color pair"
        );
    }
}

#[test]
fn signet_cycle_batch_fingerprints_match_the_pinned_oracle_text() {
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
