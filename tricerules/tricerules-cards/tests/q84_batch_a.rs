//! Registry conformance for the queue-v84 batch A card data.
//!
//! Module 1 covers the four enters-scry riders, module 2 the four dies-draw riders, and module 3
//! the three enters-gain-life riders. Fingerprints are checked after the presentation metadata
//! refresh, so their values are asserted only through the shared fingerprint rows.

use tricerules_cards::primitives::{
    AbilityCost, Amount, CardTypeFilter, CastTriggerPlayer, EffectSubject, GameCondition,
    RelativePlayerSet, SpellCastFilter, SpellEffectKind, StaticAbilityDef, TargetingSourceFilter,
    TriggerCondition,
};
use tricerules_cards::{AbilityPresentation, CardFace, Keyword, Layout};

// ── 01 etb scry riders ─────────────────────────────────────────────────────

struct ScryCase {
    id: &'static str,
    name: &'static str,
    mana: &'static str,
    type_line: &'static [&'static str],
    body: (u32, u32),
    keyword: Option<Keyword>,
    scry: u32,
}

fn scry_cases() -> [ScryCase; 4] {
    [
        ScryCase {
            id: "grey_havens_navigator",
            name: "Grey Havens Navigator",
            mana: "{2}{U}",
            type_line: &["Creature", "Elf", "Pilot"],
            body: (3, 2),
            keyword: Some(Keyword::Flash),
            scry: 1,
        },
        ScryCase {
            id: "faithful_pikemaster",
            name: "Faithful Pikemaster",
            mana: "{3}{W}",
            type_line: &["Creature", "Rhino", "Monk", "Soldier"],
            body: (3, 4),
            keyword: None,
            scry: 2,
        },
        ScryCase {
            id: "myr_custodian",
            name: "Myr Custodian",
            mana: "{3}",
            type_line: &["Artifact", "Creature", "Myr"],
            body: (2, 3),
            keyword: None,
            scry: 2,
        },
        ScryCase {
            id: "inquisitive_puppet",
            name: "Inquisitive Puppet",
            mana: "{1}",
            type_line: &["Artifact", "Creature", "Construct"],
            body: (0, 2),
            keyword: None,
            scry: 1,
        },
    ]
}

#[test]
fn q84_etb_scry_riders_map_definitions() {
    let registry = tricerules_cards::registry::global();
    for case in scry_cases() {
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
        assert_eq!(face.keywords, case.keyword.into_iter().collect::<Vec<_>>());

        let [trigger] = face.triggered_abilities.as_slice() else {
            panic!("{}: one enters trigger", case.id);
        };
        assert_eq!(trigger.ability_id.as_str(), "triggered_01", "{}", case.id);
        assert!(matches!(
            trigger.trigger,
            TriggerCondition::WhenSelfEntersBattlefield
        ));
        let trigger_line = if case.id == "grey_havens_navigator" {
            2
        } else {
            1
        };
        assert_eq!(
            trigger.presentation,
            AbilityPresentation::OracleLines(vec![trigger_line]),
            "{}",
            case.id
        );

        match case.id {
            "myr_custodian" => {
                assert!(matches!(
                    trigger.effect.as_slice(),
                    [
                        SpellEffectKind::Scry {
                            count: Amount::Fixed(2)
                        },
                        SpellEffectKind::EachOpponentMayScry { count: 1 }
                    ]
                ));
                assert!(trigger.targeting.is_none());
                assert!(face.activated_abilities.is_empty());
            }
            "inquisitive_puppet" => {
                assert!(matches!(
                    trigger.effect.as_slice(),
                    [SpellEffectKind::Scry {
                        count: Amount::Fixed(1)
                    }]
                ));
                let [ability] = face.activated_abilities.as_slice() else {
                    panic!("{case_id}: one activated ability", case_id = case.id);
                };
                assert_eq!(ability.costs, [AbilityCost::ExileSelf]);
                assert!(matches!(
                    ability.effect.as_slice(),
                    [SpellEffectKind::CreateTokens {
                        token,
                        count: Amount::Fixed(1),
                        ..
                    }] if token == "human_w_1_1"
                ));
            }
            _ => {
                assert!(matches!(
                    trigger.effect.as_slice(),
                    [SpellEffectKind::Scry { count: Amount::Fixed(value) }]
                        if *value == case.scry
                ));
                assert!(trigger.targeting.is_none());
                assert!(face.activated_abilities.is_empty());
            }
        }

        if case.id == "faithful_pikemaster" {
            let [static_ability] = face.static_abilities.as_slice() else {
                panic!("Faithful Pikemaster: one static ability");
            };
            assert!(matches!(
                &static_ability.definition,
                StaticAbilityDef::ConditionalSelfModifier {
                    condition: GameCondition::ActivePlayer {
                        players: RelativePlayerSet::Controller
                    },
                    keywords,
                    ..
                } if keywords == &[Keyword::FirstStrike]
            ));
            assert_eq!(
                static_ability.presentation,
                AbilityPresentation::OracleLines(vec![2])
            );
        } else {
            assert!(face.static_abilities.is_empty(), "{}", case.id);
        }
    }
}

