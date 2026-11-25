//! Registry conformance for the queue-v78 02-attack-removal-rider cohort.
//!
//! Every card has one targeted instruction against an attacking creature followed by one printed
//! rider. Fingerprints are checked after the presentation metadata refresh.

use tricerules_cards::primitives::{
    Amount, CombatRole, EffectSubject, PlayerRecipient, SpellEffectKind, TargetKind,
};
use tricerules_cards::Layout;

#[derive(Clone, Copy, Debug)]
enum Removal {
    Destroy,
    Exile,
}

#[derive(Clone, Copy, Debug)]
enum Rider {
    Draw,
    GainLife(u32),
}

struct Card {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    removal: Removal,
    rider: Rider,
    fingerprint: &'static str,
}

fn cases() -> [Card; 3] {
    [
        Card {
            id: "bright_reprisal",
            name: "Bright Reprisal",
            mana_cost: "{4}{W}",
            removal: Removal::Destroy,
            rider: Rider::Draw,
            fingerprint: "ba49f95c1362ef3ba69a29f1c777f594f2ccc2914d8d4732fa3ebd18a59c8b46",
        },
        Card {
            id: "second_thoughts",
            name: "Second Thoughts",
            mana_cost: "{4}{W}",
            removal: Removal::Exile,
            rider: Rider::Draw,
            fingerprint: "fd269e9c0c4e4acd32b07c6cd8299d15d0b70febdf0a8cb3e10f14e3520afb0f",
        },
        Card {
            id: "silverstrike",
            name: "Silverstrike",
            mana_cost: "{3}{W}",
            removal: Removal::Destroy,
            rider: Rider::GainLife(3),
            fingerprint: "8f82786139097a005002d38350f40f6c6eda6c0af750ab6e61ff5e39aa257801",
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
fn q78_attack_removal_rider_map_definitions() {
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
        assert_eq!(f.types, ["Instant"], "{} types", card.id);
        assert!(f.keywords.is_empty(), "{}", card.id);
        assert!(f.static_abilities.is_empty(), "{}", card.id);
        assert!(f.triggered_abilities.is_empty(), "{}", card.id);
        assert!(f.activated_abilities.is_empty(), "{}", card.id);

        let [removal, rider] = f.spell_effect.as_slice() else {
            panic!("{}: exactly two spell effects", card.id);
        };
        let filter = match (card.removal, removal) {
            (
                Removal::Destroy,
                SpellEffectKind::Destroy {
                    subject: EffectSubject::Chosen(filter),
                },
            ) => filter.as_ref(),
            (
                Removal::Exile,
                SpellEffectKind::Exile {
                    subject: EffectSubject::Chosen(filter),
                    ..
                },
            ) => filter.as_ref(),
            other => panic!("{}: unexpected removal {other:?}", card.id),
        };
        assert_eq!(filter.kind, TargetKind::Creature, "{}", card.id);
        assert_eq!(
            filter.combat_role,
            Some(CombatRole::Attacking),
            "{}",
            card.id
        );
        match (card.rider, rider) {
            (
                Rider::Draw,
                SpellEffectKind::Draw {
                    who: PlayerRecipient::Controller,
                    count: Amount::Fixed(1),
                },
            ) => {}
            (
                Rider::GainLife(expected),
                SpellEffectKind::GainLife {
                    amount: Amount::Fixed(actual),
                    who: PlayerRecipient::Controller,
                },
            ) if *actual == expected => {}
            other => panic!("{}: unexpected rider {other:?}", card.id),
        }

        let targeting = f
            .targeting
            .as_ref()
            .unwrap_or_else(|| panic!("{}: targeting", card.id));
        let [group] = targeting.groups.as_slice() else {
            panic!("{}: exactly one target group", card.id);
        };
        assert_eq!((group.min, group.max), (1, 1), "{}", card.id);
        assert_eq!(group.effect_indices, [0], "{}", card.id);
        assert_eq!(
            group.prompt, "Choose target attacking creature",
            "{}",
            card.id
        );
    }
}

#[test]
fn q78_attack_removal_rider_fingerprints_match_the_pinned_oracle_text() {
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
