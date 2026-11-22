//! Registry conformance for the queue-v27 enters-tap-freeze cohort.
//!
//! Each card is a normal-layout printed card whose complete typed shape is asserted
//! against the pinned Oracle text. Fingerprints are SHA-256 of the exact normalized
//! Oracle lines.

use tricerules_cards::primitives::{
    EffectSubject, SpellEffectKind, TargetController, TargetKind, TriggerCondition,
};
use tricerules_cards::{AbilityPresentation, Layout};

struct Card {
    id: &'static str,
    name: &'static str,
    mana: &'static str,
    supertypes: &'static [&'static str],
    types: &'static [&'static str],
    power: Option<u32>,
    toughness: Option<u32>,
    keywords: &'static [&'static str],
    fingerprint: &'static str,
}

const CARDS: &[Card] = &[
    Card {
        id: "frost_lynx",
        name: "Frost Lynx",
        mana: "{2}{U}",
        supertypes: &[],
        types: &["Creature", "Elemental", "Cat"],
        power: Some(2),
        toughness: Some(2),
        keywords: &[],
        fingerprint: "feceed7909e2097b36ef8577bbbad915a0abe94842fac6eab13f0d10395383e7",
    },
    Card {
        id: "kor_hookmaster",
        name: "Kor Hookmaster",
        mana: "{2}{W}",
        supertypes: &[],
        types: &["Creature", "Kor", "Soldier"],
        power: Some(2),
        toughness: Some(2),
        keywords: &[],
        fingerprint: "feceed7909e2097b36ef8577bbbad915a0abe94842fac6eab13f0d10395383e7",
    },
    Card {
        id: "watertrap_weaver",
        name: "Watertrap Weaver",
        mana: "{2}{U}",
        supertypes: &[],
        types: &["Creature", "Merfolk", "Wizard"],
        power: Some(2),
        toughness: Some(2),
        keywords: &[],
        fingerprint: "feceed7909e2097b36ef8577bbbad915a0abe94842fac6eab13f0d10395383e7",
    },
    Card {
        id: "frost_trickster",
        name: "Frost Trickster",
        mana: "{2}{U}",
        supertypes: &[],
        types: &["Creature", "Bird", "Wizard"],
        power: Some(2),
        toughness: Some(2),
        keywords: &["Flying"],
        fingerprint: "b5d2504cebbfe1923ed2a997647ba814ae6446921a28c7dd5e4c15cc5baa3d8b",
    },
    Card {
        id: "spire_patrol",
        name: "Spire Patrol",
        mana: "{2}{W}{U}",
        supertypes: &[],
        types: &["Creature", "Human", "Soldier"],
        power: Some(3),
        toughness: Some(2),
        keywords: &["Flying"],
        fingerprint: "b5d2504cebbfe1923ed2a997647ba814ae6446921a28c7dd5e4c15cc5baa3d8b",
    },
];

fn face<'a>(
    registry: &'a tricerules_cards::CardRegistry,
    id: &str,
) -> &'a tricerules_cards::CardFace {
    registry
        .get(id)
        .unwrap_or_else(|| panic!("missing {id}"))
        .primary_face()
}

