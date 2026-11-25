//! Registry conformance for the queue-v85 batch A card data.
//!
//! Module 1 covers the four tap prevention-shield clerics, module 2 the three Flowstone
//! `{R}: +1/-1` self-pump creatures, and module 3 the three `{B}: gains deathtouch` creatures.
//! Fingerprints are asserted against the pinned bulk, after the presentation metadata refresh.

use tricerules_cards::primitives::{AbilityCost, SpellEffectKind, TargetKind};
use tricerules_cards::{AbilityPresentation, CharacteristicDefiningAbility, Keyword, Layout};

// ── 01 prevention shield clerics ──────────────────────────────────────────

struct ShieldCase {
    id: &'static str,
    name: &'static str,
    mana: &'static str,
    type_line: &'static [&'static str],
    body: (u32, u32),
    keyword: Keyword,
    amount: u32,
}

fn shield_cases() -> [ShieldCase; 4] {
    [
        ShieldCase {
            id: "militant_monk",
            name: "Militant Monk",
            mana: "{1}{W}{W}",
            type_line: &["Creature", "Human", "Monk", "Cleric"],
            body: (2, 1),
            keyword: Keyword::Vigilance,
            amount: 1,
        },
        ShieldCase {
            id: "benevolent_ancestor",
            name: "Benevolent Ancestor",
            mana: "{2}{W}",
            type_line: &["Creature", "Spirit"],
            body: (0, 4),
            keyword: Keyword::Defender,
            amount: 1,
        },
        ShieldCase {
            id: "alabaster_wall",
            name: "Alabaster Wall",
            mana: "{2}{W}",
            type_line: &["Creature", "Wall"],
            body: (0, 4),
            keyword: Keyword::Defender,
            amount: 1,
        },
        ShieldCase {
            id: "aven_redeemer",
            name: "Aven Redeemer",
            mana: "{3}{W}",
            type_line: &["Creature", "Bird", "Cleric"],
            body: (2, 2),
            keyword: Keyword::Flying,
            amount: 2,
        },
    ]
}

#[test]
fn q85_prevention_shield_clerics_map_definitions() {
    let registry = tricerules_cards::registry::global();
    for case in shield_cases() {
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
        assert_eq!(
            face.keywords,
            vec![case.keyword],
            "{} printed keyword",
            case.id
        );
        assert!(
            face.triggered_abilities.is_empty() && face.static_abilities.is_empty(),
            "{}: the card is one keyword plus one activated ability",
            case.id
        );

        let [ability] = face.activated_abilities.as_slice() else {
            panic!("{}: exactly one activated ability", case.id);
        };
        assert_eq!(ability.ability_id.as_str(), "activated_01", "{}", case.id);
        assert_eq!(ability.costs, [AbilityCost::Tap], "{} tap cost", case.id);
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![2]),
            "{} presentation",
            case.id
        );
        assert!(matches!(
            ability.effect.as_slice(),
            [SpellEffectKind::PreventNextDamage {
                amount,
                target,
            }] if *amount == case.amount && target.kind == TargetKind::AnyTarget
        ));
    }
}

// ── 02 Flowstone self-pump ────────────────────────────────────────────────

struct PumpCase {
    id: &'static str,
    name: &'static str,
    mana: &'static str,
    type_line: &'static [&'static str],
    body: (u32, u32),
    keyword: Keyword,
}

fn pump_cases() -> [PumpCase; 3] {
    [
        PumpCase {
            id: "flowstone_kavu",
            name: "Flowstone Kavu",
            mana: "{2}{R}",
            type_line: &["Creature", "Kavu"],
            body: (2, 3),
            keyword: Keyword::Menace,
        },
        PumpCase {
            id: "flowstone_wall",
            name: "Flowstone Wall",
            mana: "{2}{R}",
            type_line: &["Creature", "Wall"],
            body: (0, 6),
            keyword: Keyword::Defender,
        },
        PumpCase {
            id: "flowstone_mauler",
            name: "Flowstone Mauler",
            mana: "{4}{R}{R}",
            type_line: &["Creature", "Beast"],
            body: (4, 5),
            keyword: Keyword::Trample,
        },
    ]
}

#[test]
fn q85_flowstone_self_pump_map_definitions() {
    let registry = tricerules_cards::registry::global();
    for case in pump_cases() {
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
        assert_eq!(
            face.keywords,
            vec![case.keyword],
            "{} printed keyword",
            case.id
        );
        assert!(
            face.triggered_abilities.is_empty() && face.static_abilities.is_empty(),
            "{}: the card is one keyword plus one activated ability",
            case.id
        );

        let [ability] = face.activated_abilities.as_slice() else {
            panic!("{}: exactly one activated ability", case.id);
        };
        assert_eq!(ability.ability_id.as_str(), "activated_01", "{}", case.id);
        assert!(
            matches!(ability.costs.as_slice(), [AbilityCost::Mana(cost)] if cost.to_string() == "{R}"),
            "{} printed {{R}} cost",
            case.id
        );
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![2]),
            "{} presentation",
            case.id
        );
        assert!(
            ability.targeting.is_none(),
            "{}: the pump affects only the source",
            case.id
        );
        assert!(matches!(
            ability.effect.as_slice(),
            [SpellEffectKind::PumpTarget {
                power: 1,
                toughness: -1,
                ..
            }]
        ));
    }
}

