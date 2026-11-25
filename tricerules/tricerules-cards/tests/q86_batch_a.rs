//! Registry conformance for the queue-v86 batch A card data.
//!
//! Module 1 covers the six Ally entry-counter creatures, module 2 the five crew Vehicles, and
//! module 3 the five "enters with X +1/+1 counters" creatures. Fingerprints are asserted against
//! the pinned bulk, after the presentation metadata refresh.

use tricerules_cards::primitives::{
    AbilityCost, Amount, CreatureEventFilter, EffectSubject, EntersWithCountersAffected,
    PermanentEventFilter, PermanentTypeFilter, SpellEffectKind, StaticAbilityDef, TargetController,
    TargetKind, TriggerCondition,
};
use tricerules_cards::{
    AbilityPresentation, CounterKind, Keyword, Layout, ObjectContributionKind,
    ObjectPaymentConstraint,
};

// ── 01 ally entry counter ─────────────────────────────────────────────────

struct AllyCase {
    id: &'static str,
    name: &'static str,
    mana: &'static str,
    type_line: &'static [&'static str],
    body: (u32, u32),
    keywords: &'static [Keyword],
    cant_block: bool,
}

fn ally_cases() -> [AllyCase; 6] {
    [
        AllyCase {
            id: "bojuka_brigand",
            name: "Bojuka Brigand",
            mana: "{1}{B}",
            type_line: &["Creature", "Human", "Warrior", "Ally"],
            body: (1, 1),
            keywords: &[],
            cant_block: true,
        },
        AllyCase {
            id: "graypelt_hunter",
            name: "Graypelt Hunter",
            mana: "{3}{G}",
            type_line: &["Creature", "Human", "Warrior", "Ally"],
            body: (2, 2),
            keywords: &[Keyword::Trample],
            cant_block: false,
        },
        AllyCase {
            id: "kazandu_blademaster",
            name: "Kazandu Blademaster",
            mana: "{W}{W}",
            type_line: &["Creature", "Human", "Soldier", "Ally"],
            body: (1, 1),
            keywords: &[Keyword::FirstStrike, Keyword::Vigilance],
            cant_block: false,
        },
        AllyCase {
            id: "makindi_shieldmate",
            name: "Makindi Shieldmate",
            mana: "{2}{W}",
            type_line: &["Creature", "Kor", "Soldier", "Ally"],
            body: (0, 3),
            keywords: &[Keyword::Defender],
            cant_block: false,
        },
        AllyCase {
            id: "tuktuk_grunts",
            name: "Tuktuk Grunts",
            mana: "{4}{R}",
            type_line: &["Creature", "Goblin", "Warrior", "Ally"],
            body: (2, 2),
            keywords: &[Keyword::Haste],
            cant_block: false,
        },
        AllyCase {
            id: "umara_raptor",
            name: "Umara Raptor",
            mana: "{2}{U}",
            type_line: &["Creature", "Bird", "Ally"],
            body: (1, 1),
            keywords: &[Keyword::Flying],
            cant_block: false,
        },
    ]
}

