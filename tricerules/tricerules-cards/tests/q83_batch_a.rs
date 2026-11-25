//! Registry conformance for the queue-v83 batch A card data.
//!
//! Module 1 covers the five Implements: a normal artifact with one activated ability that pays a
//! colored mana and sacrifices it, plus a battlefield-to-graveyard draw trigger. Module 2 covers
//! the three entry life-loss creatures, including Eviscerator's printed protection. Module 3 covers
//! the four Heroic counter creatures. Fingerprints are checked after the presentation metadata
//! refresh, so their values are asserted only through the shared fingerprint rows.

use tricerules_cards::primitives::{
    AbilityCost, Amount, EffectSubject, HandCardChooser, LifeAmount, PlayerRecipient,
    ProtectionQuality, SpellEffectKind, TargetKind,
};
use tricerules_cards::{
    AbilityPresentation, ActivationTiming, Color, CounterKind, Keyword, Layout,
};

// ── 01 artifact death draw ─────────────────────────────────────────────────

struct ImplementCase {
    id: &'static str,
    name: &'static str,
    mana: &'static str,
    activation: &'static str,
    body: ImplementBody,
    timing: ActivationTiming,
    fingerprint: &'static str,
}

#[derive(Clone, Copy)]
enum ImplementBody {
    DamagePlayer,
    Draw,
    Counter,
    GainLife(u32),
    DiscardPlayer,
}

fn implement_cases() -> [ImplementCase; 5] {
    [
        ImplementCase {
            id: "implement_of_combustion",
            name: "Implement of Combustion",
            mana: "{1}",
            activation: "{R}",
            body: ImplementBody::DamagePlayer,
            timing: ActivationTiming::Normal,
            fingerprint: "1d290816e80a85496935760068a5968286c8c673416c6e761ca1ed846e4076d6",
        },
        ImplementCase {
            id: "implement_of_examination",
            name: "Implement of Examination",
            mana: "{3}",
            activation: "{U}",
            body: ImplementBody::Draw,
            timing: ActivationTiming::Normal,
            fingerprint: "34bc4eb8e2d79e5181265b12d5b4323b5b6f91e5fdbf53edf46d4b75e4d4ce9d",
        },
        ImplementCase {
            id: "implement_of_ferocity",
            name: "Implement of Ferocity",
            mana: "{1}",
            activation: "{G}",
            body: ImplementBody::Counter,
            timing: ActivationTiming::SorcerySpeed,
            fingerprint: "652de619d1f494afa93e9b25ee4eb7d6b63ff2149d551709bb720e206be86a23",
        },
        ImplementCase {
            id: "implement_of_improvement",
            name: "Implement of Improvement",
            mana: "{1}",
            activation: "{W}",
            body: ImplementBody::GainLife(2),
            timing: ActivationTiming::Normal,
            fingerprint: "00b6772e899ec699504974b326ea6c3fa3c5d768f22781993305988e2dd391d5",
        },
        ImplementCase {
            id: "implement_of_malice",
            name: "Implement of Malice",
            mana: "{2}",
            activation: "{B}",
            body: ImplementBody::DiscardPlayer,
            timing: ActivationTiming::SorcerySpeed,
            fingerprint: "7df34dd544256b7fd196ef5ec556819e89265f16e94cad968fedfb66d6ce53b4",
        },
    ]
}