#[test]
fn etb_freeze_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for card in CARDS {
        let definition = registry
            .get(card.id)
            .unwrap_or_else(|| panic!("missing {}", card.id));
        assert_eq!(
            registry.id_for_name(card.name),
            Some(card.id),
            "{}",
            card.id
        );
        let f = face(registry, card.id);
        assert_eq!(f.face_id.as_str(), card.id, "{} face id", card.id);
        assert_eq!(f.name, card.name, "{} name", card.id);
        assert_eq!(f.mana_cost.to_string(), card.mana, "{} mana", card.id);
        assert_eq!(f.supertypes, card.supertypes, "{} supertypes", card.id);
        assert_eq!(f.types, card.types, "{} types", card.id);
        assert_eq!(f.power, card.power, "{} power", card.id);
        assert_eq!(f.toughness, card.toughness, "{} toughness", card.id);
        assert_eq!(
            f.keywords.len(),
            card.keywords.len(),
            "{} keywords",
            card.id
        );
        for keyword in card.keywords {
            assert!(
                f.keywords
                    .iter()
                    .any(|value| format!("{value:?}") == *keyword),
                "{} keyword {keyword}",
                card.id
            );
        }
        match card.id {
            "frost_lynx" => {
                assert_eq!(definition.layout, Layout::Normal, "{}", card.id);
                assert!(f.activated_abilities.is_empty(), "{}", card.id);
                assert!(f.spell_effect.is_empty(), "{}", card.id);
                assert!(f.static_abilities.is_empty(), "{}", card.id);
                assert!(f.targeting.is_none(), "{}", card.id);
                let [trigger] = f.triggered_abilities.as_slice() else {
                    panic!("{}: exactly one triggered ability", card.id);
                };
                assert_eq!(
                    trigger.trigger,
                    TriggerCondition::WhenSelfEntersBattlefield,
                    "{}",
                    card.id
                );
                assert!(!trigger.may, "{}", card.id);
                assert_eq!(
                    trigger.presentation,
                    AbilityPresentation::OracleLines(vec![if card.keywords.is_empty() {
                        1
                    } else {
                        2
                    }]),
                    "{}",
                    card.id
                );
                let [SpellEffectKind::Tap { subject }, SpellEffectKind::SkipNextUntap { target }] =
                    trigger.effect.as_slice()
                else {
                    panic!("{}: tap then skip next untap", card.id);
                };
                let EffectSubject::Chosen(tap_target) = subject else {
                    panic!("{}: chosen tap target", card.id);
                };
                assert_eq!(tap_target.kind, TargetKind::Creature, "{}", card.id);
                assert_eq!(
                    tap_target.controller,
                    TargetController::Opponent,
                    "{}",
                    card.id
                );
                assert_eq!(target.kind, TargetKind::Creature, "{}", card.id);
                assert_eq!(target.controller, TargetController::Opponent, "{}", card.id);
                let targeting = trigger.targeting.as_ref().expect("trigger target");
                assert_eq!(targeting.groups.len(), 1, "{}", card.id);
                assert_eq!(
                    (targeting.groups[0].min, targeting.groups[0].max),
                    (1, 1),
                    "{}",
                    card.id
                );
                assert_eq!(targeting.groups[0].effect_indices, [0, 1], "{}", card.id);
            }
            "kor_hookmaster" => {
                assert_eq!(definition.layout, Layout::Normal, "{}", card.id);
                assert!(f.activated_abilities.is_empty(), "{}", card.id);
                assert!(f.spell_effect.is_empty(), "{}", card.id);
                assert!(f.static_abilities.is_empty(), "{}", card.id);
                assert!(f.targeting.is_none(), "{}", card.id);
                let [trigger] = f.triggered_abilities.as_slice() else {
                    panic!("{}: exactly one triggered ability", card.id);
                };
                assert_eq!(
                    trigger.trigger,
                    TriggerCondition::WhenSelfEntersBattlefield,
                    "{}",
                    card.id
                );
                assert!(!trigger.may, "{}", card.id);
                assert_eq!(
                    trigger.presentation,
                    AbilityPresentation::OracleLines(vec![if card.keywords.is_empty() {
                        1
                    } else {
                        2
                    }]),
                    "{}",
                    card.id
                );
                let [SpellEffectKind::Tap { subject }, SpellEffectKind::SkipNextUntap { target }] =
                    trigger.effect.as_slice()
                else {
                    panic!("{}: tap then skip next untap", card.id);
                };
                let EffectSubject::Chosen(tap_target) = subject else {
                    panic!("{}: chosen tap target", card.id);
                };
                assert_eq!(tap_target.kind, TargetKind::Creature, "{}", card.id);
                assert_eq!(
                    tap_target.controller,
                    TargetController::Opponent,
                    "{}",
                    card.id
                );
                assert_eq!(target.kind, TargetKind::Creature, "{}", card.id);
                assert_eq!(target.controller, TargetController::Opponent, "{}", card.id);
                let targeting = trigger.targeting.as_ref().expect("trigger target");
                assert_eq!(targeting.groups.len(), 1, "{}", card.id);
                assert_eq!(
                    (targeting.groups[0].min, targeting.groups[0].max),
                    (1, 1),
                    "{}",
                    card.id
                );
                assert_eq!(targeting.groups[0].effect_indices, [0, 1], "{}", card.id);
            }
            "watertrap_weaver" => {
                assert_eq!(definition.layout, Layout::Normal, "{}", card.id);
                assert!(f.activated_abilities.is_empty(), "{}", card.id);
                assert!(f.spell_effect.is_empty(), "{}", card.id);
                assert!(f.static_abilities.is_empty(), "{}", card.id);
                assert!(f.targeting.is_none(), "{}", card.id);
                let [trigger] = f.triggered_abilities.as_slice() else {
                    panic!("{}: exactly one triggered ability", card.id);
                };
                assert_eq!(
                    trigger.trigger,
                    TriggerCondition::WhenSelfEntersBattlefield,
                    "{}",
                    card.id
                );
                assert!(!trigger.may, "{}", card.id);
                assert_eq!(
                    trigger.presentation,
                    AbilityPresentation::OracleLines(vec![if card.keywords.is_empty() {
                        1
                    } else {
                        2
                    }]),
                    "{}",
                    card.id
                );
                let [SpellEffectKind::Tap { subject }, SpellEffectKind::SkipNextUntap { target }] =
                    trigger.effect.as_slice()
                else {
                    panic!("{}: tap then skip next untap", card.id);
                };
                let EffectSubject::Chosen(tap_target) = subject else {
                    panic!("{}: chosen tap target", card.id);
                };
                assert_eq!(tap_target.kind, TargetKind::Creature, "{}", card.id);
                assert_eq!(
                    tap_target.controller,
                    TargetController::Opponent,
                    "{}",
                    card.id
                );
                assert_eq!(target.kind, TargetKind::Creature, "{}", card.id);
                assert_eq!(target.controller, TargetController::Opponent, "{}", card.id);
                let targeting = trigger.targeting.as_ref().expect("trigger target");
                assert_eq!(targeting.groups.len(), 1, "{}", card.id);
                assert_eq!(
                    (targeting.groups[0].min, targeting.groups[0].max),
                    (1, 1),
                    "{}",
                    card.id
                );
                assert_eq!(targeting.groups[0].effect_indices, [0, 1], "{}", card.id);
            }
            "frost_trickster" => {
                assert_eq!(definition.layout, Layout::Normal, "{}", card.id);
                assert!(f.activated_abilities.is_empty(), "{}", card.id);
                assert!(f.spell_effect.is_empty(), "{}", card.id);
                assert!(f.static_abilities.is_empty(), "{}", card.id);
                assert!(f.targeting.is_none(), "{}", card.id);
                let [trigger] = f.triggered_abilities.as_slice() else {
                    panic!("{}: exactly one triggered ability", card.id);
                };
                assert_eq!(
                    trigger.trigger,
                    TriggerCondition::WhenSelfEntersBattlefield,
                    "{}",
                    card.id
                );
                assert!(!trigger.may, "{}", card.id);
                assert_eq!(
                    trigger.presentation,
                    AbilityPresentation::OracleLines(vec![if card.keywords.is_empty() {
                        1
                    } else {
                        2
                    }]),
                    "{}",
                    card.id
                );
                let [SpellEffectKind::Tap { subject }, SpellEffectKind::SkipNextUntap { target }] =
                    trigger.effect.as_slice()
                else {
                    panic!("{}: tap then skip next untap", card.id);
                };
                let EffectSubject::Chosen(tap_target) = subject else {
                    panic!("{}: chosen tap target", card.id);
                };
                assert_eq!(tap_target.kind, TargetKind::Creature, "{}", card.id);
                assert_eq!(
                    tap_target.controller,
                    TargetController::Opponent,
                    "{}",
                    card.id
                );
                assert_eq!(target.kind, TargetKind::Creature, "{}", card.id);
                assert_eq!(target.controller, TargetController::Opponent, "{}", card.id);
                let targeting = trigger.targeting.as_ref().expect("trigger target");
                assert_eq!(targeting.groups.len(), 1, "{}", card.id);
                assert_eq!(
                    (targeting.groups[0].min, targeting.groups[0].max),
                    (1, 1),
                    "{}",
                    card.id
                );
                assert_eq!(targeting.groups[0].effect_indices, [0, 1], "{}", card.id);
            }
            "spire_patrol" => {
                assert_eq!(definition.layout, Layout::Normal, "{}", card.id);
                assert!(f.activated_abilities.is_empty(), "{}", card.id);
                assert!(f.spell_effect.is_empty(), "{}", card.id);
                assert!(f.static_abilities.is_empty(), "{}", card.id);
                assert!(f.targeting.is_none(), "{}", card.id);
                let [trigger] = f.triggered_abilities.as_slice() else {
                    panic!("{}: exactly one triggered ability", card.id);
                };
                assert_eq!(
                    trigger.trigger,
                    TriggerCondition::WhenSelfEntersBattlefield,
                    "{}",
                    card.id
                );
                assert!(!trigger.may, "{}", card.id);
                assert_eq!(
                    trigger.presentation,
                    AbilityPresentation::OracleLines(vec![if card.keywords.is_empty() {
                        1
                    } else {
                        2
                    }]),
                    "{}",
                    card.id
                );
                let [SpellEffectKind::Tap { subject }, SpellEffectKind::SkipNextUntap { target }] =
                    trigger.effect.as_slice()
                else {
                    panic!("{}: tap then skip next untap", card.id);
                };
                let EffectSubject::Chosen(tap_target) = subject else {
                    panic!("{}: chosen tap target", card.id);
                };
                assert_eq!(tap_target.kind, TargetKind::Creature, "{}", card.id);
                assert_eq!(
                    tap_target.controller,
                    TargetController::Opponent,
                    "{}",
                    card.id
                );
                assert_eq!(target.kind, TargetKind::Creature, "{}", card.id);
                assert_eq!(target.controller, TargetController::Opponent, "{}", card.id);
                let targeting = trigger.targeting.as_ref().expect("trigger target");
                assert_eq!(targeting.groups.len(), 1, "{}", card.id);
                assert_eq!(
                    (targeting.groups[0].min, targeting.groups[0].max),
                    (1, 1),
                    "{}",
                    card.id
                );
                assert_eq!(targeting.groups[0].effect_indices, [0, 1], "{}", card.id);
            }
            _ => unreachable!("known card"),
        }
    }
}

#[test]
fn etb_freeze_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for card in CARDS {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{}\t", card.id)))
            .unwrap_or_else(|| panic!("missing fingerprint row for {}", card.id));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[0], card.id);
        assert_eq!(fields[1], card.name);
        assert_eq!(fields[2], card.id);
        assert_eq!(
            fields[4], card.fingerprint,
            "fingerprint drift for {}",
            card.id
        );
    }
}