#[test]
fn q86_ally_entry_counter_map_definitions() {
    let registry = tricerules_cards::registry::global();
    for case in ally_cases() {
        let definition = registry
            .get(case.id)
            .unwrap_or_else(|| panic!("missing {}", case.id));
        assert_eq!(definition.layout, Layout::Normal, "{}", case.id);
        assert_eq!(definition.name, case.name, "{}", case.id);
        assert_eq!(
            registry.id_for_name(case.name),
            Some(case.id),
            "{}",
            case.id
        );
        let face = definition.primary_face();
        assert_eq!(face.face_id.as_str(), case.id, "{} face id", case.id);
        assert_eq!(face.mana_cost.to_string(), case.mana, "{} mana", case.id);
        assert_eq!(face.types, case.type_line, "{} types", case.id);
        assert_eq!(face.power, Some(case.body.0), "{} power", case.id);
        assert_eq!(face.toughness, Some(case.body.1), "{} toughness", case.id);
        assert_eq!(face.keywords, case.keywords, "{} keywords", case.id);
        assert!(
            face.activated_abilities.is_empty() && face.spell_effect.is_empty(),
            "{}: no activated ability or spell effect",
            case.id
        );

        if case.cant_block {
            let [static_ability] = face.static_abilities.as_slice() else {
                panic!("{}: exactly one static ability", case.id);
            };
            assert_eq!(
                static_ability.presentation,
                AbilityPresentation::OracleLines(vec![1]),
                "{} block-restriction presentation",
                case.id
            );
            assert!(
                matches!(
                    &static_ability.definition,
                    StaticAbilityDef::SelfCombatRestriction { restriction, .. }
                        if restriction.cant_block
                ),
                "{}: printed 'this creature can't block'",
                case.id
            );
        } else {
            assert!(
                face.static_abilities.is_empty(),
                "{}: no static ability",
                case.id
            );
        }

        let [ability] = face.triggered_abilities.as_slice() else {
            panic!("{}: exactly one triggered ability", case.id);
        };
        assert_eq!(
            ability.trigger,
            TriggerCondition::WheneverPermanentEntersBattlefield {
                controller: tricerules_cards::primitives::CastTriggerPlayer::Controller,
                filter: PermanentEventFilter {
                    permanent_type: Some(PermanentTypeFilter::Creature),
                    ..PermanentEventFilter::default()
                },
                creature_filter: Some(CreatureEventFilter {
                    required_subtypes: vec!["Ally".to_string()],
                    ..CreatureEventFilter::default()
                }),
            },
            "{}",
            case.id
        );
        assert!(ability.may, "{}: printed 'you may'", case.id);
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![2]),
            "{} counter-trigger presentation",
            case.id
        );
        let [effect] = ability.effect.as_slice() else {
            panic!("{}: exactly one effect", case.id);
        };
        assert!(
            matches!(
                effect,
                SpellEffectKind::PutCounters {
                    counter: CounterKind::PlusOnePlusOne,
                    count: Amount::Fixed(1),
                    subject: EffectSubject::Source,
                }
            ),
            "{}: one +1/+1 counter on the source",
            case.id
        );
    }
}

// ── 02 crew vehicles ──────────────────────────────────────────────────────

struct VehicleCase {
    id: &'static str,
    name: &'static str,
    mana: &'static str,
    body: (u32, u32),
    keywords: &'static [Keyword],
    crew: u32,
}

fn vehicle_cases() -> [VehicleCase; 5] {
    [
        VehicleCase {
            id: "aradara_express",
            name: "Aradara Express",
            mana: "{5}",
            body: (8, 6),
            keywords: &[Keyword::Menace],
            crew: 4,
        },
        VehicleCase {
            id: "dragonfly_suit",
            name: "Dragonfly Suit",
            mana: "{2}{W}",
            body: (3, 2),
            keywords: &[Keyword::Flying],
            crew: 1,
        },
        VehicleCase {
            id: "ovalchase_dragster",
            name: "Ovalchase Dragster",
            mana: "{4}",
            body: (6, 1),
            keywords: &[Keyword::Trample, Keyword::Haste],
            crew: 1,
        },
        VehicleCase {
            id: "sky_skiff",
            name: "Sky Skiff",
            mana: "{2}",
            body: (2, 3),
            keywords: &[Keyword::Flying],
            crew: 1,
        },
        VehicleCase {
            id: "thundersteel_colossus",
            name: "Thundersteel Colossus",
            mana: "{7}",
            body: (7, 7),
            keywords: &[Keyword::Trample, Keyword::Haste],
            crew: 2,
        },
    ]
}

