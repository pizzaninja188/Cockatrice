//! Registry conformance for the queue-v89 batch A card data.
//!
//! Module 1 covers the three remaining Turbulent dual lands (conditional entry and two printed
//! basic-land-type mana options), module 2 the four once-each-turn self-pumps, and module 3 the
//! three sacrifice-another-creature counter creatures. Fingerprints are asserted against the
//! pinned bulk after the presentation metadata refresh.

use tricerules_cards::primitives::{
    AbilityCost, ActivationLimit, ActivationTiming, Amount, BattlefieldAggregate, EffectSubject,
    EntersTappedAffected, GameCondition, Keyword, PermanentTypeFilter, RelativePlayerSet,
    SpellEffectKind, StaticAbilityDef,
};
use tricerules_cards::{AbilityPresentation, CardDefinition, CardFace, Layout};

fn face(definition: &CardDefinition) -> &CardFace {
    definition.primary_face()
}

fn amounts(amount: &tricerules_cards::ManaAmount) -> [u32; 6] {
    [amount.w, amount.u, amount.b, amount.r, amount.g, amount.c]
}

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

// ── 01 Turbulent dual lands ───────────────────────────────────────────────

struct LandCase {
    id: &'static str,
    name: &'static str,
    types: &'static [&'static str],
    first: char,
    second: char,
    fingerprint: &'static str,
}

const LANDS: &[LandCase] = &[
    LandCase {
        id: "turbulent_crater",
        name: "Turbulent Crater",
        types: &["Land", "Swamp", "Mountain"],
        first: 'b',
        second: 'r',
        fingerprint: "afcc03314d5b28d8c558f8b6a3fa2ae5f1fbe45f52dcf7c9862db75652db2142",
    },
    LandCase {
        id: "turbulent_shore",
        name: "Turbulent Shore",
        types: &["Land", "Plains", "Island"],
        first: 'w',
        second: 'u',
        fingerprint: "0088bfb7d023fc39a8e3f638348213b0794ce8bb87808713fe927ef5b6a1ae67",
    },
    LandCase {
        id: "turbulent_wetlands",
        name: "Turbulent Wetlands",
        types: &["Land", "Island", "Swamp"],
        first: 'u',
        second: 'b',
        fingerprint: "7e26a280e314e050552d81dfddac9bc79429136109cf4a76c81272f66f3599f3",
    },
];

#[test]
fn q89_turbulent_dual_lands_map_definitions() {
    let registry = tricerules_cards::registry::global();
    for case in LANDS {
        let registered = registry
            .get(case.id)
            .unwrap_or_else(|| panic!("missing {}", case.id));
        assert_eq!(registered.layout, Layout::Normal, "{}", case.id);
        assert_eq!(registered.name, case.name, "{}", case.id);
        assert_eq!(
            registry.id_for_name(case.name),
            Some(case.id),
            "{}",
            case.id
        );
        let f = face(registered);
        assert_eq!(f.face_id.as_str(), case.id, "{} face id", case.id);
        assert_eq!(f.mana_cost.to_string(), "", "{} mana cost", case.id);
        assert_eq!(f.types, case.types, "{} types", case.id);
        // Both printed rulings say the land has basic land types but is not basic, so
        // searches for basic lands cannot find it.
        assert!(
            f.supertypes.is_empty(),
            "{}: basic land types do not make it a basic land",
            case.id
        );
        assert_eq!((f.power, f.toughness), (None, None), "{} body", case.id);
        assert!(f.keywords.is_empty(), "{}", case.id);
        assert!(f.triggered_abilities.is_empty(), "{}", case.id);
        assert!(f.spell_effect.is_empty(), "{}", case.id);
        assert!(f.targeting.is_none(), "{}", case.id);

        let [mana] = f.activated_abilities.as_slice() else {
            panic!("{}: exactly one activated ability", case.id);
        };
        assert_eq!(
            mana.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{}",
            case.id
        );
        assert!(mana.intrinsic_land_mana, "{}", case.id);
        assert!(mana.is_mana_ability(), "{}", case.id);
        assert_eq!(mana.costs, [AbilityCost::Tap], "{}", case.id);
        assert!(mana.targeting.is_none(), "{}", case.id);
        assert_eq!(
            mana.timing,
            ActivationTiming::Normal,
            "{}: no printed timing restriction",
            case.id
        );
        assert!(
            mana.conditions.is_empty() && mana.activation_limit.is_none(),
            "{}: no printed condition or activation limit",
            case.id
        );
        let [SpellEffectKind::ProduceMana { options, .. }] = mana.effect.as_slice() else {
            panic!("{}: one mana production", case.id);
        };
        assert_eq!(
            options.len(),
            2,
            "{}: one option per basic land type",
            case.id
        );
        assert_eq!(
            amounts(&options[0]),
            mask(case.first),
            "{} first color",
            case.id
        );
        assert_eq!(
            amounts(&options[1]),
            mask(case.second),
            "{} second color",
            case.id
        );

        let [static_ability] = f.static_abilities.as_slice() else {
            panic!("{}: exactly one static ability", case.id);
        };
        assert_eq!(
            static_ability.presentation,
            AbilityPresentation::OracleLines(vec![2]),
            "{}",
            case.id
        );
        assert_eq!(
            static_ability.ability_id.as_str(),
            "enters_tapped",
            "{} static id",
            case.id
        );
        // Exhaustive: the printed eight-or-more condition is the only difference from an
        // unconditional entry replacement, and there is no entry-cost escape.
        let StaticAbilityDef::EntersTapped {
            affected,
            condition,
            unless_cost,
        } = &static_ability.definition
        else {
            panic!("{}: conditional enters tapped", case.id);
        };
        assert_eq!(*affected, EntersTappedAffected::Self_, "{}", case.id);
        assert_eq!(*unless_cost, None, "{}: no entry cost", case.id);
        let Some(GameCondition::BattlefieldAggregate {
            filter,
            aggregate,
            max,
            min,
            ..
        }) = condition
        else {
            panic!("{}: battlefield aggregate condition", case.id);
        };
        assert_eq!(*aggregate, BattlefieldAggregate::Count, "{}", case.id);
        assert_eq!(*max, Some(7), "{}: eight or more opponents' lands", case.id);
        assert_eq!(*min, None, "{}", case.id);
        assert_eq!(
            filter.controllers,
            RelativePlayerSet::Opponents,
            "{}",
            case.id
        );
        assert_eq!(
            filter.matching.permanent_types,
            [PermanentTypeFilter::Land],
            "{}",
            case.id
        );
    }
}