// ── 02 dies draw riders ────────────────────────────────────────────────────

struct DiesCase {
    id: &'static str,
    name: &'static str,
    mana: &'static str,
    types: &'static [&'static str],
    body: (u32, u32),
}

fn dies_cases() -> [DiesCase; 4] {
    [
        DiesCase {
            id: "jeskai_sage",
            name: "Jeskai Sage",
            mana: "{1}{U}",
            types: &["Creature", "Human", "Monk"],
            body: (1, 1),
        },
        DiesCase {
            id: "filigree_familiar",
            name: "Filigree Familiar",
            mana: "{3}",
            types: &["Artifact", "Creature", "Fox"],
            body: (2, 2),
        },
        DiesCase {
            id: "living_lies_of_loki",
            name: "Living Lies of Loki",
            mana: "{1}{U}",
            types: &["Creature", "Illusion", "Villain"],
            body: (1, 3),
        },
        DiesCase {
            id: "phantasmal_shieldback",
            name: "Phantasmal Shieldback",
            mana: "{U}",
            types: &["Creature", "Turtle", "Illusion"],
            body: (1, 3),
        },
    ]
}

fn dies_draw_trigger(face: &CardFace) -> &tricerules_cards::TriggeredAbilityDef {
    face.triggered_abilities
        .iter()
        .find(|ability| matches!(ability.trigger, TriggerCondition::WhenSelfDies))
        .unwrap_or_else(|| panic!("{}: dies draw trigger", face.face_id))
}