#[test]
fn q86_crew_vehicles_map_definitions() {
    let registry = tricerules_cards::registry::global();
    for case in vehicle_cases() {
        let definition = registry
            .get(case.id)
            .unwrap_or_else(|| panic!("missing {}", case.id));
        assert_eq!(definition.layout, Layout::Normal, "{}", case.id);
        assert_eq!(definition.name, case.name, "{}", case.id);
        assert_eq!(
            registry.id_for_name(case.name),
            Some(case.id),
            "{}",
            case.id
        );
        let face = definition.primary_face();
        assert_eq!(face.face_id.as_str(), case.id, "{} face id", case.id);
        assert_eq!(face.mana_cost.to_string(), case.mana, "{} mana", case.id);
        assert_eq!(face.types, ["Artifact", "Vehicle"], "{} types", case.id);
        assert_eq!(face.power, Some(case.body.0), "{} power", case.id);
        assert_eq!(face.toughness, Some(case.body.1), "{} toughness", case.id);
        assert_eq!(face.keywords, case.keywords, "{} keywords", case.id);
        assert!(
            face.triggered_abilities.is_empty()
                && face.static_abilities.is_empty()
                && face.spell_effect.is_empty(),
            "{}: only the printed keywords plus crew",
            case.id
        );

        let [ability] = face.activated_abilities.as_slice() else {
            panic!("{}: exactly one activated ability", case.id);
        };
        assert_eq!(ability.ability_id.as_str(), "crew", "{}", case.id);
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![2]),
            "{} crew presentation",
            case.id
        );
        let [taps] = ability.costs.as_slice() else {
            panic!("{}: crew has one cost", case.id);
        };
        let AbilityCost::TapPermanents {
            constraint,
            filter,
            exclude_source,
        } = taps
        else {
            panic!("{}: crew is a tap-permanents cost", case.id);
        };
        assert_eq!(
            *constraint,
            ObjectPaymentConstraint::AggregateMinimum {
                minimum: case.crew,
                contribution: ObjectContributionKind::CurrentPower,
            },
            "{} printed crew threshold",
            case.id
        );
        assert_eq!(filter.kind, TargetKind::Creature, "{}", case.id);
        assert_eq!(filter.controller, TargetController::You, "{}", case.id);
        assert!(
            *exclude_source,
            "{}: the Vehicle cannot pay its own crew",
            case.id
        );
        assert!(
            matches!(
                ability.effect.as_slice(),
                [SpellEffectKind::AddTypes { subject: EffectSubject::Source, addition }]
                    if addition.card_types == [PermanentTypeFilter::Creature]
                        && addition.creature_types.is_empty()
                        && addition.land_types.is_empty()
            ),
            "{}: crew makes it a creature until end of turn with no added creature type",
            case.id
        );
        assert!(
            ability.targeting.is_none(),
            "{}: crew takes no targets",
            case.id
        );
    }
}

// ── 03 X entry counters ───────────────────────────────────────────────────

struct XCase {
    id: &'static str,
    name: &'static str,
    mana: &'static str,
    type_line: &'static [&'static str],
    keywords: &'static [Keyword],
    line: u16,
}

fn x_cases() -> [XCase; 5] {
    [
        XCase {
            id: "ivy_elemental",
            name: "Ivy Elemental",
            mana: "{X}{G}",
            type_line: &["Creature", "Elemental"],
            keywords: &[],
            line: 1,
        },
        XCase {
            id: "nimbus_swimmer",
            name: "Nimbus Swimmer",
            mana: "{X}{G}{U}",
            type_line: &["Creature", "Leviathan"],
            keywords: &[Keyword::Flying],
            line: 2,
        },
        XCase {
            id: "shifting_wall",
            name: "Shifting Wall",
            mana: "{X}",
            type_line: &["Artifact", "Creature", "Wall"],
            keywords: &[Keyword::Defender],
            line: 2,
        },
        XCase {
            id: "shivan_devastator",
            name: "Shivan Devastator",
            mana: "{X}{R}",
            type_line: &["Creature", "Dragon", "Hydra"],
            keywords: &[Keyword::Flying, Keyword::Haste],
            line: 2,
        },
        XCase {
            id: "worldsoul_colossus",
            name: "Worldsoul Colossus",
            mana: "{X}{G}{W}",
            type_line: &["Creature", "Elemental"],
            keywords: &[Keyword::Convoke],
            line: 2,
        },
    ]
}