// ── 02 once-each-turn self-pumps ──────────────────────────────────────────

struct PumpCase {
    id: &'static str,
    /// Canonical snake_case face id. The card id keeps printed punctuation (a hyphen), so the
    /// two differ exactly the way `kavaron,_memorial_world` differs from its face id.
    face_id: &'static str,
    name: &'static str,
    mana: &'static str,
    types: &'static [&'static str],
    body: (u32, u32),
    keywords: &'static [Keyword],
    activation: &'static str,
    delta: (i32, i32),
    line: u16,
    madness_cost: Option<&'static str>,
    fingerprint: &'static str,
}

const PUMPS: &[PumpCase] = &[
    PumpCase {
        id: "basking_rootwalla",
        face_id: "basking_rootwalla",
        name: "Basking Rootwalla",
        mana: "{G}",
        types: &["Creature", "Lizard"],
        body: (1, 1),
        keywords: &[],
        activation: "{1}{G}",
        delta: (2, 2),
        line: 1,
        madness_cost: Some("{0}"),
        fingerprint: "0f37cae90cd1131d4e1ea39481cf12e2e9068ed76ae26b66b2a17cb9083e7859",
    },
    PumpCase {
        // Engine slugify keeps the printed hyphen, so the card id keeps it too while the
        // face id (and the review map span) use canonical snake_case.
        id: "ghor-clan_bloodscale",
        face_id: "ghor_clan_bloodscale",
        name: "Ghor-Clan Bloodscale",
        mana: "{3}{R}",
        types: &["Creature", "Lizard", "Warrior"],
        body: (2, 1),
        keywords: &[Keyword::FirstStrike],
        activation: "{3}{G}",
        delta: (2, 2),
        line: 2,
        madness_cost: None,
        fingerprint: "294c113160ee136cac69d7dc5e6fff43a6c297fd212657ef5554114668f43613",
    },
    PumpCase {
        id: "twinblade_slasher",
        face_id: "twinblade_slasher",
        name: "Twinblade Slasher",
        mana: "{G}",
        types: &["Creature", "Elf", "Warrior"],
        body: (1, 1),
        keywords: &[Keyword::Wither],
        activation: "{1}{G}",
        delta: (2, 2),
        line: 2,
        madness_cost: None,
        fingerprint: "818d91745708a0088f4a15fc393875b4c83ad10e43d1bc0290560acbe25b0b15",
    },
    PumpCase {
        id: "viashino_slaughtermaster",
        face_id: "viashino_slaughtermaster",
        name: "Viashino Slaughtermaster",
        mana: "{1}{R}",
        types: &["Creature", "Lizard", "Warrior"],
        body: (1, 1),
        keywords: &[Keyword::DoubleStrike],
        activation: "{B}{G}",
        delta: (1, 1),
        line: 2,
        madness_cost: None,
        fingerprint: "0d7e6316da6147d41bbe7145679c3a6436a45de0513734331453f7ca42f1b121",
    },
];

