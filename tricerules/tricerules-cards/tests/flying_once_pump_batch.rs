//! Registry conformance for the queue-v26 flying once-each-turn self-pump cohort.
//!
//! Every card is a normal-layout flying creature whose only activated ability
//! pumps only itself and is limited to one activation each turn. Each
//! fingerprint is SHA-256 of the exact pinned normalized Oracle text.

use tricerules_cards::primitives::{AbilityCost, ActivationLimit, EffectSubject, SpellEffectKind};
use tricerules_cards::{AbilityPresentation, Keyword, Layout, ManaCost};

struct Card {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    power: u32,
    toughness: u32,
    activation: &'static str,
    pump_power: i32,
    pump_toughness: i32,
    fingerprint: &'static str,
}

const CARDS: &[Card] = &[
    Card {
        id: "azimaet_drake",
        name: "Azimaet Drake",
        mana_cost: "{2}{U}",
        power: 1,
        toughness: 3,
        activation: "{U}",
        pump_power: 1,
        pump_toughness: 0,
        fingerprint: "5b5f2f53db513a332f2766e14de05cd2cf59a35a48c8c87de70b0677afda4790",
    },
    Card {
        id: "drake_hatchling",
        name: "Drake Hatchling",
        mana_cost: "{2}{U}",
        power: 1,
        toughness: 3,
        activation: "{U}",
        pump_power: 1,
        pump_toughness: 0,
        fingerprint: "5b5f2f53db513a332f2766e14de05cd2cf59a35a48c8c87de70b0677afda4790",
    },
    Card {
        id: "spitting_drake",
        name: "Spitting Drake",
        mana_cost: "{3}{R}",
        power: 2,
        toughness: 2,
        activation: "{R}",
        pump_power: 1,
        pump_toughness: 0,
        fingerprint: "ba7b0e4aa8c86ee878116ab329628387b29a6a004f5f34b93feb85f1ed82dd3b",
    },
    Card {
        id: "fire_drake",
        name: "Fire Drake",
        mana_cost: "{1}{R}{R}",
        power: 1,
        toughness: 2,
        activation: "{R}",
        pump_power: 1,
        pump_toughness: 0,
        fingerprint: "ba7b0e4aa8c86ee878116ab329628387b29a6a004f5f34b93feb85f1ed82dd3b",
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
fn flying_once_pump_batch_maps_definitions() {
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
        assert_eq!(f.types, ["Creature", "Drake"], "{} types", card.id);
        assert!(f.supertypes.is_empty(), "{}", card.id);
        assert_eq!(
            (f.power, f.toughness),
            (Some(card.power), Some(card.toughness)),
            "{} body",
            card.id
        );
        assert_eq!(f.keywords, [Keyword::Flying], "{}", card.id);
        assert!(f.triggered_abilities.is_empty(), "{}", card.id);
        assert!(f.static_abilities.is_empty(), "{}", card.id);
        assert!(f.spell_effect.is_empty(), "{}", card.id);
        assert!(f.targeting.is_none(), "{}", card.id);

        let [pump] = f.activated_abilities.as_slice() else {
            panic!("{}: exactly one activated ability", card.id);
        };
        assert_eq!(pump.ability_id.as_str(), "activated_01", "{}", card.id);
        assert_eq!(
            pump.presentation,
            AbilityPresentation::OracleLines(vec![2]),
            "{}",
            card.id
        );
        assert_eq!(
            pump.costs,
            [AbilityCost::Mana(ManaCost::parse(card.activation).unwrap())],
            "{}",
            card.id
        );
        assert_eq!(
            pump.activation_limit,
            Some(ActivationLimit::PerTurn { max_activations: 1 }),
            "{}",
            card.id
        );
        assert!(
            matches!(
                pump.effect.as_slice(),
                [SpellEffectKind::PumpTarget {
                    power,
                    toughness,
                    subject: EffectSubject::Source,
                    ..
                }] if *power == card.pump_power && *toughness == card.pump_toughness
            ),
            "{}: source-only pump",
            card.id
        );
    }
}

#[test]
fn flying_once_pump_batch_fingerprints_match_the_pinned_oracle_text() {
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