#[test]
fn q84_dies_draw_riders_map_definitions() {
    let registry = tricerules_cards::registry::global();
    for case in dies_cases() {
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
        assert_eq!(face.types, case.types, "{} types", case.id);
        assert_eq!(face.power, Some(case.body.0), "{} power", case.id);
        assert_eq!(face.toughness, Some(case.body.1), "{} toughness", case.id);
        let draw = dies_draw_trigger(face);
        assert!(matches!(
            draw.effect.as_slice(),
            [SpellEffectKind::Draw {
                count: Amount::Fixed(1),
                ..
            }]
        ));
        assert!(draw.targeting.is_none());

        match case.id {
            "jeskai_sage" => {
                let prowess_abilities = face
                    .triggered_abilities
                    .iter()
                    .filter(|ability| {
                        matches!(
                            ability.trigger,
                            TriggerCondition::WheneverPlayerCastsSpell { .. }
                        )
                    })
                    .collect::<Vec<_>>();
                let [prowess] = prowess_abilities.as_slice() else {
                    panic!("Jeskai Sage: one Prowess trigger");
                };
                assert!(matches!(
                    prowess.trigger,
                    TriggerCondition::WheneverPlayerCastsSpell {
                        caster: CastTriggerPlayer::Controller,
                        filter: SpellCastFilter {
                            card_type: Some(CardTypeFilter::Noncreature),
                            ..
                        },
                        ..
                    }
                ));
                assert!(matches!(
                    prowess.effect.as_slice(),
                    [SpellEffectKind::PumpTarget {
                        power: 1,
                        toughness: 1,
                        subject: EffectSubject::Source,
                        ..
                    }]
                ));
            }
            "filigree_familiar" => {
                let entry = face
                    .triggered_abilities
                    .iter()
                    .find(|ability| {
                        matches!(ability.trigger, TriggerCondition::WhenSelfEntersBattlefield)
                    })
                    .expect("Filigree Familiar: entry trigger");
                assert!(matches!(
                    entry.effect.as_slice(),
                    [SpellEffectKind::GainLife {
                        amount: Amount::Fixed(2),
                        ..
                    }]
                ));
            }
            "living_lies_of_loki" => {
                let [static_ability] = face.static_abilities.as_slice() else {
                    panic!("Living Lies of Loki: one static ability");
                };
                assert!(matches!(
                    static_ability.definition,
                    StaticAbilityDef::CountScaledSelfPt {
                        power_per_match: 1,
                        toughness_per_match: 0,
                        ..
                    }
                ));
            }
            "phantasmal_shieldback" => {
                let sacrifice = face
                    .triggered_abilities
                    .iter()
                    .find(|ability| {
                        matches!(
                            ability.trigger,
                            TriggerCondition::WheneverSelfBecomesTarget { .. }
                        )
                    })
                    .expect("Phantasmal Shieldback: becomes-target trigger");
                assert!(matches!(
                    sacrifice.effect.as_slice(),
                    [SpellEffectKind::Sacrifice {
                        subject: EffectSubject::Source
                    }]
                ));
            }
            _ => unreachable!(),
        }
    }
}

#[test]
fn q84_phantasmal_shieldback_target_trigger_scope() {
    let registry = tricerules_cards::registry::global();
    let face = registry
        .get("phantasmal_shieldback")
        .unwrap()
        .primary_face();
    let sacrifice = face
        .triggered_abilities
        .iter()
        .find(|ability| {
            matches!(
                ability.trigger,
                TriggerCondition::WheneverSelfBecomesTarget { .. }
            )
        })
        .unwrap();
    assert_eq!(
        sacrifice.trigger,
        TriggerCondition::WheneverSelfBecomesTarget {
            source: TargetingSourceFilter::SpellOrAbility,
            source_controller: tricerules_cards::primitives::CastTriggerPlayer::AnyPlayer,
        }
    );
}

// ── 03 etb gain life riders ────────────────────────────────────────────────

struct LifeCase {
    id: &'static str,
    name: &'static str,
    mana: &'static str,
    types: &'static [&'static str],
    body: (u32, u32),
    life: u32,
    keyword: Option<Keyword>,
}

fn life_cases() -> [LifeCase; 3] {
    [
        LifeCase {
            id: "elite_guardmage",
            name: "Elite Guardmage",
            mana: "{2}{W}{U}",
            types: &["Creature", "Human", "Wizard"],
            body: (2, 3),
            life: 3,
            keyword: Some(Keyword::Flying),
        },
        LifeCase {
            id: "loxodon_restorer",
            name: "Loxodon Restorer",
            mana: "{4}{W}{W}",
            types: &["Creature", "Elephant", "Cleric"],
            body: (3, 4),
            life: 4,
            keyword: Some(Keyword::Convoke),
        },
        LifeCase {
            id: "circuit_mender",
            name: "Circuit Mender",
            mana: "{3}",
            types: &["Artifact", "Creature", "Insect"],
            body: (2, 3),
            life: 2,
            keyword: None,
        },
    ]
}