#[test]
fn q89_global_once_pump_cards_map_definitions() {
    let registry = tricerules_cards::registry::global();
    for case in PUMPS {
        let registered = registry
            .get(case.id)
            .unwrap_or_else(|| panic!("missing {}", case.id));
        assert_eq!(registered.layout, Layout::Normal, "{}", case.id);
        assert_eq!(registered.name, case.name, "{}", case.id);
        assert_eq!(
            registry.id_for_name(case.name),
            Some(case.id),
            "{}",
            case.id
        );
        let f = face(registered);
        assert_eq!(f.face_id.as_str(), case.face_id, "{} face id", case.id);
        assert_eq!(f.mana_cost.to_string(), case.mana, "{} mana", case.id);
        assert_eq!(f.types, case.types, "{} types", case.id);
        assert_eq!(f.power, Some(case.body.0), "{} power", case.id);
        assert_eq!(f.toughness, Some(case.body.1), "{} toughness", case.id);
        assert_eq!(f.keywords, case.keywords, "{} keywords", case.id);
        assert!(
            f.triggered_abilities.is_empty() && f.spell_effect.is_empty(),
            "{}: only the activated ability and static rider remain",
            case.id
        );

        let [ability] = f.activated_abilities.as_slice() else {
            panic!("{}: exactly one activated ability", case.id);
        };
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![case.line]),
            "{}",
            case.id
        );
        assert_eq!(
            ability.costs,
            [AbilityCost::Mana(
                tricerules_cards::ManaCost::parse(case.activation)
                    .expect("printed activation cost")
            )],
            "{}: exactly the printed mana cost and no {{T}}",
            case.id
        );
        assert_eq!(
            ability.timing,
            ActivationTiming::Normal,
            "{}: no printed timing restriction",
            case.id
        );
        assert!(
            ability.conditions.is_empty(),
            "{}: no printed activation condition",
            case.id
        );
        // The printed "Activate only once each turn" resets with the turn, so a controller
        // change must not hand out a fresh activation.
        assert_eq!(
            ability.activation_limit,
            Some(ActivationLimit::PerTurn { max_activations: 1 }),
            "{}: once each turn",
            case.id
        );
        assert!(ability.targeting.is_none(), "{}: no target", case.id);
        assert_eq!(
            ability.effect,
            [SpellEffectKind::PumpTarget {
                power: case.delta.0,
                toughness: case.delta.1,
                scale: None,
                subject: EffectSubject::Source,
            }],
            "{}: printed self-pump",
            case.id
        );

        match case.madness_cost {
            Some(cost) => {
                let [static_ability] = f.static_abilities.as_slice() else {
                    panic!("{}: exactly one static ability", case.id);
                };
                assert_eq!(
                    static_ability.presentation,
                    AbilityPresentation::OracleLines(vec![2]),
                    "{} madness presentation",
                    case.id
                );
                assert_eq!(
                    static_ability.definition,
                    StaticAbilityDef::Madness {
                        cost: tricerules_cards::ManaCost::parse(cost)
                            .expect("printed madness cost")
                    },
                    "{}",
                    case.id
                );
            }
            None => assert!(
                f.static_abilities.is_empty(),
                "{}: no static ability",
                case.id
            ),
        }
    }
}

// ── 03 sacrifice another creature for a counter ───────────────────────────

struct CounterCase {
    id: &'static str,
    name: &'static str,
    mana: &'static str,
    types: &'static [&'static str],
    body: (u32, u32),
    keywords: &'static [Keyword],
    activation: &'static str,
    line: u16,
    fingerprint: &'static str,
}

const COUNTERS: &[CounterCase] = &[
    CounterCase {
        id: "gobbling_ooze",
        name: "Gobbling Ooze",
        mana: "{4}{G}",
        types: &["Creature", "Ooze"],
        body: (3, 3),
        keywords: &[],
        activation: "{G}",
        line: 1,
        fingerprint: "f97a6ba08494209f24998665117d62a693e98d7bcf44068782955628dd674063",
    },
    CounterCase {
        id: "hobblefiend",
        name: "Hobblefiend",
        mana: "{1}{R}",
        types: &["Creature", "Devil"],
        body: (2, 1),
        keywords: &[Keyword::Trample],
        activation: "{1}",
        line: 2,
        fingerprint: "da64a34c8924a6aff778bd495557b99d9d4e60f8faacc49b2c370f8d5fea31b0",
    },
    CounterCase {
        id: "ravenous_harpy",
        name: "Ravenous Harpy",
        mana: "{2}{B}",
        types: &["Creature", "Harpy"],
        body: (1, 2),
        keywords: &[Keyword::Flying],
        activation: "{1}",
        line: 2,
        fingerprint: "c6414ec3bfccd8fdb5613a2d4ccabd2ede7d73f660078d264ba347954f5360ae",
    },
];

