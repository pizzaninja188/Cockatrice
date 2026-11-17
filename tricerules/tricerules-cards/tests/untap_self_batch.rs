//! Registry conformance for the queue-v28 "{U}: Untap this creature" cohort.
//!
//! Each card is a normal-layout printed creature whose complete typed shape is asserted
//! against the pinned Oracle text. Fingerprints are SHA-256 of the exact normalized
//! Oracle lines.

use tricerules_cards::primitives::{AbilityCost, SpellEffectKind};
use tricerules_cards::{Layout, ManaCost};

struct Card {
    id: &'static str,
    name: &'static str,
    mana: &'static str,
    types: &'static [&'static str],
    power: u32,
    toughness: u32,
    fingerprint: &'static str,
}

const CARDS: &[Card] = &[
    Card {
        id: "simic_ragworm",
        name: "Simic Ragworm",
        mana: "{3}{G}",
        types: &["Creature", "Worm"],
        power: 3,
        toughness: 3,
        fingerprint: "11460bcbf58e3037148000a846cb75d6c53ec5676c10d663d04a00e5584af262",
    },
    Card {
        id: "horseshoe_crab",
        name: "Horseshoe Crab",
        mana: "{2}{U}",
        types: &["Creature", "Crab"],
        power: 1,
        toughness: 3,
        fingerprint: "11460bcbf58e3037148000a846cb75d6c53ec5676c10d663d04a00e5584af262",
    },
    Card {
        id: "soliton",
        name: "Soliton",
        mana: "{5}",
        types: &["Artifact", "Creature", "Construct"],
        power: 3,
        toughness: 4,
        fingerprint: "11460bcbf58e3037148000a846cb75d6c53ec5676c10d663d04a00e5584af262",
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
fn untap_self_batch_maps_definitions() {
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
        assert_eq!(
            ability.costs,
            [AbilityCost::Mana(ManaCost::parse("{U}").unwrap())],
            "{}",
            card.id
        );
        assert_eq!(
            ability.effect.as_slice(),
            [SpellEffectKind::Untap {
                subject: tricerules_cards::primitives::EffectSubject::Source,
            }],
            "{}",
            card.id
        );
        assert!(ability.targeting.is_none(), "{}", card.id);
        assert!(ability.conditions.is_empty(), "{}", card.id);
    }
}

#[test]
fn untap_self_batch_fingerprints_match_the_pinned_oracle_text() {
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
