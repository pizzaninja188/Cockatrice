//! Registry conformance for the queue-v28 attack-tap cohort.
//!
//! Each card is a normal-layout printed creature whose complete typed shape is asserted
//! against the pinned Oracle text. Fingerprints are SHA-256 of the exact normalized
//! Oracle lines.

use tricerules_cards::primitives::{
    EffectSubject, SpellEffectKind, TargetController, TargetGroupDef, TargetKind, TargetingDef,
    TriggerCondition,
};
use tricerules_cards::Layout;

struct Card {
    id: &'static str,
    face_id: &'static str,
    name: &'static str,
    mana: &'static str,
    types: &'static [&'static str],
    power: u32,
    toughness: u32,
    fingerprint: &'static str,
}

const CARDS: &[Card] = &[
    Card {
        id: "fiend_binder",
        face_id: "fiend_binder",
        name: "Fiend Binder",
        mana: "{3}{W}",
        types: &["Creature", "Human", "Soldier"],
        power: 3,
        toughness: 2,
        fingerprint: "d24c70a23ae931fd2273081fa68040317356ed412e8f8cd33937719fa27c318c",
    },
    Card {
        id: "master_of_diversion",
        face_id: "master_of_diversion",
        name: "Master of Diversion",
        mana: "{2}{W}",
        types: &["Creature", "Human", "Scout"],
        power: 2,
        toughness: 2,
        fingerprint: "d24c70a23ae931fd2273081fa68040317356ed412e8f8cd33937719fa27c318c",
    },
    Card {
        id: "star-crowned_stag",
        face_id: "star_crowned_stag",
        name: "Star-Crowned Stag",
        mana: "{3}{W}",
        types: &["Creature", "Elk"],
        power: 3,
        toughness: 3,
        fingerprint: "d24c70a23ae931fd2273081fa68040317356ed412e8f8cd33937719fa27c318c",
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
fn attack_tap_batch_maps_definitions() {
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
        assert_eq!(f.face_id.as_str(), card.face_id, "{} face id", card.id);
        assert_eq!(f.name, card.name, "{} name", card.id);
        assert_eq!(f.mana_cost.to_string(), card.mana, "{} mana", card.id);
        assert_eq!(f.types, card.types, "{} types", card.id);
        assert_eq!(f.power, Some(card.power), "{} power", card.id);
        assert_eq!(f.toughness, Some(card.toughness), "{} toughness", card.id);
        assert_eq!(definition.layout, Layout::Normal, "{}", card.id);
        assert!(f.activated_abilities.is_empty(), "{}", card.id);
        assert!(f.static_abilities.is_empty(), "{}", card.id);
        assert!(f.spell_effect.is_empty(), "{}", card.id);
        let [ability] = f.triggered_abilities.as_slice() else {
            panic!("{}: exactly one triggered ability", card.id);
        };
        assert_eq!(
            ability.trigger,
            TriggerCondition::WheneverSelfAttacks {
                minimum_other_attackers: 0
            },
            "{}",
            card.id
        );
        let [effect] = ability.effect.as_slice() else {
            panic!("{}: exactly one effect", card.id);
        };
        let SpellEffectKind::Tap {
            subject: EffectSubject::Chosen(filter),
        } = effect
        else {
            panic!("{}: tap the chosen creature", card.id);
        };
        assert_eq!(filter.kind, TargetKind::Creature, "{}", card.id);
        assert_eq!(
            filter.controller,
            TargetController::DefendingPlayer,
            "{}: defending-player scope",
            card.id
        );
        let targeting = ability
            .targeting
            .as_ref()
            .unwrap_or_else(|| panic!("{}: needs one target group", card.id));
        let group = single_group(targeting);
        assert_eq!((group.min, group.max), (1, 1), "{}", card.id);
        assert_eq!(group.effect_indices, [0], "{}", card.id);
    }
}

#[test]
fn attack_tap_batch_fingerprints_match_the_pinned_oracle_text() {
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
        assert_eq!(fields[2], card.face_id);
        assert_eq!(
            fields[4], card.fingerprint,
            "fingerprint drift for {}",
            card.id
        );
    }
}
