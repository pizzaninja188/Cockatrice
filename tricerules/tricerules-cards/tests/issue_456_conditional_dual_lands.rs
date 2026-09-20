//! Issue #456 registry and presentation conformance for the conditional dual-land mana family.
//!
//! The four identities print exactly `{T}: Add {C}.` followed by
//! `{T}: Add <c1> or <c2>. Activate only if this land entered this turn or if you control a basic
//! land.` in the pinned Scryfall snapshot `27bf3214-1271-490b-bdfe-c0be6c23d02e`; exact-name
//! records and `rulings_uri` responses were fetched 2026-09-20 and none returned rulings. The
//! expectations below are the reviewed printed Oracle behavior and the handwritten Hidden Lair
//! anchor, not a copy of generator output. CR 605.1a / 605.3 (mana abilities and their normal
//! timing), CR 602.5 (activation restrictions), CR 400.7 and 603.6 (source-relative "entered this
//! turn"), and CR 205.4a (basic land supertype) govern the asserted shapes.

mod common;

use common::FaceExpectation;
use tricerules_cards::primitives::{
    BattlefieldAggregate, BattlefieldPermanentFilter, CardTypeFilter, GameCondition,
    PermanentEventFilter,
};
use tricerules_cards::{
    AbilityCost, AbilityPresentation, AbilitySourceZone, ActivatedAbilityDef, ActivationTiming,
    CardFace, CardRegistry, ManaAmount, RelativePlayerSet, SpellEffectKind,
};

type ManaOption = (u32, u32, u32, u32, u32, u32);

struct ConditionalDualLand {
    id: &'static str,
    name: &'static str,
    face_id: &'static str,
    options: [ManaOption; 2],
}

const CONDITIONAL_DUAL_LANDS: [ConditionalDualLand; 4] = [
    ConditionalDualLand {
        id: "dark_fortress",
        name: "Dark Fortress",
        face_id: "dark_fortress",
        options: [(0, 0, 1, 0, 0, 0), (0, 0, 0, 1, 0, 0)],
    },
    ConditionalDualLand {
        id: "gathering_place",
        name: "Gathering Place",
        face_id: "gathering_place",
        options: [(0, 0, 0, 0, 1, 0), (1, 0, 0, 0, 0, 0)],
    },
    ConditionalDualLand {
        id: "training_compound",
        name: "Training Compound",
        face_id: "training_compound",
        options: [(0, 0, 0, 1, 0, 0), (0, 0, 0, 0, 1, 0)],
    },
    ConditionalDualLand {
        id: "gleaming_bastion",
        name: "Gleaming Bastion",
        face_id: "gleaming_bastion",
        options: [(1, 0, 0, 0, 0, 0), (0, 1, 0, 0, 0, 0)],
    },
];

fn face(id: &str) -> &'static CardFace {
    CardRegistry::global()
        .get(id)
        .unwrap_or_else(|| panic!("missing reviewed card {id}"))
        .primary_face()
}

fn mana_options(ability: &ActivatedAbilityDef) -> Vec<ManaOption> {
    ability
        .mana_options()
        .expect("generated mana ability publishes typed options")
        .iter()
        .map(|option: &ManaAmount| (option.w, option.u, option.b, option.r, option.g, option.c))
        .collect()
}

/// The exact Hidden Lair condition shape: the source's own entry this turn (any controller,
/// source-only event filter) or a basic land controlled by the activating player.
fn hidden_lair_condition() -> GameCondition {
    GameCondition::AnyOf(vec![
        GameCondition::PermanentsEnteredThisTurn {
            controllers: RelativePlayerSet::All,
            filter: PermanentEventFilter {
                source_only: true,
                ..PermanentEventFilter::default()
            },
            min: Some(1),
            max: None,
        },
        GameCondition::BattlefieldAggregate {
            filter: BattlefieldPermanentFilter {
                token: None,
                any_of: None,
                controllers: RelativePlayerSet::Controller,
                card_type: Some(CardTypeFilter::BasicLand),
                color: None,
                name: None,
                required_subtypes: Vec::new(),
                exclude_source: false,
            },
            aggregate: BattlefieldAggregate::Count,
            min: Some(1),
            max: None,
        },
    ])
}