#[test]
fn q89_sac_creature_counter_cards_map_definitions() {
    let registry = tricerules_cards::registry::global();
    for case in COUNTERS {
        let registered = registry
            .get(case.id)
            .unwrap_or_else(|| panic!("missing {}", case.id));
        assert_eq!(registered.layout, Layout::Normal, "{}", case.id);
        assert_eq!(registered.name, case.name, "{}", case.id);
        assert_eq!(
            registry.id_for_name(case.name),
            Some(case.id),
            "{}",
            case.id
        );
        let f = face(registered);
        assert_eq!(f.face_id.as_str(), case.id, "{} face id", case.id);
        assert_eq!(f.mana_cost.to_string(), case.mana, "{} mana", case.id);
        assert_eq!(f.types, case.types, "{} types", case.id);
        assert_eq!(f.power, Some(case.body.0), "{} power", case.id);
        assert_eq!(f.toughness, Some(case.body.1), "{} toughness", case.id);
        assert_eq!(f.keywords, case.keywords, "{} keywords", case.id);
        assert!(
            f.triggered_abilities.is_empty()
                && f.static_abilities.is_empty()
                && f.spell_effect.is_empty(),
            "{}: only the activated ability remains",
            case.id
        );

        let [ability] = f.activated_abilities.as_slice() else {
            panic!("{}: exactly one activated ability", case.id);
        };
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![case.line]),
            "{}",
            case.id
        );
        let [AbilityCost::Mana(mana), AbilityCost::SacrificePermanent { filter, count }] =
            ability.costs.as_slice()
        else {
            panic!(
                "{}: printed mana plus sacrifice cost, got {:?}",
                case.id, ability.costs
            );
        };
        assert_eq!(
            mana.to_string(),
            case.activation,
            "{}: printed activation mana",
            case.id
        );
        assert_eq!(*count, 1, "{}: exactly one sacrificed creature", case.id);
        assert_eq!(
            filter.kind,
            tricerules_cards::primitives::TargetKind::Creature,
            "{}: a creature is sacrificed",
            case.id
        );
        assert_eq!(
            filter.controller,
            tricerules_cards::primitives::TargetController::You,
            "{}: only the activator's creature",
            case.id
        );
        assert_eq!(
            filter.excluded_objects,
            [tricerules_cards::TargetObjectExclusion::Source],
            "{}: the excluded object is exactly the source",
            case.id
        );
        assert_eq!(
            ability.timing,
            ActivationTiming::Normal,
            "{}: no printed timing restriction",
            case.id
        );
        assert!(
            ability.conditions.is_empty() && ability.activation_limit.is_none(),
            "{}: no printed condition or activation limit",
            case.id
        );
        assert!(ability.targeting.is_none(), "{}: no target", case.id);
        assert_eq!(
            ability.effect,
            [SpellEffectKind::PutCounters {
                counter: tricerules_cards::CounterKind::PlusOnePlusOne,
                count: Amount::Fixed(1),
                subject: EffectSubject::Source,
            }],
            "{}: one +1/+1 counter on the source",
            case.id
        );
    }
}

// ── fingerprints ──────────────────────────────────────────────────────────

#[test]
fn q89_batch_a_fingerprints_match_the_pinned_oracle_text() {
    let registry = tricerules_cards::registry::global();
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    let mut seen = 0usize;
    let mut assert_one = |id: &str, expected: &str| {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        let face_id = definition.primary_face().face_id.as_str();
        // Columns: card id, printed name, face id, printed face name, digest.
        let row = fingerprints
            .lines()
            .find(|line| {
                let mut fields = line.split('\t');
                fields.next() == Some(id) && fields.nth(1) == Some(face_id)
            })
            .unwrap_or_else(|| panic!("missing fingerprint row for {id} / face {face_id}"));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[4], expected, "{id} fingerprint");
        seen += 1;
    };
    for case in LANDS {
        assert_one(case.id, case.fingerprint);
    }
    for case in PUMPS {
        assert_one(case.id, case.fingerprint);
    }
    for case in COUNTERS {
        assert_one(case.id, case.fingerprint);
    }
    assert_eq!(seen, 10, "all ten queue-v89 cards were checked");
}