#[test]
fn q83_artifact_death_draw_map_definitions() {
    let registry = tricerules_cards::registry::global();
    for case in implement_cases() {
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
        assert_eq!(face.types, ["Artifact"], "{} types", case.id);
        assert!(face.keywords.is_empty(), "{}", case.id);
        assert!(face.protections.is_empty(), "{}", case.id);
        assert!(face.spell_effect.is_empty(), "{}", case.id);
        assert!(face.static_abilities.is_empty(), "{}", case.id);

        let [ability] = face.activated_abilities.as_slice() else {
            panic!("{}: one activated ability", case.id);
        };
        assert_eq!(ability.ability_id.as_str(), "activated_01", "{}", case.id);
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{}",
            case.id
        );
        assert_eq!(ability.timing, case.timing, "{} timing", case.id);
        assert!(
            matches!(
                ability.costs.as_slice(),
                [AbilityCost::Mana(mana), AbilityCost::SacrificeSelf]
                    if mana.to_string() == case.activation
            ),
            "{}: {:?}",
            case.id,
            ability.costs
        );
        match case.body {
            ImplementBody::Draw => assert!(
                matches!(
                    ability.effect.as_slice(),
                    [SpellEffectKind::Draw {
                        count: Amount::Fixed(1),
                        ..
                    }]
                ),
                "{}: draw one",
                case.id
            ),
            ImplementBody::GainLife(amount) => assert!(
                matches!(
                    ability.effect.as_slice(),
                    [SpellEffectKind::GainLife {
                        amount: Amount::Fixed(value),
                        ..
                    }] if *value == amount
                ),
                "{}: gain life",
                case.id
            ),
            ImplementBody::DamagePlayer => {
                let [SpellEffectKind::DamageTarget { amount, target }] = ability.effect.as_slice()
                else {
                    panic!("{}: damage effect", case.id);
                };
                assert!(matches!(amount, Amount::Fixed(1)), "{}", case.id);
                assert!(
                    target.any_of.as_ref().is_some_and(|branches| {
                        branches.len() == 2
                            && branches
                                .iter()
                                .any(|branch| branch.kind == TargetKind::AnyPlayer)
                            && branches.iter().any(|branch| {
                                branch.kind == TargetKind::AnyPermanent
                                    && branch.permanent_types
                                        == [tricerules_cards::PermanentTypeFilter::Planeswalker]
                            })
                    }),
                    "{}: player-or-planeswalker branches",
                    case.id
                );
                assert_eq!(
                    ability.targeting.as_ref().expect("targeting").groups.len(),
                    1,
                    "{}",
                    case.id
                );
            }
            ImplementBody::Counter => {
                let [SpellEffectKind::PutCounters {
                    counter: CounterKind::PlusOnePlusOne,
                    count: Amount::Fixed(1),
                    subject: EffectSubject::Chosen(filter),
                }] = ability.effect.as_slice()
                else {
                    panic!("{}: one +1/+1 counter on the chosen creature", case.id);
                };
                assert_eq!(filter.kind, TargetKind::Creature, "{}", case.id);
                assert_eq!(
                    ability.targeting.as_ref().expect("targeting").groups.len(),
                    1,
                    "{}",
                    case.id
                );
            }
            ImplementBody::DiscardPlayer => {
                let [SpellEffectKind::ChooseHandCards {
                    count: 1,
                    target,
                    chooser,
                    ..
                }] = ability.effect.as_slice()
                else {
                    panic!("{}: discard one chosen by its owner", case.id);
                };
                assert_eq!(target.kind, TargetKind::AnyPlayer, "{}", case.id);
                assert_eq!(*chooser, HandCardChooser::AffectedPlayer, "{}", case.id);
                assert_eq!(
                    ability.targeting.as_ref().expect("targeting").groups.len(),
                    1,
                    "{}",
                    case.id
                );
            }
        }

        let [trigger] = face.triggered_abilities.as_slice() else {
            panic!("{}: one death trigger", case.id);
        };
        assert_eq!(trigger.ability_id.as_str(), "triggered_01", "{}", case.id);
        assert_eq!(
            trigger.presentation,
            AbilityPresentation::OracleLines(vec![2]),
            "{}",
            case.id
        );
        assert_eq!(
            trigger.trigger,
            tricerules_cards::TriggerCondition::WhenSelfDies,
            "{}",
            case.id
        );
        assert!(
            matches!(
                trigger.effect.as_slice(),
                [SpellEffectKind::Draw {
                    count: Amount::Fixed(1),
                    ..
                }]
            ),
            "{}: death draw",
            case.id
        );
        assert!(trigger.targeting.is_none(), "{}", case.id);
    }
}

// ── 02 entry life loss ─────────────────────────────────────────────────────

#[test]
fn q83_etb_lose_life_map_definitions() {
    let registry = tricerules_cards::registry::global();
    let cases = [
        (
            "serpent_warrior",
            "Serpent Warrior",
            "{2}{B}",
            3u32,
            3u32,
            3u32,
            false,
        ),
        (
            "toxic_abomination",
            "Toxic Abomination",
            "{1}{B}",
            3,
            2,
            2,
            false,
        ),
        ("eviscerator", "Eviscerator", "{3}{B}{B}", 5, 5, 5, true),
    ];
    for (id, name, mana, power, toughness, amount, protection) in cases {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        assert_eq!(definition.name, name, "{id}");
        assert_eq!(registry.id_for_name(name), Some(id), "{id}");
        let face = definition.primary_face();
        assert_eq!(face.face_id.as_str(), id.replace('-', "_"), "{id} face id");
        assert_eq!(face.mana_cost.to_string(), mana, "{id} mana");
        assert!(
            face.types.iter().any(|card_type| card_type == "Creature"),
            "{id}"
        );
        assert_eq!(
            (face.power, face.toughness),
            (Some(power), Some(toughness)),
            "{id} body"
        );
        assert!(face.keywords.is_empty(), "{id}");
        assert_eq!(
            face.protections == [ProtectionQuality::Color(Color::White)],
            protection,
            "{id} protection"
        );
        assert!(face.spell_effect.is_empty(), "{id}");
        assert!(face.activated_abilities.is_empty(), "{id}");
        assert!(face.static_abilities.is_empty(), "{id}");
        let [trigger] = face.triggered_abilities.as_slice() else {
            panic!("{id}: one entry trigger");
        };
        assert_eq!(
            trigger.presentation,
            AbilityPresentation::OracleLines(vec![1 + u16::from(protection)]),
            "{id} presentation"
        );
        assert_eq!(
            trigger.trigger,
            tricerules_cards::TriggerCondition::WhenSelfEntersBattlefield,
            "{id}"
        );
        assert!(
            matches!(
                trigger.effect.as_slice(),
                [SpellEffectKind::LoseLife {
                    amount: LifeAmount::Fixed(value),
                    who: PlayerRecipient::Controller,
                }] if *value == amount
            ),
            "{id}: printed life loss"
        );
        assert!(trigger.targeting.is_none(), "{id}");
    }
}