// ── 03 deathtouch grants ──────────────────────────────────────────────────

struct DeathtouchCase {
    id: &'static str,
    name: &'static str,
    mana: &'static str,
    type_line: &'static [&'static str],
    body: (u32, u32),
    keyword: Option<Keyword>,
    changeling: bool,
}

fn deathtouch_cases() -> [DeathtouchCase; 3] {
    [
        DeathtouchCase {
            id: "battlefly_swarm",
            name: "Battlefly Swarm",
            mana: "{B}",
            type_line: &["Creature", "Phyrexian", "Insect"],
            body: (1, 1),
            keyword: Some(Keyword::Flying),
            changeling: false,
        },
        DeathtouchCase {
            id: "fetid_imp",
            name: "Fetid Imp",
            mana: "{1}{B}",
            type_line: &["Creature", "Imp"],
            body: (1, 2),
            keyword: Some(Keyword::Flying),
            changeling: false,
        },
        DeathtouchCase {
            id: "moonglove_changeling",
            name: "Moonglove Changeling",
            mana: "{2}{B}",
            type_line: &["Creature", "Shapeshifter"],
            body: (2, 2),
            keyword: None,
            changeling: true,
        },
    ]
}

#[test]
fn q85_deathtouch_grant_map_definitions() {
    let registry = tricerules_cards::registry::global();
    for case in deathtouch_cases() {
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
        assert_eq!(
            face.keywords,
            case.keyword.into_iter().collect::<Vec<_>>(),
            "{} printed keyword",
            case.id
        );
        assert!(
            face.triggered_abilities.is_empty() && face.static_abilities.is_empty(),
            "{}: the card is one grant plus its printed keyword or CDA",
            case.id
        );
        assert!(
            !face.keywords.contains(&Keyword::Deathtouch),
            "{}: deathtouch is granted, not printed",
            case.id
        );

        if case.changeling {
            let [cda] = face.characteristic_defining_abilities.as_slice() else {
                panic!("{}: exactly one characteristic-defining ability", case.id);
            };
            assert!(matches!(
                cda.definition,
                CharacteristicDefiningAbility::Changeling
            ));
            assert_eq!(
                cda.presentation,
                AbilityPresentation::OracleLines(vec![1]),
                "{} changeling presentation",
                case.id
            );
            assert_eq!(
                face.activated_abilities[0].presentation,
                AbilityPresentation::OracleLines(vec![2]),
                "{} grant presentation",
                case.id
            );
        } else {
            assert!(
                face.characteristic_defining_abilities.is_empty(),
                "{}: no CDA",
                case.id
            );
            assert_eq!(
                face.activated_abilities[0].presentation,
                AbilityPresentation::OracleLines(vec![2]),
                "{} grant presentation",
                case.id
            );
        }

        let [ability] = face.activated_abilities.as_slice() else {
            panic!("{}: exactly one activated ability", case.id);
        };
        assert!(
            matches!(ability.costs.as_slice(), [AbilityCost::Mana(cost)] if cost.to_string() == "{B}"),
            "{} printed {{B}} cost",
            case.id
        );
        assert!(ability.targeting.is_none());
        assert!(matches!(
            ability.effect.as_slice(),
            [SpellEffectKind::GrantKeywords {
                keywords,
                ..
            }] if keywords.as_slice() == [Keyword::Deathtouch]
        ));
    }
}

#[test]
fn q85_batch_a_fingerprints_match_the_pinned_oracle_text() {
    const FINGERPRINTS: [(&str, &str); 10] = [
        (
            "militant_monk",
            "00ecfaf0a92ce64326157aa579539232935de288cf21508e7aea0be10a19084d",
        ),
        (
            "benevolent_ancestor",
            "a4fad1cf9236219c5282d03b6d5d8c642ffaf27a1ef395127ef157680bdbd8d6",
        ),
        (
            "alabaster_wall",
            "a4fad1cf9236219c5282d03b6d5d8c642ffaf27a1ef395127ef157680bdbd8d6",
        ),
        (
            "aven_redeemer",
            "28bc2fb007302fc8dd75e521fc173b36a082a2123b6e07be795564a45fc539a0",
        ),
        (
            "flowstone_kavu",
            "485b5427fcb6d877282b215c88d948643bfd52fe9c3b997828050080690bec9a",
        ),
        (
            "flowstone_wall",
            "268d10cdd3f442b5dc0491db18dad01df5d6b5b75ee77f4a0acaebe792227e6c",
        ),
        (
            "flowstone_mauler",
            "0c7bea7428c4c6ff3d21b0adcd4b24b3c238026d80d291f114d25b7d4d169163",
        ),
        (
            "battlefly_swarm",
            "616d8d14764f9cedaa003502845ff5504a7082886288a88898aa2baf74624c56",
        ),
        (
            "fetid_imp",
            "f159c854241e5bfa2c7a434d2058f565feeb6733b3a3b5dd5f7557b9933889c5",
        ),
        (
            "moonglove_changeling",
            "df0af4ca12535770627aceca6395e11ce4a86facff5b4d49415a602c71e68cd3",
        ),
    ];
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, expected) in FINGERPRINTS {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")))
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}"));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[0], id);
        assert_eq!(fields[4], expected, "fingerprint drift for {id}");
    }
}
