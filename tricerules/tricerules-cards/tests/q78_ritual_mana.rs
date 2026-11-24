//! Registry conformance for the queue-v78 01-ritual-mana cohort.
//!
//! Every card is a one-instruction spell that adds a fixed bag of mana while it resolves. The
//! pinned 2010-08-15 Pyretic Ritual rulings confirm these are ordinary instants, not mana
//! abilities. Fingerprints are checked after the presentation metadata refresh.

use tricerules_cards::primitives::{ManaRetention, SpellEffectKind};
use tricerules_cards::{Layout, ManaAmount};

struct Card {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    types: &'static [&'static str],
    amount: ManaAmount,
    fingerprint: &'static str,
}

fn amount(w: u32, u: u32, b: u32, r: u32, g: u32, c: u32) -> ManaAmount {
    ManaAmount { w, u, b, r, g, c }
}

fn cases() -> Vec<Card> {
    vec![
        Card {
            id: "dark_ritual",
            name: "Dark Ritual",
            mana_cost: "{B}",
            types: &["Instant"],
            amount: amount(0, 0, 3, 0, 0, 0),
            fingerprint: "639f6e6a77582568b611b9f910c76dd765ae92ce0b8b1f28b0ba4a321761409c",
        },
        Card {
            id: "pyretic_ritual",
            name: "Pyretic Ritual",
            mana_cost: "{1}{R}",
            types: &["Instant"],
            amount: amount(0, 0, 0, 3, 0, 0),
            fingerprint: "281051f248957a0d3272a045dd59f6481ba4bec0a69a50c741ce3f09e50c9859",
        },
        Card {
            id: "seething_song",
            name: "Seething Song",
            mana_cost: "{2}{R}",
            types: &["Instant"],
            amount: amount(0, 0, 0, 5, 0, 0),
            fingerprint: "037b484f5bc3fc46756d3783c4ea7b8c462af2eec30434352b022b1f8dabe431",
        },
        Card {
            id: "channel_the_suns",
            name: "Channel the Suns",
            mana_cost: "{3}{G}",
            types: &["Sorcery"],
            amount: amount(1, 1, 1, 1, 1, 0),
            fingerprint: "53715d86faa3bccb694a1ad72d2810c5b2e069fe7a2a0e0dd44fa03e885dfe15",
        },
    ]
}

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
fn q78_ritual_mana_map_definitions() {
    let registry = tricerules_cards::registry::global();
    for card in cases() {
        let definition = registry
            .get(card.id)
            .unwrap_or_else(|| panic!("missing {}", card.id));
        assert_eq!(definition.layout, Layout::Normal, "{}", card.id);
        assert_eq!(
            registry.id_for_name(card.name),
            Some(card.id),
            "{}",
            card.id
        );
        let f = face(registry, card.id);
        assert_eq!(f.face_id.as_str(), card.id, "{} face id", card.id);
        assert_eq!(f.name, card.name, "{} name", card.id);
        assert_eq!(f.mana_cost.to_string(), card.mana_cost, "{} mana", card.id);
        assert_eq!(f.types, card.types, "{} types", card.id);
        assert!(f.keywords.is_empty(), "{}", card.id);
        assert!(f.static_abilities.is_empty(), "{}", card.id);
        assert!(f.triggered_abilities.is_empty(), "{}", card.id);
        assert!(f.activated_abilities.is_empty(), "{}", card.id);
        assert!(f.targeting.is_none(), "{}", card.id);

        let [effect] = f.spell_effect.as_slice() else {
            panic!("{}: exactly one spell effect", card.id);
        };
        match effect {
            SpellEffectKind::AddMana {
                amount,
                retention: ManaRetention::EndOfStep,
            } => assert_eq!(*amount, card.amount, "{} mana amount", card.id),
            other => panic!("{}: unexpected effect {other:?}", card.id),
        }
    }
}

#[test]
fn q78_ritual_mana_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for card in cases() {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{}\t", card.id)))
            .unwrap_or_else(|| panic!("missing fingerprint row for {}", card.id));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[0], card.id);
        assert_eq!(fields[1], card.name);
        assert_eq!(fields[2], card.id, "face id drift for {}", card.id);
        assert_eq!(
            fields[4], card.fingerprint,
            "fingerprint drift for {}",
            card.id
        );
    }
}
