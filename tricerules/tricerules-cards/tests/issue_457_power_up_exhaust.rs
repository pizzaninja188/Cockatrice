//! Issue #457 registry and presentation conformance for the Power-up and Exhaust two-counter
//! family cohort.
//!
//! Serpent Specialist, Brave Brawler, Volcanic Villain, Prowcatcher Specialist, and Skystreak
//! Engineer each print exactly `<Keyword> — {cost}: Put two +1/+1 counters on this creature.`
//! plus reminder text in the pinned Scryfall snapshot `27bf3214-1271-490b-bdfe-c0be6c23d02e`.
//! Exact-name records and `rulings_uri` responses were fetched 2026-09-20; the Exhaust rulings
//! confirm normal activation timing and that a permanent which leaves and returns is a new object
//! whose Exhaust ability can be activated again (CR 400.7). The expectations below are the
//! reviewed printed Oracle behavior and the shipped typed vocabulary, not a copy of generator
//! output. CR 602.2b (activation costs), CR 702.193 (Power-up; verified against the current
//! official 2026-09-25 Comprehensive Rules), CR 702.177 (Exhaust), and CR 122.1 (counters) govern
//! the asserted shapes.

mod common;

use common::FaceExpectation;
use tricerules_cards::primitives::{
    ActivatedCostModifier, ActivationLimit, EffectSubject, GameCondition, PermanentEventFilter,
    RelativePlayerSet, SpellEffectKind,
};
use tricerules_cards::{
    AbilityCost, AbilityPresentation, AbilitySourceZone, ActivationTiming, Amount, CardFace,
    CardRegistry, CounterKind, Keyword, ManaCost,
};

struct TwoCounterCase {
    id: &'static str,
    name: &'static str,
    face_id: &'static str,
    mana_cost: &'static str,
    types: &'static [&'static str],
    power_toughness: (u32, u32),
    keywords: &'static [Keyword],
    ability_cost: &'static str,
    power_up: bool,
}

const TWO_COUNTER_CASES: [TwoCounterCase; 5] = [
    TwoCounterCase {
        id: "serpent_specialist",
        name: "Serpent Specialist",
        face_id: "serpent_specialist",
        mana_cost: "{G}",
        types: &["Creature", "Human", "Snake", "Villain"],
        power_toughness: (1, 1),
        keywords: &[Keyword::Deathtouch],
        ability_cost: "{3}{G}",
        power_up: true,
    },
    TwoCounterCase {
        id: "brave_brawler",
        name: "Brave Brawler",
        face_id: "brave_brawler",
        mana_cost: "{1}{W}",
        types: &["Creature", "Human", "Warrior", "Hero"],
        power_toughness: (2, 1),
        keywords: &[Keyword::Lifelink],
        ability_cost: "{4}{W}",
        power_up: true,
    },
    TwoCounterCase {
        id: "volcanic_villain",
        name: "Volcanic Villain",
        face_id: "volcanic_villain",
        mana_cost: "{2}{R}",
        types: &["Creature", "Elemental", "Villain"],
        power_toughness: (3, 2),
        keywords: &[Keyword::Haste],
        ability_cost: "{5}{R}",
        power_up: true,
    },
    TwoCounterCase {
        id: "prowcatcher_specialist",
        name: "Prowcatcher Specialist",
        face_id: "prowcatcher_specialist",
        mana_cost: "{1}{R}",
        types: &["Creature", "Goblin", "Warrior"],
        power_toughness: (2, 1),
        keywords: &[Keyword::Haste],
        ability_cost: "{3}{R}",
        power_up: false,
    },
    TwoCounterCase {
        id: "skystreak_engineer",
        name: "Skystreak Engineer",
        face_id: "skystreak_engineer",
        mana_cost: "{1}{U}",
        types: &["Creature", "Human", "Pilot"],
        power_toughness: (1, 3),
        keywords: &[Keyword::Flying],
        ability_cost: "{4}{U}",
        power_up: false,
    },
];

fn face(id: &str) -> &'static CardFace {
    CardRegistry::global()
        .get(id)
        .unwrap_or_else(|| panic!("missing reviewed card {id}"))
        .primary_face()
}

