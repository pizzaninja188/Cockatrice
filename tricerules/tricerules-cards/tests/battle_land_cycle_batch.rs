//! Registry conformance for the reviewed two-basic-land entry cycle.
//!
//! Every card is a normal-layout land with two basic land subtypes, a tap mana
//! ability for the printed pair, and a conditional enters-tapped replacement
//! that checks for two or more basic lands controlled by the entering player.
//! Each fingerprint is SHA-256 of the exact pinned normalized Oracle text.

use tricerules_cards::primitives::{
    AbilityCost, BattlefieldAggregate, CardTypeFilter, EntersTappedAffected, GameCondition,
    RelativePlayerSet, SpellEffectKind, StaticAbilityDef,
};
use tricerules_cards::{AbilityPresentation, Layout, ManaAmount};

type Card = (
    &'static str,
    &'static str,
    &'static [&'static str],
    &'static str,
    &'static str,
    &'static str,
);

const CARDS: &[Card] = &[
    (
        "eclipsed_steppe",
        "Eclipsed Steppe",
        &["Land", "Plains", "Swamp"],
        "w",
        "b",
        "caab2de8423e8235c3823776d924abf2712722f959a0d68aa0489846539d7a8d",
    ),
    (
        "radiant_summit",
        "Radiant Summit",
        &["Land", "Mountain", "Plains"],
        "r",
        "w",
        "fc7cfcc8dbbde4c9e2b9d5385e21b764e284b46409faca0197c8d980f7b13390",
    ),
    (
        "scorched_geyser",
        "Scorched Geyser",
        &["Land", "Island", "Mountain"],
        "u",
        "r",
        "119a3cb43c1a0f79e5b0799ccf89a94fd7f741c2964e0e788fdb560c7c8604d8",
    ),
    (
        "smoldering_marsh",
        "Smoldering Marsh",
        &["Land", "Swamp", "Mountain"],
        "b",
        "r",
        "0de25a61d11a126d82fd51eba4348ce9ceffc239ec1bbed97fc891774c3500c6",
    ),
    (
        "sodden_verdure",
        "Sodden Verdure",
        &["Land", "Forest", "Island"],
        "g",
        "u",
        "3c7d8956f40c899dd41db025b3793c45b9de3a79785df014dee8812e0b50584c",
    ),
    (
        "vernal_fen",
        "Vernal Fen",
        &["Land", "Swamp", "Forest"],
        "b",
        "g",
        "6d2c74541847e89b10cf5d3a884313795ccb39b9fe843a406d05f1464111cfa6",
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
fn battle_land_cycle_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, types, first, second, _) in CARDS {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        assert_eq!(registry.id_for_name(name), Some(id), "{id}");
        let f = face(registry, id);
        assert_eq!(f.face_id.as_str(), id.replace('-', "_"), "{id} face id");
        assert_eq!(f.name, name, "{id} name");
        assert_eq!(f.mana_cost.to_string(), "", "{id} mana cost");
        assert_eq!(f.types, types, "{id} types");
        assert_eq!((f.power, f.toughness), (None, None), "{id} body");
        assert!(f.keywords.is_empty(), "{id}");
        let [mana] = f.activated_abilities.as_slice() else {
            panic!("{id}: exactly one activated ability")
        };
        assert_eq!(
            mana.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{id}"
        );
        assert_eq!(mana.costs, [AbilityCost::Tap], "{id}");
        assert!(mana.is_mana_ability(), "{id}");
        assert!(mana.intrinsic_land_mana, "{id} intrinsic land mana");
        let [SpellEffectKind::ProduceMana { options, .. }] = mana.effect.as_slice() else {
            panic!("{id}: one mana effect")
        };
        assert_eq!(options.len(), 2, "{id}");
        assert_eq!(amounts(&options[0]), mask(first), "{id} first color");
        assert_eq!(amounts(&options[1]), mask(second), "{id} second color");

        let [entry] = f.static_abilities.as_slice() else {
            panic!("{id}: exactly one static ability")
        };
        assert_eq!(
            entry.presentation,
            AbilityPresentation::OracleLines(vec![2]),
            "{id}"
        );
        let StaticAbilityDef::EntersTapped {
            affected: EntersTappedAffected::Self_,
            condition:
                Some(GameCondition::BattlefieldAggregate {
                    filter,
                    aggregate: BattlefieldAggregate::Count,
                    min: None,
                    max: Some(1),
                    ..
                }),
            unless_cost: None,
        } = &entry.definition
        else {
            panic!("{id}: conditional self enters tapped")
        };
        assert_eq!(filter.controllers, RelativePlayerSet::Controller, "{id}");
        assert_eq!(filter.card_type, Some(CardTypeFilter::BasicLand), "{id}");
        assert!(filter.exclude_source, "{id}");
    }
}

#[test]
fn battle_land_cycle_batch_fingerprints_match_the_pinned_oracle_text() {
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
