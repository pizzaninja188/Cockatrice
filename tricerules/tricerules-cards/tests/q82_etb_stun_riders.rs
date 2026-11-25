//! Registry conformance for the queue-v82 02-etb-stun-riders cohort.
//!
//! Every card has an enters trigger that taps one chosen opponent creature and puts a stun counter
//! on it, plus one printed rider. Fingerprints are checked after the presentation metadata refresh.

use tricerules_cards::primitives::{
    Amount, CastTriggerPlayer, CounterKind, EffectSubject, SpellEffectKind, StaticAbilityDef,
    TargetController, TargetKind, TargetingSourceFilter,
};
use tricerules_cards::{
    registry, AbilityPresentation, CardFace, Keyword, Layout, TriggerCondition,
};

struct Case {
    id: &'static str,
    name: &'static str,
    mana: &'static str,
    types: &'static [&'static str],
    power: u32,
    toughness: u32,
    keywords: &'static [Keyword],
    ward: bool,
    unblockable: bool,
    oracle_line: u16,
    fingerprint: &'static str,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "gilded_scuttler",
            name: "Gilded Scuttler",
            mana: "{2}{U}",
            types: &["Artifact", "Creature", "Crab"],
            power: 1,
            toughness: 3,
            keywords: &[],
            ward: false,
            unblockable: true,
            oracle_line: 2,
            fingerprint: "91df2940da7a0cb4f4e699cd9dd6b659b651046298be5e9560217cb588f3f699",
        },
        Case {
            id: "frostfist_strider",
            name: "Frostfist Strider",
            mana: "{3}{U}{U}",
            types: &["Creature", "Elemental", "Giant"],
            power: 4,
            toughness: 4,
            keywords: &[],
            ward: true,
            unblockable: false,
            oracle_line: 2,
            fingerprint: "5ae7b279de748ad9d6b0d16ae378c01d3caed1fa7adcd88bc79ab6ca59a52c1f",
        },
        Case {
            id: "referee_squad",
            name: "Referee Squad",
            mana: "{2}{U}",
            types: &["Creature", "Homunculus"],
            power: 2,
            toughness: 2,
            keywords: &[Keyword::Convoke, Keyword::Vigilance],
            ward: false,
            unblockable: false,
            oracle_line: 3,
            fingerprint: "6af0c869c85289c9f1f8507689cb5563093360e9f8654789215ecd9b662f1c22",
        },
    ]
}

fn face(case: &Case) -> &'static CardFace {
    let definition = registry::global()
        .get(case.id)
        .unwrap_or_else(|| panic!("missing {}", case.id));
    assert_eq!(definition.layout, Layout::Normal, "{}", case.id);
    assert_eq!(
        registry::global().id_for_name(case.name),
        Some(case.id),
        "{}",
        case.id
    );
    let face = definition.primary_face();
    assert_eq!(face.face_id.as_str(), case.id, "{} face id", case.id);
    assert_eq!(face.name, case.name, "{} name", case.id);
    assert_eq!(face.mana_cost.to_string(), case.mana, "{} mana", case.id);
    assert_eq!(face.types, case.types, "{} types", case.id);
    assert_eq!(
        (face.power, face.toughness),
        (Some(case.power), Some(case.toughness)),
        "{} body",
        case.id
    );
    assert_eq!(face.keywords, case.keywords, "{} keywords", case.id);
    assert!(face.spell_effect.is_empty(), "{}", case.id);
    assert!(face.activated_abilities.is_empty(), "{}", case.id);
    face
}

fn stun_trigger<'a>(face: &'a CardFace, case: &Case) -> &'a tricerules_cards::TriggeredAbilityDef {
    let trigger = face
        .triggered_abilities
        .iter()
        .find(|trigger| trigger.trigger == TriggerCondition::WhenSelfEntersBattlefield)
        .unwrap_or_else(|| panic!("{}: missing entry trigger", case.id));
    assert_eq!(
        trigger.presentation,
        AbilityPresentation::OracleLines(vec![case.oracle_line]),
        "{}",
        case.id
    );
    let [SpellEffectKind::Tap {
        subject: EffectSubject::Chosen(tap_filter),
    }, SpellEffectKind::PutCounters {
        counter: CounterKind::Stun,
        count: Amount::Fixed(1),
        subject: EffectSubject::Chosen(stun_filter),
    }] = trigger.effect.as_slice()
    else {
        panic!("{}: tap then one stun counter", case.id);
    };
    for filter in [tap_filter, stun_filter] {
        assert_eq!(filter.kind, TargetKind::Creature, "{}", case.id);
        assert_eq!(filter.controller, TargetController::Opponent, "{}", case.id);
    }
    let [group] = trigger
        .targeting
        .as_ref()
        .unwrap_or_else(|| panic!("{}: targeting", case.id))
        .groups
        .as_slice()
    else {
        panic!("{}: one target group", case.id);
    };
    assert_eq!(group.min, 1);
    assert_eq!(group.max, 1);
    assert_eq!(group.effect_indices, [0, 1]);
    trigger
}

#[test]
fn q82_stun_map_definitions() {
    for case in cases() {
        let f = face(&case);
        assert_eq!(f.triggered_abilities.len(), 1 + usize::from(case.ward));
        assert_eq!(f.static_abilities.len(), usize::from(case.unblockable));
        stun_trigger(f, &case);
        if case.unblockable {
            let [static_ability] = f.static_abilities.as_slice() else {
                panic!("Gilded Scuttler: one static");
            };
            assert_eq!(
                static_ability.presentation,
                AbilityPresentation::OracleLines(vec![1])
            );
            assert!(matches!(
                &static_ability.definition,
                StaticAbilityDef::SelfCombatRestriction { restriction, .. }
                    if restriction.cant_be_blocked
            ));
        }
        if case.ward {
            let ward = f
                .triggered_abilities
                .iter()
                .find(|trigger| trigger.ability_id.as_str() == "triggered_01")
                .unwrap_or_else(|| panic!("Frostfist Strider: Ward trigger"));
            assert_eq!(ward.presentation, AbilityPresentation::OracleLines(vec![1]));
            assert_eq!(
                ward.trigger,
                TriggerCondition::WheneverSelfBecomesTarget {
                    source: TargetingSourceFilter::SpellOrAbility,
                    source_controller: CastTriggerPlayer::Opponent,
                }
            );
            assert!(ward.program.is_some(), "Ward {{2}} uses its Program");
        }
    }
}

#[test]
fn q82_stun_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for case in cases() {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{}\t", case.id)))
            .unwrap_or_else(|| panic!("missing fingerprint row for {}", case.id));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[0], case.id);
        assert_eq!(fields[1], case.name);
        assert_eq!(fields[2], case.id, "face id drift for {}", case.id);
        assert_eq!(
            fields[4], case.fingerprint,
            "fingerprint drift for {}",
            case.id
        );
    }
}
