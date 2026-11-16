//! Registry conformance for the reviewed block-pump cohort.
//!
//! Every included card is a normal-layout creature with one blocking trigger and
//! an until-end-of-turn self pump. The attacker filter is empty because the
//! Oracle line has no restriction. Each fingerprint is SHA-256 of the exact
//! pinned Oracle lines.

use tricerules_cards::primitives::{SpellEffectKind, TriggerCondition};
use tricerules_cards::{AbilityPresentation, CardRegistry, Layout};

struct Card {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    types: &'static [&'static str],
    power: u32,
    toughness: u32,
    pump: (i32, i32),
    fingerprint: &'static str,
}

const CARDS: &[Card] = &[
    Card {
        id: "folk_of_an-havva",
        name: "Folk of An-Havva",
        mana_cost: "{G}",
        types: &["Creature", "Human"],
        power: 1,
        toughness: 1,
        pump: (2, 0),
        fingerprint: "538870f1a50cad65a0199e5bbdefe057bb46ac3b883032e342175db73e2b7170",
    },
    Card {
        id: "giant_badger",
        name: "Giant Badger",
        mana_cost: "{1}{G}{G}",
        types: &["Creature", "Badger"],
        power: 2,
        toughness: 2,
        pump: (2, 2),
        fingerprint: "e2d2a4075ef07c3175b2611ca5247f8307376f153db9744a6dd96bd8a33ebd55",
    },
    Card {
        id: "royal_trooper",
        name: "Royal Trooper",
        mana_cost: "{2}{W}",
        types: &["Creature", "Human", "Soldier"],
        power: 2,
        toughness: 2,
        pump: (2, 2),
        fingerprint: "e2d2a4075ef07c3175b2611ca5247f8307376f153db9744a6dd96bd8a33ebd55",
    },
    Card {
        id: "shu_defender",
        name: "Shu Defender",
        mana_cost: "{2}{W}",
        types: &["Creature", "Human", "Soldier"],
        power: 2,
        toughness: 2,
        pump: (0, 2),
        fingerprint: "bc6dbc7f2c6dfb54d2f1ac986f9c8b3c325da2a8b7f4a5330306883d30aeb5ca",
    },
    Card {
        id: "town_sentry",
        name: "Town Sentry",
        mana_cost: "{2}{W}",
        types: &["Creature", "Human", "Soldier"],
        power: 2,
        toughness: 2,
        pump: (0, 2),
        fingerprint: "bc6dbc7f2c6dfb54d2f1ac986f9c8b3c325da2a8b7f4a5330306883d30aeb5ca",
    },
];

fn face<'a>(registry: &'a CardRegistry, id: &str) -> &'a tricerules_cards::CardFace {
    registry
        .get(id)
        .unwrap_or_else(|| panic!("missing {id}"))
        .primary_face()
}

#[test]
fn blocks_pump_batch_maps_definitions() {
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
        assert_eq!(
            f.face_id.as_str(),
            card.id.replace('-', "_"),
            "{} face id",
            card.id
        );
        assert_eq!(f.name, card.name, "{} name", card.id);
        assert_eq!(
            f.mana_cost.to_string(),
            card.mana_cost,
            "{} mana cost",
            card.id
        );
        assert_eq!(f.types, card.types, "{} types", card.id);
        assert_eq!(f.power, Some(card.power), "{} power", card.id);
        assert_eq!(f.toughness, Some(card.toughness), "{} toughness", card.id);
        assert!(f.keywords.is_empty(), "{} no keyword", card.id);
        assert!(
            f.static_abilities.is_empty(),
            "{} no static ability",
            card.id
        );
        assert!(
            f.activated_abilities.is_empty(),
            "{} no activated ability",
            card.id
        );
        assert!(f.spell_effect.is_empty(), "{} no spell effect", card.id);

        let [trigger] = f.triggered_abilities.as_slice() else {
            panic!("{}: exactly one triggered ability", card.id);
        };
        assert_eq!(trigger.ability_id.as_str(), "triggered_01", "{}", card.id);
        assert_eq!(
            trigger.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{} presentation",
            card.id
        );
        let TriggerCondition::WheneverSelfBlocksCreature { attacker } = &trigger.trigger else {
            panic!("{}: {:?}", card.id, trigger.trigger);
        };
        assert!(
            attacker.required_keywords.is_empty(),
            "{} no keyword filter",
            card.id
        );
        assert!(
            attacker.required_subtypes.is_empty(),
            "{} no subtype filter",
            card.id
        );
        assert!(
            attacker.excluded_keywords.is_empty(),
            "{} no excluded keyword",
            card.id
        );
        assert!(attacker.power.is_none(), "{} no power filter", card.id);
        assert!(
            matches!(
                trigger.effect.as_slice(),
                [SpellEffectKind::PumpTarget { power, toughness, .. }]
                    if *power == card.pump.0 && *toughness == card.pump.1
            ),
            "{}: printed self pump",
            card.id
        );
    }
}

#[test]
fn blocks_pump_batch_fingerprints_match_the_pinned_oracle_text() {
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
        assert_eq!(fields[2], card.id.replace('-', "_"));
        assert_eq!(
            fields[4], card.fingerprint,
            "fingerprint drift for {}",
            card.id
        );
    }
}
