//! Registry conformance for the queue-v27 graveyard-to-top cohort.
//!
//! Each card is a normal-layout printed card whose complete typed shape is asserted
//! against the pinned Oracle text. Fingerprints are SHA-256 of the exact normalized
//! Oracle lines.

use tricerules_cards::primitives::{GraveyardDestination, GraveyardOwner, SpellEffectKind};
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
        id: "false_mourning",
        name: "False Mourning",
        mana: "{G}",
        supertypes: &[],
        types: &["Sorcery"],
        power: None,
        toughness: None,
        keywords: &[],
        fingerprint: "84026055350891843ffc266aab18e34eb3e5cbeaba4eae9abd2ea97b0e961dc2",
    },
    Card {
        id: "reclaim",
        name: "Reclaim",
        mana: "{G}",
        supertypes: &[],
        types: &["Instant"],
        power: None,
        toughness: None,
        keywords: &[],
        fingerprint: "84026055350891843ffc266aab18e34eb3e5cbeaba4eae9abd2ea97b0e961dc2",
    },
    Card {
        id: "salvage",
        name: "Salvage",
        mana: "{G}",
        supertypes: &[],
        types: &["Sorcery"],
        power: None,
        toughness: None,
        keywords: &[],
        fingerprint: "84026055350891843ffc266aab18e34eb3e5cbeaba4eae9abd2ea97b0e961dc2",
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
fn graveyard_to_top_batch_maps_definitions() {
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
            "false_mourning" => {
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
                assert_eq!(
                    *destination,
                    GraveyardDestination::LibraryTop,
                    "{}",
                    card.id
                );
                assert!(
                    filter.card.is_none() && filter.owner == GraveyardOwner::Controller,
                    "{}",
                    card.id
                );
                assert!(linked_exile_id.is_none() && entry_modifiers.is_empty());
            }
            "reclaim" => {
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
                assert_eq!(
                    *destination,
                    GraveyardDestination::LibraryTop,
                    "{}",
                    card.id
                );
                assert!(
                    filter.card.is_none() && filter.owner == GraveyardOwner::Controller,
                    "{}",
                    card.id
                );
                assert!(linked_exile_id.is_none() && entry_modifiers.is_empty());
            }
            "salvage" => {
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
                assert_eq!(
                    *destination,
                    GraveyardDestination::LibraryTop,
                    "{}",
                    card.id
                );
                assert!(
                    filter.card.is_none() && filter.owner == GraveyardOwner::Controller,
                    "{}",
                    card.id
                );
                assert!(linked_exile_id.is_none() && entry_modifiers.is_empty());
            }
            _ => unreachable!("known card"),
        }
    }
}

#[test]
fn graveyard_to_top_batch_fingerprints_match_the_pinned_oracle_text() {
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
