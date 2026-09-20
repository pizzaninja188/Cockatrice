//! Issue #458 registry and presentation conformance for the pump-and-life and seven-lands
//! conditional self-pump families.
//!
//! Moment of Craving and Syphon Fuel each print exactly
//! `Target creature gets -P/-T until end of turn. You gain 2 life.`; Gigantoad and Scorpion
//! Sentinel each print exactly `As long as you control seven or more lands, this creature gets
//! +P/+T.` in the pinned Scryfall snapshot `27bf3214-1271-490b-bdfe-c0be6c23d02e`. Exact-name
//! records and `rulings_uri` responses were fetched 2026-09-20; the Moment of Craving ruling
//! confirms the spell fizzles entirely when its target is illegal, so no life is gained. The
//! expectations below are the reviewed printed Oracle behavior and the shipped typed vocabulary,
//! not a copy of generator output. CR 611.2a / 613.4c (until-end-of-turn P/T modification),
//! CR 608.2b (target revalidation), CR 608.2c (printed instruction order), CR 118.1 (life gain),
//! CR 604.1 / 604.2 (static abilities), CR 611.3 (continuous effects with a live condition), and
//! CR 109.2 (land card type) govern the asserted shapes.

mod common;

use common::FaceExpectation;
use tricerules_cards::primitives::{
    BattlefieldAggregate, BattlefieldPermanentFilter, CardTypeFilter, EffectSubject, GameCondition,
    RelativePlayerSet, SpellEffectKind, StaticAbilityDef, TargetFilter, TargetKind,
    TypeLineAddition,
};
use tricerules_cards::{AbilityPresentation, Amount, CardFace, CardRegistry};

struct PumpLifeCase {
    id: &'static str,
    name: &'static str,
    face_id: &'static str,
    mana_cost: &'static str,
    types: &'static [&'static str],
    power: i32,
    toughness: i32,
}

const PUMP_LIFE_CASES: [PumpLifeCase; 2] = [
    PumpLifeCase {
        id: "moment_of_craving",
        name: "Moment of Craving",
        face_id: "moment_of_craving",
        mana_cost: "{1}{B}",
        types: &["Instant"],
        power: -2,
        toughness: -2,
    },
    PumpLifeCase {
        id: "syphon_fuel",
        name: "Syphon Fuel",
        face_id: "syphon_fuel",
        mana_cost: "{4}{B}",
        types: &["Instant"],
        power: -6,
        toughness: -6,
    },
];

struct SevenLandsCase {
    id: &'static str,
    name: &'static str,
    face_id: &'static str,
    mana_cost: &'static str,
    types: &'static [&'static str],
    power_toughness: (u32, u32),
    delta_power: i32,
    delta_toughness: i32,
}

const SEVEN_LANDS_CASES: [SevenLandsCase; 2] = [
    SevenLandsCase {
        id: "gigantoad",
        name: "Gigantoad",
        face_id: "gigantoad",
        mana_cost: "{3}{G}",
        types: &["Creature", "Frog"],
        power_toughness: (4, 4),
        delta_power: 2,
        delta_toughness: 2,
    },
    SevenLandsCase {
        id: "scorpion_sentinel",
        name: "Scorpion Sentinel",
        face_id: "scorpion_sentinel",
        mana_cost: "{1}{U}",
        types: &["Artifact", "Creature", "Robot", "Scorpion"],
        power_toughness: (1, 4),
        delta_power: 3,
        delta_toughness: 0,
    },
];

fn face(id: &str) -> &'static CardFace {
    CardRegistry::global()
        .get(id)
        .unwrap_or_else(|| panic!("missing reviewed card {id}"))
        .primary_face()
}

/// The reviewed condition: the source's controller controls seven or more lands.
fn seven_lands_condition() -> GameCondition {
    GameCondition::BattlefieldAggregate {
        filter: BattlefieldPermanentFilter {
            token: None,
            any_of: None,
            controllers: RelativePlayerSet::Controller,
            card_type: Some(CardTypeFilter::Land),
            color: None,
            name: None,
            required_subtypes: Vec::new(),
            exclude_source: false,
        },
        aggregate: BattlefieldAggregate::Count,
        min: Some(7),
        max: None,
    }
}

