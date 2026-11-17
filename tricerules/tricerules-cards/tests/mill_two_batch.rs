//! Registry conformance for the queue-v26 activated mill-two cohort.
//!
//! Every card is a normal-layout artifact or creature with one activated
//! ability that mills any target player for exactly two cards. Each fingerprint
//! is SHA-256 of the exact pinned normalized Oracle text.

use tricerules_cards::primitives::{AbilityCost, SpellEffectKind, TargetKind};
use tricerules_cards::{AbilityPresentation, Layout};

struct Card {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    types: &'static [&'static str],
    power: Option<u32>,
    toughness: Option<u32>,
    activation: &'static str,
    fingerprint: &'static str,
}

const CARDS: &[Card] = &[
    Card {
        id: "millstone",
        name: "Millstone",
        mana_cost: "{2}",
        types: &["Artifact"],
        power: None,
        toughness: None,
        activation: "{2}",
        fingerprint: "520ee05d8e3766b7cf2558845dd75999e99268c2300d31ae7b9419fff3747ebc",
    },
    Card {
        id: "vedalken_entrancer",
        name: "Vedalken Entrancer",
        mana_cost: "{3}{U}",
        types: &["Creature", "Vedalken", "Wizard"],
        power: Some(1),
        toughness: Some(4),
        activation: "{U}",
        fingerprint: "c1c7ab9c333d647cd2e2e3ad66a603ee18c3322d9de3cdc15812066d8df69ad2",
    },
    Card {
        id: "merfolk_mesmerist",
        name: "Merfolk Mesmerist",
        mana_cost: "{1}{U}",
        types: &["Creature", "Merfolk", "Wizard"],
        power: Some(1),
        toughness: Some(2),
        activation: "{U}",
        fingerprint: "c1c7ab9c333d647cd2e2e3ad66a603ee18c3322d9de3cdc15812066d8df69ad2",
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
fn mill_two_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for card in CARDS {
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
        assert_eq!(
            f.mana_cost.to_string(),
            card.mana_cost,
            "{} mana cost",
            card.id
        );
        assert_eq!(f.types, card.types, "{} types", card.id);
        assert!(f.supertypes.is_empty(), "{}", card.id);
        assert_eq!(
            (f.power, f.toughness),
            (card.power, card.toughness),
            "{} body",
            card.id
        );
        assert!(f.keywords.is_empty(), "{}", card.id);
        assert!(f.triggered_abilities.is_empty(), "{}", card.id);
        assert!(f.static_abilities.is_empty(), "{}", card.id);
        assert!(f.spell_effect.is_empty(), "{}", card.id);
        assert!(f.targeting.is_none(), "{}", card.id);

        let [mill] = f.activated_abilities.as_slice() else {
            panic!("{}: exactly one activated ability", card.id);
        };
        assert_eq!(mill.ability_id.as_str(), "activated_01", "{}", card.id);
        assert_eq!(
            mill.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{}",
            card.id
        );
        assert_eq!(
            mill.costs,
            [
                AbilityCost::Mana(tricerules_cards::ManaCost::parse(card.activation).unwrap()),
                AbilityCost::Tap,
            ],
            "{}",
            card.id
        );
        assert!(
            matches!(
                mill.effect.as_slice(),
                [SpellEffectKind::MillTargetPlayer { count: 2, target }]
                    if target.kind == TargetKind::AnyPlayer
            ),
            "{}: mill any player for exactly two",
            card.id
        );
    }
}

#[test]
fn mill_two_batch_fingerprints_match_the_pinned_oracle_text() {
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
