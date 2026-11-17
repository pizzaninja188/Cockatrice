//! Registry conformance for the queue-v27 typed graveyard recursion cohort.
//!
//! Each card is a normal-layout printed card whose complete typed shape is asserted
//! against the pinned Oracle text. Fingerprints are SHA-256 of the exact normalized
//! Oracle lines.

use tricerules_cards::primitives::{
    CardTypeFilter, GraveyardDestination, GraveyardOwner, SpellEffectKind,
};
use tricerules_cards::Layout;

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
        id: "return_to_battle",
        name: "Return to Battle",
        mana: "{B}",
        supertypes: &[],
        types: &["Sorcery"],
        power: None,
        toughness: None,
        keywords: &[],
        fingerprint: "b852587da41bbcd0e1ac6d868a2110319303b1b1af2333b70f603cd6f0ac3812",
    },
    Card {
        id: "wildwood_rebirth",
        name: "Wildwood Rebirth",
        mana: "{1}{G}",
        supertypes: &[],
        types: &["Instant"],
        power: None,
        toughness: None,
        keywords: &[],
        fingerprint: "b852587da41bbcd0e1ac6d868a2110319303b1b1af2333b70f603cd6f0ac3812",
    },
    Card {
        id: "call_to_mind",
        name: "Call to Mind",
        mana: "{2}{U}",
        supertypes: &[],
        types: &["Sorcery"],
        power: None,
        toughness: None,
        keywords: &[],
        fingerprint: "4a3efdd91ff3960cb1abbe81ee60aa41533d25f924559762e51c932295da054e",
    },
    Card {
        id: "relearn",
        name: "Relearn",
        mana: "{1}{U}{U}",
        supertypes: &[],
        types: &["Sorcery"],
        power: None,
        toughness: None,
        keywords: &[],
        fingerprint: "4a3efdd91ff3960cb1abbe81ee60aa41533d25f924559762e51c932295da054e",
    },
    Card {
        id: "reconstruction",
        name: "Reconstruction",
        mana: "{U}",
        supertypes: &[],
        types: &["Sorcery"],
        power: None,
        toughness: None,
        keywords: &[],
        fingerprint: "f81f3752d4f239988b7a6f37d662a7f5edad9db22f7aba2f0506994dca961a18",
    },
    Card {
        id: "ritual_of_restoration",
        name: "Ritual of Restoration",
        mana: "{W}",
        supertypes: &[],
        types: &["Sorcery"],
        power: None,
        toughness: None,
        keywords: &[],
        fingerprint: "f81f3752d4f239988b7a6f37d662a7f5edad9db22f7aba2f0506994dca961a18",
    },
    Card {
        id: "sages_knowledge",
        name: "Sage's Knowledge",
        mana: "{2}{U}",
        supertypes: &[],
        types: &["Sorcery"],
        power: None,
        toughness: None,
        keywords: &[],
        fingerprint: "88f985fa2908cacfd4a140f396377f7b1c0e4be56681d3b5c2d71a7bd272e88a",
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
fn graveyard_type_recursion_batch_maps_definitions() {
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
            "return_to_battle" => {
                assert_eq!(definition.layout, Layout::Normal, "{}", card.id);
                assert!(f.activated_abilities.is_empty(), "{}", card.id);
                assert!(f.triggered_abilities.is_empty(), "{}", card.id);
                assert!(f.static_abilities.is_empty(), "{}", card.id);
                assert!(f.targeting.is_none(), "{}", card.id);
                let [SpellEffectKind::MoveGraveyardCards {
                    filter,
                    destination,
                    linked_exile_id,
                    entry_modifiers,
                }] = f.spell_effect.as_slice()
                else {
                    panic!("{}: graveyard move", card.id);
                };
                assert_eq!(*destination, GraveyardDestination::Hand, "{}", card.id);
                assert!(linked_exile_id.is_none() && entry_modifiers.is_empty());
                assert_eq!(filter.owner, GraveyardOwner::Controller, "{}", card.id);
                assert!(!filter.mana_value_less_than_trigger_object);
                let zone = filter.card.as_ref().expect("typed card filter");
                assert_eq!(
                    zone.card_type,
                    Some(CardTypeFilter::Creature),
                    "{}",
                    card.id
                );
                assert!(zone.excluded_card_types.is_empty());
            }
            "wildwood_rebirth" => {
                assert_eq!(definition.layout, Layout::Normal, "{}", card.id);
                assert!(f.activated_abilities.is_empty(), "{}", card.id);
                assert!(f.triggered_abilities.is_empty(), "{}", card.id);
                assert!(f.static_abilities.is_empty(), "{}", card.id);
                assert!(f.targeting.is_none(), "{}", card.id);
                let [SpellEffectKind::MoveGraveyardCards {
                    filter,
                    destination,
                    linked_exile_id,
                    entry_modifiers,
                }] = f.spell_effect.as_slice()
                else {
                    panic!("{}: graveyard move", card.id);
                };
                assert_eq!(*destination, GraveyardDestination::Hand, "{}", card.id);
                assert!(linked_exile_id.is_none() && entry_modifiers.is_empty());
                assert_eq!(filter.owner, GraveyardOwner::Controller, "{}", card.id);
                assert!(!filter.mana_value_less_than_trigger_object);
                let zone = filter.card.as_ref().expect("typed card filter");
                assert_eq!(
                    zone.card_type,
                    Some(CardTypeFilter::Creature),
                    "{}",
                    card.id
                );
                assert!(zone.excluded_card_types.is_empty());
            }
            "call_to_mind" => {
                assert_eq!(definition.layout, Layout::Normal, "{}", card.id);
                assert!(f.activated_abilities.is_empty(), "{}", card.id);
                assert!(f.triggered_abilities.is_empty(), "{}", card.id);
                assert!(f.static_abilities.is_empty(), "{}", card.id);
                assert!(f.targeting.is_none(), "{}", card.id);
                let [SpellEffectKind::MoveGraveyardCards {
                    filter,
                    destination,
                    linked_exile_id,
                    entry_modifiers,
                }] = f.spell_effect.as_slice()
                else {
                    panic!("{}: graveyard move", card.id);
                };
                assert_eq!(*destination, GraveyardDestination::Hand, "{}", card.id);
                assert!(linked_exile_id.is_none() && entry_modifiers.is_empty());
                assert_eq!(filter.owner, GraveyardOwner::Controller, "{}", card.id);
                assert!(!filter.mana_value_less_than_trigger_object);
                let zone = filter.card.as_ref().expect("typed card filter");
                assert_eq!(
                    zone.card_type,
                    Some(CardTypeFilter::InstantOrSorcery),
                    "{}",
                    card.id
                );
                assert!(zone.excluded_card_types.is_empty());
            }
            "relearn" => {
                assert_eq!(definition.layout, Layout::Normal, "{}", card.id);
                assert!(f.activated_abilities.is_empty(), "{}", card.id);
                assert!(f.triggered_abilities.is_empty(), "{}", card.id);
                assert!(f.static_abilities.is_empty(), "{}", card.id);
                assert!(f.targeting.is_none(), "{}", card.id);
                let [SpellEffectKind::MoveGraveyardCards {
                    filter,
                    destination,
                    linked_exile_id,
                    entry_modifiers,
                }] = f.spell_effect.as_slice()
                else {
                    panic!("{}: graveyard move", card.id);
                };
                assert_eq!(*destination, GraveyardDestination::Hand, "{}", card.id);
                assert!(linked_exile_id.is_none() && entry_modifiers.is_empty());
                assert_eq!(filter.owner, GraveyardOwner::Controller, "{}", card.id);
                assert!(!filter.mana_value_less_than_trigger_object);
                let zone = filter.card.as_ref().expect("typed card filter");
                assert_eq!(
                    zone.card_type,
                    Some(CardTypeFilter::InstantOrSorcery),
                    "{}",
                    card.id
                );
                assert!(zone.excluded_card_types.is_empty());
            }
            "reconstruction" => {
                assert_eq!(definition.layout, Layout::Normal, "{}", card.id);
                assert!(f.activated_abilities.is_empty(), "{}", card.id);
                assert!(f.triggered_abilities.is_empty(), "{}", card.id);
                assert!(f.static_abilities.is_empty(), "{}", card.id);
                assert!(f.targeting.is_none(), "{}", card.id);
                let [SpellEffectKind::MoveGraveyardCards {
                    filter,
                    destination,
                    linked_exile_id,
                    entry_modifiers,
                }] = f.spell_effect.as_slice()
                else {
                    panic!("{}: graveyard move", card.id);
                };
                assert_eq!(*destination, GraveyardDestination::Hand, "{}", card.id);
                assert!(linked_exile_id.is_none() && entry_modifiers.is_empty());
                assert_eq!(filter.owner, GraveyardOwner::Controller, "{}", card.id);
                assert!(!filter.mana_value_less_than_trigger_object);
                let zone = filter.card.as_ref().expect("typed card filter");
                assert_eq!(
                    zone.card_type,
                    Some(CardTypeFilter::Artifact),
                    "{}",
                    card.id
                );
                assert!(zone.excluded_card_types.is_empty());
            }
            "ritual_of_restoration" => {
                assert_eq!(definition.layout, Layout::Normal, "{}", card.id);
                assert!(f.activated_abilities.is_empty(), "{}", card.id);
                assert!(f.triggered_abilities.is_empty(), "{}", card.id);
                assert!(f.static_abilities.is_empty(), "{}", card.id);
                assert!(f.targeting.is_none(), "{}", card.id);
                let [SpellEffectKind::MoveGraveyardCards {
                    filter,
                    destination,
                    linked_exile_id,
                    entry_modifiers,
                }] = f.spell_effect.as_slice()
                else {
                    panic!("{}: graveyard move", card.id);
                };
                assert_eq!(*destination, GraveyardDestination::Hand, "{}", card.id);
                assert!(linked_exile_id.is_none() && entry_modifiers.is_empty());
                assert_eq!(filter.owner, GraveyardOwner::Controller, "{}", card.id);
                assert!(!filter.mana_value_less_than_trigger_object);
                let zone = filter.card.as_ref().expect("typed card filter");
                assert_eq!(
                    zone.card_type,
                    Some(CardTypeFilter::Artifact),
                    "{}",
                    card.id
                );
                assert!(zone.excluded_card_types.is_empty());
            }
            "sages_knowledge" => {
                assert_eq!(definition.layout, Layout::Normal, "{}", card.id);
                assert!(f.activated_abilities.is_empty(), "{}", card.id);
                assert!(f.triggered_abilities.is_empty(), "{}", card.id);
                assert!(f.static_abilities.is_empty(), "{}", card.id);
                assert!(f.targeting.is_none(), "{}", card.id);
                let [SpellEffectKind::MoveGraveyardCards {
                    filter,
                    destination,
                    linked_exile_id,
                    entry_modifiers,
                }] = f.spell_effect.as_slice()
                else {
                    panic!("{}: graveyard move", card.id);
                };
                assert_eq!(*destination, GraveyardDestination::Hand, "{}", card.id);
                assert!(linked_exile_id.is_none() && entry_modifiers.is_empty());
                assert_eq!(filter.owner, GraveyardOwner::Controller, "{}", card.id);
                assert!(!filter.mana_value_less_than_trigger_object);
                let zone = filter.card.as_ref().expect("typed card filter");
                assert_eq!(zone.card_type, Some(CardTypeFilter::Sorcery), "{}", card.id);
                assert!(zone.excluded_card_types.is_empty());
            }
            _ => unreachable!("known card"),
        }
    }
}

#[test]
fn graveyard_type_recursion_batch_fingerprints_match_the_pinned_oracle_text() {
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