fn seven_lands_definition(delta_power: i32, delta_toughness: i32) -> StaticAbilityDef {
    StaticAbilityDef::ConditionalSelfModifier {
        condition: seven_lands_condition(),
        set_types: None,
        add_types: TypeLineAddition::default(),
        base_power: None,
        base_toughness: None,
        delta_power,
        delta_toughness,
        keywords: Vec::new(),
        activated_abilities: Vec::new(),
        triggered_abilities: Vec::new(),
        can_attack_as_though_without_defender: false,
    }
}

#[test]
fn issue_458_registers_the_four_reviewed_identities() {
    let registry = CardRegistry::global();
    for case in &PUMP_LIFE_CASES {
        assert_eq!(
            registry.id_for_name(case.name),
            Some(case.id),
            "{}",
            case.id
        );
        FaceExpectation {
            id: case.id,
            name: case.name,
            face_id: case.face_id,
            mana_cost: case.mana_cost,
            types: case.types,
            keywords: &[],
            power_toughness: None,
        }
        .check();
    }
    for case in &SEVEN_LANDS_CASES {
        assert_eq!(
            registry.id_for_name(case.name),
            Some(case.id),
            "{}",
            case.id
        );
        FaceExpectation {
            id: case.id,
            name: case.name,
            face_id: case.face_id,
            mana_cost: case.mana_cost,
            types: case.types,
            keywords: &[],
            power_toughness: Some(case.power_toughness),
        }
        .check();
    }
}

#[test]
fn issue_458_pump_life_spells_apply_the_ordered_effects_to_one_mandatory_target() {
    for case in &PUMP_LIFE_CASES {
        let face = face(case.id);
        assert!(
            face.static_abilities.is_empty()
                && face.triggered_abilities.is_empty()
                && face.activated_abilities.is_empty(),
            "{} is a plain spell",
            case.id
        );
        assert_eq!(
            face.spell_effect,
            [
                SpellEffectKind::PumpTarget {
                    power: case.power,
                    toughness: case.toughness,
                    scale: None,
                    subject: EffectSubject::Chosen(Box::new(TargetFilter {
                        kind: TargetKind::Creature,
                        ..TargetFilter::default()
                    })),
                },
                SpellEffectKind::GainLife {
                    amount: Amount::Fixed(2),
                },
            ],
            "{} prints the pump and then the life gain",
            case.id
        );
        let targeting = face
            .targeting
            .as_ref()
            .unwrap_or_else(|| panic!("{} must author its single target group", case.id));
        let [group] = targeting.groups.as_slice() else {
            panic!("{} must author exactly one target group", case.id);
        };
        assert_eq!((group.min, group.max), (1, 1), "{}", case.id);
        assert_eq!(group.prompt, "Choose target creature", "{}", case.id);
        assert_eq!(
            group.effect_indices,
            vec![0],
            "{}: only the pump consumes the target",
            case.id
        );
        assert!(group.distinct_from.is_empty(), "{}", case.id);
        assert!(!group.same_graveyard, "{}", case.id);
        assert!(group.cast_cost_expansion.is_none(), "{}", case.id);
    }
}

#[test]
fn issue_458_seven_lands_creatures_carry_the_exact_live_conditional_modifier() {
    for case in &SEVEN_LANDS_CASES {
        let face = face(case.id);
        assert!(
            face.spell_effect.is_empty()
                && face.triggered_abilities.is_empty()
                && face.activated_abilities.is_empty(),
            "{} is a static-only creature",
            case.id
        );
        let [ability] = face.static_abilities.as_slice() else {
            panic!("{} must own exactly one static ability", case.id);
        };
        assert_eq!(ability.ability_id.as_str(), "static_01", "{}", case.id);
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{}",
            case.id
        );
        assert_eq!(
            ability.definition,
            seven_lands_definition(case.delta_power, case.delta_toughness),
            "{}",
            case.id
        );
    }
}