/// The exact Power-up reduction the handwritten `ultron_drone.ron` anchor carries.
fn power_up_reduction() -> ActivatedCostModifier {
    ActivatedCostModifier::ConditionalSourceManaCostReduction {
        condition: GameCondition::PermanentsEnteredThisTurn {
            controllers: RelativePlayerSet::All,
            filter: PermanentEventFilter {
                source_only: true,
                ..PermanentEventFilter::default()
            },
            min: Some(1),
            max: None,
        },
    }
}

#[test]
fn issue_457_registers_the_five_reviewed_identities() {
    let registry = CardRegistry::global();
    for case in &TWO_COUNTER_CASES {
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
            keywords: case.keywords,
            power_toughness: Some(case.power_toughness),
        }
        .check();
    }
}

#[test]
fn issue_457_each_identity_prints_the_typed_two_counter_ability() {
    for case in &TWO_COUNTER_CASES {
        let face = face(case.id);
        assert!(
            face.spell_effect.is_empty(),
            "{} is a permanent, not a spell",
            case.id
        );
        assert!(
            face.triggered_abilities.is_empty() && face.static_abilities.is_empty(),
            "{} prints only the reviewed activated ability",
            case.id
        );
        let [ability] = face.activated_abilities.as_slice() else {
            panic!("{} must have exactly one activated ability", case.id);
        };
        assert_eq!(ability.ability_id.as_str(), "activated_01", "{}", case.id);
        // The printed keyword line is line 1, so the ability maps to line 2.
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![2]),
            "{}",
            case.id
        );
        assert_eq!(
            ability.source_zone,
            AbilitySourceZone::Battlefield,
            "{}",
            case.id
        );
        assert_eq!(
            ability.costs,
            [AbilityCost::Mana(
                ManaCost::parse(case.ability_cost).expect("reviewed ability cost")
            )],
            "{}",
            case.id
        );
        assert_eq!(
            ability.effect,
            [SpellEffectKind::PutCounters {
                counter: CounterKind::PlusOnePlusOne,
                count: Amount::Fixed(2),
                subject: EffectSubject::Source,
            }],
            "{}",
            case.id
        );
        assert_eq!(
            ability.activation_limit,
            Some(ActivationLimit::PerObject { max_activations: 1 }),
            "{}",
            case.id
        );
        assert!(ability.targeting.is_none(), "{}", case.id);
        assert_eq!(ability.timing, ActivationTiming::Normal, "{}", case.id);
        assert!(ability.conditions.is_empty(), "{}", case.id);
        let expected = if case.power_up {
            vec![power_up_reduction()]
        } else {
            Vec::new()
        };
        assert_eq!(ability.cost_modifiers, expected, "{}", case.id);
    }
}

#[test]
fn issue_457_the_power_up_only_reduction_carries_the_source_entry_condition() {
    for case in &TWO_COUNTER_CASES {
        let ability = &face(case.id).activated_abilities[0];
        let has_reduction = ability.cost_modifiers.iter().any(|modifier| {
            matches!(
                modifier,
                ActivatedCostModifier::ConditionalSourceManaCostReduction {
                    condition: GameCondition::PermanentsEnteredThisTurn {
                        controllers: RelativePlayerSet::All,
                        filter: PermanentEventFilter {
                            source_only: true,
                            ..
                        },
                        min: Some(1),
                        max: None,
                    }
                }
            )
        });
        assert_eq!(has_reduction, case.power_up, "{}", case.id);
    }
}

#[test]
fn issue_457_near_miss_identities_stay_unregistered() {
    let registry = CardRegistry::global();
    for (id, name) in [
        // `{2}{G}{G}` duplicated color symbols and an unrelated graveyard trigger.
        ("afterburner_expert", "Afterburner Expert"),
        // `{5}` generic-only cost and an unrelated combat trigger.
        ("hog_monkey", "Hog-Monkey"),
    ] {
        assert!(
            registry.get(id).is_none(),
            "{id} prints an unreviewed shape and must stay unregistered"
        );
        assert!(
            registry.id_for_name(name).is_none(),
            "{id} must not resolve by name"
        );
    }
}
