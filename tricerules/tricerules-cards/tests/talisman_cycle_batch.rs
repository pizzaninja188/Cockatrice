//! Registry conformance for the reviewed Talisman artifact cycle.
//!
//! Every card is a {2} artifact with a colorless mana ability and a second mana ability that adds one of two printed colors and deals one damage to its controller.
//! Each fingerprint is SHA-256 of the exact pinned normalized Oracle text.

use tricerules_cards::primitives::{AbilityCost, Amount, PlayerRecipient, SpellEffectKind};
use tricerules_cards::{AbilityPresentation, Layout};

type Card = (&'static str, &'static str, &'static str);

const CARDS: &[Card] = &[
    (
        "talisman_of_conviction",
        "Talisman of Conviction",
        "5ade3842ebdc4d27b0f7b6f519b00febcfcd9c67cd576f79df46710e86658a51",
    ),
    (
        "talisman_of_creativity",
        "Talisman of Creativity",
        "d0a5f8ffdc48a3c3a7d160dc7aa1df9dcc82694c35212a1bba88afa5bd0b873b",
    ),
    (
        "talisman_of_curiosity",
        "Talisman of Curiosity",
        "bd1e07e86691a3d7034485c618495dc57fb6b106328de5cbb55ad3ba4473e35d",
    ),
    (
        "talisman_of_dominance",
        "Talisman of Dominance",
        "7ff2ef420254b21bf620b55143c5f4183ac4e6845bba2607f01229a9f906a356",
    ),
    (
        "talisman_of_hierarchy",
        "Talisman of Hierarchy",
        "94e9976598bec41a4ae081ff3d99a5c469ae73f7b804fc7c6912fa5609ca6cb0",
    ),
    (
        "talisman_of_indulgence",
        "Talisman of Indulgence",
        "a1f3c8967c5612e5aad193948da7f997c578e570b58eef745c319f2dea9cdd82",
    ),
    (
        "talisman_of_progress",
        "Talisman of Progress",
        "61d8b1d0205b37862eb7d9a18431f40ddcd1a4f39ed95e1bc0a2d74dfe43ee1e",
    ),
    (
        "talisman_of_resilience",
        "Talisman of Resilience",
        "d63d362d38274797c339cdad0ac16e7aecb002b170ac6596df46267402de9ec0",
    ),
    (
        "talisman_of_unity",
        "Talisman of Unity",
        "d5f0feb910cea9b7bb9fbad702787049ce7f80f3acdfe49111c38e6f3b220c28",
    ),
];

fn mask(color: char) -> [u32; 6] {
    match color {
        'w' => [1, 0, 0, 0, 0, 0],
        'u' => [0, 1, 0, 0, 0, 0],
        'b' => [0, 0, 1, 0, 0, 0],
        'r' => [0, 0, 0, 1, 0, 0],
        'g' => [0, 0, 0, 0, 1, 0],
        _ => unreachable!("printed color"),
    }
}

fn amounts(amount: &tricerules_cards::ManaAmount) -> [u32; 6] {
    [amount.w, amount.u, amount.b, amount.r, amount.g, amount.c]
}

#[test]
fn talisman_cycle_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, _) in CARDS {
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

        let [colorless, pain] = f.activated_abilities.as_slice() else {
            panic!("{id}: exactly two activated abilities");
        };
        assert_eq!(
            colorless.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{id}"
        );
        assert_eq!(colorless.costs, [AbilityCost::Tap], "{id}");
        assert!(colorless.is_mana_ability(), "{id}");
        assert!(
            matches!(
                colorless.effect.as_slice(),
                [SpellEffectKind::ProduceMana { options, .. }]
                    if options.len() == 1 && amounts(&options[0]) == [0, 0, 0, 0, 0, 1]
            ),
            "{id}: one colorless option"
        );

        assert_eq!(
            pain.presentation,
            AbilityPresentation::OracleLines(vec![2]),
            "{id}"
        );
        assert_eq!(pain.costs, [AbilityCost::Tap], "{id}");
        assert!(pain.is_mana_ability(), "{id}");
        let printed = match id {
            "talisman_of_conviction" => ['r', 'w'],
            "talisman_of_creativity" => ['u', 'r'],
            "talisman_of_curiosity" => ['g', 'u'],
            "talisman_of_dominance" => ['u', 'b'],
            "talisman_of_hierarchy" => ['w', 'b'],
            "talisman_of_indulgence" => ['b', 'r'],
            "talisman_of_progress" => ['w', 'u'],
            "talisman_of_resilience" => ['b', 'g'],
            "talisman_of_unity" => ['g', 'w'],
            _ => unreachable!("printed card"),
        };
        let [SpellEffectKind::ProduceMana { options, .. }, SpellEffectKind::DamagePlayer { amount, who, .. }] =
            pain.effect.as_slice()
        else {
            panic!("{id}: colored mana plus self damage");
        };
        assert_eq!(options.len(), 2, "{id}");
        assert_eq!(amounts(&options[0]), mask(printed[0]), "{id} first color");
        assert_eq!(amounts(&options[1]), mask(printed[1]), "{id} second color");
        assert_eq!(*amount, Amount::Fixed(1), "{id}: exactly one damage");
        assert_eq!(*who, PlayerRecipient::Controller, "{id}: to its controller");
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
fn talisman_cycle_batch_fingerprints_match_the_pinned_oracle_text() {
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
