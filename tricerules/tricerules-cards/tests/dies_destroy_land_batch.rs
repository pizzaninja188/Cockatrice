//! Registry conformance for the queue-v27 dies-destroy-land cohort.
//!
//! Each card is a normal-layout printed card whose complete typed shape is asserted
//! against the pinned Oracle text. Fingerprints are SHA-256 of the exact normalized
//! Oracle lines.

use tricerules_cards::primitives::{
    EffectSubject, PermanentTypeFilter, SpellEffectKind, TargetKind, TriggerCondition,
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
        id: "fire_snake",
        name: "Fire Snake",
        mana: "{4}{R}",
        supertypes: &[],
        types: &["Creature", "Snake"],
        power: Some(3),
        toughness: Some(1),
        keywords: &[],
        fingerprint: "47d47e29dd39d76c49b1225a3bbd72fa848ae6a703938aa1625cc9b45662ad2a",
    },
    Card {
        id: "goblin_gardener",
        name: "Goblin Gardener",
        mana: "{3}{R}",
        supertypes: &[],
        types: &["Creature", "Goblin"],
        power: Some(2),
        toughness: Some(1),
        keywords: &[],
        fingerprint: "47d47e29dd39d76c49b1225a3bbd72fa848ae6a703938aa1625cc9b45662ad2a",
    },
    Card {
        id: "polluted_dead",
        name: "Polluted Dead",
        mana: "{4}{B}",
        supertypes: &[],
        types: &["Creature", "Zombie"],
        power: Some(3),
        toughness: Some(3),
        keywords: &[],
        fingerprint: "47d47e29dd39d76c49b1225a3bbd72fa848ae6a703938aa1625cc9b45662ad2a",
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
fn dies_destroy_land_batch_maps_definitions() {
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
            "fire_snake" => {
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
                    TriggerCondition::WhenSelfDies,
                    "{}",
                    card.id
                );
                assert_eq!(
                    trigger.presentation,
                    AbilityPresentation::OracleLines(vec![1]),
                    "{}",
                    card.id
                );
                let [SpellEffectKind::Destroy { subject }] = trigger.effect.as_slice() else {
                    panic!("{}: destroy target land", card.id);
                };
                let EffectSubject::Chosen(target) = subject else {
                    panic!("{}: chosen target", card.id);
                };
                assert_eq!(target.kind, TargetKind::AnyPermanent, "{}", card.id);
                assert_eq!(
                    target.permanent_types,
                    [PermanentTypeFilter::Land],
                    "{}",
                    card.id
                );
                let targeting = trigger.targeting.as_ref().expect("trigger target");
                assert_eq!(
                    (targeting.groups[0].min, targeting.groups[0].max),
                    (1, 1),
                    "{}",
                    card.id
                );
            }
            "goblin_gardener" => {
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
                    TriggerCondition::WhenSelfDies,
                    "{}",
                    card.id
                );
                assert_eq!(
                    trigger.presentation,
                    AbilityPresentation::OracleLines(vec![1]),
                    "{}",
                    card.id
                );
                let [SpellEffectKind::Destroy { subject }] = trigger.effect.as_slice() else {
                    panic!("{}: destroy target land", card.id);
                };
                let EffectSubject::Chosen(target) = subject else {
                    panic!("{}: chosen target", card.id);
                };
                assert_eq!(target.kind, TargetKind::AnyPermanent, "{}", card.id);
                assert_eq!(
                    target.permanent_types,
                    [PermanentTypeFilter::Land],
                    "{}",
                    card.id
                );
                let targeting = trigger.targeting.as_ref().expect("trigger target");
                assert_eq!(
                    (targeting.groups[0].min, targeting.groups[0].max),
                    (1, 1),
                    "{}",
                    card.id
                );
            }
            "polluted_dead" => {
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
                    TriggerCondition::WhenSelfDies,
                    "{}",
                    card.id
                );
                assert_eq!(
                    trigger.presentation,
                    AbilityPresentation::OracleLines(vec![1]),
                    "{}",
                    card.id
                );
                let [SpellEffectKind::Destroy { subject }] = trigger.effect.as_slice() else {
                    panic!("{}: destroy target land", card.id);
                };
                let EffectSubject::Chosen(target) = subject else {
                    panic!("{}: chosen target", card.id);
                };
                assert_eq!(target.kind, TargetKind::AnyPermanent, "{}", card.id);
                assert_eq!(
                    target.permanent_types,
                    [PermanentTypeFilter::Land],
                    "{}",
                    card.id
                );
                let targeting = trigger.targeting.as_ref().expect("trigger target");
                assert_eq!(
                    (targeting.groups[0].min, targeting.groups[0].max),
                    (1, 1),
                    "{}",
                    card.id
                );
            }
            _ => unreachable!("known card"),
        }
    }
}

#[test]
fn dies_destroy_land_batch_fingerprints_match_the_pinned_oracle_text() {
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