#[test]
fn q84_etb_gain_life_riders_map_definitions() {
    let registry = tricerules_cards::registry::global();
    for case in life_cases() {
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
        assert_eq!(face.types, case.types, "{} types", case.id);
        assert_eq!(face.power, Some(case.body.0), "{} power", case.id);
        assert_eq!(face.toughness, Some(case.body.1), "{} toughness", case.id);
        assert_eq!(face.keywords, case.keyword.into_iter().collect::<Vec<_>>());
        let entry = face
            .triggered_abilities
            .iter()
            .find(|ability| matches!(ability.trigger, TriggerCondition::WhenSelfEntersBattlefield))
            .unwrap_or_else(|| panic!("{}: entry trigger", case.id));
        match case.id {
            "elite_guardmage" => assert!(matches!(
                entry.effect.as_slice(),
                [
                    SpellEffectKind::GainLife {
                        amount: Amount::Fixed(3),
                        ..
                    },
                    SpellEffectKind::Draw {
                        count: Amount::Fixed(1),
                        ..
                    }
                ]
            )),
            _ => assert!(matches!(
                entry.effect.as_slice(),
                [SpellEffectKind::GainLife {
                    amount: Amount::Fixed(value),
                    ..
                }] if *value == case.life
            )),
        }
        if case.id == "circuit_mender" {
            let leaves = face
                .triggered_abilities
                .iter()
                .find(|ability| {
                    matches!(ability.trigger, TriggerCondition::WhenSelfLeavesBattlefield)
                })
                .expect("Circuit Mender: leaves trigger");
            assert!(matches!(
                leaves.effect.as_slice(),
                [SpellEffectKind::Draw {
                    count: Amount::Fixed(1),
                    ..
                }]
            ));
        }
    }
}

#[test]
fn q84_batch_a_fingerprints_match_the_pinned_oracle_text() {
    const FINGERPRINTS: [(&str, &str); 11] = [
        (
            "grey_havens_navigator",
            "cae84c0c237044da24da94305d6eb82d88ae3f96c3b52b4a6cdb27be643051bb",
        ),
        (
            "faithful_pikemaster",
            "fc007dba434ee5e6871043259277fbd52a541c273bcfa3920016e441762487b3",
        ),
        (
            "myr_custodian",
            "4c3b8c9635cb31144d0201890ef14302f5aae62a2c691309dadac5667b2e00d8",
        ),
        (
            "inquisitive_puppet",
            "21ce0e2c5a7e265e9489e1459ac03794ac8a6ae84824d882510a80bd04b3f099",
        ),
        (
            "jeskai_sage",
            "19705994e939f10505d39eb6bd321afb24eb399a861cf4890c5347cb389ff254",
        ),
        (
            "filigree_familiar",
            "806d0b2b670db819fbf8c2ad861de771ef5be8c01fd8def9cd60b904c3e4a688",
        ),
        (
            "living_lies_of_loki",
            "6f8b607f021fcc8563e12b62c18b6682d1e4ffdb37e91bb1a9f47fdb04a89ff0",
        ),
        (
            "phantasmal_shieldback",
            "8accfc4c7ab7387da7ea99f383de392a373a760476f810f1c9f8416242d7808b",
        ),
        (
            "elite_guardmage",
            "1a71f9ea7f8dacfc38636ea20a81051aa10f696579e498b5a5249a21645816f2",
        ),
        (
            "loxodon_restorer",
            "9162cdf4e3a562499f515e1b3dfe286b4a71f216bd0a627d675060ca55ac5858",
        ),
        (
            "circuit_mender",
            "93db5140d955665788bdbacf8a9caeeeba3debc7738c64ff789bae79f25f0bfe",
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
        assert_eq!(fields[2], id.replace('-', "_"), "face id drift for {id}");
        assert_eq!(fields[4], expected, "fingerprint drift for {id}");
    }
}