fn assert_colorless_mana_ability(ability: &ActivatedAbilityDef, card_id: &str) {
    assert_eq!(ability.ability_id.as_str(), "activated_01", "{card_id}");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1]),
        "{card_id}"
    );
    assert_eq!(ability.costs, [AbilityCost::Tap], "{card_id}");
    assert_eq!(mana_options(ability), [(0, 0, 0, 0, 0, 1)], "{card_id}");
}

fn assert_conditional_pair_ability(
    ability: &ActivatedAbilityDef,
    card_id: &str,
    options: [ManaOption; 2],
) {
    assert_eq!(ability.ability_id.as_str(), "activated_02", "{card_id}");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![2]),
        "{card_id}"
    );
    assert_eq!(
        ability.source_zone,
        AbilitySourceZone::Battlefield,
        "{card_id}"
    );
    assert_eq!(ability.costs, [AbilityCost::Tap], "{card_id}");
    assert_eq!(
        ability.effect,
        [SpellEffectKind::ProduceMana {
            options: options
                .iter()
                .map(|(w, u, b, r, g, c)| ManaAmount {
                    w: *w,
                    u: *u,
                    b: *b,
                    r: *r,
                    g: *g,
                    c: *c,
                })
                .collect(),
            restriction: None,
            conditional: None,
        }],
        "{card_id}"
    );
    assert_eq!(ability.conditions, [hidden_lair_condition()], "{card_id}");
    assert!(ability.cost_modifiers.is_empty(), "{card_id}");
    assert!(ability.targeting.is_none(), "{card_id}");
    assert_eq!(ability.timing, ActivationTiming::Normal, "{card_id}");
    assert!(ability.activation_limit.is_none(), "{card_id}");
}

#[test]
fn issue_456_registers_exactly_the_reviewed_four() {
    let registry = CardRegistry::global();
    for land in &CONDITIONAL_DUAL_LANDS {
        assert_eq!(
            registry.id_for_name(land.name),
            Some(land.id),
            "{}",
            land.id
        );
        FaceExpectation {
            id: land.id,
            name: land.name,
            face_id: land.face_id,
            mana_cost: "",
            types: &["Land"],
            keywords: &[],
            power_toughness: None,
        }
        .check();
    }
}

#[test]
fn issue_456_lands_produce_colorless_or_their_conditional_pair() {
    for land in &CONDITIONAL_DUAL_LANDS {
        let face = face(land.id);
        assert!(
            face.static_abilities.is_empty(),
            "{} does not enter tapped",
            land.id
        );
        assert!(
            face.triggered_abilities.is_empty(),
            "{} has no triggered abilities",
            land.id
        );
        assert!(
            face.spell_effect.is_empty(),
            "{} is a land with no spell effect",
            land.id
        );
        let [mana, pair] = face.activated_abilities.as_slice() else {
            panic!("{} must have exactly two activated abilities", land.id);
        };
        assert_colorless_mana_ability(mana, land.id);
        assert_conditional_pair_ability(pair, land.id, land.options);
    }
}

#[test]
fn issue_456_hidden_lair_keeps_its_handwritten_definition() {
    let registry = CardRegistry::global();
    assert_eq!(registry.id_for_name("Hidden Lair"), Some("hidden_lair"));
    FaceExpectation {
        id: "hidden_lair",
        name: "Hidden Lair",
        face_id: "hidden_lair",
        mana_cost: "",
        types: &["Land"],
        keywords: &[],
        power_toughness: None,
    }
    .check();

    let hidden_lair = face("hidden_lair");
    let [mana, pair] = hidden_lair.activated_abilities.as_slice() else {
        panic!("Hidden Lair keeps its two handwritten activated abilities");
    };
    assert_colorless_mana_ability(mana, "hidden_lair");
    assert_eq!(pair.ability_id.as_str(), "activated_02");
    assert_eq!(pair.presentation, AbilityPresentation::OracleLines(vec![2]));
    assert_eq!(
        mana_options(pair),
        [(0, 1, 0, 0, 0, 0), (0, 0, 1, 0, 0, 0)],
        "Hidden Lair keeps its `{{U}}` then `{{B}}` printed option order"
    );
    assert_eq!(
        pair.conditions,
        [hidden_lair_condition()],
        "the generated family must mirror this exact handwritten condition shape"
    );
}
