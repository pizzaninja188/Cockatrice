//! Registry conformance for the queue-v82 01-etb-tap-riders cohort.
//!
//! Every card has an enters trigger that taps one chosen permanent, plus one printed rider. The
//! Merfolk Trickster removal and Harrier Strix loot reuse existing typed actions. Fingerprints are
//! checked after the presentation metadata refresh.

use tricerules_cards::primitives::{
    Amount, DiscardQuantity, EffectSubject, ResolvingEffectDuration, SpellEffectKind,
    TargetController, TargetKind,
};
use tricerules_cards::{registry, AbilityPresentation, CardFace, InstructionKind, Keyword, Layout};

struct Case {
    id: &'static str,
    name: &'static str,
    mana: &'static str,
    types: &'static [&'static str],
    power: u32,
    toughness: u32,
    keywords: &'static [Keyword],
    creature_only: bool,
    oracle_line: u16,
    fingerprint: &'static str,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "network_disruptor",
            name: "Network Disruptor",
            mana: "{U}",
            types: &["Artifact", "Creature", "Moonfolk", "Rogue"],
            power: 1,
            toughness: 1,
            keywords: &[Keyword::Flying],
            creature_only: false,
            oracle_line: 2,
            fingerprint: "29d2436418147d3050dc16b664f37d219823710885785e51a52cee8c861b63ce",
        },
        Case {
            id: "harrier_strix",
            name: "Harrier Strix",
            mana: "{U}",
            types: &["Creature", "Bird"],
            power: 1,
            toughness: 1,
            keywords: &[Keyword::Flying],
            creature_only: false,
            oracle_line: 2,
            fingerprint: "c25a7686eecba7f4c4e49b2112c6347091a5a11a86362a8c1d25b491585f8f75",
        },
        Case {
            id: "merfolk_trickster",
            name: "Merfolk Trickster",
            mana: "{U}{U}",
            types: &["Creature", "Merfolk", "Wizard"],
            power: 2,
            toughness: 2,
            keywords: &[Keyword::Flash],
            creature_only: true,
            oracle_line: 2,
            fingerprint: "7036197aa09c2a2ca1d4ecaf15911953137a71a111f1f330e5081de09466346d",
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
    assert!(face.static_abilities.is_empty(), "{}", case.id);
    assert!(face.spell_effect.is_empty(), "{}", case.id);
    face
}

#[test]
fn q82_tap_map_definitions() {
    for case in cases() {
        let f = face(&case);
        let [trigger] = f.triggered_abilities.as_slice() else {
            panic!("{}: one entry trigger", case.id);
        };
        assert_eq!(
            trigger.presentation,
            AbilityPresentation::OracleLines(vec![case.oracle_line]),
            "{}",
            case.id
        );
        assert_eq!(
            trigger.trigger,
            tricerules_cards::TriggerCondition::WhenSelfEntersBattlefield,
            "{}",
            case.id
        );
        let [SpellEffectKind::Tap {
            subject: EffectSubject::Chosen(filter),
        }, ..] = trigger.effect.as_slice()
        else {
            panic!("{}: first effect taps the chosen target", case.id);
        };
        if case.creature_only {
            assert_eq!(filter.kind, TargetKind::Creature, "{}", case.id);
            assert_eq!(filter.controller, TargetController::Opponent, "{}", case.id);
        } else {
            assert_eq!(filter.kind, TargetKind::AnyPermanent, "{}", case.id);
            assert_eq!(filter.controller, TargetController::Any, "{}", case.id);
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
        if case.creature_only {
            assert_eq!(group.effect_indices, [0, 1]);
            let [_, SpellEffectKind::RemoveAllAbilities {
                subject: EffectSubject::Chosen(remove_filter),
                duration,
            }] = trigger.effect.as_slice()
            else {
                panic!("Merfolk Trickster: tap then remove all abilities");
            };
            assert_eq!(remove_filter.kind, TargetKind::Creature);
            assert_eq!(remove_filter.controller, TargetController::Opponent);
            assert_eq!(*duration, ResolvingEffectDuration::UntilEndOfTurn);
        } else {
            assert_eq!(group.effect_indices, [0]);
        }
    }
}

#[test]
fn q82_tap_harrier_strix_has_the_printed_loot_activation() {
    let case = &cases()[1];
    let f = face(case);
    let [ability] = f.activated_abilities.as_slice() else {
        panic!("Harrier Strix: one activated ability");
    };
    assert_eq!(ability.ability_id.as_str(), "activated_01");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![3])
    );
    assert!(matches!(
        ability.costs.as_slice(),
        [tricerules_cards::AbilityCost::Mana(mana)] if mana.to_string() == "{2}{U}"
    ));
    assert!(ability.effect.is_empty());
    assert!(ability.targeting.is_none());
    let program = ability.program.as_ref().expect("draw-then-discard Program");
    let [draw, discard] = program.body.steps.as_slice() else {
        panic!("Harrier Strix: two steps");
    };
    let InstructionKind::Perform(draw_effect) = &draw.kind else {
        panic!("Harrier Strix: draw instruction");
    };
    assert!(matches!(
        draw_effect.as_ref(),
        SpellEffectKind::Draw {
            count: Amount::Fixed(1),
            ..
        }
    ));
    let InstructionKind::Perform(discard_effect) = &discard.kind else {
        panic!("Harrier Strix: discard instruction");
    };
    assert!(matches!(
        discard_effect.as_ref(),
        SpellEffectKind::Discard {
            quantity: DiscardQuantity::Exact(1),
            ..
        }
    ));
}

#[test]
fn q82_tap_fingerprints_match_the_pinned_oracle_text() {
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
