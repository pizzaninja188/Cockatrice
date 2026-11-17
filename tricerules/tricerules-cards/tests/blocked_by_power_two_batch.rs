//! Registry conformance for the queue-v27 power-two evasion cohort.
//!
//! Each card is a normal-layout printed card whose complete typed shape is asserted
//! against the pinned Oracle text. Fingerprints are SHA-256 of the exact normalized
//! Oracle lines.

use tricerules_cards::primitives::{PowerComparison, StaticAbilityDef, TargetKind};
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
        id: "arlinns_wolf",
        name: "Arlinn's Wolf",
        mana: "{2}{G}",
        supertypes: &[],
        types: &["Creature", "Wolf"],
        power: Some(3),
        toughness: Some(2),
        keywords: &[],
        fingerprint: "14f667d2371fd0f82039d2af8ff9fe204e006b8f2ab1d0e9c0b81f991f4ee0e9",
    },
    Card {
        id: "enraged_ceratok",
        name: "Enraged Ceratok",
        mana: "{2}{G}{G}",
        supertypes: &[],
        types: &["Creature", "Rhino"],
        power: Some(4),
        toughness: Some(4),
        keywords: &[],
        fingerprint: "14f667d2371fd0f82039d2af8ff9fe204e006b8f2ab1d0e9c0b81f991f4ee0e9",
    },
    Card {
        id: "giltgrove_stalker",
        name: "Giltgrove Stalker",
        mana: "{1}{G}",
        supertypes: &[],
        types: &["Creature", "Merfolk", "Warrior"],
        power: Some(2),
        toughness: Some(1),
        keywords: &[],
        fingerprint: "14f667d2371fd0f82039d2af8ff9fe204e006b8f2ab1d0e9c0b81f991f4ee0e9",
    },
    Card {
        id: "outland_boar",
        name: "Outland Boar",
        mana: "{2}{R}{G}",
        supertypes: &[],
        types: &["Creature", "Boar"],
        power: Some(4),
        toughness: Some(4),
        keywords: &[],
        fingerprint: "14f667d2371fd0f82039d2af8ff9fe204e006b8f2ab1d0e9c0b81f991f4ee0e9",
    },
    Card {
        id: "steel_leaf_champion",
        name: "Steel Leaf Champion",
        mana: "{G}{G}{G}",
        supertypes: &[],
        types: &["Creature", "Elf", "Knight"],
        power: Some(5),
        toughness: Some(4),
        keywords: &[],
        fingerprint: "14f667d2371fd0f82039d2af8ff9fe204e006b8f2ab1d0e9c0b81f991f4ee0e9",
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
fn blocked_by_power_two_batch_maps_definitions() {
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
            "arlinns_wolf" => {
                assert_eq!(definition.layout, Layout::Normal, "{}", card.id);
                assert!(f.activated_abilities.is_empty(), "{}", card.id);
                assert!(f.triggered_abilities.is_empty(), "{}", card.id);
                assert!(f.spell_effect.is_empty(), "{}", card.id);
                assert!(f.targeting.is_none(), "{}", card.id);
                let [entry] = f.static_abilities.as_slice() else {
                    panic!("{}: exactly one static ability", card.id);
                };
                assert_eq!(
                    entry.presentation,
                    AbilityPresentation::OracleLines(vec![1]),
                    "{}",
                    card.id
                );
                let StaticAbilityDef::SelfCombatRestriction {
                    restriction,
                    condition,
                } = &entry.definition
                else {
                    panic!("{}: self combat restriction", card.id);
                };
                assert!(condition.is_none(), "{}", card.id);
                assert_eq!(restriction.maximum_blockers, None, "{}", card.id);
                assert!(
                    !restriction.cant_block && !restriction.cant_attack,
                    "{}",
                    card.id
                );
                assert!(!restriction.cant_be_blocked, "{}", card.id);
                assert_eq!(restriction.cant_be_blocked_by.len(), 1, "{}", card.id);
                let blocker = &restriction.cant_be_blocked_by[0];
                assert_eq!(blocker.kind, TargetKind::Creature, "{}", card.id);
                assert_eq!(
                    blocker.power,
                    Some(PowerComparison::AtMost(2)),
                    "{}",
                    card.id
                );
                assert_eq!(blocker.toughness, None, "{}", card.id);
            }
            "enraged_ceratok" => {
                assert_eq!(definition.layout, Layout::Normal, "{}", card.id);
                assert!(f.activated_abilities.is_empty(), "{}", card.id);
                assert!(f.triggered_abilities.is_empty(), "{}", card.id);
                assert!(f.spell_effect.is_empty(), "{}", card.id);
                assert!(f.targeting.is_none(), "{}", card.id);
                let [entry] = f.static_abilities.as_slice() else {
                    panic!("{}: exactly one static ability", card.id);
                };
                assert_eq!(
                    entry.presentation,
                    AbilityPresentation::OracleLines(vec![1]),
                    "{}",
                    card.id
                );
                let StaticAbilityDef::SelfCombatRestriction {
                    restriction,
                    condition,
                } = &entry.definition
                else {
                    panic!("{}: self combat restriction", card.id);
                };
                assert!(condition.is_none(), "{}", card.id);
                assert_eq!(restriction.maximum_blockers, None, "{}", card.id);
                assert!(
                    !restriction.cant_block && !restriction.cant_attack,
                    "{}",
                    card.id
                );
                assert!(!restriction.cant_be_blocked, "{}", card.id);
                assert_eq!(restriction.cant_be_blocked_by.len(), 1, "{}", card.id);
                let blocker = &restriction.cant_be_blocked_by[0];
                assert_eq!(blocker.kind, TargetKind::Creature, "{}", card.id);
                assert_eq!(
                    blocker.power,
                    Some(PowerComparison::AtMost(2)),
                    "{}",
                    card.id
                );
                assert_eq!(blocker.toughness, None, "{}", card.id);
            }
            "giltgrove_stalker" => {
                assert_eq!(definition.layout, Layout::Normal, "{}", card.id);
                assert!(f.activated_abilities.is_empty(), "{}", card.id);
                assert!(f.triggered_abilities.is_empty(), "{}", card.id);
                assert!(f.spell_effect.is_empty(), "{}", card.id);
                assert!(f.targeting.is_none(), "{}", card.id);
                let [entry] = f.static_abilities.as_slice() else {
                    panic!("{}: exactly one static ability", card.id);
                };
                assert_eq!(
                    entry.presentation,
                    AbilityPresentation::OracleLines(vec![1]),
                    "{}",
                    card.id
                );
                let StaticAbilityDef::SelfCombatRestriction {
                    restriction,
                    condition,
                } = &entry.definition
                else {
                    panic!("{}: self combat restriction", card.id);
                };
                assert!(condition.is_none(), "{}", card.id);
                assert_eq!(restriction.maximum_blockers, None, "{}", card.id);
                assert!(
                    !restriction.cant_block && !restriction.cant_attack,
                    "{}",
                    card.id
                );
                assert!(!restriction.cant_be_blocked, "{}", card.id);
                assert_eq!(restriction.cant_be_blocked_by.len(), 1, "{}", card.id);
                let blocker = &restriction.cant_be_blocked_by[0];
                assert_eq!(blocker.kind, TargetKind::Creature, "{}", card.id);
                assert_eq!(
                    blocker.power,
                    Some(PowerComparison::AtMost(2)),
                    "{}",
                    card.id
                );
                assert_eq!(blocker.toughness, None, "{}", card.id);
            }
            "outland_boar" => {
                assert_eq!(definition.layout, Layout::Normal, "{}", card.id);
                assert!(f.activated_abilities.is_empty(), "{}", card.id);
                assert!(f.triggered_abilities.is_empty(), "{}", card.id);
                assert!(f.spell_effect.is_empty(), "{}", card.id);
                assert!(f.targeting.is_none(), "{}", card.id);
                let [entry] = f.static_abilities.as_slice() else {
                    panic!("{}: exactly one static ability", card.id);
                };
                assert_eq!(
                    entry.presentation,
                    AbilityPresentation::OracleLines(vec![1]),
                    "{}",
                    card.id
                );
                let StaticAbilityDef::SelfCombatRestriction {
                    restriction,
                    condition,
                } = &entry.definition
                else {
                    panic!("{}: self combat restriction", card.id);
                };
                assert!(condition.is_none(), "{}", card.id);
                assert_eq!(restriction.maximum_blockers, None, "{}", card.id);
                assert!(
                    !restriction.cant_block && !restriction.cant_attack,
                    "{}",
                    card.id
                );
                assert!(!restriction.cant_be_blocked, "{}", card.id);
                assert_eq!(restriction.cant_be_blocked_by.len(), 1, "{}", card.id);
                let blocker = &restriction.cant_be_blocked_by[0];
                assert_eq!(blocker.kind, TargetKind::Creature, "{}", card.id);
                assert_eq!(
                    blocker.power,
                    Some(PowerComparison::AtMost(2)),
                    "{}",
                    card.id
                );
                assert_eq!(blocker.toughness, None, "{}", card.id);
            }
            "steel_leaf_champion" => {
                assert_eq!(definition.layout, Layout::Normal, "{}", card.id);
                assert!(f.activated_abilities.is_empty(), "{}", card.id);
                assert!(f.triggered_abilities.is_empty(), "{}", card.id);
                assert!(f.spell_effect.is_empty(), "{}", card.id);
                assert!(f.targeting.is_none(), "{}", card.id);
                let [entry] = f.static_abilities.as_slice() else {
                    panic!("{}: exactly one static ability", card.id);
                };
                assert_eq!(
                    entry.presentation,
                    AbilityPresentation::OracleLines(vec![1]),
                    "{}",
                    card.id
                );
                let StaticAbilityDef::SelfCombatRestriction {
                    restriction,
                    condition,
                } = &entry.definition
                else {
                    panic!("{}: self combat restriction", card.id);
                };
                assert!(condition.is_none(), "{}", card.id);
                assert_eq!(restriction.maximum_blockers, None, "{}", card.id);
                assert!(
                    !restriction.cant_block && !restriction.cant_attack,
                    "{}",
                    card.id
                );
                assert!(!restriction.cant_be_blocked, "{}", card.id);
                assert_eq!(restriction.cant_be_blocked_by.len(), 1, "{}", card.id);
                let blocker = &restriction.cant_be_blocked_by[0];
                assert_eq!(blocker.kind, TargetKind::Creature, "{}", card.id);
                assert_eq!(
                    blocker.power,
                    Some(PowerComparison::AtMost(2)),
                    "{}",
                    card.id
                );
                assert_eq!(blocker.toughness, None, "{}", card.id);
            }
            _ => unreachable!("known card"),
        }
    }
}

#[test]
fn blocked_by_power_two_batch_fingerprints_match_the_pinned_oracle_text() {
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