#[test]
fn q86_x_entry_counters_map_definitions() {
    let registry = tricerules_cards::registry::global();
    for case in x_cases() {
        let definition = registry
            .get(case.id)
            .unwrap_or_else(|| panic!("missing {}", case.id));
        assert_eq!(definition.layout, Layout::Normal, "{}", case.id);
        assert_eq!(definition.name, case.name, "{}", case.id);
        assert_eq!(
            registry.id_for_name(case.name),
            Some(case.id),
            "{}",
            case.id
        );
        let face = definition.primary_face();
        assert_eq!(face.face_id.as_str(), case.id, "{} face id", case.id);
        assert_eq!(face.mana_cost.to_string(), case.mana, "{} mana", case.id);
        assert_eq!(face.types, case.type_line, "{} types", case.id);
        assert_eq!(face.power, Some(0), "{} printed power", case.id);
        assert_eq!(face.toughness, Some(0), "{} printed toughness", case.id);
        assert_eq!(face.keywords, case.keywords, "{} keywords", case.id);
        assert!(
            face.triggered_abilities.is_empty()
                && face.activated_abilities.is_empty()
                && face.spell_effect.is_empty(),
            "{}: only the printed keywords plus the entry replacement",
            case.id
        );

        let [static_ability] = face.static_abilities.as_slice() else {
            panic!("{}: exactly one static ability", case.id);
        };
        assert_eq!(
            static_ability.presentation,
            AbilityPresentation::OracleLines(vec![case.line]),
            "{} entry-counter presentation",
            case.id
        );
        assert!(
            matches!(
                static_ability.definition,
                StaticAbilityDef::EntersWithCounters {
                    affected: EntersWithCountersAffected::Self_,
                    counter: CounterKind::PlusOnePlusOne,
                    amount: Amount::X,
                    ..
                }
            ),
            "{}: X +1/+1 counters",
            case.id
        );
    }
}

#[test]
fn q86_batch_a_fingerprints_match_the_pinned_oracle_text() {
    // Pinned-bulk normalized fingerprints; Dragonfly Suit and Sky Skiff share a normalized
    // text ("Flying / Crew 1") and therefore the same hash.
    const FINGERPRINTS: &[(&str, &str)] = &[
        (
            "aradara_express",
            "ce43b9e5b29d99e39aaee48daceecb2649a6d54889b17c823216ac3d61c1c21e",
        ),
        (
            "bojuka_brigand",
            "4f7f24e25cf839989045809623b5992b819fcb6260df8f38033de7ffe32e9d0d",
        ),
        (
            "dragonfly_suit",
            "d469e474920b9abeb0345c182fced260512929026a0298a54258909a9efbcaa9",
        ),
        (
            "graypelt_hunter",
            "0db3b75cac35433bdf50271a05ee66b264110c6c84cbb707d6838f2412800595",
        ),
        (
            "ivy_elemental",
            "c4f475662885c5975da1e7650071729e8fc52a37dcdd8505378aa3a08c6a8451",
        ),
        (
            "kazandu_blademaster",
            "1d06ead1e05b75d313c2d9af009317d8936842c7980b9d978e94ee4b29af32b5",
        ),
        (
            "makindi_shieldmate",
            "9b7aa5d05c32e5282f605cbc78d6b63260e509019b771a6ceea88d10b230995d",
        ),
        (
            "nimbus_swimmer",
            "1be538e68254233f4750e315b504551f39733fff52f9f8d769debb1e2e3a3333",
        ),
        (
            "ovalchase_dragster",
            "68d967ef107cb348f413fe8a4d67f2b962e26634ace43c9f9da744283e93d6aa",
        ),
        (
            "shifting_wall",
            "0ceb463fc9fee135fcbe7ee0315f6f822c390648263660ec587b7a9403ac0f00",
        ),
        (
            "shivan_devastator",
            "dc15a89ec8dd745816c10d1cb3c668ee6073d08f8954c9e0467f6f516641214a",
        ),
        (
            "sky_skiff",
            "d469e474920b9abeb0345c182fced260512929026a0298a54258909a9efbcaa9",
        ),
        (
            "thundersteel_colossus",
            "efc516064e9ace5f5e0980ecd75a22d2af5036e44b992e5065514989610d3399",
        ),
        (
            "tuktuk_grunts",
            "65e507b763155e0cc03a03dd2ee12a0b733f0c8a911b895068ae759dd100e287",
        ),
        (
            "umara_raptor",
            "ef0293be896c2ddfb46ec99413bb2b2be01aa24845d20b2b698315d4c1fa3ea9",
        ),
        (
            "worldsoul_colossus",
            "158f22c5f0c2a6912a833181bc8c556d16952d87e2a3ae795cd3f25d2be120d2",
        ),
    ];
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    let mut seen = 0usize;
    for (id, expected) in FINGERPRINTS {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")))
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}"));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[0], *id);
        assert_eq!(fields[4], *expected, "fingerprint drift for {id}");
        seen += 1;
    }
    assert_eq!(seen, 16, "all sixteen new cards are asserted");
}