// ── 03 heroic counter ──────────────────────────────────────────────────────

struct HeroicCase {
    id: &'static str,
    name: &'static str,
    mana: &'static str,
    body: (u32, u32),
    keywords: &'static [Keyword],
}

#[test]
fn q83_heroic_map_definitions() {
    let registry = tricerules_cards::registry::global();
    let cases: [HeroicCase; 4] = [
        HeroicCase {
            id: "satyr_hoplite",
            name: "Satyr Hoplite",
            mana: "{R}",
            body: (1, 1),
            keywords: &[],
        },
        HeroicCase {
            id: "war-wing_siren",
            name: "War-Wing Siren",
            mana: "{2}{U}",
            body: (1, 3),
            keywords: &[Keyword::Flying],
        },
        HeroicCase {
            id: "dawnbringer_charioteers",
            name: "Dawnbringer Charioteers",
            mana: "{2}{W}{W}",
            body: (2, 4),
            keywords: &[Keyword::Flying, Keyword::Lifelink],
        },
        HeroicCase {
            id: "fabled_hero",
            name: "Fabled Hero",
            mana: "{1}{W}{W}",
            body: (2, 2),
            keywords: &[Keyword::DoubleStrike],
        },
    ];
    for case in cases {
        let HeroicCase {
            id,
            name,
            mana,
            body,
            keywords,
        } = case;
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        assert_eq!(definition.name, name, "{id}");
        assert_eq!(registry.id_for_name(name), Some(id), "{id}");
        let face = definition.primary_face();
        assert_eq!(face.face_id.as_str(), id.replace('-', "_"), "{id} face id");
        assert_eq!(face.mana_cost.to_string(), mana, "{id} mana");
        assert_eq!(
            (face.power, face.toughness),
            (Some(body.0), Some(body.1)),
            "{id} body"
        );
        assert_eq!(face.keywords, keywords, "{id} keywords");
        assert!(face.spell_effect.is_empty(), "{id}");
        assert!(face.activated_abilities.is_empty(), "{id}");
        assert!(face.static_abilities.is_empty(), "{id}");
        let [trigger] = face.triggered_abilities.as_slice() else {
            panic!("{id}: one heroic trigger");
        };
        assert_eq!(
            trigger.presentation,
            AbilityPresentation::OracleLines(vec![if id == "satyr_hoplite" { 1 } else { 2 }]),
            "{id}"
        );
        assert!(
            matches!(
                trigger.trigger,
                tricerules_cards::TriggerCondition::WheneverSelfBecomesTarget {
                    source: tricerules_cards::primitives::TargetingSourceFilter::SpellCast,
                    source_controller: tricerules_cards::primitives::CastTriggerPlayer::Controller,
                }
            ),
            "{id}: heroic trigger"
        );
        assert!(
            matches!(
                trigger.effect.as_slice(),
                [SpellEffectKind::PutCounters {
                    counter: CounterKind::PlusOnePlusOne,
                    count: Amount::Fixed(1),
                    subject: EffectSubject::Source,
                }]
            ),
            "{id}: self +1/+1 counter"
        );
        assert!(trigger.targeting.is_none(), "{id}");
    }
}

// ── fingerprints ───────────────────────────────────────────────────────────

const OTHER_FINGERPRINTS: [(&str, &str); 7] = [
    (
        "serpent_warrior",
        "6b692617ad68347692bd474331a5e6afb915b4cc0c8ebfcd6287dcd564e175b4",
    ),
    (
        "toxic_abomination",
        "18fd2b35354a970a636fdb7ff9175b8b5c0c9bb37c770bf0fcfd47c6343b7358",
    ),
    (
        "eviscerator",
        "b0098ffd6932bb8d3b4c011ee573f83de761dc66d042709a6aeb4834f7cd22b1",
    ),
    (
        "satyr_hoplite",
        "b8f8686ddad744980f83c52bf885c25bfd964a9d3d7010dc248c39ab677cb1e2",
    ),
    (
        "war-wing_siren",
        "5383c9e6100e24685f4fa6888565819c320844599b7c391b8f4c10fe83b80041",
    ),
    (
        "dawnbringer_charioteers",
        "bc245faff8f57ac71ba85e27850c6dc21979ccc655c54bf9903a5e8571f22fad",
    ),
    (
        "fabled_hero",
        "a07b027ce127f3ea04f7ee3eded2d32001f5c1ca4e70b3b51a1eb606ec93cef0",
    ),
];

#[test]
fn q83_batch_a_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    let mut cases: Vec<(&str, &str)> = implement_cases()
        .iter()
        .map(|case| (case.id, case.fingerprint))
        .collect();
    cases.extend(OTHER_FINGERPRINTS);
    for (id, expected) in cases {
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
