//! Registry conformance for the reviewed pain-land cycle.
//!
//! Every card is a normal-layout land with the printed colorless tap ability and
//! the printed two-color tap ability that deals exactly 1 damage to its
//! controller. Each fingerprint is SHA-256 of the exact pinned normalized Oracle
//! text.

use tricerules_cards::primitives::{AbilityCost, Amount, SpellEffectKind};
use tricerules_cards::{AbilityPresentation, Layout};

type Card = (&'static str, &'static str, [u32; 6], [u32; 6], &'static str);

const CARDS: &[Card] = &[
    (
        "shivan_reef",
        "Shivan Reef",
        [0, 1, 0, 0, 0, 0],
        [0, 0, 0, 1, 0, 0],
        "ad131b3706727a7b2ee7ff25cf7c6686579a6573c118ff1443b2d93d28e9adb8",
    ),
    (
        "llanowar_wastes",
        "Llanowar Wastes",
        [0, 0, 1, 0, 0, 0],
        [0, 0, 0, 0, 1, 0],
        "cebf8df54301248b90690b78bd9055ab581b4a8df4e68be656899a07d57c25ac",
    ),
    (
        "caves_of_koilos",
        "Caves of Koilos",
        [1, 0, 0, 0, 0, 0],
        [0, 0, 1, 0, 0, 0],
        "f31fcdbb9e70984cf6b2478413908bb909ab8e7216a39bd1d6e8a59a064b289e",
    ),
    (
        "brushland",
        "Brushland",
        [0, 0, 0, 0, 1, 0],
        [1, 0, 0, 0, 0, 0],
        "0cbbde32df5a468051aa547b6605ca3768e109504a973da68d4bad6918be8193",
    ),
    (
        "battlefield_forge",
        "Battlefield Forge",
        [0, 0, 0, 1, 0, 0],
        [1, 0, 0, 0, 0, 0],
        "b0b6be240eb6f9e66ec2dacc6b242d3e76611e70555c81c58e2c11465f0fffdb",
    ),
    (
        "underground_river",
        "Underground River",
        [0, 1, 0, 0, 0, 0],
        [0, 0, 1, 0, 0, 0],
        "4e396a80d3cce3b2e641260b1e4a4b02f8f1a1e7836d3e823ac4a9092e756501",
    ),
    (
        "adarkar_wastes",
        "Adarkar Wastes",
        [1, 0, 0, 0, 0, 0],
        [0, 1, 0, 0, 0, 0],
        "d2254dd98594656caeb4cb9e0c094184f8ef7876f381d715c65d583fb6c581b5",
    ),
    (
        "sulfurous_springs",
        "Sulfurous Springs",
        [0, 0, 1, 0, 0, 0],
        [0, 0, 0, 1, 0, 0],
        "92bc640cf54100dbb53dcc62b9554f5187bf5b2b5fe6ff788caf6d5935fa3b75",
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

fn mask(amount: &tricerules_cards::ManaAmount) -> [u32; 6] {
    [amount.w, amount.u, amount.b, amount.r, amount.g, amount.c]
}

#[test]
fn pain_lands_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, first, second, _) in CARDS {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        assert_eq!(registry.id_for_name(name), Some(id), "{id}");
        let f = face(registry, id);
        assert_eq!(f.face_id.as_str(), id, "{id} face id");
        assert_eq!(f.name, name, "{id} name");
        assert_eq!(f.mana_cost.to_string(), "", "{id} mana cost");
        assert_eq!(f.types, ["Land"], "{id} types");
        assert_eq!((f.power, f.toughness), (None, None), "{id} body");
        assert!(f.keywords.is_empty(), "{id}");
        assert!(f.triggered_abilities.is_empty(), "{id}");
        assert!(f.static_abilities.is_empty(), "{id}");
        assert!(f.spell_effect.is_empty(), "{id}");
        assert!(f.targeting.is_none(), "{id}");

        let [colorless, pain] = f.activated_abilities.as_slice() else {
            panic!("{id}: exactly two activated abilities");
        };
        assert_eq!(colorless.ability_id.as_str(), "activated_01", "{id}");
        assert_eq!(
            colorless.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{id}"
        );
        assert_eq!(colorless.costs, [AbilityCost::Tap], "{id}");
        assert!(colorless.is_mana_ability(), "{id}: colorless mana ability");
        assert!(
            matches!(
                colorless.effect.as_slice(),
                [SpellEffectKind::ProduceMana { options, .. }]
                    if options.len() == 1 && mask(&options[0]) == [0, 0, 0, 0, 0, 1]
            ),
            "{id}: one colorless option"
        );

        assert_eq!(pain.ability_id.as_str(), "activated_02", "{id}");
        assert_eq!(
            pain.presentation,
            AbilityPresentation::OracleLines(vec![2]),
            "{id}"
        );
        assert_eq!(pain.costs, [AbilityCost::Tap], "{id}: tap only");
        assert!(pain.is_mana_ability(), "{id}: pain mana ability");
        let [SpellEffectKind::ProduceMana { options, .. }, SpellEffectKind::DamagePlayer { amount, .. }] =
            pain.effect.as_slice()
        else {
            panic!("{id}: produce mana then damage its controller");
        };
        assert_eq!(options.len(), 2, "{id}");
        assert_eq!(mask(&options[0]), first, "{id} first option");
        assert_eq!(mask(&options[1]), second, "{id} second option");
        assert_eq!(*amount, Amount::Fixed(1), "{id}: exactly one damage");
        assert!(pain.targeting.is_none(), "{id}: no target");
    }
}

#[test]
fn pain_lands_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for &(id, name, _, _, expected) in CARDS {
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
