//! Registry conformance for the queue-v28 "target player draws two and loses 2 life" cohort.
//!
//! Each card is a normal-layout printed spell whose complete typed shape is asserted
//! against the pinned Oracle text. Fingerprints are SHA-256 of the exact normalized
//! Oracle lines.

use tricerules_cards::primitives::{Amount, SpellEffectKind, TargetGroupDef, TargetingDef};
use tricerules_cards::Layout;

struct Card {
    id: &'static str,
    name: &'static str,
    mana: &'static str,
    types: &'static [&'static str],
    fingerprint: &'static str,
}

const CARDS: &[Card] = &[
    Card {
        id: "painful_lesson",
        name: "Painful Lesson",
        mana: "{2}{B}",
        types: &["Sorcery"],
        fingerprint: "3f5a30580f494fb753f6eaecf5459da71874af81c58a46fb77b74643aabc12c5",
    },
    Card {
        id: "sign_in_blood",
        name: "Sign in Blood",
        mana: "{B}{B}",
        types: &["Sorcery"],
        fingerprint: "3f5a30580f494fb753f6eaecf5459da71874af81c58a46fb77b74643aabc12c5",
    },
    Card {
        id: "blood_pact",
        name: "Blood Pact",
        mana: "{2}{B}",
        types: &["Instant"],
        fingerprint: "3f5a30580f494fb753f6eaecf5459da71874af81c58a46fb77b74643aabc12c5",
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

fn single_group(targeting: &TargetingDef) -> &TargetGroupDef {
    let [group] = targeting.groups.as_slice() else {
        panic!("expected exactly one target group");
    };
    group
}

#[test]
fn draw_two_lose_two_batch_maps_definitions() {
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
        assert_eq!(f.types, card.types, "{} types", card.id);
        assert_eq!(definition.layout, Layout::Normal, "{}", card.id);
        assert!(f.activated_abilities.is_empty(), "{}", card.id);
        assert!(f.triggered_abilities.is_empty(), "{}", card.id);
        assert!(f.static_abilities.is_empty(), "{}", card.id);
        assert_eq!(
            f.spell_effect.as_slice(),
            [
                SpellEffectKind::TargetPlayerDraws {
                    count: Amount::Fixed(2),
                    target: tricerules_cards::primitives::TargetFilter {
                        kind: tricerules_cards::primitives::TargetKind::AnyPlayer,
                        ..Default::default()
                    },
                },
                SpellEffectKind::TargetPlayerLosesLife {
                    amount: 2,
                    target: tricerules_cards::primitives::TargetFilter {
                        kind: tricerules_cards::primitives::TargetKind::AnyPlayer,
                        ..Default::default()
                    },
                },
            ],
            "{}",
            card.id
        );
        let targeting = f
            .targeting
            .as_ref()
            .unwrap_or_else(|| panic!("{}: needs one target group", card.id));
        let group = single_group(targeting);
        assert_eq!((group.min, group.max), (1, 1), "{}", card.id);
        assert_eq!(group.effect_indices, [0, 1], "{}", card.id);
    }
}

#[test]
fn draw_two_lose_two_batch_fingerprints_match_the_pinned_oracle_text() {
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
