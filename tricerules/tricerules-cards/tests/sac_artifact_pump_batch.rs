//! Registry conformance for the queue-v28 "sacrifice an artifact: pump this creature" cohort.
//!
//! Each card is a normal-layout printed creature whose complete typed shape is asserted
//! against the pinned Oracle text. Fingerprints are SHA-256 of the exact normalized
//! Oracle lines.

use tricerules_cards::primitives::{
    AbilityCost, EffectSubject, ObjectFilter, PermanentTypeFilter, SpellEffectKind,
    TargetController, TargetKind,
};
use tricerules_cards::Layout;

struct Card {
    id: &'static str,
    name: &'static str,
    mana: &'static str,
    types: &'static [&'static str],
    power: u32,
    toughness: u32,
    keywords: &'static [&'static str],
    pump: (i32, i32),
    fingerprint: &'static str,
}

const CARDS: &[Card] = &[
    Card {
        id: "atog",
        name: "Atog",
        mana: "{1}{R}",
        types: &["Creature", "Atog"],
        power: 1,
        toughness: 2,
        keywords: &[],
        pump: (2, 2),
        fingerprint: "2881d5c26d46bc60d42bec52afcf29a05c4eed999b2fa96c2dab011eb5a16c1b",
    },
    Card {
        id: "ravenous_intruder",
        name: "Ravenous Intruder",
        mana: "{1}{R}",
        types: &["Creature", "Gremlin"],
        power: 1,
        toughness: 2,
        keywords: &[],
        pump: (2, 2),
        fingerprint: "2881d5c26d46bc60d42bec52afcf29a05c4eed999b2fa96c2dab011eb5a16c1b",
    },
    Card {
        id: "rust_monster",
        name: "Rust Monster",
        mana: "{2}{R}",
        types: &["Creature", "Beast"],
        power: 2,
        toughness: 1,
        keywords: &["FirstStrike"],
        pump: (2, 0),
        fingerprint: "212bf69de4f0b780431797b9a9b181c87ef4a0169b14d2879ce77946d7ad0ef4",
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

fn artifact_cost() -> AbilityCost {
    AbilityCost::SacrificePermanent {
        filter: ObjectFilter {
            kind: TargetKind::AnyPermanent,
            controller: TargetController::You,
            permanent_types: vec![PermanentTypeFilter::Artifact],
            ..ObjectFilter::default()
        },
        count: 1,
    }
}

#[test]
fn sac_artifact_pump_batch_maps_definitions() {
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
        assert_eq!(f.power, Some(card.power), "{} power", card.id);
        assert_eq!(f.toughness, Some(card.toughness), "{} toughness", card.id);
        assert_eq!(definition.layout, Layout::Normal, "{}", card.id);
        assert!(f.triggered_abilities.is_empty(), "{}", card.id);
        assert!(f.static_abilities.is_empty(), "{}", card.id);
        assert!(f.spell_effect.is_empty(), "{}", card.id);
        assert!(f.targeting.is_none(), "{}", card.id);
        let [ability] = f.activated_abilities.as_slice() else {
            panic!("{}: exactly one activated ability", card.id);
        };
        assert_eq!(ability.costs, [artifact_cost()], "{}", card.id);
        let [effect] = ability.effect.as_slice() else {
            panic!("{}: exactly one effect", card.id);
        };
        let SpellEffectKind::PumpTarget {
            power,
            toughness,
            scale,
            subject,
        } = effect
        else {
            panic!("{}: pump this creature", card.id);
        };
        assert_eq!(
            (*power, *toughness, scale, subject),
            (card.pump.0, card.pump.1, &None, &EffectSubject::Source),
            "{}",
            card.id
        );
        assert!(ability.targeting.is_none(), "{}", card.id);
        assert_eq!(f.keywords.len(), card.keywords.len(), "{}", card.id);
        for keyword in card.keywords {
            assert!(
                f.keywords
                    .iter()
                    .any(|value| format!("{value:?}") == *keyword),
                "{} keyword {keyword}",
                card.id
            );
        }
    }
}

#[test]
fn sac_artifact_pump_batch_fingerprints_match_the_pinned_oracle_text() {
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
